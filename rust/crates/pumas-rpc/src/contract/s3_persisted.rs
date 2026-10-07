//! Closed informational persisted S3 projection; never an import/recovery command.
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum S3PersistedImportsWire {
    Complete {
        #[cfg_attr(feature = "export-contract", schemars(length(max = 32)))]
        imports: Vec<S3PersistedImportWire>,
    },
    Incomplete,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3PersistedImportWire {
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(
            pattern = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
        ))
    )]
    operation_id: String,
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(
            pattern = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
        ))
    )]
    acquisition_id: String,
    phase: S3PersistedPhaseWire,
    receipt_present: bool,
    model_binding: Option<S3RecordedModelBindingWire>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum S3PersistedPhaseWire {
    Transferring,
    FilesReady,
    Using,
    Adopted,
    Withdrawn,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct S3RecordedModelBindingWire {
    #[cfg_attr(feature = "export-contract", schemars(length(min = 1, max = 1024)))]
    model_id: String,
    #[cfg_attr(
        feature = "export-contract",
        schemars(regex(
            pattern = "^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$"
        ))
    )]
    publication_id: String,
    publication_state: S3RecordedPublicationStateWire,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum S3RecordedPublicationStateWire {
    Pending,
    Confirmed,
}
