use serde::{Deserialize, Deserializer, Serialize};
use std::collections::{HashMap, HashSet};
use thiserror::Error;

/// Current serialized artifact-manifest version.
pub const CURRENT_MANIFEST_VERSION: u16 = 1;

/// Whether a source revision is immutable or only identifies one complete read.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevisionStrength {
    Immutable,
    Weak,
}

/// A resolver's claim about the selected revision; construction validates its
/// shape, not its authority or mapping to the bytes that a reader returns.
///
/// The resolver supplies an authority such as `huggingface.commit` or
/// `s3.version_id`. Acquisition verifies that evidence with the corresponding
/// source adapter; the value is not itself permission to access or execute bytes.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactRevisionEvidence {
    authority: String,
    value: String,
    strength: RevisionStrength,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactRevisionEvidenceWire {
    authority: String,
    value: String,
    strength: RevisionStrength,
}

impl ArtifactRevisionEvidence {
    pub fn new(
        authority: impl Into<String>,
        value: impl Into<String>,
        strength: RevisionStrength,
    ) -> Result<Self, ManifestValidationError> {
        let authority = authority.into();
        let value = value.into();
        validate_evidence_authority(&authority)?;
        validate_stable_text(&value)?;
        Ok(Self {
            authority,
            value,
            strength,
        })
    }

    pub fn authority(&self) -> &str {
        &self.authority
    }

    pub fn value(&self) -> &str {
        &self.value
    }

    pub fn strength(&self) -> RevisionStrength {
        self.strength
    }
}

impl<'de> Deserialize<'de> for ArtifactRevisionEvidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ArtifactRevisionEvidenceWire::deserialize(deserializer)?;
        Self::new(wire.authority, wire.value, wire.strength).map_err(serde::de::Error::custom)
    }
}

/// Stable, non-secret identity of a selected source revision.
///
/// Retrieval URLs, credentials, and signed query strings belong to ephemeral
/// source access and are intentionally absent from this value.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactSourceIdentity {
    provider: String,
    source_id: String,
    revision: ArtifactRevisionEvidence,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactSourceIdentityWire {
    provider: String,
    source_id: String,
    revision: ArtifactRevisionEvidence,
}

impl ArtifactSourceIdentity {
    pub fn new(
        provider: impl Into<String>,
        source_id: impl Into<String>,
        revision: ArtifactRevisionEvidence,
    ) -> Result<Self, ManifestValidationError> {
        let provider = provider.into();
        let source_id = source_id.into();
        if !valid_provider(&provider) {
            return Err(ManifestValidationError::InvalidSourceIdentity);
        }
        validate_identity_text(&source_id)?;
        Ok(Self {
            provider,
            source_id,
            revision,
        })
    }

    pub fn provider(&self) -> &str {
        &self.provider
    }

    pub fn source_id(&self) -> &str {
        &self.source_id
    }

    pub fn revision(&self) -> &ArtifactRevisionEvidence {
        &self.revision
    }
}

impl<'de> Deserialize<'de> for ArtifactSourceIdentity {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ArtifactSourceIdentityWire::deserialize(deserializer)?;
        Self::new(wire.provider, wire.source_id, wire.revision).map_err(serde::de::Error::custom)
    }
}

/// Provenance for an expected SHA-256 supplied during source resolution.
/// Admission must still decide whether this authority is trusted for its use.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Sha256Evidence {
    authority: String,
    value: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Sha256EvidenceWire {
    authority: String,
    value: String,
}

impl Sha256Evidence {
    pub fn new(
        authority: impl Into<String>,
        value: impl Into<String>,
    ) -> Result<Self, ManifestValidationError> {
        let authority = authority.into();
        validate_evidence_authority(&authority)?;
        let value = canonical_sha256(&value.into())?;
        Ok(Self { authority, value })
    }

    pub fn authority(&self) -> &str {
        &self.authority
    }

    pub fn value(&self) -> &str {
        &self.value
    }
}

impl<'de> Deserialize<'de> for Sha256Evidence {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = Sha256EvidenceWire::deserialize(deserializer)?;
        Self::new(wire.authority, wire.value).map_err(serde::de::Error::custom)
    }
}

/// Minimum byte evidence the acquisition implementation must establish.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FileVerificationRequirement {
    Sha256,
    SizeAndImmutableRevision,
    CompleteRepresentation,
}

/// One selected logical file, opaque source key, and required verification
/// policy. The policy is a declared requirement, not byte verification.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactFile {
    logical_path: String,
    source_key: String,
    expected_size: Option<u64>,
    expected_sha256: Option<Sha256Evidence>,
    verification: FileVerificationRequirement,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactFileWire {
    logical_path: String,
    source_key: String,
    expected_size: Option<u64>,
    expected_sha256: Option<Sha256Evidence>,
    verification: FileVerificationRequirement,
}

impl ArtifactFile {
    pub fn new(
        logical_path: impl Into<String>,
        source_key: impl Into<String>,
        expected_size: Option<u64>,
        expected_sha256: Option<Sha256Evidence>,
        verification: FileVerificationRequirement,
    ) -> Result<Self, ManifestValidationError> {
        let logical_path = logical_path.into();
        let source_key = source_key.into();
        validate_logical_path(&logical_path)?;
        validate_source_key(&source_key)?;
        match (verification, expected_sha256.as_ref(), expected_size) {
            (FileVerificationRequirement::Sha256, None, _) => {
                return Err(ManifestValidationError::Sha256Required)
            }
            (FileVerificationRequirement::Sha256, Some(_), _) => {}
            (_, Some(_), _) => return Err(ManifestValidationError::UnexpectedSha256),
            (FileVerificationRequirement::SizeAndImmutableRevision, None, None) => {
                return Err(ManifestValidationError::ExpectedSizeRequired)
            }
            _ => {}
        }
        Ok(Self {
            logical_path,
            source_key,
            expected_size,
            expected_sha256,
            verification,
        })
    }

    pub fn logical_path(&self) -> &str {
        &self.logical_path
    }

    /// The protocol key is preserved exactly; it is never converted to a path
    /// or used to retain a retrieval URL. Source resolution owns that distinction.
    pub fn source_key(&self) -> &str {
        &self.source_key
    }

    pub fn expected_size(&self) -> Option<u64> {
        self.expected_size
    }

    pub fn expected_sha256(&self) -> Option<&Sha256Evidence> {
        self.expected_sha256.as_ref()
    }

    pub fn verification(&self) -> FileVerificationRequirement {
        self.verification
    }
}

impl<'de> Deserialize<'de> for ArtifactFile {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ArtifactFileWire::deserialize(deserializer)?;
        Self::new(
            wire.logical_path,
            wire.source_key,
            wire.expected_size,
            wire.expected_sha256,
            wire.verification,
        )
        .map_err(serde::de::Error::custom)
    }
}

/// A structurally validated selection of source objects and local logical
/// names. Construction does not authenticate the resolver's revision or digest
/// claims; the acquisition admission boundary must establish that mapping.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ArtifactManifest {
    schema_version: u16,
    source: ArtifactSourceIdentity,
    files: Vec<ArtifactFile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactManifestWire {
    schema_version: u16,
    source: ArtifactSourceIdentity,
    files: Vec<ArtifactFile>,
}

impl ArtifactManifest {
    pub fn new(
        source: ArtifactSourceIdentity,
        files: Vec<ArtifactFile>,
    ) -> Result<Self, ManifestValidationError> {
        Self::with_version(CURRENT_MANIFEST_VERSION, source, files)
    }

    fn with_version(
        schema_version: u16,
        source: ArtifactSourceIdentity,
        files: Vec<ArtifactFile>,
    ) -> Result<Self, ManifestValidationError> {
        if schema_version != CURRENT_MANIFEST_VERSION {
            return Err(ManifestValidationError::UnsupportedVersion(schema_version));
        }
        if files.is_empty() {
            return Err(ManifestValidationError::EmptyFileSet);
        }
        if source.revision.strength == RevisionStrength::Weak
            && files.iter().any(|file| {
                file.verification == FileVerificationRequirement::SizeAndImmutableRevision
            })
        {
            return Err(ManifestValidationError::ImmutableRevisionRequired);
        }

        let mut exact_paths = HashSet::with_capacity(files.len());
        let mut portable_paths = HashSet::with_capacity(files.len());
        let mut source_evidence = HashMap::with_capacity(files.len());
        let mut total = Some(0_u64);
        for file in &files {
            if !exact_paths.insert(file.logical_path.as_str()) {
                return Err(ManifestValidationError::DuplicateLogicalPath);
            }
            if !portable_paths.insert(case_insensitive_path_key(&file.logical_path)) {
                return Err(ManifestValidationError::CollidingLogicalPath);
            }
            let evidence = source_evidence
                .entry(file.source_key.as_str())
                .or_insert((None, None));
            if evidence
                .0
                .is_some_and(|known| file.expected_size.is_some_and(|expected| known != expected))
                || evidence.1.is_some_and(|known| {
                    file.expected_sha256
                        .as_ref()
                        .is_some_and(|expected| known != expected.value.as_str())
                })
            {
                return Err(ManifestValidationError::ConflictingSourceEvidence);
            }
            if evidence.0.is_none() {
                evidence.0 = file.expected_size;
            }
            if evidence.1.is_none() {
                evidence.1 = file
                    .expected_sha256
                    .as_ref()
                    .map(|digest| digest.value.as_str());
            }
            total = total.and_then(|sum| file.expected_size.and_then(|size| sum.checked_add(size)));
        }

        for path in &portable_paths {
            let mut prefix = String::new();
            let components: Vec<_> = path.split('/').collect();
            for component in components.iter().take(components.len().saturating_sub(1)) {
                if !prefix.is_empty() {
                    prefix.push('/');
                }
                prefix.push_str(component);
                if portable_paths.contains(&prefix) {
                    return Err(ManifestValidationError::CollidingLogicalPath);
                }
            }
        }

        if files.iter().all(|file| file.expected_size.is_some()) && total.is_none() {
            return Err(ManifestValidationError::SizeOverflow);
        }

        Ok(Self {
            schema_version,
            source,
            files,
        })
    }

    pub fn schema_version(&self) -> u16 {
        self.schema_version
    }

    pub fn source(&self) -> &ArtifactSourceIdentity {
        &self.source
    }

    pub fn files(&self) -> &[ArtifactFile] {
        &self.files
    }

    /// Whether the selected file has evidence that makes a retained partial
    /// safe to combine with a later response. An immutable source revision
    /// pins the representation; otherwise a whole-file digest can reject any
    /// mixed or changed representation before publication.
    pub fn permits_resume(&self, file_index: usize) -> bool {
        self.files.get(file_index).is_some_and(|file| {
            self.source.revision.strength == RevisionStrength::Immutable
                || file.expected_sha256.is_some()
        })
    }

    /// Whether a final or complete partial file has selected integrity
    /// evidence that must be checked before it can be reused or published.
    pub fn requires_file_verification(&self, file_index: usize) -> bool {
        self.files.get(file_index).is_some_and(|file| {
            self.source.revision.strength == RevisionStrength::Immutable
                || file.expected_sha256.is_some()
        })
    }

    /// Returns `None` when any selected file has an unknown size.
    pub fn total_bytes(&self) -> Option<u64> {
        self.files
            .iter()
            .try_fold(0_u64, |sum, file| sum.checked_add(file.expected_size?))
    }
}

impl<'de> Deserialize<'de> for ArtifactManifest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let wire = ArtifactManifestWire::deserialize(deserializer)?;
        Self::with_version(wire.schema_version, wire.source, wire.files)
            .map_err(serde::de::Error::custom)
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ManifestValidationError {
    #[error("artifact source identity is invalid")]
    InvalidSourceIdentity,
    #[error(
        "artifact source identity field is empty, contains URL material, or contains controls"
    )]
    InvalidSourceText,
    #[error("artifact source revision or digest authority is invalid")]
    InvalidEvidenceAuthority,
    #[error("artifact revision evidence contains an absolute retrieval URL")]
    InvalidRevisionEvidence,
    #[error("size-and-revision verification requires an immutable source revision")]
    ImmutableRevisionRequired,
    #[error("SHA-256 verification requires an expected digest")]
    Sha256Required,
    #[error("expected SHA-256 is inconsistent with the selected verification policy")]
    UnexpectedSha256,
    #[error("size-and-revision verification requires an expected size")]
    ExpectedSizeRequired,
    #[error("artifact file set is empty")]
    EmptyFileSet,
    #[error("artifact manifest version {0} is unsupported")]
    UnsupportedVersion(u16),
    #[error("artifact logical path is invalid")]
    InvalidLogicalPath,
    #[error("artifact source key is empty or contains control characters")]
    InvalidSourceKey,
    #[error("artifact SHA-256 must contain exactly 64 hexadecimal characters")]
    InvalidSha256,
    #[error("artifact manifest contains the same logical path more than once")]
    DuplicateLogicalPath,
    #[error("artifact manifest contains file/directory paths that collide under case-insensitive comparison")]
    CollidingLogicalPath,
    #[error("artifact source object has contradictory size or digest evidence")]
    ConflictingSourceEvidence,
    #[error("artifact manifest size total overflows u64")]
    SizeOverflow,
    #[error(
        "artifact partial cannot resume without immutable revision or expected digest evidence"
    )]
    ResumeIdentityRequired,
}

fn valid_provider(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
        && value.as_bytes()[0].is_ascii_lowercase()
}

fn validate_evidence_authority(value: &str) -> Result<(), ManifestValidationError> {
    if value.is_empty()
        || value.len() > 128
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte)
        })
        || !value.as_bytes()[0].is_ascii_lowercase()
    {
        return Err(ManifestValidationError::InvalidEvidenceAuthority);
    }
    Ok(())
}

fn validate_stable_text(value: &str) -> Result<(), ManifestValidationError> {
    if value.is_empty() || value.len() > 16 * 1024 || value.chars().any(char::is_control) {
        return Err(ManifestValidationError::InvalidRevisionEvidence);
    }
    if value.contains("://") {
        return Err(ManifestValidationError::InvalidRevisionEvidence);
    }
    Ok(())
}

fn validate_identity_text(value: &str) -> Result<(), ManifestValidationError> {
    if value.is_empty()
        || value.len() > 16 * 1024
        || value.chars().any(char::is_control)
        || value.contains("://")
        || value.contains(['?', '#'])
    {
        return Err(ManifestValidationError::InvalidSourceText);
    }
    Ok(())
}

fn validate_source_key(value: &str) -> Result<(), ManifestValidationError> {
    if value.is_empty() || value.len() > 16 * 1024 || value.chars().any(char::is_control) {
        return Err(ManifestValidationError::InvalidSourceKey);
    }
    Ok(())
}

fn validate_logical_path(value: &str) -> Result<(), ManifestValidationError> {
    // This checks source-independent traversal and common portability hazards.
    // Materialization still checks the actual target filesystem's name and
    // normalization collisions before creating any destination entry.
    if value.is_empty()
        || value.len() > 4096
        || value.starts_with('/')
        || value.contains(['\\', ':'])
        || value.chars().any(char::is_control)
    {
        return Err(ManifestValidationError::InvalidLogicalPath);
    }

    for component in value.split('/') {
        if component.is_empty()
            || component == "."
            || component == ".."
            || component.ends_with(['.', ' '])
        {
            return Err(ManifestValidationError::InvalidLogicalPath);
        }
        let stem = component
            .split('.')
            .next()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
            || (stem.len() == 4
                && (stem.starts_with("COM") || stem.starts_with("LPT"))
                && matches!(stem.as_bytes()[3], b'1'..=b'9'))
        {
            return Err(ManifestValidationError::InvalidLogicalPath);
        }
    }
    Ok(())
}

fn case_insensitive_path_key(value: &str) -> String {
    value
        .split('/')
        .map(|component| component.to_lowercase())
        .collect::<Vec<_>>()
        .join("/")
}

fn canonical_sha256(value: &str) -> Result<String, ManifestValidationError> {
    if value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(ManifestValidationError::InvalidSha256);
    }
    Ok(value.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn revision(strength: RevisionStrength) -> ArtifactRevisionEvidence {
        ArtifactRevisionEvidence::new("huggingface.commit", "commit-123", strength).unwrap()
    }

    fn source() -> ArtifactSourceIdentity {
        ArtifactSourceIdentity::new(
            "huggingface",
            "org/repository",
            revision(RevisionStrength::Immutable),
        )
        .unwrap()
    }

    fn complete_file(path: &str, source_key: &str, size: Option<u64>) -> ArtifactFile {
        ArtifactFile::new(
            path,
            source_key,
            size,
            None,
            FileVerificationRequirement::CompleteRepresentation,
        )
        .unwrap()
    }

    #[test]
    fn manifest_preserves_source_keys_and_reports_unknown_totals_truthfully() {
        let manifest = ArtifactManifest::new(
            source(),
            vec![
                complete_file(
                    "model/weights.safetensors",
                    "model/weights.safetensors",
                    Some(0),
                ),
                complete_file("model/config.json", "model/config.json?version=1", None),
            ],
        )
        .unwrap();

        assert_eq!(manifest.schema_version(), CURRENT_MANIFEST_VERSION);
        assert_eq!(
            manifest.files()[1].source_key(),
            "model/config.json?version=1"
        );
        assert_eq!(manifest.total_bytes(), None);
    }

    #[test]
    fn manifest_rejects_empty_duplicate_and_case_or_prefix_collisions() {
        assert_eq!(
            ArtifactManifest::new(source(), Vec::new()).unwrap_err(),
            ManifestValidationError::EmptyFileSet
        );
        let duplicate = complete_file("weights.bin", "one", Some(1));
        assert_eq!(
            ArtifactManifest::new(source(), vec![duplicate.clone(), duplicate]).unwrap_err(),
            ManifestValidationError::DuplicateLogicalPath
        );
        for paths in [
            ["Model/Weights.bin", "model/weights.BIN"],
            ["a", "a/b"],
            ["a/b", "a"],
            ["A", "a/b"],
        ] {
            let files = paths
                .into_iter()
                .map(|path| complete_file(path, path, Some(1)))
                .collect();
            assert_eq!(
                ArtifactManifest::new(source(), files).unwrap_err(),
                ManifestValidationError::CollidingLogicalPath
            );
        }
    }

    #[test]
    fn manifest_rejects_contradictory_evidence_for_one_source_object() {
        let first = ArtifactFile::new(
            "weights-a.bin",
            "objects/weights",
            Some(4),
            Some(Sha256Evidence::new("publisher.sha256", "a".repeat(64)).unwrap()),
            FileVerificationRequirement::Sha256,
        )
        .unwrap();
        let conflicting_digest = ArtifactFile::new(
            "weights-b.bin",
            "objects/weights",
            Some(4),
            Some(Sha256Evidence::new("publisher.sha256", "b".repeat(64)).unwrap()),
            FileVerificationRequirement::Sha256,
        )
        .unwrap();
        assert_eq!(
            ArtifactManifest::new(source(), vec![first.clone(), conflicting_digest]).unwrap_err(),
            ManifestValidationError::ConflictingSourceEvidence
        );

        let conflicting_size = complete_file("weights-b.bin", "objects/weights", Some(5));
        assert_eq!(
            ArtifactManifest::new(source(), vec![first, conflicting_size]).unwrap_err(),
            ManifestValidationError::ConflictingSourceEvidence
        );
    }

    #[test]
    fn manifest_rejects_unsafe_paths_but_preserves_opaque_protocol_keys() {
        for path in [
            "../weights.bin",
            "/weights.bin",
            "a\\b",
            "NUL.txt",
            "folder/../x",
        ] {
            assert!(
                ArtifactFile::new(
                    path,
                    "object",
                    Some(1),
                    None,
                    FileVerificationRequirement::CompleteRepresentation,
                )
                .is_err(),
                "{path}"
            );
        }
        let opaque_key = "object://bucket/key?version=1";
        let file = complete_file("weights.bin", opaque_key, Some(1));
        assert_eq!(file.source_key(), opaque_key);
    }

    #[test]
    fn sha256_policy_records_digest_authority_and_zero_size() {
        let digest = Sha256Evidence::new("huggingface.lfs.sha256", "A".repeat(64)).unwrap();
        let file = ArtifactFile::new(
            "empty",
            "empty-file",
            Some(0),
            Some(digest),
            FileVerificationRequirement::Sha256,
        )
        .unwrap();
        let manifest = ArtifactManifest::new(source(), vec![file]).unwrap();
        assert_eq!(manifest.total_bytes(), Some(0));
        assert_eq!(
            manifest.files()[0].expected_sha256().unwrap().value(),
            "a".repeat(64)
        );
        assert_eq!(
            manifest.files()[0].expected_sha256().unwrap().authority(),
            "huggingface.lfs.sha256"
        );
    }

    #[test]
    fn verification_policy_requires_its_declared_evidence() {
        assert_eq!(
            ArtifactFile::new(
                "weights.bin",
                "weights.bin",
                Some(1),
                None,
                FileVerificationRequirement::Sha256,
            )
            .unwrap_err(),
            ManifestValidationError::Sha256Required
        );
        let weak_source = ArtifactSourceIdentity::new(
            "huggingface",
            "org/repository",
            revision(RevisionStrength::Weak),
        )
        .unwrap();
        let file = ArtifactFile::new(
            "weights.bin",
            "weights.bin",
            Some(1),
            None,
            FileVerificationRequirement::SizeAndImmutableRevision,
        )
        .unwrap();
        assert_eq!(
            ArtifactManifest::new(weak_source.clone(), vec![file]).unwrap_err(),
            ManifestValidationError::ImmutableRevisionRequired
        );
        assert!(ArtifactManifest::new(
            weak_source,
            vec![complete_file("weights.bin", "weights.bin", None)]
        )
        .is_ok());
        let weak_source = ArtifactSourceIdentity::new(
            "huggingface",
            "org/repository",
            revision(RevisionStrength::Weak),
        )
        .unwrap();
        let hashed_file = ArtifactFile::new(
            "weights.bin",
            "weights.bin",
            Some(1),
            Some(Sha256Evidence::new("publisher.sha256", "a".repeat(64)).unwrap()),
            FileVerificationRequirement::Sha256,
        )
        .unwrap();
        assert!(ArtifactManifest::new(weak_source, vec![hashed_file]).is_ok());
    }

    #[test]
    fn deserialization_rejects_unknown_versions_fields_and_unvalidated_values() {
        let source = serde_json::to_value(source()).unwrap();
        let value = serde_json::json!({
            "schema_version": CURRENT_MANIFEST_VERSION + 1,
            "source": source,
            "files": [{"logical_path":"x","source_key":"x","expected_size":1,"expected_sha256":null,"verification":"complete_representation"}]
        });
        assert!(serde_json::from_value::<ArtifactManifest>(value).is_err());

        let malformed = serde_json::json!({
            "schema_version": CURRENT_MANIFEST_VERSION,
            "source": {"provider":"huggingface","source_id":"org/repo","revision":{"authority":"huggingface.commit","value":"commit","strength":"immutable"}},
            "files": [{"logical_path":"../x","source_key":"x","expected_size":1,"expected_sha256":null,"verification":"complete_representation"}]
        });
        assert!(serde_json::from_value::<ArtifactManifest>(malformed).is_err());

        let unknown_field = serde_json::json!({
            "schema_version": CURRENT_MANIFEST_VERSION,
            "source": {"provider":"huggingface","source_id":"org/repo","revision":{"authority":"huggingface.commit","value":"commit","strength":"immutable"}},
            "files": [{"logical_path":"x","source_key":"x","expected_size":1,"expected_sha256":null,"verification":"complete_representation","url":"https://example.invalid/x"}]
        });
        assert!(serde_json::from_value::<ArtifactManifest>(unknown_field).is_err());
    }

    #[test]
    fn source_evidence_and_manifest_totals_reject_unsafe_values() {
        assert_eq!(
            ArtifactRevisionEvidence::new(
                "huggingface.commit",
                "https://host/object?token=secret",
                RevisionStrength::Immutable,
            )
            .unwrap_err(),
            ManifestValidationError::InvalidRevisionEvidence
        );
        let files = vec![
            complete_file("first", "first", Some(u64::MAX)),
            complete_file("second", "second", Some(1)),
        ];
        assert_eq!(
            ArtifactManifest::new(source(), files).unwrap_err(),
            ManifestValidationError::SizeOverflow
        );
    }
}
