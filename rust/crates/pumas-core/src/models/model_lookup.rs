//! Operation-scoped canonical ID observations. No redirect history is retained.
use serde::{Deserialize, Serialize};

pub const MODEL_LOOKUP_CONTRACT_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case", deny_unknown_fields)]
pub enum ModelLookupResolution {
    Found,
    /// This read observed a legitimate canonical path reclassification.
    Reclassified {
        replacement_model_id: String,
    },
    /// No record exists at the requested ID. Earlier moves are not reconstructed.
    Missing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ModelLookupReport {
    pub contract_version: u32,
    pub requested_model_id: String,
    pub resolution: ModelLookupResolution,
}
