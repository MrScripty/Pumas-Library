//! Authored mutable-object file sets. Completeness belongs to the explicit set,
//! never to a listing or a claim that the bucket has an atomic snapshot.
use super::*;
use std::collections::BTreeMap;

/// Exact caller-authored conditional object facts. All members are mandatory;
/// ETag is a strong HTTP validator, while full SHA-256 supplies byte integrity.
#[derive(Clone)]
pub struct S3ConditionalManifestEntry {
    pub source_key: String,
    pub logical_path: String,
    pub expected_etag: String,
    pub expected_size: u64,
    pub expected_sha256: Sha256Evidence,
}

impl S3ConditionalManifestEntry {
    pub(crate) fn require_bounded_set(len: usize) -> Result<(), S3ReaderError> {
        if !(2..=32).contains(&len) {
            return Err(S3ReaderError::Configuration(
                "conditional manifests require 2–32 explicit members",
            ));
        }
        Ok(())
    }
}

impl S3Reader {
    /// Pure all-set preflight, before HEAD, acquisition or workspace admission.
    /// This bounded additive contract accepts 2–32 explicit members.
    pub fn validate_conditional_manifest_entries(
        &self,
        entries: &[S3ConditionalManifestEntry],
    ) -> Result<(), S3ReaderError> {
        S3ConditionalManifestEntry::require_bounded_set(entries.len())?;
        let mut entries: Vec<_> = entries.iter().collect();
        entries.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
        self.authored_conditional_manifest(&entries).map(|_| ())
    }

    fn authored_conditional_manifest(
        &self,
        entries: &[&S3ConditionalManifestEntry],
    ) -> Result<ArtifactManifest, S3ReaderError> {
        S3ConditionalManifestEntry::require_bounded_set(entries.len())?;
        let mut tags = BTreeMap::new();
        let mut pins = Vec::with_capacity(entries.len());
        let mut files = Vec::with_capacity(entries.len());
        for entry in entries {
            self.validated_key(&entry.source_key)?;
            if entry.expected_size > i64::MAX as u64
                || !conditional::strong_etag(&entry.expected_etag)
            {
                return Err(S3ReaderError::Configuration(
                    "conditional object requires SDK-representable size and strong ETag",
                ));
            }
            if tags
                .insert(&entry.source_key, &entry.expected_etag)
                .is_some_and(|known| known != &entry.expected_etag)
            {
                return Err(S3ReaderError::Configuration(
                    "one conditional object key has conflicting ETags",
                ));
            }
            pins.push((&entry.source_key, &entry.expected_etag, entry.expected_size));
            files.push(ArtifactFile::new(
                &entry.logical_path,
                &entry.source_key,
                Some(entry.expected_size),
                Some(entry.expected_sha256.clone()),
                FileVerificationRequirement::Sha256,
            )?);
        }
        let source = ArtifactSourceIdentity::new(
            "s3",
            format!("manifest:{}", self.explicit_scope_identity()),
            ArtifactRevisionEvidence::new(
                "s3.explicit_conditional_objects",
                serde_json::to_string(&pins).map_err(|_| sdk::protocol_failure())?,
                RevisionStrength::Weak,
            )?,
        )?;
        // Raw keys let shared validation reject conflicting size/digest evidence.
        // Logical paths, staging aliases, prefixes and aggregate sizes stay shared.
        Ok(ArtifactManifest::new(source, files)?)
    }

    /// Match every HEAD to the authored facts, then return that exact manifest.
    /// An absent/changed member returns no selection. GET still checks If-Match,
    /// range, returned size and actual bytes; the shared owner verifies all hashes.
    pub async fn select_conditional_manifest(
        &self,
        mut entries: Vec<S3ConditionalManifestEntry>,
    ) -> Result<S3ManifestSelection, S3ReaderError> {
        S3ConditionalManifestEntry::require_bounded_set(entries.len())?;
        entries.sort_by(|a, b| a.logical_path.cmp(&b.logical_path));
        let refs: Vec<_> = entries.iter().collect();
        let manifest = self.authored_conditional_manifest(&refs)?;
        let mut objects = Vec::with_capacity(entries.len());
        for entry in entries {
            let object = self
                .select_conditional(
                    &entry.source_key,
                    &entry.logical_path,
                    entry.expected_sha256,
                )
                .await?;
            if object.etag != entry.expected_etag || object.size != entry.expected_size {
                return Err(S3ReaderError::Changed);
            }
            objects.push(object);
        }
        Ok(S3ManifestSelection { manifest, objects })
    }
}
