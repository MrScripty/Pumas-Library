//! Closed S3 desktop wire. Credentials are one-request access material only.
use super::{PublicError, PublicErrorClass};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3CredentialParams {
    #[cfg_attr(
        feature = "export-contract",
        schemars(
            length(min = 1, max = 4096),
            regex(pattern = "^(?!.*[/,=])[!-~]+(?![\\s\\S])")
        )
    )]
    access_key_id: String,
    #[cfg_attr(
        feature = "export-contract",
        schemars(length(min = 1, max = 4096), regex(pattern = "^[!-~]+(?![\\s\\S])"))
    )]
    secret_access_key: String,
    #[cfg_attr(
        feature = "export-contract",
        schemars(length(min = 1, max = 4096), regex(pattern = "^[!-~]+(?![\\s\\S])"))
    )]
    session_token: Option<String>,
}
impl S3CredentialParams {
    pub(super) fn validate(&self) -> Result<(), PublicError> {
        for value in [&self.access_key_id, &self.secret_access_key]
            .into_iter()
            .chain(self.session_token.as_ref())
        {
            if value.is_empty()
                || value.len() > 4096
                || !value.bytes().all(|b| (0x21..=0x7e).contains(&b))
            {
                return Err(PublicError::invalid_params());
            }
        }
        if self.access_key_id.contains(['/', ',', '=']) {
            return Err(PublicError::invalid_params());
        }
        Ok(())
    }
    #[cfg(feature = "s3")]
    pub(crate) fn preflight(
        &self,
        config: pumas_library::acquisition::S3ReaderConfig,
    ) -> Result<(), PublicError> {
        self.validate()?;
        // Bounded temporary copies validate with the real constructor before
        // admission. They are dropped here; the owned originals enter the job.
        let credentials = pumas_library::acquisition::S3Credentials::new(
            self.access_key_id.clone(),
            self.secret_access_key.clone(),
            self.session_token.clone(),
        )
        .map_err(|_| PublicError::invalid_params())?;
        pumas_library::acquisition::S3Reader::new_authenticated(config, credentials)
            .map(|_| ())
            .map_err(|_| PublicError::invalid_params())
    }
    #[cfg(feature = "s3")]
    pub(crate) fn into_native(
        self,
    ) -> Result<pumas_library::acquisition::S3Credentials, PublicError> {
        self.validate()?;
        pumas_library::acquisition::S3Credentials::new(
            self.access_key_id,
            self.secret_access_key,
            self.session_token,
        )
        .map_err(|_| PublicError::invalid_params())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3AuthenticatedImportParams {
    pub source: S3ImportParams,
    pub credentials: S3CredentialParams,
}
impl S3AuthenticatedImportParams {
    pub(crate) fn validate(&self) -> Result<(), PublicError> {
        self.source.validate()?;
        self.credentials.validate()
    }
}

/// Opt-in acquisition contract; omission preserves mandatory VersionId pins.
#[derive(Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum S3ReadMode {
    #[default]
    VersionId,
    Conditional,
}

pub(crate) fn require_versioned_set(mode: S3ReadMode) -> Result<(), PublicError> {
    if mode == S3ReadMode::Conditional {
        return Err(PublicError {
            code: -32000,
            class: PublicErrorClass::Unavailable,
            message: "Conditional S3 mode is unsupported for prefix discovery.",
        });
    }
    Ok(())
}

// Only omission produces the private empty default. A supplied empty value is
// refused, so conditional mode must omit VersionId rather than invent a pin.
fn supplied_version_id<'de, D: serde::Deserializer<'de>>(decoder: D) -> Result<String, D::Error> {
    let value = String::deserialize(decoder)?;
    if value.is_empty() {
        return Err(serde::de::Error::custom(
            "Supplied VersionId must be nonempty",
        ));
    }
    Ok(value)
}

#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3ImportParams {
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(
            pattern = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
        ))
    )]
    pub operation_id: String,
    #[cfg_attr(
        feature = "export-contract",
        schemars(length(min = 1, max = 4096), regex(pattern = "^https://[^/@?#]+/?$"))
    )]
    pub endpoint: String,
    #[cfg_attr(feature = "export-contract", schemars(length(min = 1, max = 255)))]
    pub region: String,
    #[cfg_attr(feature = "export-contract", schemars(length(min = 1, max = 255)))]
    pub bucket: String,
    pub addressing: S3AddressingWire,
    #[cfg_attr(
        feature = "export-contract",
        schemars(
            length(min = 1, max = 1024),
            regex(
                pattern = "^(?!\\.{1,2}(?:/|$))[^/\\u0000-\\u001F\\u007F-\\u009F]+(?:/(?!\\.{1,2}(?:/|$))[^/\\u0000-\\u001F\\u007F-\\u009F]+)*$"
            )
        )
    )]
    pub key: String,
    #[serde(default)]
    pub read_mode: S3ReadMode,
    #[serde(default, deserialize_with = "supplied_version_id")]
    #[cfg_attr(
        feature = "export-contract",
        schemars(
            length(min = 1, max = 4096),
            regex(pattern = "^(?!null$)[^\\u0000-\\u001F\\u007F-\\u009F]+$")
        )
    )]
    pub version_id: String,
    #[cfg_attr(
        feature = "export-contract",
        schemars(
            length(min = 1, max = 1024),
            regex(pattern = "^[A-Za-z0-9][A-Za-z0-9._-]*(?:/[A-Za-z0-9][A-Za-z0-9._-]*)*$")
        )
    )]
    pub filename: String,
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(pattern = "^[0-9a-fA-F]{64}$"))
    )]
    pub sha256: String,
    #[cfg_attr(feature = "export-contract", schemars(length(min = 1, max = 255)))]
    pub family: String,
    #[cfg_attr(feature = "export-contract", schemars(length(min = 1, max = 255)))]
    pub official_name: String,
}
#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum S3AddressingWire {
    Path,
    VirtualHosted,
}

/// Exact selected model/package bytes. Shared import qualification follows transfer.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3BundleImportParams {
    #[serde(default)]
    pub read_mode: S3ReadMode,
    pub operation_id: String,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub addressing: S3AddressingWire,
    #[cfg_attr(feature = "export-contract", schemars(length(min = 2, max = 32)))]
    pub files: Vec<S3SelectedFileParams>,
    pub primary_logical_path: String,
    pub family: String,
    pub official_name: String,
}
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3PinnedFileParams {
    pub key: String,
    pub version_id: String,
    pub logical_path: String,
    pub sha256: String,
}
/// Closed authored facts; no VersionId can be supplied for a mutable object.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3ConditionalFileParams {
    pub key: String,
    pub logical_path: String,
    pub sha256: String,
    pub expected_etag: String,
    /// Canonical decimal preserves the SDK's full signed-64 size range in JS.
    pub expected_size: String,
}
#[derive(Clone, Deserialize)]
#[serde(untagged)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum S3SelectedFileParams {
    Versioned(S3PinnedFileParams),
    Conditional(S3ConditionalFileParams),
}
impl S3SelectedFileParams {
    fn facts(&self) -> (&str, &str, &str, &str) {
        match self {
            Self::Versioned(file) => (
                &file.key,
                &file.logical_path,
                &file.sha256,
                &file.version_id,
            ),
            Self::Conditional(file) => (&file.key, &file.logical_path, &file.sha256, ""),
        }
    }
}
impl S3ConditionalFileParams {
    fn size(&self) -> Result<u64, PublicError> {
        let value = &self.expected_size;
        if value.is_empty()
            || value.len() > 19
            || !value.bytes().all(|b| b.is_ascii_digit())
            || (value.len() > 1 && value.starts_with('0'))
        {
            return Err(PublicError::invalid_params());
        }
        value
            .parse::<u64>()
            .ok()
            .filter(|size| *size <= i64::MAX as u64)
            .ok_or_else(PublicError::invalid_params)
    }
    fn validate_etag(&self) -> Result<(), PublicError> {
        let tag = &self.expected_etag;
        // Exact native HTTP opaque-tag grammar, including empty/Unicode tags.
        if tag.len() >= 2
            && tag.starts_with('"')
            && tag.ends_with('"')
            && tag[1..tag.len() - 1]
                .bytes()
                .all(|byte| byte == 0x21 || (0x23..=0x7e).contains(&byte) || byte >= 0x80)
        {
            Ok(())
        } else {
            Err(PublicError::invalid_params())
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3AuthenticatedBundleImportParams {
    pub source: S3BundleImportParams,
    pub credentials: S3CredentialParams,
}
impl S3AuthenticatedBundleImportParams {
    pub(crate) fn validate(&self) -> Result<(), PublicError> {
        self.source.validate()?;
        self.credentials.validate()
    }
}
impl S3BundleImportParams {
    pub(crate) fn primary(&self) -> Result<S3ImportParams, PublicError> {
        if !(2..=32).contains(&self.files.len()) {
            return Err(PublicError::invalid_params());
        }
        let primary = self
            .files
            .iter()
            .find(|file| file.facts().1 == self.primary_logical_path)
            .ok_or_else(PublicError::invalid_params)?;
        let (key, _, sha256, version_id) = primary.facts();
        Ok(S3ImportParams {
            operation_id: self.operation_id.clone(),
            endpoint: self.endpoint.clone(),
            region: self.region.clone(),
            bucket: self.bucket.clone(),
            addressing: self.addressing,
            key: key.into(),
            version_id: version_id.into(),
            read_mode: self.read_mode,
            filename: self.primary_logical_path.clone(),
            sha256: sha256.into(),
            family: self.family.clone(),
            official_name: self.official_name.clone(),
        })
    }
    pub(crate) fn validate(&self) -> Result<(), PublicError> {
        if !(2..=32).contains(&self.files.len()) {
            return Err(PublicError::invalid_params());
        }
        let primary = self.primary()?;
        primary.validate()?;
        let mut sorted: Vec<_> = self.files.iter().collect();
        sorted.sort_by(|a, b| a.facts().1.cmp(b.facts().1));
        let mut files = Vec::with_capacity(sorted.len());
        let mut tags = std::collections::BTreeMap::new();
        let mut pins = Vec::with_capacity(sorted.len());
        for file in sorted {
            let (key, path, sha256, version_id) = file.facts();
            if path.is_empty() || path.len() > 1024 || path.chars().any(char::is_control) {
                return Err(PublicError::invalid_params());
            }
            let member = S3ImportParams {
                key: key.into(),
                version_id: version_id.into(),
                sha256: sha256.into(),
                ..primary.clone()
            };
            member.validate()?;
            let (source_key, size) = match (self.read_mode, file) {
                (S3ReadMode::VersionId, S3SelectedFileParams::Versioned(_)) => (
                    serde_json::to_string(&(key, version_id))
                        .map_err(|_| PublicError::invalid_params())?,
                    None,
                ),
                (S3ReadMode::Conditional, S3SelectedFileParams::Conditional(file)) => {
                    file.validate_etag()?;
                    let size = file.size()?;
                    if tags
                        .insert(key, file.expected_etag.as_str())
                        .is_some_and(|old| old != file.expected_etag)
                    {
                        return Err(PublicError::invalid_params());
                    }
                    pins.push((key, file.expected_etag.as_str(), size));
                    (key.into(), Some(size))
                }
                _ => return Err(PublicError::invalid_params()),
            };
            files.push(
                pumas_library::acquisition::ArtifactFile::new(
                    path,
                    source_key,
                    size,
                    Some(
                        pumas_library::acquisition::Sha256Evidence::new("caller.sha256", sha256)
                            .map_err(|_| PublicError::invalid_params())?,
                    ),
                    pumas_library::acquisition::FileVerificationRequirement::Sha256,
                )
                .map_err(|_| PublicError::invalid_params())?,
            );
        }
        let (authority, revision, strength) = match self.read_mode {
            S3ReadMode::VersionId => (
                "s3.explicit_versions",
                "desktop.explicit".into(),
                pumas_library::acquisition::RevisionStrength::Immutable,
            ),
            S3ReadMode::Conditional => (
                "s3.explicit_conditional_objects",
                serde_json::to_string(&pins).map_err(|_| PublicError::invalid_params())?,
                pumas_library::acquisition::RevisionStrength::Weak,
            ),
        };
        let source = pumas_library::acquisition::ArtifactSourceIdentity::new(
            "s3",
            "desktop.preflight",
            pumas_library::acquisition::ArtifactRevisionEvidence::new(
                authority, revision, strength,
            )
            .map_err(|_| PublicError::invalid_params())?,
        )
        .map_err(|_| PublicError::invalid_params())?;
        pumas_library::acquisition::ArtifactManifest::new(source, files)
            .map_err(|_| PublicError::invalid_params())?;
        pumas_library::model_library::ModelImporter::validate_acquired_payload_paths(
            &self
                .files
                .iter()
                .map(|file| file.facts().1)
                .collect::<Vec<_>>(),
        )
        .map_err(|_| PublicError::invalid_params())
    }
    #[cfg(feature = "s3")]
    pub(crate) fn native_entries(&self) -> Result<Vec<BundleEntry>, PublicError> {
        self.files
            .iter()
            .map(|file| match file {
                S3SelectedFileParams::Versioned(file) => Ok(BundleEntry {
                    source_key: file.key.clone(),
                    version: file.version_id.clone(),
                    logical_path: file.logical_path.clone(),
                    expected_sha256: pumas_library::acquisition::Sha256Evidence::new(
                        "caller.sha256",
                        file.sha256.clone(),
                    )
                    .map_err(|_| PublicError::invalid_params())?,
                }),
                _ => Err(PublicError::invalid_params()),
            })
            .collect()
    }
    #[cfg(feature = "s3")]
    pub(crate) fn native_conditional_entries(
        &self,
    ) -> Result<Vec<pumas_library::acquisition::S3ConditionalManifestEntry>, PublicError> {
        self.files
            .iter()
            .map(|file| match file {
                S3SelectedFileParams::Conditional(file) => {
                    Ok(pumas_library::acquisition::S3ConditionalManifestEntry {
                        source_key: file.key.clone(),
                        logical_path: file.logical_path.clone(),
                        expected_etag: file.expected_etag.clone(),
                        expected_size: file.size()?,
                        expected_sha256: pumas_library::acquisition::Sha256Evidence::new(
                            "caller.sha256",
                            file.sha256.clone(),
                        )
                        .map_err(|_| PublicError::invalid_params())?,
                    })
                }
                _ => Err(PublicError::invalid_params()),
            })
            .collect()
    }
}
// Pure DTO/shared-manifest preflight above also applies to builds without S3.
#[cfg(feature = "s3")]
type BundleEntry = pumas_library::acquisition::S3ManifestEntry;
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3BundleProgressWire {
    pub file_index: Option<u32>,
    pub files_total: u32,
    pub files_acquired: u32,
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))
    )]
    pub bytes_acquired: String,
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))
    )]
    pub total_expected_bytes: Option<String>,
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))
    )]
    pub total_bytes_observed: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3BundleImportObservation {
    pub outcome: S3ImportOutcome,
    pub bundle_progress: Option<S3BundleProgressWire>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3ImportStatusParams {
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(
            pattern = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
        ))
    )]
    pub operation_id: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3ImportCancelParams {
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(
            pattern = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
        ))
    )]
    pub operation_id: String,
}
/// Explicit same-process retry. Original source/pins/intent are held by the owner.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3TransferRetryParams {
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(
            pattern = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
        ))
    )]
    pub operation_id: String,
    pub credentials: Option<S3CredentialParams>,
}
impl S3TransferRetryParams {
    pub(crate) fn validate(&self) -> Result<(), PublicError> {
        validate_s3_id(&self.operation_id)?;
        if let Some(credentials) = &self.credentials {
            credentials.validate()?;
        }
        Ok(())
    }
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum S3TransferRetryState {
    Ready {
        #[cfg_attr(
            feature = "export-contract",
            schemars(regex(
                pattern = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
            ))
        )]
        operation_id: String,
        authentication_required: bool,
    },
    Unavailable {
        reason: S3TransferRetryReason,
    },
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum S3TransferRetryReason {
    NoLiveCustody,
    Busy,
    NotRetryable,
}

fn valid_id(id: &str) -> bool {
    id.len() == 36
        && id.bytes().enumerate().all(|(i, c)| {
            if [8, 13, 18, 23].contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_digit() || (b'a'..=b'f').contains(&c)
            }
        })
}
pub(crate) fn validate_s3_id(id: &str) -> Result<(), PublicError> {
    if valid_id(id) {
        Ok(())
    } else {
        Err(PublicError::invalid_params())
    }
}
impl S3ImportParams {
    pub(crate) fn validate(&self) -> Result<(), PublicError> {
        validate_s3_id(&self.operation_id)?;
        for (value, max) in [
            (&self.endpoint, 4096),
            (&self.region, 255),
            (&self.bucket, 255),
            (&self.key, 1024),
            (&self.filename, 1024),
            (&self.family, 255),
            (&self.official_name, 255),
        ] {
            if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
                return Err(PublicError::invalid_params());
            }
        }
        match self.read_mode {
            S3ReadMode::VersionId
                if self.version_id.trim().is_empty()
                    || self.version_id == "null"
                    || self.version_id.len() > 4096
                    || self.version_id.chars().any(char::is_control) =>
            {
                return Err(PublicError::invalid_params());
            }
            S3ReadMode::Conditional if !self.version_id.is_empty() => {
                return Err(PublicError::invalid_params());
            }
            _ => {}
        }
        // The frozen reader's validated_object uses object_store::Path::parse
        // and requires exact preservation: no empty, dot or parent segments,
        // including leading/trailing delimiters. Reject the same structural
        // pins before admitting a job or allocating its reservation.
        if self
            .key
            .split('/')
            .any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(PublicError::invalid_params());
        }
        if self.filename.split('/').any(|component| {
            component.is_empty()
                || !component.as_bytes()[0].is_ascii_alphanumeric()
                || !component
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(&b))
        }) {
            return Err(PublicError::invalid_params());
        }
        // Source-independent logical-path admission; extensions grant no model
        // authority. Full namespace/hash and package validation happen later.
        pumas_library::acquisition::ArtifactFile::new(
            self.filename.clone(),
            "desktop.preflight",
            None,
            None,
            pumas_library::acquisition::FileVerificationRequirement::CompleteRepresentation,
        )
        .map_err(|_| PublicError::invalid_params())?;
        // Check destination normalization and the namespace root. Nested
        // primaries also enter package requests, which preserve their layout.
        // Importer documents must be refused before workspace allocation.
        for path in [
            self.filename.as_str(),
            self.filename.split('/').next().unwrap_or_default(),
        ] {
            pumas_library::model_library::ModelImporter::validate_acquired_payload_paths(&[path])
                .map_err(|_| PublicError::invalid_params())?;
        }
        if self.sha256.len() != 64 || !self.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(PublicError::invalid_params());
        }
        let url = url::Url::parse(&self.endpoint).map_err(|_| PublicError::invalid_params())?;
        if !self.endpoint.starts_with("https://")
            || self.endpoint.trim() != self.endpoint
            || url.scheme() != "https"
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
        {
            return Err(PublicError::invalid_params());
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum S3ImportPhaseWire {
    Pending,
    Selecting,
    Acquiring,
    Cancelling,
    Finalizing,
    Completed,
    Cancelled,
    Failed,
    Interrupted,
}
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3ImportProgressWire {
    pub phase: S3ImportPhaseWire,
    /// Decimal string preserves u64 evidence across JavaScript's integer boundary.
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))
    )]
    pub downloaded_for_current_file: String,
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum S3ImportResultWire {
    Completed {
        model_id: String,
    },
    Cancelled {
        retained_work: bool,
    },
    Failed {
        error: PublicError,
        retained_work: bool,
        published_model_id: Option<String>,
    },
}
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum S3ImportOutcome {
    Unavailable,
    Rejected {
        error: PublicError,
    },
    Idle,
    NotFound {
        operation_id: String,
    },
    Running {
        operation_id: String,
        progress: S3ImportProgressWire,
    },
    Finished {
        operation_id: String,
        result: S3ImportResultWire,
    },
}
#[derive(Clone, Debug, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3ImportCancelOutcome {
    pub accepted: bool,
    pub outcome: S3ImportOutcome,
}

pub(crate) fn s3_conflict() -> PublicError {
    PublicError {
        code: -32003,
        class: PublicErrorClass::Conflict,
        message: "S3 import needs its existing result or retained-work reconciliation.",
    }
}

#[cfg(test)]
mod tests {
    use super::super::{AdmittedRpcRequest, RpcCommand};
    use serde_json::{json, Value};
    const ID: &str = "c3f7d104-1234-4321-abcd-aaaaaaaaaaaa";
    pub(crate) fn params() -> Value {
        json!({"operation_id":ID,"endpoint":"https://source.invalid","region":"fixture-region","bucket":"fixture-bucket","addressing":"path","key":"models/weights.gguf","version_id":"desktop-v1","filename":"weights.gguf","sha256":"a".repeat(64),"family":"fixture","official_name":"Desktop GGUF"})
    }
    fn decode(
        method: &str,
        params: Value,
    ) -> Result<AdmittedRpcRequest, super::super::RpcAdmissionError> {
        AdmittedRpcRequest::decode(
            &serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
                .unwrap(),
        )
    }
    #[test]
    fn retry_wire_never_admits_replacement_pins_or_credentials_on_status() {
        assert!(decode("get_s3_transfer_retry", json!({"operation_id":ID})).is_ok());
        assert!(decode(
            "retry_s3_model_transfer",
            json!({"operation_id":ID,"credentials":null})
        )
        .is_ok());
        for key in [
            "source",
            "endpoint",
            "region",
            "bucket",
            "addressing",
            "key",
            "version_id",
            "sha256",
            "files",
            "family",
            "stage",
        ] {
            let input = json!({"operation_id":ID,"credentials":null,key:"altered"});
            assert_eq!(
                decode("retry_s3_model_transfer", input)
                    .err()
                    .unwrap()
                    .error
                    .code,
                -32602
            );
        }
        assert!(decode(
            "get_s3_transfer_retry",
            json!({"operation_id":ID,"credentials":{}})
        )
        .is_err());
    }

    #[test]
    fn source_commands_are_closed_and_reject_secrets_bad_pins_and_origins() {
        assert!(matches!(
            decode("start_s3_model_import", params())
                .ok()
                .unwrap()
                .command,
            RpcCommand::StartS3ModelImport { .. }
        ));
        assert!(matches!(
            decode("get_s3_model_import", json!({}))
                .ok()
                .unwrap()
                .command,
            RpcCommand::GetS3ModelImport { .. }
        ));
        assert!(matches!(
            decode("cancel_s3_model_import", json!({"operation_id":ID}))
                .ok()
                .unwrap()
                .command,
            RpcCommand::CancelS3ModelImport { .. }
        ));
        for (field, value) in [
            ("credentials", json!({"secret":"synthetic-source-secret"})),
            ("session_token", json!("synthetic-session-token")),
            ("allow_http", json!(true)),
            ("version_id", json!("")),
            ("version_id", json!("null")),
            ("key", json!("../weights.gguf")),
            ("key", json!("models/./weights.gguf")),
            ("key", json!("models//weights.gguf")),
            ("key", json!("/weights.gguf")),
            ("key", json!("weights.gguf/")),
            ("operation_id", json!("not-an-operation")),
            ("sha256", json!("a".repeat(63))),
            ("filename", json!("../weights.gguf")),
            ("addressing", json!("automatic")),
            ("endpoint", json!("http://127.0.0.1:1")),
            (
                "endpoint",
                json!("https://synthetic-key:synthetic-secret@source.invalid"),
            ),
            ("endpoint", json!("https://source.invalid/path")),
            (
                "endpoint",
                json!("https://source.invalid?token=synthetic-secret"),
            ),
        ] {
            let mut input = params();
            input[field] = value;
            let error = decode("start_s3_model_import", input)
                .err()
                .expect("must reject before dispatch");
            assert_eq!(error.error.code, -32602);
            let projected = serde_json::to_string(&error.error).unwrap();
            assert!(!projected.contains("synthetic"));
        }
        for method in ["get_s3_model_import", "cancel_s3_model_import"] {
            assert!(decode(
                method,
                json!({"operation_id":ID,"secret_access_key":"synthetic-source-secret"})
            )
            .is_err());
            assert!(decode(method, json!({"operation_id":""})).is_err());
        }
    }

    #[test]
    fn source_authenticated_command_is_closed_bounded_and_never_echoes_credentials() {
        let credentials = json!({"access_key_id":"synthetic-contract-key","secret_access_key":"synthetic-contract-secret","session_token":"synthetic-contract-token"});
        for token in [Value::Null, json!("synthetic-contract-token")] {
            let mut input = json!({"source":params(),"credentials":credentials});
            input["credentials"]["session_token"] = token;
            assert!(matches!(
                decode("start_authenticated_s3_model_import", input)
                    .ok()
                    .unwrap()
                    .command,
                RpcCommand::StartAuthenticatedS3ModelImport { .. }
            ));
        }
        for (field, value) in [
            ("access_key_id", json!("bad/key")),
            ("secret_access_key", json!("")),
            ("secret_access_key", json!("bad\n")),
            ("secret_access_key", json!("x".repeat(4097))),
            ("session_token", json!("")),
            ("session_token", json!("bad\n")),
            ("profile", json!("synthetic-contract-secret")),
        ] {
            let mut input = json!({"source":params(),"credentials":credentials});
            input["credentials"][field] = value;
            let failure = decode("start_authenticated_s3_model_import", input)
                .err()
                .expect("invalid credentials must be refused");
            assert_eq!(failure.error.code, -32602);
            assert!(!serde_json::to_string(&failure.error)
                .unwrap()
                .contains("synthetic-contract"));
        }
        for input in [
            json!({"source":params(),"credentials":credentials,"saved":true}),
            json!({"source":params()}),
            json!({"credentials":credentials}),
        ] {
            assert!(decode("start_authenticated_s3_model_import", input).is_err());
        }
    }
}

#[cfg(test)]
mod bundle_contract_tests {
    use super::super::{AdmittedRpcRequest, RpcCommand};
    use serde_json::{json, Value};
    fn params() -> Value {
        json!({"operation_id":"c3f7d104-1234-4321-abcd-aaaaaaaaaaaa","endpoint":"https://source.invalid","region":"fixture-region","bucket":"fixture-bucket","addressing":"path","primary_logical_path":"weights.gguf","family":"fixture","official_name":"Fixture",
        "files":[{"key":"models/shared","version_id":"v1","logical_path":"weights.gguf","sha256":"a".repeat(64)}, {"key":"models/shared","version_id":"v2","logical_path":"config/data.json","sha256":"b".repeat(64)}]})
    }
    fn decode(
        method: &str,
        params: Value,
    ) -> Result<super::super::AdmittedRpcRequest, super::super::RpcAdmissionError> {
        AdmittedRpcRequest::decode(
            &serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
                .unwrap(),
        )
    }
    #[test]
    fn complete_bundle_wire_requires_every_pin_and_closed_ephemeral_credentials() {
        assert!(matches!(
            decode("start_s3_model_bundle_import", params())
                .ok()
                .unwrap()
                .command,
            RpcCommand::StartS3ModelBundleImport { .. }
        ));
        let credentials = json!({"access_key_id":"synthetic-bundle-key","secret_access_key":"synthetic-bundle-secret","session_token":null});
        assert!(decode(
            "start_authenticated_s3_model_bundle_import",
            json!({"source":params(),"credentials":credentials})
        )
        .is_ok());
        for field in ["key", "version_id", "logical_path", "sha256"] {
            let mut input = params();
            input["files"][1].as_object_mut().unwrap().remove(field);
            assert!(decode("start_s3_model_bundle_import", input).is_err());
        }
        for input in [
            json!({"source":params(),"credentials":credentials,"saved":true}),
            json!({"source":params(),"credentials":{"access_key_id":"synthetic-bundle-key","secret_access_key":"synthetic-bundle-secret","profile":"synthetic-bundle-secret"}}),
        ] {
            let failed = decode("start_authenticated_s3_model_bundle_import", input)
                .err()
                .unwrap();
            assert!(!serde_json::to_string(&failed.error)
                .unwrap()
                .contains("synthetic-bundle"));
        }
        for length in [0, 1, 33] {
            let mut input = params();
            input["files"] = json!(vec![input["files"][0].clone(); length]);
            assert!(decode("start_s3_model_bundle_import", input).is_err());
        }
        let mut duplicate = params();
        duplicate["files"][1]["logical_path"] = json!("weights.gguf");
        assert!(decode("start_s3_model_bundle_import", duplicate).is_err());
        let mut input = params();
        input["files"][1]["credentials"] = credentials;
        assert!(decode("start_s3_model_bundle_import", input).is_err());
        assert!(decode("get_s3_model_bundle_import", json!({})).is_ok());
    }
}

#[cfg(test)]
mod public_bridge_contract_tests {
    use crate::contract::{AdmittedRpcRequest, RpcAdmissionError};

    fn decode(method: &str, params: Value) -> Result<AdmittedRpcRequest, RpcAdmissionError> {
        AdmittedRpcRequest::decode(
            &serde_json::to_vec(&json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}))
                .unwrap(),
        )
    }
    use serde_json::{json, Value};

    fn file(path: &str) -> Value {
        json!({"operation_id":"c3f7d104-1234-4321-abcd-aaaaaaaaaaaa",
            "endpoint":"https://source.invalid","region":"fixture-region",
            "bucket":"fixture-bucket","addressing":"path","key":"models/exact",
            "version_id":"v1","filename":path,"sha256":"a".repeat(64),
            "family":"fixture","official_name":"Public bridge"})
    }
    fn package(primary: &str) -> Value {
        let mut request = file(primary);
        for key in ["key", "version_id", "filename", "sha256"] {
            request.as_object_mut().unwrap().remove(key);
        }
        request["primary_logical_path"] = json!(primary);
        request["files"] = json!([
            {"key":"models/weights","version_id":"v1","logical_path":primary,"sha256":"a".repeat(64)},
            {"key":"models/other","version_id":"v2","logical_path":"model-00002.safetensors","sha256":"b".repeat(64)},
            {"key":"models/index","version_id":"v3","logical_path":"model.safetensors.index.json","sha256":"c".repeat(64)}]);
        request
    }
    #[test]
    fn transport_admission_does_not_claim_model_qualification() {
        for path in [
            "weights.gguf",
            "model.safetensors",
            "model.onnx",
            "unknown.data",
        ] {
            assert!(
                decode("start_s3_model_import", file(path)).is_ok(),
                "{path}"
            );
            assert!(decode("start_authenticated_s3_model_import", json!({
                "source":file(path),"credentials":{"access_key_id":"fixture-key", "secret_access_key":"fixture-secret"}
            })).is_ok());
        }
    }
    #[test]
    fn package_selection_admits_weight_members_and_nested_primary() {
        for primary in [
            "model-00001.safetensors",
            "unet/diffusion_pytorch_model.safetensors",
        ] {
            assert!(decode("start_s3_model_bundle_import", package(primary)).is_ok());
        }
    }
    #[test]
    fn primary_paths_remain_safe_and_bounded() {
        for path in [
            "../model.safetensors",
            "unet/../model.safetensors",
            "/model.safetensors",
            "unet//model.safetensors",
            "unet/CON.safetensors",
            "unet/model.safetensors.",
            "unet/model.safetensors ",
            "unet\\model.safetensors",
        ] {
            assert!(
                decode("start_s3_model_import", file(path)).is_err(),
                "{path}"
            );
            assert!(
                decode("start_s3_model_bundle_import", package(path)).is_err(),
                "{path}"
            );
        }
        assert!(decode("start_s3_model_import", file(&"a".repeat(1025))).is_err());
    }
    #[test]
    fn missing_primary_aliasing_and_reserved_output_remain_refused() {
        for path in [
            "metadata.json",
            "METADATA.JSON",
            "metadata.json/weights.safetensors",
        ] {
            assert!(
                decode("start_s3_model_import", file(path)).is_err(),
                "{path}"
            );
            assert!(decode("start_authenticated_s3_model_import", json!({
                "source":file(path),"credentials":{"access_key_id":"fixture-key", "secret_access_key":"fixture-secret"}
            })).is_err(), "{path}");
            assert!(
                decode("start_s3_model_bundle_import", package(path)).is_err(),
                "{path}"
            );
        }
        let mut request = package("model.safetensors");
        request["primary_logical_path"] = json!("unselected.safetensors");
        assert!(decode("start_s3_model_bundle_import", request).is_err());
        let mut request = package("model.safetensors");
        request["files"][1]["logical_path"] = json!("MODEL.SAFETENSORS");
        assert!(decode("start_s3_model_bundle_import", request).is_err());
    }
}

#[cfg(test)]
mod authored_bundle_tests {
    use super::*;
    use serde_json::{json, Value};
    fn params() -> Value {
        json!({"operation_id":"c3f7d104-1234-4321-abcd-aaaaaaaaaaaa","endpoint":"https://source.invalid",
            "region":"fixture","bucket":"fixture","addressing":"path","family":"fixture","official_name":"Authored",
            "read_mode":"conditional","primary_logical_path":"weights.gguf","files":[
                {"key":"objects/weights","logical_path":"weights.gguf","sha256":"0".repeat(64),"expected_etag":"\"selected\"","expected_size":"24"},
                {"key":"objects/notes","logical_path":"notes.txt","sha256":"0".repeat(64),"expected_etag":"\"\"","expected_size":"0"}]})
    }
    fn valid(value: Value) -> bool {
        serde_json::from_value::<S3BundleImportParams>(value)
            .is_ok_and(|wire| wire.validate().is_ok())
    }
    #[test]
    fn exact_sizes_tags_and_uniform_modes_match_both_feature_profiles() {
        for size in ["0", "9007199254740993", "9223372036854775807"] {
            for tag in ["\"\"", "\"opaque-é-😀\""] {
                let mut value = params();
                value["files"][1]["expected_size"] = size.into();
                value["files"][1]["expected_etag"] = tag.into();
                assert!(valid(value));
            }
        }
        for size in [
            json!(null),
            json!(1),
            json!(""),
            json!("01"),
            json!("-1"),
            json!("1e2"),
            json!("1\n"),
            json!("9223372036854775808"),
            json!("18446744073709551616"),
        ] {
            let mut value = params();
            value["files"][1]["expected_size"] = size;
            assert!(!valid(value));
        }
        for tag in ["selected", "W/\"selected\"", "\"a\"b\"", "\"a\nb\""] {
            let mut value = params();
            value["files"][1]["expected_etag"] = tag.into();
            assert!(!valid(value));
        }
        let mut value = params();
        value["files"][1]["logical_path"] = format!("{}data.json", "a/".repeat(520)).into();
        assert!(!valid(value));
        for version in [json!(null), json!(""), json!("null"), json!("v1")] {
            let mut value = params();
            value["files"][1]["version_id"] = version;
            assert!(!valid(value));
        }
        for mode in ["version_id", "unknown"] {
            let mut value = params();
            value["read_mode"] = mode.into();
            assert!(!valid(value));
        }
        let mut value = params();
        value.as_object_mut().unwrap().remove("read_mode");
        assert!(!valid(value));
        let mut value = params();
        value["files"][1] = json!({"key":"objects/notes","logical_path":"notes.txt","sha256":"0".repeat(64),"version_id":"v1"});
        assert!(!valid(value));
    }
}
