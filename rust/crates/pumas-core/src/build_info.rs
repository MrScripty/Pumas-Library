//! Shared build/protocol/schema identity for discovery and distribution.
//!
//! Compile-time provenance is optional; absence never means a guessed Git
//! revision, artifact identity, or filesystem/library identity. Compiled features
//! are not runtime capabilities, resource availability, or model readiness.
use serde::{Deserialize, Serialize};

pub const BUILD_INFO_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtocolAdvertisement {
    pub name: String,
    pub versions: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SchemaAdvertisement {
    pub name: String,
    pub version: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PumasBuildInfo {
    pub build_info_schema_version: u32,
    pub component: String,
    pub package_version: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub build_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_revision: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Namespaced actual compile features, e.g. `pumas-library/s3`.
    pub compiled_features: Vec<String>,
    pub protocols: Vec<ProtocolAdvertisement>,
    pub schemas: Vec<SchemaAdvertisement>,
}

impl PumasBuildInfo {
    pub fn supports_schema(&self, name: &str, version: u32) -> bool {
        self.schemas
            .iter()
            .any(|schema| schema.name == name && schema.version == version)
    }

    pub fn protocol_version(&self, name: &str, accepted: &[u32]) -> Option<u32> {
        self.protocols
            .iter()
            .filter(|p| p.name == name)
            .flat_map(|p| &p.versions)
            .filter(|v| accepted.contains(v))
            .max()
            .copied()
    }

    /// The library's actual compiled contract, with packaging-supplied optional provenance.
    pub fn library() -> Self {
        let features = [
            ("hf-client", cfg!(feature = "hf-client")),
            ("process-manager", cfg!(feature = "process-manager")),
            ("gpu-monitor", cfg!(feature = "gpu-monitor")),
            ("onnx-runtime", cfg!(feature = "onnx-runtime")),
            ("s3", cfg!(feature = "s3")),
            ("contract-schema", cfg!(feature = "contract-schema")),
            ("test-support", cfg!(feature = "test-support")),
            ("uniffi", cfg!(feature = "uniffi")),
        ];
        Self {
            build_info_schema_version: BUILD_INFO_SCHEMA_VERSION,
            component: "pumas-library".into(),
            package_version: env!("CARGO_PKG_VERSION").into(),
            build_id: provenance(option_env!("PUMAS_BUILD_ID")),
            source_revision: provenance(option_env!("PUMAS_SOURCE_REVISION")),
            target: provenance(option_env!("PUMAS_BUILD_TARGET")),
            compiled_features: features
                .into_iter()
                .filter(|(_, enabled)| *enabled)
                .map(|(name, _)| format!("pumas-library/{name}"))
                .collect(),
            protocols: vec![ProtocolAdvertisement {
                name: crate::discovery::LOCAL_IPC_PROTOCOL.into(),
                versions: vec![crate::discovery::LOCAL_IPC_VERSION],
            }],
            schemas: vec![
                SchemaAdvertisement {
                    name: "pumas.build-info".into(),
                    version: BUILD_INFO_SCHEMA_VERSION,
                },
                SchemaAdvertisement {
                    name: "pumas.discovery".into(),
                    version: crate::discovery::DISCOVERY_SCHEMA_VERSION,
                },
                SchemaAdvertisement {
                    name: "pumas.model-ref".into(),
                    version: crate::models::PUMAS_MODEL_REF_CONTRACT_VERSION,
                },
                SchemaAdvertisement {
                    name: "pumas.model-selector".into(),
                    version: crate::models::MODEL_LIBRARY_SELECTOR_SNAPSHOT_CONTRACT_VERSION,
                },
            ],
        }
    }
}

fn provenance(value: Option<&str>) -> Option<String> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_roundtrips_without_inventing_provenance_or_runtime_capabilities() {
        let info = PumasBuildInfo::library();
        let value = serde_json::to_value(&info).unwrap();
        let decoded: PumasBuildInfo = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(decoded, info);
        assert_eq!(info.component, "pumas-library");
        assert!(!value.as_object().unwrap().contains_key("capabilities"));
        assert_eq!(
            info.compiled_features.contains(&"pumas-library/s3".into()),
            cfg!(feature = "s3")
        );
        assert_eq!(
            info.compiled_features
                .contains(&"pumas-library/onnx-runtime".into()),
            cfg!(feature = "onnx-runtime")
        );
        assert!(info
            .protocols
            .iter()
            .any(|p| p.name == crate::discovery::LOCAL_IPC_PROTOCOL));
        assert!(info.schemas.iter().any(|s| s.name == "pumas.model-ref"));
        assert_eq!(provenance(None), None);
        assert_eq!(provenance(Some("  ")), None);
        assert_eq!(provenance(Some("abc")), Some("abc".into()));
    }
}
