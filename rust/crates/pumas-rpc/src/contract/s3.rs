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
    fn validate(&self) -> Result<(), PublicError> {
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
            length(min = 1, max = 255),
            regex(pattern = "^[A-Za-z0-9][A-Za-z0-9._-]*\\.[gG][gG][uU][fF]$")
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

/// Explicit complete GGUF + selected inert data/text file set.
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3BundleImportParams {
    pub operation_id: String,
    pub endpoint: String,
    pub region: String,
    pub bucket: String,
    pub addressing: S3AddressingWire,
    #[cfg_attr(feature = "export-contract", schemars(length(min = 2, max = 32)))]
    pub files: Vec<S3PinnedFileParams>,
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
        let primary = self
            .files
            .iter()
            .find(|file| file.logical_path == self.primary_logical_path)
            .ok_or_else(PublicError::invalid_params)?;
        Ok(S3ImportParams {
            operation_id: self.operation_id.clone(),
            endpoint: self.endpoint.clone(),
            region: self.region.clone(),
            bucket: self.bucket.clone(),
            addressing: self.addressing,
            key: primary.key.clone(),
            version_id: primary.version_id.clone(),
            filename: self.primary_logical_path.clone(),
            sha256: primary.sha256.clone(),
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
        for file in &self.files {
            // Reuse existing pin/source validation; only the logical primary
            // basename has the GGUF restriction.
            let member = S3ImportParams {
                key: file.key.clone(),
                version_id: file.version_id.clone(),
                sha256: file.sha256.clone(),
                ..primary.clone()
            };
            member.validate()?;
            let path = &file.logical_path;
            if path.is_empty() || path.len() > 1024 || path.chars().any(char::is_control) {
                return Err(PublicError::invalid_params());
            }
            if path != &self.primary_logical_path
                && !std::path::Path::new(path)
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .is_some_and(|ext| {
                        ["json", "txt", "md", "model", "tiktoken", "vocab", "merges"]
                            .contains(&ext.to_ascii_lowercase().as_str())
                    })
            {
                return Err(PublicError::invalid_params());
            }
        }
        // Shared manifest validation is the final pure namespace/evidence
        // authority even when S3 support is compiled out.
        let files = self.native_entries()?;
        let source = pumas_library::acquisition::ArtifactSourceIdentity::new(
            "s3",
            "desktop.preflight",
            pumas_library::acquisition::ArtifactRevisionEvidence::new(
                "s3.explicit_versions",
                "desktop.explicit",
                pumas_library::acquisition::RevisionStrength::Immutable,
            )
            .map_err(|_| PublicError::invalid_params())?,
        )
        .map_err(|_| PublicError::invalid_params())?;
        let files = files
            .into_iter()
            .map(|entry| {
                pumas_library::acquisition::ArtifactFile::new(
                    entry.logical_path,
                    serde_json::to_string(&(entry.source_key, entry.version))
                        .map_err(|_| PublicError::invalid_params())?,
                    None,
                    Some(entry.expected_sha256),
                    pumas_library::acquisition::FileVerificationRequirement::Sha256,
                )
                .map_err(|_| PublicError::invalid_params())
            })
            .collect::<Result<Vec<_>, _>>()?;
        pumas_library::acquisition::ArtifactManifest::new(source, files)
            .map_err(|_| PublicError::invalid_params())?;
        pumas_library::model_library::ModelImporter::validate_acquired_payload_paths(
            &self
                .files
                .iter()
                .map(|file| file.logical_path.as_str())
                .collect::<Vec<_>>(),
        )
        .map_err(|_| PublicError::invalid_params())
    }
    pub(crate) fn native_entries(&self) -> Result<Vec<BundleEntry>, PublicError> {
        self.files
            .iter()
            .map(|file| {
                Ok(BundleEntry {
                    source_key: file.key.clone(),
                    version: file.version_id.clone(),
                    logical_path: file.logical_path.clone(),
                    expected_sha256: pumas_library::acquisition::Sha256Evidence::new(
                        "caller.sha256",
                        file.sha256.clone(),
                    )
                    .map_err(|_| PublicError::invalid_params())?,
                })
            })
            .collect()
    }
}
// The native S3 entry is optional; keep the non-S3 parser using shared evidence.
#[cfg(feature = "s3")]
type BundleEntry = pumas_library::acquisition::S3ManifestEntry;
#[cfg(not(feature = "s3"))]
pub(crate) struct BundleEntry {
    pub source_key: String,
    pub version: String,
    pub logical_path: String,
    pub expected_sha256: pumas_library::acquisition::Sha256Evidence,
}
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
            (&self.version_id, 4096),
            (&self.filename, 255),
            (&self.family, 255),
            (&self.official_name, 255),
        ] {
            if value.trim().is_empty() || value.len() > max || value.chars().any(char::is_control) {
                return Err(PublicError::invalid_params());
            }
        }
        // The frozen reader's validated_object uses object_store::Path::parse
        // and requires exact preservation: no empty, dot or parent segments,
        // including leading/trailing delimiters. Reject the same structural
        // pins before admitting a job or allocating its reservation.
        if self.version_id == "null"
            || self
                .key
                .split('/')
                .any(|part| matches!(part, "" | "." | ".."))
        {
            return Err(PublicError::invalid_params());
        }
        let filename = self.filename.as_bytes();
        if !filename[0].is_ascii_alphanumeric()
            || !filename
                .iter()
                .all(|b| b.is_ascii_alphanumeric() || b"._-".contains(b))
            || !self.filename.to_ascii_lowercase().ends_with(".gguf")
        {
            return Err(PublicError::invalid_params());
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
