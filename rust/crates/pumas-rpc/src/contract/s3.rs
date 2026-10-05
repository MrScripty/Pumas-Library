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
