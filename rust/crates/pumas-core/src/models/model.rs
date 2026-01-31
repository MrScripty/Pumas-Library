//! Model metadata types.

use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashMap;

/// Hashes for a model's primary file.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ModelHashes {
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub blake3: Option<String>,
}

/// Custom deserializer that accepts either a string or array of strings for base_model.
/// Returns the first element if array, or the string itself.
fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Option<Vec<String>>, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de;

    struct StringOrVec;

    impl<'de> de::Visitor<'de> for StringOrVec {
        type Value = Option<Vec<String>>;

        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("null, a string, or an array of strings")
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_unit<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(vec![value.to_string()]))
        }

        fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(Some(vec![value]))
        }

        fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
        where
            A: de::SeqAccess<'de>,
        {
            let mut vec = Vec::new();
            while let Some(item) = seq.next_element::<String>()? {
                vec.push(item);
            }
            if vec.is_empty() {
                Ok(None)
            } else {
                Ok(Some(vec))
            }
        }
    }

    deserializer.deserialize_any(StringOrVec)
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
    #[serde(default, deserialize_with = "deserialize_string_or_vec")]
    pub base_model: Option<Vec<String>>,
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
///
/// Note: Fields like `developer`, `kind`, `formats`, `quants`, and `url` are
/// non-optional to match frontend TypeScript expectations. The frontend cannot
/// handle null values for these fields. Use empty strings/arrays as defaults.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HuggingFaceModel {
    pub repo_id: String,
    pub name: String,
    /// Developer/organization name (empty string if unknown)
    #[serde(default)]
    pub developer: String,
    /// Model kind/type (e.g., "text-generation", defaults to "unknown")
    #[serde(default = "default_kind")]
    pub kind: String,
    /// Supported formats (e.g., ["safetensors", "gguf"])
    #[serde(default)]
    pub formats: Vec<String>,
    /// Available quantization levels (e.g., ["Q4_K_M", "Q8_0"])
    #[serde(default)]
    pub quants: Vec<String>,
    /// Detailed download options with sizes
    #[serde(default)]
    pub download_options: Vec<DownloadOption>,
    /// URL to the model page (empty string if unknown)
    #[serde(default)]
    pub url: String,
    /// Release/last modified date
    #[serde(default)]
    pub release_date: Option<String>,
    /// Total download count
    #[serde(default)]
    pub downloads: Option<u64>,
    /// Total size in bytes (all files)
    #[serde(default)]
    pub total_size_bytes: Option<u64>,
    /// Size in bytes per quantization level
    #[serde(default)]
    pub quant_sizes: Option<HashMap<String, u64>>,
    /// Compatible inference engines based on model formats
    #[serde(default)]
    pub compatible_engines: Vec<String>,
}

fn default_kind() -> String {
    "unknown".to_string()
}

/// Detect compatible inference engines based on model formats.
///
/// Maps file formats to the inference engines that can load them:
/// - GGUF: Ollama, llama.cpp
/// - SafeTensors: Candle, Diffusers, transformers
/// - ONNX: ONNX Runtime
/// - PyTorch (.bin, .pt, .pth): transformers, Diffusers
pub fn detect_compatible_engines(formats: &[String]) -> Vec<String> {
    use std::collections::HashSet;

    let mut engines: HashSet<&str> = HashSet::new();

    for format in formats {
        match format.to_lowercase().as_str() {
            "gguf" => {
                engines.insert("ollama");
                engines.insert("llama.cpp");
            }
            "ggml" => {
                engines.insert("llama.cpp");
            }
            "safetensors" => {
                engines.insert("candle");
                engines.insert("transformers");
                engines.insert("diffusers");
            }
            "pytorch" | "bin" => {
                engines.insert("transformers");
                engines.insert("diffusers");
            }
            "onnx" => {
                engines.insert("onnx-runtime");
            }
            "tensorrt" => {
                engines.insert("tensorrt");
            }
            _ => {}
        }
    }

    let mut sorted: Vec<String> = engines.into_iter().map(String::from).collect();
    sorted.sort();
    sorted
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
