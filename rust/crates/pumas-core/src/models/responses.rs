//! API response types matching the frontend TypeScript interfaces.

use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Base response with success flag.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct BaseResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl BaseResponse {
    /// Create a successful base response with no error.
    pub fn success() -> Self {
        Self {
            success: true,
            error: None,
        }
    }

    /// Create a failed base response with the given error message.
    pub fn error(message: impl Into<String>) -> Self {
        Self {
            success: false,
            error: Some(message.into()),
        }
    }
}

/// Disk space response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DiskSpaceResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub total: u64,
    pub used: u64,
    pub free: u64,
    pub percent: f32,
}

/// CPU resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct CpuResources {
    pub usage: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp: Option<f32>,
}

/// GPU resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct GpuResources {
    pub usage: f32,
    pub memory: u64,
    pub memory_total: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temp: Option<f32>,
}

/// RAM resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct RamResources {
    pub usage: f32,
    pub total: u64,
}

/// Disk resources.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DiskResources {
    pub usage: f32,
    pub total: u64,
    pub free: u64,
}

/// System resources container.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SystemResources {
    pub cpu: CpuResources,
    pub gpu: GpuResources,
    pub ram: RamResources,
    pub disk: DiskResources,
}

/// System resources response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SystemResourcesResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub resources: SystemResources,
}

/// App-specific resource usage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct AppResourceUsage {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gpu_memory: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ram_memory: Option<u64>,
}

/// App resources container.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct AppResources {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comfyui: Option<AppResourceUsage>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ollama: Option<AppResourceUsage>,
}

/// Status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct StatusResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub version: String,
    pub deps_ready: bool,
    pub patched: bool,
    pub menu_shortcut: bool,
    pub desktop_shortcut: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shortcut_version: Option<String>,
    pub message: String,
    pub comfyui_running: bool,
    pub ollama_running: bool,
    pub torch_running: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_launch_error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_launch_log: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_resources: Option<AppResources>,
}

/// Models response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub models: HashMap<String, ModelData>,
}

/// Search HF models response.
///
/// Note: Not FFI-compatible due to HuggingFaceModel containing HashMap.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SearchHfModelsResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub models: Vec<HuggingFaceModel>,
}

/// Model download response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ModelDownloadResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub download_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_path: Option<String>,
}

/// FTS search response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct FtsSearchResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub models: Vec<FtsSearchModel>,
    pub total_count: u32,
    pub query_time_ms: u64,
    pub query: String,
}

/// Import batch response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ImportBatchResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub imported: u32,
    pub failed: u32,
    pub results: Vec<ModelImportResult>,
}

/// Network status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct NetworkStatusResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub total_requests: u64,
    pub successful_requests: u64,
    pub failed_requests: u64,
    pub circuit_breaker_rejections: u64,
    pub retries: u64,
    pub success_rate: f64,
    pub circuit_states: HashMap<String, String>,
    pub is_offline: bool,
}

/// Library status response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LibraryStatusResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub indexing: bool,
    pub deep_scan_in_progress: bool,
    pub model_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pending_lookups: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deep_scan_progress: Option<DeepScanProgress>,
}

/// File type validation response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct FileTypeValidationResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub valid: bool,
    pub detected_type: String,
}

/// Sync-with-resolutions response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SyncWithResolutionsResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub links_created: usize,
    pub links_skipped: usize,
    pub links_renamed: usize,
    pub overwrites: usize,
    pub errors: Vec<String>,
}

/// Cross-filesystem warning response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CrossFilesystemWarningResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub cross_filesystem: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub library_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recommendation: Option<String>,
}

/// Deep scan progress.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DeepScanProgress {
    pub current: u32,
    pub total: u32,
    pub stage: String,
}

/// Link health status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum HealthStatus {
    Healthy,
    Warnings,
    Errors,
}

/// Link type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum LinkType {
    Symlink,
    Hardlink,
    Copy,
}

/// Broken link information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct BrokenLinkInfo {
    pub link_id: i64,
    pub target_path: String,
    pub expected_source: String,
    pub model_id: String,
    pub reason: String,
}

/// Link health response.
///
/// Note: Not FFI-compatible due to `usize` fields. Use wrapper types in pumas-uniffi.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LinkHealthResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub status: String,
    pub total_links: usize,
    pub healthy_links: usize,
    pub broken_links: Vec<String>,
    pub orphaned_links: Vec<String>,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Clean broken links response.
///
/// Note: Not FFI-compatible due to `usize` fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct CleanBrokenLinksResponse {
    pub success: bool,
    pub cleaned: usize,
}

/// Link information for a model.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LinkInfo {
    pub source: String,
    pub target: String,
    pub link_type: String,
    pub app_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub app_version: Option<String>,
    pub created_at: String,
}

/// Links for model response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LinksForModelResponse {
    pub success: bool,
    pub links: Vec<LinkInfo>,
}

/// Delete model response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct DeleteModelResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Serialisable mapping action for preview responses.
///
/// Note: Not FFI-compatible due to `PathBuf` fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MappingActionInfo {
    pub model_id: String,
    pub model_name: String,
    pub source_path: String,
    pub target_path: String,
    pub reason: String,
}

/// Broken-link entry for preview responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct BrokenLinkEntry {
    pub target_path: String,
    pub existing_target: String,
    pub reason: String,
}

/// Mapping preview response matching the frontend `MappingPreviewResponse`.
///
/// Note: Not FFI-compatible.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MappingPreviewResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub to_create: Vec<MappingActionInfo>,
    pub to_skip_exists: Vec<MappingActionInfo>,
    pub conflicts: Vec<MappingActionInfo>,
    pub broken_to_remove: Vec<BrokenLinkEntry>,
    pub total_actions: usize,
    pub warnings: Vec<String>,
    pub errors: Vec<String>,
}

/// Mapping apply response matching the frontend `ApplyModelMappingResponse`.
///
/// Note: Not FFI-compatible due to `usize` fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct MappingApplyResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub links_created: usize,
    pub links_removed: usize,
    pub total_links: usize,
}

/// Sync models response.
///
/// Note: Not FFI-compatible due to `usize` fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct SyncModelsResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub synced: usize,
    pub errors: Vec<String>,
}

/// Response for getting excluded model IDs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub struct LinkExclusionsResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub excluded_model_ids: Vec<String>,
}

/// Shortcut state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct ShortcutState {
    pub menu: bool,
    pub desktop: bool,
    pub tag: String,
}

/// Launcher version response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LauncherVersionResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub version: String,
    pub branch: String,
    pub is_git_repo: bool,
}

/// Check launcher updates response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct CheckLauncherUpdatesResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub has_update: bool,
    pub current_commit: String,
    pub latest_commit: String,
    pub commits_behind: u32,
    pub commits: Vec<CommitInfo>,
}

/// Commit information.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct CommitInfo {
    pub hash: String,
    pub message: String,
    pub author: String,
    pub date: String,
}

/// Launch response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct LaunchResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ready: Option<bool>,
}

/// Sandbox type.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Enum))]
pub enum SandboxType {
    Flatpak,
    Snap,
    Docker,
    Appimage,
    None,
    Unknown,
}

/// Sandbox info response.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "uniffi", derive(uniffi::Record))]
pub struct SandboxInfoResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub is_sandboxed: bool,
    pub sandbox_type: SandboxType,
    pub limitations: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_base_response() {
        let success = BaseResponse::success();
        assert!(success.success);
        assert!(success.error.is_none());

        let error = BaseResponse::error("Something went wrong");
        assert!(!error.success);
        assert_eq!(error.error, Some("Something went wrong".into()));
    }

    #[test]
    fn test_status_response_serialization() {
        let response = StatusResponse {
            success: true,
            error: None,
            version: "v1.0.0".into(),
            deps_ready: true,
            patched: false,
            menu_shortcut: true,
            desktop_shortcut: false,
            shortcut_version: Some("v1.0.0".into()),
            message: "Ready".into(),
            comfyui_running: false,
            ollama_running: false,
            torch_running: false,
            last_launch_error: None,
            last_launch_log: None,
            app_resources: None,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"success\":true"));
        assert!(json.contains("\"deps_ready\":true"));
    }
}
