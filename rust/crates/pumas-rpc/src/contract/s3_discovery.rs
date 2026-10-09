//! Complete discovery observations only; none carry import or acquisition authority.
use super::{validate_s3_id, PublicError, S3AddressingWire, S3CredentialParams};
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3DiscoveryParams {
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
            length(max = 1024),
            regex(pattern = "^[^\\u0000-\\u001F\\u007F-\\u009F]*$")
        )
    )]
    pub prefix: String,
    #[cfg_attr(feature = "export-contract", schemars(range(min = 100, max = 30000)))]
    pub timeout_ms: u32,
}
impl S3DiscoveryParams {
    pub(crate) fn validate(&self) -> Result<(), PublicError> {
        validate_s3_id(&self.operation_id)?;
        for (v, max) in [
            (&self.endpoint, 4096),
            (&self.region, 255),
            (&self.bucket, 255),
        ] {
            if v.trim().is_empty() || v.len() > max || v.chars().any(char::is_control) {
                return Err(PublicError::invalid_params());
            }
        }
        let url = url::Url::parse(&self.endpoint).map_err(|_| PublicError::invalid_params())?;
        if !self.endpoint.starts_with("https://")
            || self.endpoint.trim() != self.endpoint
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || self.prefix.len() > 1024
            || self.prefix.chars().any(char::is_control)
            || !(100..=30000).contains(&self.timeout_ms)
        {
            return Err(PublicError::invalid_params());
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3AuthenticatedDiscoveryParams {
    pub source: S3DiscoveryParams,
    pub credentials: S3CredentialParams,
}
impl S3AuthenticatedDiscoveryParams {
    pub(crate) fn validate(&self) -> Result<(), PublicError> {
        self.source.validate()?;
        self.credentials.validate()
    }
}
#[derive(Clone, Serialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3DiscoveredObject {
    #[cfg_attr(feature = "export-contract", schemars(length(min = 1, max = 1024)))]
    pub key: String,
    #[cfg_attr(feature = "export-contract", schemars(length(min = 1, max = 4096)))]
    pub version_id: String,
    #[cfg_attr(feature = "export-contract", schemars(length(min = 2, max = 1024)))]
    pub etag: String,
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(pattern = "^(0|[1-9][0-9]{0,19})$"))
    )]
    pub size_bytes: String,
}
#[derive(Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum S3DiscoveryOutcome {
    Idle,
    Running {
        operation_id: String,
    },
    Complete {
        operation_id: String,
        #[cfg_attr(feature = "export-contract", schemars(length(max = 32)))]
        objects: Vec<S3DiscoveredObject>,
        #[cfg_attr(feature = "export-contract", schemars(range(min = 1, max = 8)))]
        pages: u32,
    },
    Incomplete {
        operation_id: String,
    },
    Cancelled {
        operation_id: String,
    },
    Deadline {
        operation_id: String,
    },
    NotFound {
        operation_id: String,
    },
    Unavailable,
    Rejected {
        error: PublicError,
    },
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    const ID: &str = "c3f7d104-1234-4321-abcd-aaaaaaaaaaaa";
    fn request() -> serde_json::Value {
        json!({"operation_id":ID,"endpoint":"https://source.invalid","region":"fixture-region","bucket":"fixture-bucket","addressing":"path","prefix":"models/","timeout_ms":15000})
    }
    #[test]
    fn closed_discovery_commands_validate_without_import_authority() {
        let good = request();
        assert!(crate::contract::parse_command("start_s3_prefix_discovery", Some(&good)).is_ok());
        for (field, value) in [
            ("timeout_ms", json!(0)),
            ("timeout_ms", json!(30001)),
            ("prefix", json!("bad\n")),
            ("prefix", json!("x".repeat(1025))),
            ("endpoint", json!("http://source.invalid")),
            ("sha256", json!("a".repeat(64))),
            ("secret_access_key", json!("synthetic-secret")),
        ] {
            let mut bad = good.clone();
            bad[field] = value;
            assert!(
                crate::contract::parse_command("start_s3_prefix_discovery", Some(&bad)).is_err()
            );
        }
        let mut empty = good.clone();
        empty["prefix"] = json!("");
        assert!(crate::contract::parse_command("start_s3_prefix_discovery", Some(&empty)).is_ok());
        let auth = json!({"source":good,"credentials":{"access_key_id":"synthetic-key","secret_access_key":"synthetic-secret","session_token":null}});
        assert!(crate::contract::parse_command(
            "start_authenticated_s3_prefix_discovery",
            Some(&auth)
        )
        .is_ok());
        for method in ["get_s3_prefix_discovery", "cancel_s3_prefix_discovery"] {
            assert!(
                crate::contract::parse_command(method, Some(&json!({"operation_id":ID}))).is_ok()
            );
            assert!(crate::contract::parse_command(
                method,
                Some(&json!({"operation_id":"bad","token":"synthetic-secret"}))
            )
            .is_err());
        }
    }
}
