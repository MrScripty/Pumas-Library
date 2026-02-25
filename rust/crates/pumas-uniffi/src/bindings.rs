// Private imports — core types used in From impls, not exposed in API signatures
use pumas_library::models::HuggingFaceModel;
use pumas_library::{ModelRecord, PumasApi, SearchResult};
use std::sync::Arc;

// =============================================================================
// FfiError — Simplified error type for the FFI boundary
// =============================================================================

/// FFI-friendly error type.
///
/// This is a simplified version of `PumasError` that can cross the FFI boundary.
/// Complex error types with embedded `std::io::Error` or `rusqlite::Error` are
/// converted to string representations.
#[derive(Debug, Clone, uniffi::Error, thiserror::Error)]
pub enum FfiError {
    #[error("Network error: {message}")]
    Network { message: String },

    #[error("Timeout: {message}")]
    Timeout { message: String },

    #[error("Rate limited: {message}")]
    RateLimited { message: String },

    #[error("Database error: {message}")]
    Database { message: String },

    #[error("IO error: {message}")]
    Io { message: String },

    #[error("Not found: {resource}")]
    NotFound { resource: String },

    #[error("Version error: {message}")]
    Version { message: String },

    #[error("Model error: {message}")]
    Model { message: String },

    #[error("Download error: {message}")]
    Download { message: String },

    #[error("Validation error: {message}")]
    Validation { message: String },

    #[error("Configuration error: {message}")]
    Config { message: String },

    #[error("Launch failed: {message}")]
    Launch { message: String },

    #[error("Process error: {message}")]
    Process { message: String },

    #[error("Cancelled")]
    Cancelled,

    #[error("{0}")]
    Other(String),
}

impl From<pumas_library::PumasError> for FfiError {
    fn from(err: pumas_library::PumasError) -> Self {
        use pumas_library::PumasError;

        match err {
            PumasError::Network { message, .. } => FfiError::Network { message },
            PumasError::Timeout(duration) => FfiError::Timeout {
                message: format!("Request timed out after {:?}", duration),
            },
            PumasError::RateLimited {
                service,
                retry_after_secs,
            } => FfiError::RateLimited {
                message: format!(
                    "Rate limited by {}, retry after {:?} seconds",
                    service, retry_after_secs
                ),
            },
            PumasError::CircuitBreakerOpen { domain } => FfiError::Network {
                message: format!("Circuit breaker open for {}", domain),
            },
            PumasError::Database { message, .. } => FfiError::Database { message },
            PumasError::Io { message, path, .. } => FfiError::Io {
                message: match path {
                    Some(p) => format!("{}: {}", p.display(), message),
                    None => message,
                },
            },
            PumasError::FileNotFound(path) => FfiError::NotFound {
                resource: format!("File: {}", path.display()),
            },
            PumasError::NotFound { resource } => FfiError::NotFound { resource },
            PumasError::PermissionDenied(path) => FfiError::Io {
                message: format!("Permission denied: {}", path.display()),
            },
            PumasError::NotADirectory(path) => FfiError::Io {
                message: format!("Not a directory: {}", path.display()),
            },
            PumasError::SymlinkFailed { src, dest, reason } => FfiError::Io {
                message: format!(
                    "Failed to create symlink from {} to {}: {}",
                    src.display(),
                    dest.display(),
                    reason
                ),
            },
            PumasError::Json { message, .. } => FfiError::Io {
                message: format!("JSON error: {}", message),
            },
            PumasError::VersionNotFound { tag } => FfiError::Version {
                message: format!("Version not found: {}", tag),
            },
            PumasError::VersionAlreadyInstalled { tag } => FfiError::Version {
                message: format!("Version already installed: {}", tag),
            },
            PumasError::InstallationFailed { message } => FfiError::Version { message },
            PumasError::InstallationCancelled => FfiError::Cancelled,
            PumasError::DependencyFailed { message } => FfiError::Version { message },
            PumasError::LaunchFailed { app, message } => FfiError::Launch {
                message: format!("{}: {}", app, message),
            },
            PumasError::ProcessNotRunning { app } => FfiError::Process {
                message: format!("Process not running: {}", app),
            },
            PumasError::ModelNotFound { model_id } => FfiError::Model {
                message: format!("Model not found: {}", model_id),
            },
            PumasError::ImportFailed { message } => FfiError::Model { message },
            PumasError::DownloadFailed { url, message } => FfiError::Download {
                message: format!("{}: {}", url, message),
            },
            PumasError::DownloadCancelled | PumasError::DownloadPaused => FfiError::Cancelled,
            PumasError::HashMismatch { expected, actual } => FfiError::Validation {
                message: format!("Hash mismatch: expected {}, got {}", expected, actual),
            },
            PumasError::InvalidFileType { expected, actual } => FfiError::Validation {
                message: format!("Invalid file type: expected {}, got {}", expected, actual),
            },
            PumasError::GitHubApi { message, status_code } => FfiError::Network {
                message: format!(
                    "GitHub API error ({}): {}",
                    status_code.unwrap_or(0),
                    message
                ),
            },
            PumasError::ReleaseNotFound { tag } => FfiError::NotFound {
                resource: format!("Release: {}", tag),
            },
            PumasError::Config { message } => FfiError::Config { message },
            PumasError::InvalidAppId(id) => FfiError::Validation {
                message: format!("Invalid app ID: {}", id),
            },
            PumasError::Validation { field, message } => FfiError::Validation {
                message: format!("{}: {}", field, message),
            },
            PumasError::InvalidVersionTag { tag } => FfiError::Validation {
                message: format!("Invalid version tag: {}", tag),
            },
            PumasError::InvalidParams { message } => FfiError::Validation { message },
            PumasError::SharedInstanceLost { pid, port } => FfiError::Other(format!(
                "Shared instance lost (PID {} on port {})",
                pid, port
            )),
            PumasError::NoLibrariesRegistered => FfiError::Config {
                message: "No libraries registered".to_string(),
            },
            PumasError::TorchInference { message } => FfiError::Process {
                message: format!("Torch inference: {}", message),
            },
            PumasError::SlotNotFound { slot_id } => FfiError::NotFound {
                resource: format!("Model slot: {}", slot_id),
            },
            PumasError::DeviceNotAvailable { device } => FfiError::Config {
                message: format!("Device not available: {}", device),
            },
            PumasError::ConversionFailed { message } => FfiError::Model { message },
            PumasError::ConversionCancelled => FfiError::Cancelled,
            PumasError::QuantizationEnvNotReady { message, .. } => FfiError::Config { message },
            PumasError::Other(message) => FfiError::Other(message),
        }
    }
}

/// Result type for FFI operations.
pub type FfiResult<T> = Result<T, FfiError>;

// UniFFI scaffolding - this generates the FFI glue code
uniffi::setup_scaffolding!();

/// Get the version of the pumas-uniffi bindings.
#[uniffi::export]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

// =============================================================================
// FFI Wrapper Types
//
// All types exposed across the FFI boundary are defined here in pumas-uniffi
// rather than re-exported from pumas-library. This avoids UniFFI "external
// type" issues (Ruby backend doesn't support lifting external types) and
// provides a clean FFI boundary with explicit conversions.
// =============================================================================

// ---- Utility wrapper types (HashMap → Vec<KV>) ----

/// A key-value pair for model hashes (e.g. sha256, blake3).
#[derive(uniffi::Record)]
pub struct FfiHashEntry {
    pub key: String,
    pub value: String,
}

/// A quantization size entry mapping quant name to file size.
#[derive(uniffi::Record)]
pub struct FfiQuantSize {
    pub quant: String,
    pub size_bytes: u64,
}

// ---- Model management types ----

/// Security tier for imported models.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiSecurityTier {
    Safe,
    Unknown,
    Pickle,
}

impl From<pumas_library::models::SecurityTier> for FfiSecurityTier {
    fn from(t: pumas_library::models::SecurityTier) -> Self {
        match t {
            pumas_library::models::SecurityTier::Safe => FfiSecurityTier::Safe,
            pumas_library::models::SecurityTier::Unknown => FfiSecurityTier::Unknown,
            pumas_library::models::SecurityTier::Pickle => FfiSecurityTier::Pickle,
        }
    }
}

/// A download option (quant + optional size).
#[derive(uniffi::Record)]
pub struct FfiDownloadOption {
    pub quant: String,
    pub size_bytes: Option<u64>,
}

impl From<pumas_library::models::DownloadOption> for FfiDownloadOption {
    fn from(o: pumas_library::models::DownloadOption) -> Self {
        Self {
            quant: o.quant,
            size_bytes: o.size_bytes,
        }
    }
}

/// Status of a download.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiDownloadStatus {
    Queued,
    Downloading,
    Pausing,
    Paused,
    Cancelling,
    Completed,
    Cancelled,
    Error,
}

impl From<pumas_library::models::DownloadStatus> for FfiDownloadStatus {
    fn from(s: pumas_library::models::DownloadStatus) -> Self {
        use pumas_library::models::DownloadStatus;
        match s {
            DownloadStatus::Queued => FfiDownloadStatus::Queued,
            DownloadStatus::Downloading => FfiDownloadStatus::Downloading,
            DownloadStatus::Pausing => FfiDownloadStatus::Pausing,
            DownloadStatus::Paused => FfiDownloadStatus::Paused,
            DownloadStatus::Cancelling => FfiDownloadStatus::Cancelling,
            DownloadStatus::Completed => FfiDownloadStatus::Completed,
            DownloadStatus::Cancelled => FfiDownloadStatus::Cancelled,
            DownloadStatus::Error => FfiDownloadStatus::Error,
        }
    }
}

/// Specification for importing a model.
#[derive(uniffi::Record)]
pub struct FfiModelImportSpec {
    pub path: String,
    pub family: String,
    pub official_name: String,
    pub repo_id: Option<String>,
    pub model_type: Option<String>,
    pub subtype: Option<String>,
    pub tags: Option<Vec<String>>,
    pub security_acknowledged: Option<bool>,
}

impl From<FfiModelImportSpec> for pumas_library::models::ModelImportSpec {
    fn from(s: FfiModelImportSpec) -> Self {
        Self {
            path: s.path,
            family: s.family,
            official_name: s.official_name,
            repo_id: s.repo_id,
            model_type: s.model_type,
            subtype: s.subtype,
            tags: s.tags,
            security_acknowledged: s.security_acknowledged,
        }
    }
}

/// Result of a model import operation.
#[derive(uniffi::Record)]
pub struct FfiModelImportResult {
    pub path: String,
    pub success: bool,
    pub model_path: Option<String>,
    pub error: Option<String>,
    pub security_tier: Option<FfiSecurityTier>,
}

impl From<pumas_library::models::ModelImportResult> for FfiModelImportResult {
    fn from(r: pumas_library::models::ModelImportResult) -> Self {
        Self {
            path: r.path,
            success: r.success,
            model_path: r.model_path,
            error: r.error,
            security_tier: r.security_tier.map(FfiSecurityTier::from),
        }
    }
}

/// Progress of a model download.
#[derive(uniffi::Record)]
pub struct FfiModelDownloadProgress {
    pub download_id: String,
    pub repo_id: Option<String>,
    pub status: FfiDownloadStatus,
    pub progress: Option<f32>,
    pub downloaded_bytes: Option<u64>,
    pub total_bytes: Option<u64>,
    pub speed: Option<f64>,
    pub eta_seconds: Option<f64>,
    pub error: Option<String>,
}

impl From<pumas_library::models::ModelDownloadProgress> for FfiModelDownloadProgress {
    fn from(p: pumas_library::models::ModelDownloadProgress) -> Self {
        Self {
            download_id: p.download_id,
            repo_id: p.repo_id,
            status: FfiDownloadStatus::from(p.status),
            progress: p.progress,
            downloaded_bytes: p.downloaded_bytes,
            total_bytes: p.total_bytes,
            speed: p.speed,
            eta_seconds: p.eta_seconds,
            error: p.error,
        }
    }
}

// ---- Response types ----

/// Response from deleting a model.
#[derive(uniffi::Record)]
pub struct FfiDeleteModelResponse {
    pub success: bool,
    pub error: Option<String>,
}

impl From<pumas_library::models::DeleteModelResponse> for FfiDeleteModelResponse {
    fn from(r: pumas_library::models::DeleteModelResponse) -> Self {
        Self {
            success: r.success,
            error: r.error,
        }
    }
}

/// Disk space information.
#[derive(uniffi::Record)]
pub struct FfiDiskSpaceResponse {
    pub success: bool,
    pub error: Option<String>,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub percent: f32,
}

impl From<pumas_library::models::DiskSpaceResponse> for FfiDiskSpaceResponse {
    fn from(r: pumas_library::models::DiskSpaceResponse) -> Self {
        Self {
            success: r.success,
            error: r.error,
            total: r.total,
            used: r.used,
            free: r.free,
            percent: r.percent,
        }
    }
}

/// Per-app resource usage.
#[derive(uniffi::Record)]
pub struct FfiAppResourceUsage {
    pub gpu_memory: Option<u64>,
    pub ram_memory: Option<u64>,
}

impl From<pumas_library::models::AppResourceUsage> for FfiAppResourceUsage {
    fn from(r: pumas_library::models::AppResourceUsage) -> Self {
        Self {
            gpu_memory: r.gpu_memory,
            ram_memory: r.ram_memory,
        }
    }
}

/// Resource usage for managed apps.
#[derive(uniffi::Record)]
pub struct FfiAppResources {
    pub comfyui: Option<FfiAppResourceUsage>,
    pub ollama: Option<FfiAppResourceUsage>,
}

impl From<pumas_library::models::AppResources> for FfiAppResources {
    fn from(r: pumas_library::models::AppResources) -> Self {
        Self {
            comfyui: r.comfyui.map(FfiAppResourceUsage::from),
            ollama: r.ollama.map(FfiAppResourceUsage::from),
        }
    }
}

/// Overall system status.
#[derive(uniffi::Record)]
pub struct FfiStatusResponse {
    pub success: bool,
    pub error: Option<String>,
    pub version: String,
    pub deps_ready: bool,
    pub patched: bool,
    pub menu_shortcut: bool,
    pub desktop_shortcut: bool,
    pub shortcut_version: Option<String>,
    pub message: String,
    pub comfyui_running: bool,
    pub ollama_running: bool,
    pub torch_running: bool,
    pub last_launch_error: Option<String>,
    pub last_launch_log: Option<String>,
    pub app_resources: Option<FfiAppResources>,
}

impl From<pumas_library::models::StatusResponse> for FfiStatusResponse {
    fn from(r: pumas_library::models::StatusResponse) -> Self {
        Self {
            success: r.success,
            error: r.error,
            version: r.version,
            deps_ready: r.deps_ready,
            patched: r.patched,
            menu_shortcut: r.menu_shortcut,
            desktop_shortcut: r.desktop_shortcut,
            shortcut_version: r.shortcut_version,
            message: r.message,
            comfyui_running: r.comfyui_running,
            ollama_running: r.ollama_running,
            torch_running: r.torch_running,
            last_launch_error: r.last_launch_error,
            last_launch_log: r.last_launch_log,
            app_resources: r.app_resources.map(FfiAppResources::from),
        }
    }
}

/// CPU resource info.
#[derive(uniffi::Record)]
pub struct FfiCpuResources {
    pub usage: f32,
    pub temp: Option<f32>,
}

impl From<pumas_library::models::CpuResources> for FfiCpuResources {
    fn from(r: pumas_library::models::CpuResources) -> Self {
        Self {
            usage: r.usage,
            temp: r.temp,
        }
    }
}

/// GPU resource info.
#[derive(uniffi::Record)]
pub struct FfiGpuResources {
    pub usage: f32,
    pub memory: u64,
    pub memory_total: u64,
    pub temp: Option<f32>,
}

impl From<pumas_library::models::GpuResources> for FfiGpuResources {
    fn from(r: pumas_library::models::GpuResources) -> Self {
        Self {
            usage: r.usage,
            memory: r.memory,
            memory_total: r.memory_total,
            temp: r.temp,
        }
    }
}

/// RAM resource info.
#[derive(uniffi::Record)]
pub struct FfiRamResources {
    pub usage: f32,
    pub total: u64,
}

impl From<pumas_library::models::RamResources> for FfiRamResources {
    fn from(r: pumas_library::models::RamResources) -> Self {
        Self {
            usage: r.usage,
            total: r.total,
        }
    }
}

/// Disk resource info.
#[derive(uniffi::Record)]
pub struct FfiDiskResources {
    pub usage: f32,
    pub total: u64,
    pub free: u64,
}

impl From<pumas_library::models::DiskResources> for FfiDiskResources {
    fn from(r: pumas_library::models::DiskResources) -> Self {
        Self {
            usage: r.usage,
            total: r.total,
            free: r.free,
        }
    }
}

/// Combined system resources.
#[derive(uniffi::Record)]
pub struct FfiSystemResources {
    pub cpu: FfiCpuResources,
    pub gpu: FfiGpuResources,
    pub ram: FfiRamResources,
    pub disk: FfiDiskResources,
}

impl From<pumas_library::models::SystemResources> for FfiSystemResources {
    fn from(r: pumas_library::models::SystemResources) -> Self {
        Self {
            cpu: FfiCpuResources::from(r.cpu),
            gpu: FfiGpuResources::from(r.gpu),
            ram: FfiRamResources::from(r.ram),
            disk: FfiDiskResources::from(r.disk),
        }
    }
}

/// System resources response.
#[derive(uniffi::Record)]
pub struct FfiSystemResourcesResponse {
    pub success: bool,
    pub error: Option<String>,
    pub resources: FfiSystemResources,
}

impl From<pumas_library::models::SystemResourcesResponse> for FfiSystemResourcesResponse {
    fn from(r: pumas_library::models::SystemResourcesResponse) -> Self {
        Self {
            success: r.success,
            error: r.error,
            resources: FfiSystemResources::from(r.resources),
        }
    }
}

// ---- HuggingFace / model library types ----

/// Request to download a model from HuggingFace.
#[derive(uniffi::Record)]
pub struct FfiDownloadRequest {
    pub repo_id: String,
    pub family: String,
    pub official_name: String,
    pub model_type: Option<String>,
    pub quant: Option<String>,
    pub filename: Option<String>,
}

impl From<FfiDownloadRequest> for pumas_library::model_library::DownloadRequest {
    fn from(r: FfiDownloadRequest) -> Self {
        Self {
            repo_id: r.repo_id,
            family: r.family,
            official_name: r.official_name,
            model_type: r.model_type,
            quant: r.quant,
            filename: r.filename,
        }
    }
}

/// HuggingFace metadata lookup result.
#[derive(uniffi::Record)]
pub struct FfiHfMetadataResult {
    pub repo_id: String,
    pub official_name: Option<String>,
    pub family: Option<String>,
    pub model_type: Option<String>,
    pub subtype: Option<String>,
    pub variant: Option<String>,
    pub precision: Option<String>,
    pub tags: Vec<String>,
    pub base_model: Option<String>,
    pub download_url: Option<String>,
    pub description: Option<String>,
    pub match_confidence: f64,
    pub match_method: String,
    pub requires_confirmation: bool,
    pub hash_mismatch: bool,
    pub matched_filename: Option<String>,
    pub pending_full_verification: bool,
    pub fast_hash: Option<String>,
    pub expected_sha256: Option<String>,
}

impl From<pumas_library::model_library::HfMetadataResult> for FfiHfMetadataResult {
    fn from(r: pumas_library::model_library::HfMetadataResult) -> Self {
        Self {
            repo_id: r.repo_id,
            official_name: r.official_name,
            family: r.family,
            model_type: r.model_type,
            subtype: r.subtype,
            variant: r.variant,
            precision: r.precision,
            tags: r.tags,
            base_model: r.base_model,
            download_url: r.download_url,
            description: r.description,
            match_confidence: r.match_confidence,
            match_method: r.match_method,
            requires_confirmation: r.requires_confirmation,
            hash_mismatch: r.hash_mismatch,
            matched_filename: r.matched_filename,
            pending_full_verification: r.pending_full_verification,
            fast_hash: r.fast_hash,
            expected_sha256: r.expected_sha256,
        }
    }
}

/// LFS file info from a HuggingFace repo.
#[derive(uniffi::Record)]
pub struct FfiLfsFileInfo {
    pub filename: String,
    pub size: u64,
    pub sha256: String,
}

impl From<pumas_library::model_library::LfsFileInfo> for FfiLfsFileInfo {
    fn from(f: pumas_library::model_library::LfsFileInfo) -> Self {
        Self {
            filename: f.filename,
            size: f.size,
            sha256: f.sha256,
        }
    }
}

/// File tree for a HuggingFace repository.
#[derive(uniffi::Record)]
pub struct FfiRepoFileTree {
    pub repo_id: String,
    pub lfs_files: Vec<FfiLfsFileInfo>,
    pub regular_files: Vec<String>,
    pub cached_at: String,
    pub last_modified: Option<String>,
}

impl From<pumas_library::model_library::RepoFileTree> for FfiRepoFileTree {
    fn from(t: pumas_library::model_library::RepoFileTree) -> Self {
        Self {
            repo_id: t.repo_id,
            lfs_files: t.lfs_files.into_iter().map(FfiLfsFileInfo::from).collect(),
            regular_files: t.regular_files,
            cached_at: t.cached_at,
            last_modified: t.last_modified,
        }
    }
}

// ---- ModelRecord / SearchResult wrappers (HashMap/Value → FFI-safe) ----

/// FFI-safe wrapper for `ModelRecord`.
#[derive(uniffi::Record)]
pub struct FfiModelRecord {
    pub id: String,
    pub path: String,
    pub cleaned_name: String,
    pub official_name: String,
    pub model_type: String,
    pub tags: Vec<String>,
    pub hashes: Vec<FfiHashEntry>,
    /// Full model metadata as a JSON string.
    pub metadata_json: String,
    pub updated_at: String,
}

impl From<ModelRecord> for FfiModelRecord {
    fn from(r: ModelRecord) -> Self {
        Self {
            id: r.id,
            path: r.path,
            cleaned_name: r.cleaned_name,
            official_name: r.official_name,
            model_type: r.model_type,
            tags: r.tags,
            hashes: r
                .hashes
                .into_iter()
                .map(|(k, v)| FfiHashEntry { key: k, value: v })
                .collect(),
            metadata_json: r.metadata.to_string(),
            updated_at: r.updated_at,
        }
    }
}

/// FFI-safe wrapper for `SearchResult`.
#[derive(uniffi::Record)]
pub struct FfiSearchResult {
    pub models: Vec<FfiModelRecord>,
    pub total_count: u64,
    pub query_time_ms: f64,
    pub query: String,
}

impl From<SearchResult> for FfiSearchResult {
    fn from(r: SearchResult) -> Self {
        Self {
            models: r.models.into_iter().map(FfiModelRecord::from).collect(),
            total_count: r.total_count as u64,
            query_time_ms: r.query_time_ms,
            query: r.query,
        }
    }
}

/// FFI-safe wrapper for `HuggingFaceModel`.
#[derive(uniffi::Record)]
pub struct FfiHuggingFaceModel {
    pub repo_id: String,
    pub name: String,
    pub developer: String,
    pub kind: String,
    pub formats: Vec<String>,
    pub quants: Vec<String>,
    pub download_options: Vec<FfiDownloadOption>,
    pub url: String,
    pub release_date: Option<String>,
    pub downloads: Option<u64>,
    pub total_size_bytes: Option<u64>,
    pub quant_sizes: Vec<FfiQuantSize>,
    pub compatible_engines: Vec<String>,
}

impl From<HuggingFaceModel> for FfiHuggingFaceModel {
    fn from(m: HuggingFaceModel) -> Self {
        Self {
            repo_id: m.repo_id,
            name: m.name,
            developer: m.developer,
            kind: m.kind,
            formats: m.formats,
            quants: m.quants,
            download_options: m
                .download_options
                .into_iter()
                .map(FfiDownloadOption::from)
                .collect(),
            url: m.url,
            release_date: m.release_date,
            downloads: m.downloads,
            total_size_bytes: m.total_size_bytes,
            quant_sizes: m
                .quant_sizes
                .map(|qs| {
                    qs.into_iter()
                        .map(|(k, v)| FfiQuantSize {
                            quant: k,
                            size_bytes: v,
                        })
                        .collect()
                })
                .unwrap_or_default(),
            compatible_engines: m.compatible_engines,
        }
    }
}

// =============================================================================
// Inference Settings FFI Types
// =============================================================================

/// FFI-safe wrapper for `ParamConstraints`.
#[derive(uniffi::Record)]
pub struct FfiParamConstraints {
    pub min: Option<f64>,
    pub max: Option<f64>,
    /// JSON-encoded array of allowed values, or None if unconstrained.
    pub allowed_values_json: Option<String>,
}

impl From<pumas_library::models::ParamConstraints> for FfiParamConstraints {
    fn from(c: pumas_library::models::ParamConstraints) -> Self {
        Self {
            min: c.min,
            max: c.max,
            allowed_values_json: c
                .allowed_values
                .map(|v| serde_json::to_string(&v).unwrap_or_default()),
        }
    }
}

impl From<FfiParamConstraints> for pumas_library::models::ParamConstraints {
    fn from(c: FfiParamConstraints) -> Self {
        Self {
            min: c.min,
            max: c.max,
            allowed_values: c.allowed_values_json.and_then(|s| serde_json::from_str(&s).ok()),
        }
    }
}

/// FFI-safe wrapper for `InferenceParamSchema`.
#[derive(uniffi::Record)]
pub struct FfiInferenceParamSchema {
    pub key: String,
    pub label: String,
    /// One of: "Number", "Integer", "String", "Boolean"
    pub param_type: String,
    /// JSON-encoded default value.
    pub default_json: String,
    pub description: Option<String>,
    pub constraints: Option<FfiParamConstraints>,
}

impl From<pumas_library::models::InferenceParamSchema> for FfiInferenceParamSchema {
    fn from(s: pumas_library::models::InferenceParamSchema) -> Self {
        use pumas_library::models::ParamType;
        Self {
            key: s.key,
            label: s.label,
            param_type: match s.param_type {
                ParamType::Number => "Number".to_string(),
                ParamType::Integer => "Integer".to_string(),
                ParamType::String => "String".to_string(),
                ParamType::Boolean => "Boolean".to_string(),
            },
            default_json: serde_json::to_string(&s.default).unwrap_or_default(),
            description: s.description,
            constraints: s.constraints.map(FfiParamConstraints::from),
        }
    }
}

impl From<FfiInferenceParamSchema> for pumas_library::models::InferenceParamSchema {
    fn from(s: FfiInferenceParamSchema) -> Self {
        use pumas_library::models::ParamType;
        Self {
            key: s.key,
            label: s.label,
            param_type: match s.param_type.as_str() {
                "Integer" => ParamType::Integer,
                "String" => ParamType::String,
                "Boolean" => ParamType::Boolean,
                _ => ParamType::Number,
            },
            default: serde_json::from_str(&s.default_json).unwrap_or(serde_json::Value::Null),
            description: s.description,
            constraints: s.constraints.map(pumas_library::models::ParamConstraints::from),
        }
    }
}

// =============================================================================
// Torch Inference FFI Types
// =============================================================================

/// Compute device for model loading.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiComputeDevice {
    Cpu,
    Cuda { index: u32 },
    Mps,
    Auto,
}

/// State of a model slot.
#[derive(Debug, Clone, uniffi::Enum)]
pub enum FfiSlotState {
    Unloaded,
    Loading,
    Ready,
    Unloading,
    Error,
}

/// A loaded model slot in the Torch inference server.
#[derive(uniffi::Record)]
pub struct FfiModelSlot {
    pub slot_id: String,
    pub model_name: String,
    pub model_path: String,
    pub device: FfiComputeDevice,
    pub state: FfiSlotState,
    pub gpu_memory_bytes: Option<u64>,
    pub ram_memory_bytes: Option<u64>,
    pub model_type: Option<String>,
}

/// Configuration for the Torch inference server.
#[derive(uniffi::Record)]
pub struct FfiTorchServerConfig {
    pub api_port: u16,
    pub host: String,
    pub max_loaded_models: u32,
    pub lan_access: bool,
}

/// Information about a compute device.
#[derive(uniffi::Record)]
pub struct FfiDeviceInfo {
    pub device_id: String,
    pub name: String,
    pub memory_total: u64,
    pub memory_available: u64,
    pub is_available: bool,
}

// =============================================================================
// FfiApiConfig — Configuration record for API initialization
// =============================================================================

/// Configuration for creating an `FfiPumasApi` instance.
#[derive(uniffi::Record)]
pub struct FfiApiConfig {
    pub launcher_root: String,
    pub auto_create_dirs: bool,
    pub enable_hf: bool,
}

// =============================================================================
// FfiPumasApi — The main API object exposed to foreign languages
// =============================================================================

/// The main Pumas Library API handle.
#[derive(uniffi::Object)]
pub struct FfiPumasApi {
    inner: Arc<PumasApi>,
}

#[uniffi::export(async_runtime = "tokio")]
impl FfiPumasApi {
    /// Create a new API instance with default options.
    #[uniffi::constructor]
    pub async fn new(launcher_root: String) -> Result<Arc<Self>, FfiError> {
        let api = PumasApi::new(launcher_root).await.map_err(FfiError::from)?;
        Ok(Arc::new(Self {
            inner: Arc::new(api),
        }))
    }

    /// Create a new API instance with a configuration record.
    #[uniffi::constructor]
    pub async fn with_config(config: FfiApiConfig) -> Result<Arc<Self>, FfiError> {
        let api = PumasApi::builder(config.launcher_root)
            .auto_create_dirs(config.auto_create_dirs)
            .with_hf_client(config.enable_hf)
            .with_process_manager(false)
            .build()
            .await
            .map_err(FfiError::from)?;
        Ok(Arc::new(Self {
            inner: Arc::new(api),
        }))
    }

    // ========================================
    // Model Library Methods
    // ========================================

    /// List all models in the library.
    pub async fn list_models(&self) -> Result<Vec<FfiModelRecord>, FfiError> {
        let models = self.inner.list_models().await.map_err(FfiError::from)?;
        Ok(models.into_iter().map(FfiModelRecord::from).collect())
    }

    /// Get a single model by its ID.
    pub async fn get_model(&self, model_id: String) -> Result<Option<FfiModelRecord>, FfiError> {
        let model = self
            .inner
            .get_model(&model_id)
            .await
            .map_err(FfiError::from)?;
        Ok(model.map(FfiModelRecord::from))
    }

    /// Search models using full-text search.
    pub async fn search_models(
        &self,
        query: String,
        limit: u64,
        offset: u64,
    ) -> Result<FfiSearchResult, FfiError> {
        let result = self
            .inner
            .search_models(&query, limit as usize, offset as usize)
            .await
            .map_err(FfiError::from)?;
        Ok(FfiSearchResult::from(result))
    }

    /// Delete a model and all its links.
    pub async fn delete_model(
        &self,
        model_id: String,
    ) -> Result<FfiDeleteModelResponse, FfiError> {
        let resp = self
            .inner
            .delete_model_with_cascade(&model_id)
            .await
            .map_err(FfiError::from)?;
        Ok(FfiDeleteModelResponse::from(resp))
    }

    /// Import a model from a local file path.
    pub async fn import_model(
        &self,
        spec: FfiModelImportSpec,
    ) -> Result<FfiModelImportResult, FfiError> {
        let core_spec = pumas_library::models::ModelImportSpec::from(spec);
        let result = self
            .inner
            .import_model(&core_spec)
            .await
            .map_err(FfiError::from)?;
        Ok(FfiModelImportResult::from(result))
    }

    /// Import multiple models in a batch.
    pub async fn import_models_batch(
        &self,
        specs: Vec<FfiModelImportSpec>,
    ) -> Vec<FfiModelImportResult> {
        let core_specs: Vec<pumas_library::models::ModelImportSpec> =
            specs.into_iter().map(Into::into).collect();
        self.inner
            .import_models_batch(core_specs)
            .await
            .into_iter()
            .map(FfiModelImportResult::from)
            .collect()
    }

    /// Rebuild the full-text search index for all models.
    pub async fn rebuild_model_index(&self) -> Result<u64, FfiError> {
        self.inner
            .rebuild_model_index()
            .await
            .map(|n| n as u64)
            .map_err(FfiError::from)
    }

    /// Get the inference settings schema for a model.
    ///
    /// Returns the stored settings if present, otherwise lazily computes
    /// defaults based on model type and format.
    pub async fn get_inference_settings(
        &self,
        model_id: String,
    ) -> Result<Vec<FfiInferenceParamSchema>, FfiError> {
        let settings = self
            .inner
            .get_inference_settings(&model_id)
            .await
            .map_err(FfiError::from)?;
        Ok(settings
            .into_iter()
            .map(FfiInferenceParamSchema::from)
            .collect())
    }

    /// Replace the inference settings schema for a model.
    pub async fn update_inference_settings(
        &self,
        model_id: String,
        settings: Vec<FfiInferenceParamSchema>,
    ) -> Result<(), FfiError> {
        let core_settings: Vec<pumas_library::models::InferenceParamSchema> =
            settings.into_iter().map(Into::into).collect();
        self.inner
            .update_inference_settings(&model_id, core_settings)
            .await
            .map_err(FfiError::from)
    }

    // ========================================
    // HuggingFace Methods
    // ========================================

    /// Search for models on HuggingFace.
    pub async fn search_hf_models(
        &self,
        query: String,
        kind: Option<String>,
        limit: u64,
    ) -> Result<Vec<FfiHuggingFaceModel>, FfiError> {
        let models = self
            .inner
            .search_hf_models(&query, kind.as_deref(), limit as usize)
            .await
            .map_err(FfiError::from)?;
        Ok(models.into_iter().map(FfiHuggingFaceModel::from).collect())
    }

    /// Start downloading a model from HuggingFace.
    pub async fn start_hf_download(
        &self,
        request: FfiDownloadRequest,
    ) -> Result<String, FfiError> {
        let core_req = pumas_library::model_library::DownloadRequest::from(request);
        self.inner
            .start_hf_download(&core_req)
            .await
            .map_err(FfiError::from)
    }

    /// Get the progress of an active HuggingFace download.
    pub async fn get_hf_download_progress(
        &self,
        download_id: String,
    ) -> Option<FfiModelDownloadProgress> {
        self.inner
            .get_hf_download_progress(&download_id)
            .await
            .map(FfiModelDownloadProgress::from)
    }

    /// Cancel an active HuggingFace download.
    pub async fn cancel_hf_download(&self, download_id: String) -> Result<bool, FfiError> {
        self.inner
            .cancel_hf_download(&download_id)
            .await
            .map_err(FfiError::from)
    }

    /// Look up HuggingFace metadata for a local model file.
    pub async fn lookup_hf_metadata_for_file(
        &self,
        file_path: String,
    ) -> Result<Option<FfiHfMetadataResult>, FfiError> {
        let result = self
            .inner
            .lookup_hf_metadata_for_file(&file_path)
            .await
            .map_err(FfiError::from)?;
        Ok(result.map(FfiHfMetadataResult::from))
    }

    /// Get the file tree for a HuggingFace repository.
    pub async fn get_hf_repo_files(
        &self,
        repo_id: String,
    ) -> Result<FfiRepoFileTree, FfiError> {
        let tree = self
            .inner
            .get_hf_repo_files(&repo_id)
            .await
            .map_err(FfiError::from)?;
        Ok(FfiRepoFileTree::from(tree))
    }

    // ========================================
    // System Info Methods
    // ========================================

    /// Check if the network is currently online.
    pub fn is_online(&self) -> bool {
        self.inner.is_online()
    }

    /// Get disk space information for the launcher root.
    pub async fn get_disk_space(&self) -> Result<FfiDiskSpaceResponse, FfiError> {
        let resp = self.inner.get_disk_space().await.map_err(FfiError::from)?;
        Ok(FfiDiskSpaceResponse::from(resp))
    }

    /// Get overall system status including running processes and resources.
    pub async fn get_status(&self) -> Result<FfiStatusResponse, FfiError> {
        let resp = self.inner.get_status().await.map_err(FfiError::from)?;
        Ok(FfiStatusResponse::from(resp))
    }

    /// Get current system resource usage (CPU, GPU, RAM, disk).
    pub async fn get_system_resources(&self) -> Result<FfiSystemResourcesResponse, FfiError> {
        let resp = self
            .inner
            .get_system_resources()
            .await
            .map_err(FfiError::from)?;
        Ok(FfiSystemResourcesResponse::from(resp))
    }

    // ========================================
    // Torch Inference Methods
    // ========================================

    /// Check if the Torch inference server is running.
    pub async fn is_torch_running(&self) -> bool {
        self.inner.is_torch_running().await
    }

    /// Stop the Torch inference server.
    pub async fn torch_stop(&self) -> Result<bool, FfiError> {
        self.inner.stop_torch().await.map_err(FfiError::from)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_error_conversion() {
        let err = pumas_library::PumasError::ModelNotFound {
            model_id: "test-model".to_string(),
        };
        let ffi_err: FfiError = err.into();
        assert!(matches!(ffi_err, FfiError::Model { .. }));
    }

    #[test]
    fn test_ffi_error_launch_variant() {
        let err = pumas_library::PumasError::LaunchFailed {
            app: "ollama".to_string(),
            message: "port in use".to_string(),
        };
        let ffi_err: FfiError = err.into();
        assert!(matches!(ffi_err, FfiError::Launch { .. }));
        if let FfiError::Launch { message } = ffi_err {
            assert!(message.contains("ollama"));
            assert!(message.contains("port in use"));
        }
    }

    #[test]
    fn test_ffi_error_process_variant() {
        let err = pumas_library::PumasError::ProcessNotRunning {
            app: "comfyui".to_string(),
        };
        let ffi_err: FfiError = err.into();
        assert!(matches!(ffi_err, FfiError::Process { .. }));
    }

    #[test]
    fn test_ffi_model_record_conversion() {
        let mut hashes = std::collections::HashMap::new();
        hashes.insert("sha256".to_string(), "abc123".to_string());

        let record = ModelRecord {
            id: "test-id".to_string(),
            path: "/models/test".to_string(),
            cleaned_name: "test-model".to_string(),
            official_name: "Test Model".to_string(),
            model_type: "llm".to_string(),
            tags: vec!["tag1".to_string()],
            hashes,
            metadata: serde_json::json!({"key": "value"}),
            updated_at: "2025-01-01".to_string(),
        };

        let ffi_record = FfiModelRecord::from(record);
        assert_eq!(ffi_record.id, "test-id");
        assert_eq!(ffi_record.official_name, "Test Model");
        assert_eq!(ffi_record.hashes.len(), 1);
        assert_eq!(ffi_record.hashes[0].key, "sha256");
        assert_eq!(ffi_record.hashes[0].value, "abc123");
        assert!(ffi_record.metadata_json.contains("key"));
    }

    #[test]
    fn test_ffi_search_result_conversion() {
        let result = SearchResult {
            models: vec![],
            total_count: 42,
            query_time_ms: 1.5,
            query: "test".to_string(),
        };

        let ffi_result = FfiSearchResult::from(result);
        assert_eq!(ffi_result.total_count, 42);
        assert_eq!(ffi_result.query, "test");
    }

    #[test]
    fn test_ffi_huggingface_model_quant_sizes() {
        let mut quant_sizes = std::collections::HashMap::new();
        quant_sizes.insert("Q4_K_M".to_string(), 4_200_000_000u64);
        quant_sizes.insert("Q8_0".to_string(), 8_100_000_000u64);

        let model = HuggingFaceModel {
            repo_id: "test/model".to_string(),
            name: "Test".to_string(),
            developer: "dev".to_string(),
            kind: "llm".to_string(),
            formats: vec![],
            quants: vec![],
            download_options: vec![],
            url: "https://example.com".to_string(),
            release_date: None,
            downloads: None,
            total_size_bytes: None,
            quant_sizes: Some(quant_sizes),
            compatible_engines: vec![],
        };

        let ffi_model = FfiHuggingFaceModel::from(model);
        assert_eq!(ffi_model.quant_sizes.len(), 2);

        let q4 = ffi_model
            .quant_sizes
            .iter()
            .find(|qs| qs.quant == "Q4_K_M");
        assert!(q4.is_some());
        assert_eq!(q4.unwrap().size_bytes, 4_200_000_000);
    }

    #[test]
    fn test_ffi_huggingface_model_no_quant_sizes() {
        let model = HuggingFaceModel {
            repo_id: "test/model".to_string(),
            name: "Test".to_string(),
            developer: "dev".to_string(),
            kind: "llm".to_string(),
            formats: vec![],
            quants: vec![],
            download_options: vec![],
            url: "https://example.com".to_string(),
            release_date: None,
            downloads: None,
            total_size_bytes: None,
            quant_sizes: None,
            compatible_engines: vec![],
        };

        let ffi_model = FfiHuggingFaceModel::from(model);
        assert!(ffi_model.quant_sizes.is_empty());
    }
}
