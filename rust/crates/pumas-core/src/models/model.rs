//! Model metadata types.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Hashes for a model's primary file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelHashes {
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub blake3: Option<String>,
}

/// Metadata about an individual file in a model directory.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelFileInfo {
    pub name: String,
    #[serde(default)]
    pub original_name: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub blake3: Option<String>,
}

/// Canonical metadata stored with each model directory.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ModelMetadata {
    #[serde(default)]
    pub model_id: Option<String>,
    #[serde(default)]
    pub family: Option<String>,
    #[serde(default)]
    pub model_type: Option<String>,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub official_name: Option<String>,
    #[serde(default)]
    pub cleaned_name: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub base_model: Option<String>,
    #[serde(default)]
    pub preview_image: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub download_url: Option<String>,
    #[serde(default)]
    pub model_card: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub inference_settings: Option<HashMap<String, serde_json::Value>>,
    #[serde(default)]
    pub compatible_apps: Option<Vec<String>>,
    #[serde(default)]
    pub hashes: Option<ModelHashes>,
    #[serde(default)]
    pub notes: Option<String>,
    #[serde(default)]
    pub added_date: Option<String>,
    #[serde(default)]
    pub updated_date: Option<String>,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub files: Option<Vec<ModelFileInfo>>,
    // Metadata source tracking
    #[serde(default)]
    pub match_source: Option<String>,
    #[serde(default)]
    pub match_method: Option<String>,
    #[serde(default)]
    pub match_confidence: Option<f64>,
    // Offline fallback tracking
    #[serde(default)]
    pub pending_online_lookup: Option<bool>,
    #[serde(default)]
    pub lookup_attempts: Option<u32>,
    #[serde(default)]
    pub last_lookup_attempt: Option<String>,
}

/// User overrides for model mapping.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct ModelOverrides {
    #[serde(default)]
    pub version_ranges: Option<HashMap<String, String>>,
}

/// Model data as returned by get_models API.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelData {
    pub model_type: String,
    #[serde(default)]
    pub official_name: Option<String>,
    #[serde(default)]
    pub cleaned_name: Option<String>,
    #[serde(default)]
    pub size: Option<u64>,
    #[serde(default)]
    pub added_date: Option<String>,
    #[serde(default)]
    pub related_available: Option<bool>,
}

/// HuggingFace model search result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HuggingFaceModel {
    pub repo_id: String,
    pub name: String,
    #[serde(default)]
    pub developer: Option<String>,
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub formats: Option<Vec<String>>,
    #[serde(default)]
    pub quants: Option<Vec<String>>,
    #[serde(default)]
    pub download_options: Option<Vec<DownloadOption>>,
    #[serde(default)]
    pub url: Option<String>,
    #[serde(default)]
    pub release_date: Option<String>,
    #[serde(default)]
    pub downloads: Option<u64>,
    #[serde(default)]
    pub total_size_bytes: Option<u64>,
    #[serde(default)]
    pub quant_sizes: Option<HashMap<String, u64>>,
}

/// Download option for a quantization variant.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadOption {
    pub quant: String,
    #[serde(default)]
    pub size_bytes: Option<u64>,
}

/// Model download status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DownloadStatus {
    Queued,
    Downloading,
    Cancelling,
    Completed,
    Cancelled,
    Error,
}

/// Model download progress tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelDownloadProgress {
    pub download_id: String,
    #[serde(default)]
    pub repo_id: Option<String>,
    pub status: DownloadStatus,
    #[serde(default)]
    pub progress: Option<f32>,
    #[serde(default)]
    pub downloaded_bytes: Option<u64>,
    #[serde(default)]
    pub total_bytes: Option<u64>,
    #[serde(default)]
    pub speed: Option<f64>,
    #[serde(default)]
    pub eta_seconds: Option<f64>,
    #[serde(default)]
    pub error: Option<String>,
}

/// Security tier for pickle scanning.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum SecurityTier {
    Safe,
    Unknown,
    Pickle,
}

/// Detected file type from magic bytes.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DetectedFileType {
    Safetensors,
    Gguf,
    Ggml,
    Pickle,
    Onnx,
    Unknown,
    Error,
}

/// Match method for metadata lookup.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MatchMethod {
    Hash,
    FilenameExact,
    FilenameFuzzy,
    Manual,
    None,
}

/// Import stage for progress tracking.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ImportStage {
    Copying,
    Hashing,
    WritingMetadata,
    Indexing,
    Syncing,
    Complete,
}

/// Model import specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelImportSpec {
    pub path: String,
    pub family: String,
    pub official_name: String,
    #[serde(default)]
    pub repo_id: Option<String>,
    #[serde(default)]
    pub model_type: Option<String>,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub security_acknowledged: Option<bool>,
}

/// Model import result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct ModelImportResult {
    pub path: String,
    pub success: bool,
    #[serde(default)]
    pub model_path: Option<String>,
    #[serde(default)]
    pub error: Option<String>,
    #[serde(default)]
    pub security_tier: Option<SecurityTier>,
}

/// FTS5 search result model entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct FtsSearchModel {
    pub model_id: String,
    #[serde(default)]
    pub repo_id: Option<String>,
    pub official_name: String,
    pub family: String,
    #[serde(default)]
    pub model_type: Option<String>,
    #[serde(default)]
    pub subtype: Option<String>,
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    #[serde(default)]
    pub description: Option<String>,
    pub file_path: String,
    #[serde(default)]
    pub size_bytes: Option<u64>,
    #[serde(default)]
    pub security_tier: Option<SecurityTier>,
    #[serde(default)]
    pub added_date: Option<String>,
    #[serde(default)]
    pub last_used: Option<String>,
    #[serde(default)]
    pub related_available: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_download_status_serialization() {
        let status = DownloadStatus::Downloading;
        let json = serde_json::to_string(&status).unwrap();
        assert_eq!(json, "\"downloading\"");
    }

    #[test]
    fn test_model_metadata_default() {
        let metadata = ModelMetadata::default();
        assert!(metadata.model_id.is_none());
        assert!(metadata.tags.is_none());
    }
}
