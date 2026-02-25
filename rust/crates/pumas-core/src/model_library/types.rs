//! Model library types and data structures.
//!
//! These types match the Python TypedDict definitions and TypeScript interfaces
//! for API compatibility.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

// Re-export model types from models module
pub use crate::models::{
    DetectedFileType, DownloadOption, DownloadStatus, FtsSearchModel, HuggingFaceModel,
    ImportStage, MatchMethod, ModelData, ModelDownloadProgress, ModelFileInfo, ModelHashes,
    ModelImportResult, ModelImportSpec, ModelMetadata, ModelOverrides, SecurityTier,
};

/// Supported model types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum ModelType {
    /// Large Language Model (text generation)
    Llm,
    /// Diffusion model (image generation)
    Diffusion,
    /// Embedding model (text/image embeddings for similarity, retrieval, etc.)
    Embedding,
    /// Audio model
    Audio,
    /// Vision model
    Vision,
    /// Unknown type
    Unknown,
}

impl ModelType {
    /// Return the canonical lowercase string for this model type.
    pub fn as_str(&self) -> &'static str {
        match self {
            ModelType::Llm => "llm",
            ModelType::Diffusion => "diffusion",
            ModelType::Embedding => "embedding",
            ModelType::Audio => "audio",
            ModelType::Vision => "vision",
            ModelType::Unknown => "unknown",
        }
    }

    /// Map a HuggingFace pipeline_tag to our ModelType enum.
    ///
    /// This is the canonical mapping from HuggingFace's task taxonomy
    /// to our internal model type categorization.
    pub fn from_pipeline_tag(pipeline_tag: &str) -> Self {
        match pipeline_tag.to_lowercase().as_str() {
            // Text generation (LLMs)
            "text-generation" | "text2text-generation"
            | "question-answering" | "token-classification"
            | "text-classification" | "fill-mask"
            | "translation" | "summarization"
            | "conversational" => ModelType::Llm,

            // Diffusion / image & video generation
            "text-to-image" | "image-to-image"
            | "unconditional-image-generation"
            | "image-inpainting"
            | "text-to-video" | "video-classification"
            | "text-to-3d" | "image-to-3d" => ModelType::Diffusion,

            // Audio
            "text-to-audio" | "text-to-speech"
            | "automatic-speech-recognition"
            | "audio-classification" | "audio-to-audio"
            | "voice-activity-detection" => ModelType::Audio,

            // Vision
            "image-classification" | "image-segmentation"
            | "object-detection" | "zero-shot-image-classification"
            | "depth-estimation" | "image-feature-extraction"
            | "zero-shot-object-detection"
            | "image-to-text" | "visual-question-answering"
            | "document-question-answering"
            | "video-text-to-text" => ModelType::Vision,

            // Embedding
            "feature-extraction" | "sentence-similarity" => ModelType::Embedding,

            _ => ModelType::Unknown,
        }
    }
}

impl std::str::FromStr for ModelType {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "llm" => Ok(ModelType::Llm),
            "diffusion" => Ok(ModelType::Diffusion),
            "embedding" => Ok(ModelType::Embedding),
            "audio" => Ok(ModelType::Audio),
            "vision" => Ok(ModelType::Vision),
            "unknown" => Ok(ModelType::Unknown),
            // Fall through to pipeline_tag mapping for HF task strings
            other => Ok(ModelType::from_pipeline_tag(other)),
        }
    }
}

/// Model subtype for finer categorization.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModelSubtype {
    /// Full model checkpoint
    Checkpoints,
    /// LoRA adapter
    Loras,
    /// VAE encoder/decoder
    Vae,
    /// ControlNet
    Controlnet,
    /// Text embeddings
    Embeddings,
    /// Upscaler model
    Upscale,
    /// CLIP text encoder
    Clip,
    /// T5 encoder
    T5,
    /// Other/unknown subtype
    Other(String),
}

impl ModelSubtype {
    /// Return the canonical lowercase string for this subtype.
    pub fn as_str(&self) -> &str {
        match self {
            ModelSubtype::Checkpoints => "checkpoints",
            ModelSubtype::Loras => "loras",
            ModelSubtype::Vae => "vae",
            ModelSubtype::Controlnet => "controlnet",
            ModelSubtype::Embeddings => "embeddings",
            ModelSubtype::Upscale => "upscale",
            ModelSubtype::Clip => "clip",
            ModelSubtype::T5 => "t5",
            ModelSubtype::Other(s) => s,
        }
    }
}

impl std::str::FromStr for ModelSubtype {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "checkpoints" | "checkpoint" => Ok(ModelSubtype::Checkpoints),
            "loras" | "lora" => Ok(ModelSubtype::Loras),
            "vae" => Ok(ModelSubtype::Vae),
            "controlnet" => Ok(ModelSubtype::Controlnet),
            "embeddings" | "embedding" => Ok(ModelSubtype::Embeddings),
            "upscale" | "upscaler" | "upscalers" => Ok(ModelSubtype::Upscale),
            "clip" => Ok(ModelSubtype::Clip),
            "t5" => Ok(ModelSubtype::T5),
            other => Ok(ModelSubtype::Other(other.to_string())),
        }
    }
}

/// Model family (architecture).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ModelFamily(pub String);

impl ModelFamily {
    // Common LLM families
    pub const LLAMA: &'static str = "llama";
    pub const MISTRAL: &'static str = "mistral";
    pub const GEMMA: &'static str = "gemma";
    pub const PHI: &'static str = "phi";
    pub const QWEN: &'static str = "qwen";
    pub const FALCON: &'static str = "falcon";
    pub const DEEPSEEK: &'static str = "deepseek";
    pub const COMMAND: &'static str = "command";

    // Common diffusion families
    pub const STABLE_DIFFUSION: &'static str = "stable-diffusion";
    pub const SDXL: &'static str = "sdxl";
    pub const FLUX: &'static str = "flux";
    pub const KOLORS: &'static str = "kolors";
    pub const PIXART: &'static str = "pixart";

    /// Create a new model family, normalizing the name to lowercase.
    pub fn new(name: impl Into<String>) -> Self {
        ModelFamily(name.into().to_lowercase())
    }

    /// Return the lowercase family name.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ModelFamily {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// HuggingFace metadata lookup result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct HfMetadataResult {
    /// HuggingFace repository ID (e.g., "TheBloke/Llama-2-7B-GGUF")
    pub repo_id: String,
    /// Official model name
    #[serde(default)]
    pub official_name: Option<String>,
    /// Model family/architecture
    #[serde(default)]
    pub family: Option<String>,
    /// Model type (llm, diffusion)
    #[serde(default)]
    pub model_type: Option<String>,
    /// Model subtype
    #[serde(default)]
    pub subtype: Option<String>,
    /// Variant (e.g., "7B", "13B")
    #[serde(default)]
    pub variant: Option<String>,
    /// Precision (fp16, fp32, bf16)
    #[serde(default)]
    pub precision: Option<String>,
    /// Tags from HuggingFace
    #[serde(default)]
    pub tags: Vec<String>,
    /// Base model if fine-tuned
    #[serde(default)]
    pub base_model: Option<String>,
    /// Direct download URL
    #[serde(default)]
    pub download_url: Option<String>,
    /// Model description
    #[serde(default)]
    pub description: Option<String>,
    /// Match confidence (0.0-1.0)
    #[serde(default)]
    pub match_confidence: f64,
    /// How the match was determined
    #[serde(default)]
    pub match_method: String,
    /// Whether user confirmation is needed
    #[serde(default)]
    pub requires_confirmation: bool,
    /// Whether hash doesn't match expected
    #[serde(default)]
    pub hash_mismatch: bool,
    /// Filename that matched
    #[serde(default)]
    pub matched_filename: Option<String>,
    /// Needs full hash verification
    #[serde(default)]
    pub pending_full_verification: bool,
    /// Fast hash (first+last 8MB)
    #[serde(default)]
    pub fast_hash: Option<String>,
    /// Expected SHA256 from HuggingFace LFS
    #[serde(default)]
    pub expected_sha256: Option<String>,
}

/// File type detected from content/magic bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    /// Safetensors format (safe)
    Safetensors,
    /// GGUF format (LLMs, safe)
    Gguf,
    /// GGML format (legacy, safe)
    Ggml,
    /// ONNX format (safe)
    Onnx,
    /// PyTorch pickle format (potentially unsafe)
    Pickle,
    /// Unknown format
    Unknown,
}

impl FileFormat {
    /// Get the security tier for this format.
    pub fn security_tier(&self) -> SecurityTier {
        match self {
            FileFormat::Safetensors | FileFormat::Gguf | FileFormat::Ggml | FileFormat::Onnx => {
                SecurityTier::Safe
            }
            FileFormat::Pickle => SecurityTier::Pickle,
            FileFormat::Unknown => SecurityTier::Unknown,
        }
    }

    /// Return the canonical lowercase string for this file format.
    pub fn as_str(&self) -> &'static str {
        match self {
            FileFormat::Safetensors => "safetensors",
            FileFormat::Gguf => "gguf",
            FileFormat::Ggml => "ggml",
            FileFormat::Onnx => "onnx",
            FileFormat::Pickle => "pickle",
            FileFormat::Unknown => "unknown",
        }
    }
}

impl From<FileFormat> for DetectedFileType {
    fn from(format: FileFormat) -> Self {
        match format {
            FileFormat::Safetensors => DetectedFileType::Safetensors,
            FileFormat::Gguf => DetectedFileType::Gguf,
            FileFormat::Ggml => DetectedFileType::Ggml,
            FileFormat::Onnx => DetectedFileType::Onnx,
            FileFormat::Pickle => DetectedFileType::Pickle,
            FileFormat::Unknown => DetectedFileType::Unknown,
        }
    }
}

/// Mapping configuration for linking models to applications.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct MappingConfig {
    /// Application ID (e.g., "comfyui")
    pub app: String,
    /// Version pattern (e.g., "0.6.0" or "*")
    pub version: String,
    /// Config variant (default, custom)
    #[serde(default)]
    pub variant: Option<String>,
    /// Mapping rules
    pub mappings: Vec<MappingRule>,
}

/// Single mapping rule in a configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct MappingRule {
    /// Target directory in app's models folder
    pub target_dir: String,
    /// Filter by model type
    #[serde(default)]
    pub model_types: Option<Vec<String>>,
    /// Filter by subtype
    #[serde(default)]
    pub subtypes: Option<Vec<String>>,
    /// Filter by family
    #[serde(default)]
    pub families: Option<Vec<String>>,
    /// Filter by tags (OR logic)
    #[serde(default)]
    pub tags: Option<Vec<String>>,
    /// Exclude by tags
    #[serde(default)]
    pub exclude_tags: Option<Vec<String>>,
}

/// Action type for mapping operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum MappingActionType {
    /// Create new link
    Create,
    /// Skip because link exists
    SkipExists,
    /// Skip because of conflict
    SkipConflict,
    /// Remove broken link
    RemoveBroken,
}

/// Single mapping action to be performed or previewed.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MappingAction {
    /// Action type
    pub action: MappingActionType,
    /// Model ID
    pub model_id: String,
    /// Source path in library
    pub source: PathBuf,
    /// Target path in app directory
    pub target: PathBuf,
    /// Reason/description
    #[serde(default)]
    pub reason: Option<String>,
}

/// Preview of mapping operations before execution.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct MappingPreview {
    /// Actions to create links
    pub creates: Vec<MappingAction>,
    /// Actions skipped (already exist)
    pub skips: Vec<MappingAction>,
    /// Conflicts requiring resolution
    pub conflicts: Vec<MappingAction>,
    /// Broken links to remove
    pub broken: Vec<MappingAction>,
}

impl MappingPreview {
    /// Create an empty mapping preview.
    pub fn new() -> Self {
        Self::default()
    }

    /// Return the number of actionable operations (creates + broken link removals).
    pub fn total_actions(&self) -> usize {
        self.creates.len() + self.broken.len()
    }

    /// Return whether any conflicts exist that require resolution before applying.
    pub fn has_conflicts(&self) -> bool {
        !self.conflicts.is_empty()
    }
}

/// Conflict resolution strategy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum ConflictResolution {
    /// Skip conflicting model
    Skip,
    /// Overwrite existing file
    Overwrite,
    /// Rename new file
    Rename,
}

/// Sandbox environment detection result.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SandboxInfo {
    /// Type of sandbox (flatpak, snap, docker, none)
    pub sandbox_type: String,
    /// Whether running in a sandbox
    pub is_sandboxed: bool,
    /// Required permissions/setup
    #[serde(default)]
    pub required_permissions: Vec<String>,
}

impl Default for SandboxInfo {
    fn default() -> Self {
        Self {
            sandbox_type: "none".to_string(),
            is_sandboxed: false,
            required_permissions: vec![],
        }
    }
}

/// Download request parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DownloadRequest {
    /// HuggingFace repository ID
    pub repo_id: String,
    /// Model family
    pub family: String,
    /// Official name
    pub official_name: String,
    /// Model type (llm, diffusion)
    #[serde(default)]
    pub model_type: Option<String>,
    /// Quantization to download
    #[serde(default)]
    pub quant: Option<String>,
    /// Specific file to download (if not whole repo)
    #[serde(default)]
    pub filename: Option<String>,
}

/// Batch import progress tracking.
///
/// Note: Not FFI-compatible due to `usize` fields. Use wrapper types in pumas-uniffi.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BatchImportProgress {
    /// Total number of items
    pub total: usize,
    /// Completed items
    pub completed: usize,
    /// Currently processing item
    #[serde(default)]
    pub current: Option<String>,
    /// Current stage
    pub stage: ImportStage,
    /// List of results so far
    pub results: Vec<ModelImportResult>,
    /// Overall progress (0.0-1.0)
    pub progress: f32,
}

impl BatchImportProgress {
    /// Create a new batch import progress tracker for the given total item count.
    pub fn new(total: usize) -> Self {
        Self {
            total,
            completed: 0,
            current: None,
            stage: ImportStage::Copying,
            results: Vec::with_capacity(total),
            progress: 0.0,
        }
    }

    /// Update progress with the number of completed items, current item name, and stage.
    pub fn update(&mut self, completed: usize, current: Option<String>, stage: ImportStage) {
        self.completed = completed;
        self.current = current;
        self.stage = stage;
        self.progress = if self.total > 0 {
            completed as f32 / self.total as f32
        } else {
            1.0
        };
    }
}

/// Link type for model mapping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LinkType {
    /// Symbolic link (default, works across filesystems)
    Symlink,
    /// Hard link (same filesystem only, saves space)
    Hardlink,
    /// Copy file (fallback, uses disk space)
    Copy,
}

impl Default for LinkType {
    fn default() -> Self {
        LinkType::Symlink
    }
}

/// Link registry entry for tracking created links.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LinkEntry {
    /// Model ID this link points to
    pub model_id: String,
    /// Source path in library
    pub source: PathBuf,
    /// Target path where link was created
    pub target: PathBuf,
    /// Type of link
    pub link_type: LinkType,
    /// When the link was created
    pub created_at: String,
    /// Application ID
    pub app_id: String,
    /// Application version
    #[serde(default)]
    pub app_version: Option<String>,
}

/// HuggingFace search parameters.
///
/// Note: Not FFI-compatible due to `usize` fields. Use wrapper types in pumas-uniffi.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub struct HfSearchParams {
    /// Search query
    pub query: String,
    /// Model kind (text-generation, text-to-image, etc.)
    #[serde(default)]
    pub kind: Option<String>,
    /// Maximum results
    #[serde(default)]
    pub limit: Option<usize>,
    /// Offset for pagination
    #[serde(default)]
    pub offset: Option<usize>,
    /// Filter by format (gguf, safetensors)
    #[serde(default)]
    pub format: Option<String>,
}

/// LFS file information from HuggingFace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LfsFileInfo {
    /// Filename
    pub filename: String,
    /// File size in bytes
    pub size: u64,
    /// SHA256 OID (content hash)
    pub sha256: String,
}

/// Repository file tree from HuggingFace.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct RepoFileTree {
    /// Repository ID
    pub repo_id: String,
    /// LFS files with hashes
    pub lfs_files: Vec<LfsFileInfo>,
    /// Non-LFS files
    pub regular_files: Vec<String>,
    /// When this was cached
    pub cached_at: String,
    /// Last modified timestamp from HuggingFace (for cache invalidation)
    #[serde(default)]
    pub last_modified: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_type_parsing() {
        assert_eq!("llm".parse::<ModelType>().unwrap(), ModelType::Llm);
        assert_eq!("diffusion".parse::<ModelType>().unwrap(), ModelType::Diffusion);
        assert_eq!("embedding".parse::<ModelType>().unwrap(), ModelType::Embedding);
        assert_eq!("unknown".parse::<ModelType>().unwrap(), ModelType::Unknown);
    }

    #[test]
    fn test_pipeline_tag_mapping() {
        assert_eq!(ModelType::from_pipeline_tag("text-generation"), ModelType::Llm);
        assert_eq!(ModelType::from_pipeline_tag("text2text-generation"), ModelType::Llm);
        assert_eq!(ModelType::from_pipeline_tag("fill-mask"), ModelType::Llm);
        assert_eq!(ModelType::from_pipeline_tag("text-to-image"), ModelType::Diffusion);
        assert_eq!(ModelType::from_pipeline_tag("image-to-image"), ModelType::Diffusion);
        assert_eq!(ModelType::from_pipeline_tag("text-to-audio"), ModelType::Audio);
        assert_eq!(ModelType::from_pipeline_tag("text-to-speech"), ModelType::Audio);
        assert_eq!(ModelType::from_pipeline_tag("automatic-speech-recognition"), ModelType::Audio);
        assert_eq!(ModelType::from_pipeline_tag("audio-classification"), ModelType::Audio);
        assert_eq!(ModelType::from_pipeline_tag("image-classification"), ModelType::Vision);
        assert_eq!(ModelType::from_pipeline_tag("object-detection"), ModelType::Vision);
        assert_eq!(ModelType::from_pipeline_tag("image-to-text"), ModelType::Vision);
        assert_eq!(ModelType::from_pipeline_tag("feature-extraction"), ModelType::Embedding);
        assert_eq!(ModelType::from_pipeline_tag("sentence-similarity"), ModelType::Embedding);
        assert_eq!(ModelType::from_pipeline_tag("completely-unknown"), ModelType::Unknown);
    }

    #[test]
    fn test_from_str_recognizes_pipeline_tags() {
        // from_str should handle both canonical names and pipeline_tags
        assert_eq!("text-to-audio".parse::<ModelType>().unwrap(), ModelType::Audio);
        assert_eq!("text-to-image".parse::<ModelType>().unwrap(), ModelType::Diffusion);
        assert_eq!("text-generation".parse::<ModelType>().unwrap(), ModelType::Llm);
        assert_eq!("feature-extraction".parse::<ModelType>().unwrap(), ModelType::Embedding);
        assert_eq!("image-classification".parse::<ModelType>().unwrap(), ModelType::Vision);
    }

    #[test]
    fn test_model_type_as_str() {
        assert_eq!(ModelType::Llm.as_str(), "llm");
        assert_eq!(ModelType::Diffusion.as_str(), "diffusion");
        assert_eq!(ModelType::Embedding.as_str(), "embedding");
        assert_eq!(ModelType::Audio.as_str(), "audio");
        assert_eq!(ModelType::Vision.as_str(), "vision");
        assert_eq!(ModelType::Unknown.as_str(), "unknown");
    }

    #[test]
    fn test_model_subtype_parsing() {
        assert_eq!(
            "checkpoints".parse::<ModelSubtype>().unwrap(),
            ModelSubtype::Checkpoints
        );
        assert_eq!("lora".parse::<ModelSubtype>().unwrap(), ModelSubtype::Loras);
    }

    #[test]
    fn test_file_format_security() {
        assert_eq!(FileFormat::Safetensors.security_tier(), SecurityTier::Safe);
        assert_eq!(FileFormat::Gguf.security_tier(), SecurityTier::Safe);
        assert_eq!(FileFormat::Pickle.security_tier(), SecurityTier::Pickle);
        assert_eq!(FileFormat::Unknown.security_tier(), SecurityTier::Unknown);
    }

    #[test]
    fn test_mapping_preview() {
        let mut preview = MappingPreview::new();
        assert_eq!(preview.total_actions(), 0);
        assert!(!preview.has_conflicts());

        preview.conflicts.push(MappingAction {
            action: MappingActionType::SkipConflict,
            model_id: "test".to_string(),
            source: PathBuf::from("/src"),
            target: PathBuf::from("/dst"),
            reason: None,
        });
        assert!(preview.has_conflicts());
    }
}
