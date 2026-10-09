//! Explicit pinned file-set resolution. Every declared member resolves before
//! a selection is returned; prefix enumeration and package snapshot claims are absent.

use super::*;

/// One caller-declared member of an explicit S3 file set. Keys and logical paths
/// retain their distinct contracts; access authority comes only from the reader.
#[derive(Clone)]
pub struct S3ManifestEntry {
    pub source_key: String,
    pub version: String,
    pub logical_path: String,
    pub expected_sha256: Sha256Evidence,
}

/// Complete resolved set under one explicit reader authority. Public manifest
/// resolution requires exact versions; the model facade can wrap one digest-bound
/// conditional object without upgrading its weak revision evidence.
/// The revision preserves every per-object source identity in logical-path order.
/// This is an explicit selection, not an atomic snapshot of a remote prefix.
#[derive(Clone)]
pub struct S3ManifestSelection {
    pub(super) manifest: ArtifactManifest,
    pub(super) objects: Vec<S3ObjectSelection>,
}

impl S3ManifestSelection {
    pub fn manifest(&self) -> &ArtifactManifest {
        &self.manifest
    }

    pub(crate) fn from_single_object(object: S3ObjectSelection) -> Self {
        Self {
            manifest: object.manifest().clone(),
            objects: vec![object],
        }
    }

    pub(crate) fn into_parts(self) -> (ArtifactManifest, Vec<S3ObjectSelection>) {
        (self.manifest, self.objects)
    }
}

impl S3Reader {
    /// Resolve every explicitly requested immutable version. Structural/path/
    /// evidence refusal happens before HEAD; a missing member returns an error,
    /// never a shorter complete selection. No payload or workspace is admitted.
    /// Each HEAD retains the existing reader's operation budget. The existing
    /// 16-KiB revision bound limits the encoded pin set before any source call.
    pub async fn select_manifest(
        &self,
        mut entries: Vec<S3ManifestEntry>,
    ) -> Result<S3ManifestSelection, S3ReaderError> {
        entries.sort_by(|left, right| left.logical_path.cmp(&right.logical_path));
        let mut pins = Vec::with_capacity(entries.len());
        let mut files = Vec::with_capacity(entries.len());
        for entry in &entries {
            let (_, source) = self.validated_object(&entry.source_key, &entry.version)?;
            pins.push(source);
            files.push(ArtifactFile::new(
                &entry.logical_path,
                versioned_source_key(&entry.source_key, &entry.version)?,
                None,
                Some(entry.expected_sha256.clone()),
                FileVerificationRequirement::Sha256,
            )?);
        }
        let source = ArtifactSourceIdentity::new(
            "s3",
            format!("manifest:{}", self.explicit_scope_identity()),
            ArtifactRevisionEvidence::new(
                "s3.explicit_versions",
                serde_json::to_string(&pins)
                    .map_err(|_| S3ReaderError::Configuration("explicit pin encoding failed"))?,
                RevisionStrength::Immutable,
            )?,
        )?;
        // Shared manifest admission checks the complete namespace, duplicate
        // paths, staging aliases, evidence consistency and parent collisions.
        ArtifactManifest::new(source.clone(), files)?;
        let mut objects = Vec::with_capacity(entries.len());
        for entry in entries {
            objects.push(
                self.select(
                    &entry.source_key,
                    &entry.version,
                    &entry.logical_path,
                    entry.expected_sha256,
                )
                .await?,
            );
        }
        let files = objects
            .iter()
            .map(|object| {
                let file = &object.manifest.files()[0];
                Ok(ArtifactFile::new(
                    file.logical_path(),
                    versioned_source_key(
                        file.source_key(),
                        object.version.as_deref().ok_or(S3ReaderError::Changed)?,
                    )?,
                    file.expected_size(),
                    file.expected_sha256().cloned(),
                    file.verification(),
                )?)
            })
            .collect::<Result<Vec<_>, S3ReaderError>>()?;
        let manifest = ArtifactManifest::new(source, files)?;
        Ok(S3ManifestSelection { manifest, objects })
    }

    fn explicit_scope_identity(&self) -> String {
        let addressing = match self.addressing {
            S3Addressing::Path => "path",
            S3Addressing::VirtualHosted => "virtual_hosted",
        };
        format!(
            "{addressing}:{}:{}",
            hex::encode(self.endpoint.as_str()),
            hex::encode(&self.bucket)
        )
    }

    pub(super) fn object_scope_identity(&self, source_key: &str) -> String {
        format!(
            "{}:{}",
            self.explicit_scope_identity(),
            hex::encode(source_key)
        )
    }

    pub(super) fn validated_key(&self, source_key: &str) -> Result<Path, S3ReaderError> {
        let key = Path::parse(source_key)
            .map_err(|_| S3ReaderError::Configuration("unsupported object key"))?;
        if key.as_ref() != source_key || source_key.is_empty() || source_key.len() > 1024 {
            return Err(S3ReaderError::Configuration(
                "object key must preserve its exact identity",
            ));
        }
        Ok(key)
    }

    pub(super) fn validated_object(
        &self,
        source_key: &str,
        version: &str,
    ) -> Result<(Path, ArtifactSourceIdentity), S3ReaderError> {
        let key = self.validated_key(source_key)?;
        if version.is_empty() || version == "null" || version.chars().any(char::is_control) {
            return Err(S3ReaderError::Configuration(
                "an immutable VersionId is required",
            ));
        }
        let source = ArtifactSourceIdentity::new(
            "s3",
            self.object_scope_identity(source_key),
            ArtifactRevisionEvidence::new("s3.version_id", version, RevisionStrength::Immutable)?,
        )?;
        Ok((key, source))
    }
}

// Shared manifest admission compares evidence for equal opaque source keys.
// Within a multi-version selection, an object is identified by both key and
// VersionId. JSON tuple encoding keeps that pair unambiguous for arbitrary
// accepted keys/versions. Reader requests and per-object pins retain the raw key.
fn versioned_source_key(key: &str, version: &str) -> Result<String, S3ReaderError> {
    serde_json::to_string(&(key, version))
        .map_err(|_| S3ReaderError::Configuration("versioned object key encoding failed"))
}
