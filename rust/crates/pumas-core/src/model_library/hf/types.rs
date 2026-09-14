//! Shared types used across HuggingFace client submodules.
//!
//! Contains API response deserialization types, internal state structures,
//! and helper functions shared between search, download, and metadata operations.

use crate::model_library::artifact_identity::DownloadRevision;
use crate::model_library::download_store::PersistedDownload;
use crate::model_library::types::{DownloadRequest, DownloadStatus};
use crate::model_library::DownloadRecoveryDestination;
use crate::models::HuggingFaceEvidence;
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::Arc;

#[derive(Clone)]
pub(super) enum DownloadDestination {
    Managed(DownloadRecoveryDestination),
    Recovery(DownloadRecoveryDestination),
}

impl DownloadDestination {
    pub(super) fn capability(&self) -> &DownloadRecoveryDestination {
        match self {
            Self::Managed(value) | Self::Recovery(value) => value,
        }
    }

    pub(super) fn identity(&self) -> crate::model_library::download_recovery::DestinationIdentity {
        self.capability().identity()
    }

    pub(super) fn persisted_identity(
        &self,
    ) -> crate::Result<super::super::download_store::PersistedDestinationIdentity> {
        self.capability().persisted_identity()
    }

    pub(super) fn display_path(&self) -> &std::path::Path {
        self.capability().display_path()
    }

    pub(super) fn domain(&self) -> super::lifecycle::DestinationDomain {
        match self {
            Self::Managed(_) => super::lifecycle::DestinationDomain::Ambient,
            Self::Recovery(_) => super::lifecycle::DestinationDomain::Recovery,
        }
    }

    pub(super) fn is_recovery(&self) -> bool {
        matches!(self, Self::Recovery(_))
    }
}

/// HuggingFace API base URL.
pub(crate) const HF_API_BASE: &str = "https://huggingface.co/api";

/// HuggingFace Hub download base URL.
pub(crate) const HF_HUB_BASE: &str = "https://huggingface.co";

/// Cache TTL for repository file trees (24 hours).
pub(crate) const REPO_CACHE_TTL_SECS: u64 = 24 * 60 * 60;

/// Information passed to the download completion callback.
#[derive(Debug, Clone)]
pub struct DownloadCompletionInfo {
    /// Download ID.
    pub download_id: String,
    /// Directory where the model file was placed.
    pub dest_dir: PathBuf,
    /// Filename of the downloaded model (primary/largest file).
    pub filename: String,
    /// All filenames in this download (for multi-file provenance).
    pub filenames: Vec<String>,
    /// Original download request with model metadata.
    pub download_request: DownloadRequest,
    /// Known SHA256 from HuggingFace LFS metadata (avoids recomputation).
    pub known_sha256: Option<String>,
    /// Normalized HuggingFace evidence captured during download preflight.
    pub huggingface_evidence: Option<HuggingFaceEvidence>,
}

/// Callback invoked when a download completes successfully.
pub type DownloadCompletionCallback = Arc<dyn Fn(DownloadCompletionInfo) + Send + Sync + 'static>;

/// Information passed when all auxiliary (config/tokenizer) files have been
/// downloaded but before weight files begin. Used to create a preliminary
/// `metadata.json` so the model appears in the library index during download.
#[derive(Debug, Clone)]
pub struct AuxFilesCompleteInfo {
    /// Download ID.
    pub download_id: String,
    /// Directory where the model files are being placed.
    pub dest_dir: PathBuf,
    /// All filenames in this download (auxiliary + weight).
    pub filenames: Vec<String>,
    /// Original download request with model metadata.
    pub download_request: DownloadRequest,
    /// Total download size (sum of LFS file sizes).
    pub total_bytes: Option<u64>,
    /// Normalized HuggingFace evidence captured during download preflight.
    pub huggingface_evidence: Option<HuggingFaceEvidence>,
}

/// Callback invoked when auxiliary files finish downloading (before weight files begin).
pub type AuxFilesCompleteCallback = Arc<dyn Fn(AuxFilesCompleteInfo) + Send + Sync + 'static>;

/// A single file to download as part of a (possibly multi-file) model download.
#[derive(Debug, Clone)]
pub(crate) struct FileToDownload {
    pub filename: String,
    pub size: Option<u64>,
    pub sha256: Option<String>,
}

/// Internal state for an active download.
pub(crate) struct DownloadState {
    pub(super) admission: Option<AdmittedDownload>,
    /// Download ID
    pub download_id: String,
    /// Repository ID
    pub repo_id: String,
    /// Revision used by metadata lookup, byte transfer, resume, and import.
    pub(super) revision: DownloadRevision,
    /// Current status
    pub status: DownloadStatus,
    /// Progress (0.0-1.0)
    pub progress: f32,
    /// Downloaded bytes (across all files)
    pub downloaded_bytes: u64,
    /// Total bytes (sum across all files)
    pub total_bytes: Option<u64>,
    /// Download speed (bytes/sec)
    pub speed: f64,
    /// Cancellation flag
    pub cancel_flag: Arc<AtomicBool>,
    /// Pause flag -- signals graceful stop without deleting .part file
    pub pause_flag: Arc<AtomicBool>,
    /// Error message if failed
    pub error: Option<String>,
    /// Current retry attempt for the active file (1-based while retrying).
    pub retry_attempt: u32,
    /// Retry limit for the active file (`None` means unlimited).
    pub retry_limit: Option<u32>,
    /// Whether the downloader is currently waiting before the next retry.
    pub retrying: bool,
    /// Delay (seconds) until the next retry, when `retrying` is true.
    pub next_retry_delay_seconds: Option<f64>,
    /// Whether a Tokio task was registered for this in-memory download.
    pub task_registered: bool,
    /// A lifecycle/cleanup failure whose safety preconditions have not been
    /// re-established. Cancellation must not erase this provenance or release
    /// a recovery capability as if cleanup had been verified.
    pub lifecycle_failure_unverified: bool,
    /// Destination directory (needed for resume after restart)
    pub dest_dir: PathBuf,
    /// Whether this runtime has reserved or revoked the persisted ambient-path
    /// authority while converting the state to capability-backed recovery.
    /// Once set, generic ambient resume/relocation remains fail-closed.
    pub ambient_authority_blocked: bool,
    /// Held filesystem authority and explicit provenance. All resumable states
    /// retain it; verified terminal cleanup releases it. Never serialized.
    pub(super) destination: Option<DownloadDestination>,
    /// Exact former ordinary snapshot retained by owned recovery revocation.
    /// Needed to quarantine later cleanup without reconstructing lost metadata.
    pub(super) revoked_snapshot: Option<super::super::download_store::PersistedDownload>,
    /// Current filename being downloaded
    pub filename: String,
    /// All files in this download (for multi-file models)
    pub files: Vec<FileToDownload>,
    /// Number of files completed so far
    pub files_completed: usize,
    /// Original download request (needed for persistence/resume)
    pub download_request: Option<DownloadRequest>,
    /// Known SHA256 from HuggingFace LFS metadata (primary file).
    pub known_sha256: Option<String>,
    /// Normalized HuggingFace evidence captured during download preflight.
    pub huggingface_evidence: Option<HuggingFaceEvidence>,
}

#[derive(Clone)]
pub(super) struct AdmittedDownload {
    pub(super) attempt_id: String,
}

impl std::fmt::Debug for DownloadState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DownloadState")
            .field("download_id", &self.download_id)
            .field("repo_id", &self.repo_id)
            .field("status", &self.status)
            .field("progress", &self.progress)
            .finish()
    }
}

impl DownloadState {
    pub(super) fn matches_destination(
        &self,
        identity: &crate::model_library::download_recovery::DestinationIdentity,
    ) -> bool {
        self.destination
            .as_ref()
            .is_some_and(|destination| destination.identity() == *identity)
    }
    pub(super) fn recovery_destination(&self) -> Option<&DownloadRecoveryDestination> {
        match self.destination.as_ref() {
            Some(DownloadDestination::Recovery(value)) => Some(value),
            _ => None,
        }
    }

    #[cfg(test)]
    pub(super) fn make_managed_for_test(&mut self) {
        if let Some(destination) = self.destination.take() {
            self.destination = Some(DownloadDestination::Managed(
                destination.capability().clone(),
            ));
        }
    }
    /// Restore a download state from a persisted entry.
    ///
    /// Reconstructs in-memory state from a persistence record, including
    /// file lists, progress calculation, and status normalization.
    pub(super) fn from_persisted(
        entry: &PersistedDownload,
        downloaded_bytes: u64,
        destination: DownloadDestination,
        revision: DownloadRevision,
    ) -> Self {
        Self::restore_fields(entry, downloaded_bytes, Some(destination), revision)
    }

    /// Historical failure metadata after durable cleanup and queue settlement.
    /// This state deliberately cannot grant filesystem or resume authority.
    pub(super) fn from_verified_cleanup(
        entry: &PersistedDownload,
        revision: DownloadRevision,
    ) -> Self {
        let mut state = Self::restore_fields(entry, 0, None, revision);
        state.status = DownloadStatus::Error;
        state.error = Some("Download failed; terminal cleanup was verified".into());
        state.ambient_authority_blocked = true;
        state.lifecycle_failure_unverified = true;
        state
    }

    fn restore_fields(
        entry: &PersistedDownload,
        downloaded_bytes: u64,
        destination: Option<DownloadDestination>,
        revision: DownloadRevision,
    ) -> Self {
        let progress = entry
            .total_bytes
            .map(|total| downloaded_bytes as f32 / total as f32)
            .unwrap_or(0.0);

        // Normalize status: any non-terminal status becomes Paused since
        // the download task is no longer running after a restart.
        let restored_status = match entry.status {
            DownloadStatus::Queued | DownloadStatus::Downloading | DownloadStatus::Pausing => {
                DownloadStatus::Paused
            }
            other => other,
        };

        // Current persisted snapshots always contain an explicit file set.
        let files: Vec<FileToDownload> = entry
            .filenames
            .iter()
            .map(|f| FileToDownload {
                filename: f.clone(),
                size: None, // Not persisted per-file; verified on disk
                sha256: None,
            })
            .collect();

        Self {
            download_id: entry.download_id.clone(),
            repo_id: entry.repo_id.clone(),
            revision,
            status: restored_status,
            progress,
            downloaded_bytes,
            total_bytes: entry.total_bytes,
            speed: 0.0,
            cancel_flag: Arc::new(AtomicBool::new(false)),
            pause_flag: Arc::new(AtomicBool::new(false)),
            error: None,
            retry_attempt: 0,
            retry_limit: None,
            retrying: false,
            next_retry_delay_seconds: None,
            task_registered: false,
            lifecycle_failure_unverified: false,
            dest_dir: destination
                .as_ref()
                .map(|destination| destination.display_path().to_path_buf())
                .unwrap_or_else(|| entry.dest_dir.clone()),
            ambient_authority_blocked: false,
            admission: None,
            destination,
            revoked_snapshot: None,
            filename: entry.filename.clone(),
            files,
            files_completed: 0,
            download_request: Some(entry.download_request.clone()),
            known_sha256: entry.known_sha256.clone(),
            huggingface_evidence: entry.huggingface_evidence.clone(),
        }
    }
}

/// HuggingFace search result from API.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HfSearchResult {
    #[serde(rename = "modelId")]
    pub model_id: String,
    #[serde(default)]
    pub tags: Vec<String>,
    /// Note: HuggingFace API returns this as snake_case "pipeline_tag"
    #[serde(default, rename = "pipeline_tag")]
    pub pipeline_tag: Option<String>,
    /// Requires full=true in API request to be populated
    #[serde(default)]
    pub last_modified: Option<String>,
    #[serde(default)]
    pub downloads: Option<u64>,
    /// File list from repo (available with full=true)
    #[serde(default)]
    pub siblings: Vec<HfSibling>,
    /// Model config (available with config=true in search, always in direct endpoint)
    #[serde(default)]
    pub config: Option<HfModelConfig>,
    /// Model card metadata from HuggingFace when available.
    #[serde(default, rename = "cardData")]
    pub card_data: Option<HashMap<String, Value>>,
}

/// Subset of the HuggingFace model config relevant for type inference.
#[derive(Debug, Deserialize)]
pub(crate) struct HfModelConfig {
    #[serde(default)]
    pub architectures: Vec<String>,
    #[serde(default, rename = "model_type")]
    pub model_type: Option<String>,
}

/// HuggingFace sibling file entry from search API.
#[derive(Debug, Deserialize)]
pub(crate) struct HfSibling {
    /// Relative filename in the repo
    pub rfilename: String,
}

/// HuggingFace file entry from tree API.
#[derive(Debug, Deserialize)]
pub(crate) struct HfFileEntry {
    pub path: String,
    #[serde(default, rename = "type")]
    pub entry_type: Option<String>,
    #[serde(default)]
    pub lfs: Option<HfLfsInfo>,
}

/// LFS information from HuggingFace.
#[derive(Debug, Deserialize)]
pub(crate) struct HfLfsInfo {
    pub oid: String,
    pub size: u64,
}

/// Infer a pipeline_tag from the model config when the API doesn't provide one.
///
/// Checks in order:
/// 1. Architecture class suffix (e.g. `LlamaForCausalLM` -> `text-generation`)
/// 2. Known `model_type` values (e.g. `sdar` -> `text-generation`)
pub(crate) fn infer_pipeline_tag_from_config(config: Option<&HfModelConfig>) -> Option<String> {
    let config = config?;

    // Reranker families frequently expose reward-model architectures.
    if config
        .architectures
        .iter()
        .any(|arch| arch.trim().ends_with("ForRewardModel"))
    {
        return Some("text-ranking".to_string());
    }

    // 1. Check architecture suffix (longest suffixes first to avoid partial matches)
    if let Some(arch) = config.architectures.first() {
        let suffix_map: &[(&str, &str)] = &[
            ("ForConditionalGeneration", "text2text-generation"),
            ("ForSequenceClassification", "text-classification"),
            ("ForSemanticSegmentation", "image-segmentation"),
            ("ForImageClassification", "image-classification"),
            ("ForAudioClassification", "audio-classification"),
            ("ForTokenClassification", "token-classification"),
            ("ForQuestionAnswering", "question-answering"),
            ("ForFeatureExtraction", "feature-extraction"),
            ("ForObjectDetection", "object-detection"),
            ("ForSpeechSeq2Seq", "automatic-speech-recognition"),
            ("ForCausalLM", "text-generation"),
            ("ForMaskedLM", "fill-mask"),
        ];

        for (suffix, tag) in suffix_map {
            if arch.ends_with(suffix) {
                return Some(tag.to_string());
            }
        }
    }

    // 2. Fall back to model_type lookup
    let model_type = config.model_type.as_deref()?;
    let tag = match model_type {
        // Text generation (LLMs, DLLMs, SSMs)
        "llama" | "mistral" | "mixtral" | "gpt2" | "gpt_neo" | "gpt_neox" | "gptj"
        | "phi" | "phi3" | "phimoe"
        | "qwen2" | "qwen2_moe" | "qwen3" | "qwen3_moe"
        | "gemma" | "gemma2" | "gemma3"
        | "deepseek_v2" | "deepseek_v3"
        | "falcon" | "falcon_mamba"
        | "mpt" | "bloom" | "opt" | "codegen" | "starcoder2"
        | "cohere" | "cohere2" | "command-r"
        | "internlm2" | "internlm3"
        | "olmo" | "olmo2"
        | "rwkv" | "rwkv5" | "rwkv6"
        | "mamba" | "mamba2" | "jamba"
        | "sdar" | "recurrentgemma" | "dbrx"
        | "stablelm" | "persimmon" | "xglm" | "gpt_bigcode"
        // Diffusion LLMs (dLLM) — structurally LLMs with diffusion decoding
        | "llada" | "mdlm" | "dream" | "mercury" | "sedd" => "text-generation",

        // Seq2seq / conditional generation
        "t5" | "bart" | "mbart" | "mt5" | "longt5" | "pegasus" | "led"
        | "bigbird_pegasus" | "flan-t5" => "text2text-generation",

        // Diffusion / image generation
        "stable_diffusion" | "sdxl" | "kandinsky" | "pixart" => "text-to-image",

        // Audio / speech models
        "whisper" | "wav2vec2" | "hubert" | "wavlm" | "seamless_m4t"
        | "bark" | "musicgen" | "encodec" | "speecht5" | "mms" => {
            "automatic-speech-recognition"
        }

        // Vision-language / zero-shot
        "clip" | "siglip" | "blip" | "blip2" => "zero-shot-image-classification",

        // Image classification
        "vit" | "swin" | "convnext" | "deit" | "beit" | "dinov2" => "image-classification",

        // Masked LM / encoders
        "bert" | "roberta" | "distilbert" | "albert" | "electra"
        | "deberta" | "deberta_v2" | "xlm_roberta" | "camembert" | "flaubert" => "fill-mask",

        _ => return None,
    };

    Some(tag.to_string())
}
