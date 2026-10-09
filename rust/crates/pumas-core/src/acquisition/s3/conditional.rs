//! Opt-in selection of mutable objects with conditional reads and a declared digest.
//! An ETag is a representation validator, never a checksum or immutable revision.
use super::*;

impl S3Reader {
    /// Select an ordinary non-versioned object. Every fresh read sends If-Match;
    /// the shared acquisition owner must verify the declared whole-file SHA-256
    /// before issuing a verified artifact or consumer receipt. Raw read_range
    /// bytes remain unverified. No discovery, model admission or remote snapshot
    /// is implied. Weak/missing validators and unexpected immutable versions fail.
    pub async fn select_conditional(
        &self,
        source_key: &str,
        logical_path: &str,
        expected_sha256: Sha256Evidence,
    ) -> Result<S3ObjectSelection, S3ReaderError> {
        // Reuse exact-key rules without manufacturing immutable revision evidence.
        let key = self.validated_key(source_key)?;
        ArtifactFile::new(
            logical_path,
            source_key,
            None,
            Some(expected_sha256.clone()),
            FileVerificationRequirement::Sha256,
        )?;
        let head = tokio::time::timeout(
            self.timeout,
            sdk::scoped(
                self.store
                    .head_object()
                    .bucket(&self.bucket)
                    .key(key.as_ref())
                    .send(),
            ),
        )
        .await
        .map_err(|_| S3ReaderError::TimedOut)?
        .map_err(protocol_error)?;
        if head.version_id().is_some_and(|version| version != "null") {
            return Err(S3ReaderError::Changed);
        }
        let size = head
            .content_length()
            .filter(|size| *size >= 0)
            .ok_or_else(sdk::protocol_failure)? as u64;
        let etag = head
            .e_tag()
            .filter(|tag| strong_etag(tag))
            .ok_or(S3ReaderError::Changed)?
            .to_owned();
        let revision =
            serde_json::to_string(&(&etag, size)).map_err(|_| sdk::protocol_failure())?;
        let source = ArtifactSourceIdentity::new(
            "s3",
            self.object_scope_identity(source_key),
            ArtifactRevisionEvidence::new(
                "s3.conditional_etag_size",
                revision,
                RevisionStrength::Weak,
            )?,
        )?;
        let manifest = ArtifactManifest::new(
            source,
            vec![ArtifactFile::new(
                logical_path,
                source_key,
                Some(size),
                Some(expected_sha256),
                FileVerificationRequirement::Sha256,
            )?],
        )?;
        Ok(S3ObjectSelection {
            store: Arc::clone(&self.store),
            key,
            bucket: self.bucket.clone(),
            version: None,
            etag,
            size,
            timeout: self.timeout,
            manifest,
        })
    }
}

pub(super) fn strong_etag(tag: &str) -> bool {
    tag.len() >= 2
        && tag.starts_with('"')
        && tag.ends_with('"')
        && tag[1..tag.len() - 1]
            .bytes()
            .all(|byte| byte == 0x21 || (0x23..=0x7e).contains(&byte) || byte >= 0x80)
}

impl S3ObjectSelection {
    /// No nonempty byte range exists for an empty object. Fresh conditional
    /// acquisition still checks an actual If-Match GET, under narrow SDK authority.
    pub(super) async fn open_conditional_empty(
        &self,
    ) -> Result<aws_smithy_types::byte_stream::ByteStream, S3ReaderError> {
        let result = sdk::scoped(
            self.store
                .get_object()
                .bucket(&self.bucket)
                .key(self.key.as_ref())
                .if_match(&self.etag)
                .customize()
                .mutate_request(|request| request.add_extension(sdk::ConditionalEmptyRead))
                .send(),
        )
        .await
        .map_err(protocol_error)?;
        if !self.matches_version(result.version_id())
            || result.e_tag() != Some(&self.etag)
            || result.content_length() != Some(0)
            || result.content_range().is_some()
        {
            return Err(S3ReaderError::Changed);
        }
        Ok(result.body)
    }
}
