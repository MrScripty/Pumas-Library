//! Rustler NIFs for pumas-core.
//!
//! This crate provides Elixir/Erlang bindings for the pumas-core library
//! via Rustler NIFs (Native Implemented Functions).
//!
//! # Usage in Elixir
//!
//! ```elixir
//! defmodule Pumas.Native do
//!   use Rustler, otp_app: :pumas, crate: "pumas_rustler"
//!
//!   # NIFs will be loaded here
//!   def version(), do: :erlang.nif_error(:nif_not_loaded)
//!   def parse_model_type(_type), do: :erlang.nif_error(:nif_not_loaded)
//! end
//! ```

use rustler::{NifResult, NifStruct, NifUnitEnum};

// ============================================================================
// Elixir Enums (NifUnitEnum)
// ============================================================================

/// Model type enum for Elixir.
#[derive(NifUnitEnum)]
pub enum ElixirModelType {
    Llm,
    Diffusion,
    Embedding,
    Audio,
    Vision,
    Unknown,
}

/// Security tier enum for Elixir.
#[derive(NifUnitEnum)]
pub enum ElixirSecurityTier {
    Safe,
    Unknown,
    Pickle,
}

/// Download status enum for Elixir.
#[derive(NifUnitEnum)]
pub enum ElixirDownloadStatus {
    Queued,
    Downloading,
    Cancelling,
    Completed,
    Cancelled,
    Error,
}

/// Detected file type enum for Elixir.
#[derive(NifUnitEnum)]
pub enum ElixirDetectedFileType {
    Safetensors,
    Gguf,
    Ggml,
    Pickle,
    Onnx,
    Unknown,
    Error,
}

/// Health status enum for Elixir.
#[derive(NifUnitEnum)]
pub enum ElixirHealthStatus {
    Healthy,
    Warnings,
    Errors,
}

/// Import stage enum for Elixir.
#[derive(NifUnitEnum)]
pub enum ElixirImportStage {
    Copying,
    Hashing,
    WritingMetadata,
    Indexing,
    Syncing,
    Complete,
}

/// Match method enum for Elixir.
#[derive(NifUnitEnum)]
pub enum ElixirMatchMethod {
    Hash,
    FilenameExact,
    FilenameFuzzy,
    Manual,
    None,
}

/// Mapping action type enum for Elixir.
#[derive(NifUnitEnum)]
pub enum ElixirMappingActionType {
    Create,
    SkipExists,
    SkipConflict,
    RemoveBroken,
}

/// Conflict resolution enum for Elixir.
#[derive(NifUnitEnum)]
pub enum ElixirConflictResolution {
    Skip,
    Overwrite,
    Rename,
}

/// Sandbox type enum for Elixir.
#[derive(NifUnitEnum)]
pub enum ElixirSandboxType {
    Flatpak,
    Snap,
    Docker,
    Appimage,
    None,
    Unknown,
}

/// Link type enum for Elixir.
#[derive(NifUnitEnum)]
pub enum ElixirLinkType {
    Symlink,
    Hardlink,
    Copy,
}

// ============================================================================
// NIF Structs
// ============================================================================

/// Model hashes as an Elixir struct.
#[derive(NifStruct)]
#[module = "Pumas.ModelHashes"]
pub struct ElixirModelHashes {
    pub sha256: Option<String>,
    pub blake3: Option<String>,
}

/// Model file info as an Elixir struct.
#[derive(NifStruct)]
#[module = "Pumas.ModelFileInfo"]
pub struct ElixirModelFileInfo {
    pub name: String,
    pub original_name: Option<String>,
    pub size: Option<u64>,
    pub sha256: Option<String>,
    pub blake3: Option<String>,
}

/// Download option as an Elixir struct.
#[derive(NifStruct)]
#[module = "Pumas.DownloadOption"]
pub struct ElixirDownloadOption {
    pub quant: String,
    pub size_bytes: Option<u64>,
}

/// LFS file info as an Elixir struct.
#[derive(NifStruct)]
#[module = "Pumas.LfsFileInfo"]
pub struct ElixirLfsFileInfo {
    pub filename: String,
    pub size: u64,
    pub sha256: String,
}

/// Deep scan progress as an Elixir struct.
#[derive(NifStruct)]
#[module = "Pumas.DeepScanProgress"]
pub struct ElixirDeepScanProgress {
    pub current: u32,
    pub total: u32,
    pub stage: String,
}

/// Commit info as an Elixir struct.
#[derive(NifStruct)]
#[module = "Pumas.CommitInfo"]
pub struct ElixirCommitInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

/// Shortcut state as an Elixir struct.
#[derive(NifStruct)]
#[module = "Pumas.ShortcutState"]
pub struct ElixirShortcutState {
    pub menu: bool,
    pub desktop: bool,
    pub tag: String,
}

/// Base response as an Elixir struct.
#[derive(NifStruct)]
#[module = "Pumas.BaseResponse"]
pub struct ElixirBaseResponse {
    pub success: bool,
    pub error: Option<String>,
}

/// Download progress as an Elixir struct.
#[derive(NifStruct)]
#[module = "Pumas.DownloadProgress"]
pub struct ElixirDownloadProgress {
    pub download_id: String,
    pub repo_id: Option<String>,
    pub status: ElixirDownloadStatus,
    pub progress: Option<f64>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed: Option<f64>,
    pub eta_seconds: Option<f64>,
    pub error: Option<String>,
}

/// Model import spec as an Elixir struct.
#[derive(NifStruct)]
#[module = "Pumas.ModelImportSpec"]
pub struct ElixirModelImportSpec {
    pub path: String,
    pub family: String,
    pub official_name: String,
    pub repo_id: Option<String>,
    pub model_type: Option<String>,
    pub subtype: Option<String>,
    pub tags: Option<Vec<String>>,
    pub security_acknowledged: Option<bool>,
}

/// Model import result as an Elixir struct.
#[derive(NifStruct)]
#[module = "Pumas.ModelImportResult"]
pub struct ElixirModelImportResult {
    pub path: String,
    pub success: bool,
    pub model_path: Option<String>,
    pub error: Option<String>,
    pub security_tier: Option<ElixirSecurityTier>,
}

// ============================================================================
// NIF Functions
// ============================================================================

/// Get the version of the pumas-rustler bindings.
#[rustler::nif]
fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

/// Convert a model type string to its enum representation.
#[rustler::nif]
fn parse_model_type(type_str: String) -> ElixirModelType {
    match type_str.to_lowercase().as_str() {
        "llm" => ElixirModelType::Llm,
        "diffusion" => ElixirModelType::Diffusion,
        "embedding" => ElixirModelType::Embedding,
        "audio" => ElixirModelType::Audio,
        "vision" => ElixirModelType::Vision,
        _ => ElixirModelType::Unknown,
    }
}

/// Convert a security tier string to its enum representation.
#[rustler::nif]
fn parse_security_tier(tier_str: String) -> ElixirSecurityTier {
    match tier_str.to_lowercase().as_str() {
        "safe" => ElixirSecurityTier::Safe,
        "pickle" => ElixirSecurityTier::Pickle,
        _ => ElixirSecurityTier::Unknown,
    }
}

/// Convert a download status string to its enum representation.
#[rustler::nif]
fn parse_download_status(status_str: String) -> ElixirDownloadStatus {
    match status_str.to_lowercase().as_str() {
        "queued" => ElixirDownloadStatus::Queued,
        "downloading" => ElixirDownloadStatus::Downloading,
        "cancelling" => ElixirDownloadStatus::Cancelling,
        "completed" => ElixirDownloadStatus::Completed,
        "cancelled" => ElixirDownloadStatus::Cancelled,
        "error" => ElixirDownloadStatus::Error,
        _ => ElixirDownloadStatus::Error,
    }
}

/// Convert a file type string to its enum representation.
#[rustler::nif]
fn parse_file_type(type_str: String) -> ElixirDetectedFileType {
    match type_str.to_lowercase().as_str() {
        "safetensors" => ElixirDetectedFileType::Safetensors,
        "gguf" => ElixirDetectedFileType::Gguf,
        "ggml" => ElixirDetectedFileType::Ggml,
        "pickle" => ElixirDetectedFileType::Pickle,
        "onnx" => ElixirDetectedFileType::Onnx,
        "error" => ElixirDetectedFileType::Error,
        _ => ElixirDetectedFileType::Unknown,
    }
}

/// Convert a health status string to its enum representation.
#[rustler::nif]
fn parse_health_status(status_str: String) -> ElixirHealthStatus {
    match status_str.to_lowercase().as_str() {
        "healthy" => ElixirHealthStatus::Healthy,
        "warnings" => ElixirHealthStatus::Warnings,
        "errors" => ElixirHealthStatus::Errors,
        _ => ElixirHealthStatus::Errors,
    }
}

/// Convert an import stage string to its enum representation.
#[rustler::nif]
fn parse_import_stage(stage_str: String) -> ElixirImportStage {
    match stage_str.to_lowercase().as_str() {
        "copying" => ElixirImportStage::Copying,
        "hashing" => ElixirImportStage::Hashing,
        "writing_metadata" | "writingmetadata" => ElixirImportStage::WritingMetadata,
        "indexing" => ElixirImportStage::Indexing,
        "syncing" => ElixirImportStage::Syncing,
        "complete" => ElixirImportStage::Complete,
        _ => ElixirImportStage::Copying,
    }
}

/// Convert a sandbox type string to its enum representation.
#[rustler::nif]
fn parse_sandbox_type(type_str: String) -> ElixirSandboxType {
    match type_str.to_lowercase().as_str() {
        "flatpak" => ElixirSandboxType::Flatpak,
        "snap" => ElixirSandboxType::Snap,
        "docker" => ElixirSandboxType::Docker,
        "appimage" => ElixirSandboxType::Appimage,
        "none" => ElixirSandboxType::None,
        _ => ElixirSandboxType::Unknown,
    }
}

/// Parse JSON string and validate it.
/// Returns the JSON string if valid, or an error tuple.
#[rustler::nif]
fn validate_json(json_str: String) -> NifResult<String> {
    match serde_json::from_str::<serde_json::Value>(&json_str) {
        Ok(_) => Ok(json_str),
        Err(e) => Err(rustler::Error::Term(Box::new(format!(
            "Invalid JSON: {}",
            e
        )))),
    }
}

/// Create a model hashes struct.
#[rustler::nif]
fn new_model_hashes(sha256: Option<String>, blake3: Option<String>) -> ElixirModelHashes {
    ElixirModelHashes { sha256, blake3 }
}

/// Create a base response struct.
#[rustler::nif]
fn new_base_response(success: bool, error: Option<String>) -> ElixirBaseResponse {
    ElixirBaseResponse { success, error }
}

/// Create a download option struct.
#[rustler::nif]
fn new_download_option(quant: String, size_bytes: Option<u64>) -> ElixirDownloadOption {
    ElixirDownloadOption { quant, size_bytes }
}

// ============================================================================
// Rustler Init
// ============================================================================

rustler::init!("Elixir.Pumas.Native");

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!version().is_empty());
    }

    #[test]
    fn test_parse_model_type() {
        assert!(matches!(
            parse_model_type("llm".to_string()),
            ElixirModelType::Llm
        ));
        assert!(matches!(
            parse_model_type("LLM".to_string()),
            ElixirModelType::Llm
        ));
        assert!(matches!(
            parse_model_type("unknown_type".to_string()),
            ElixirModelType::Unknown
        ));
    }

    #[test]
    fn test_parse_security_tier() {
        assert!(matches!(
            parse_security_tier("safe".to_string()),
            ElixirSecurityTier::Safe
        ));
        assert!(matches!(
            parse_security_tier("pickle".to_string()),
            ElixirSecurityTier::Pickle
        ));
    }
}
