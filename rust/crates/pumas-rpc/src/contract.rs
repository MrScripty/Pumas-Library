//! Transport-independent public RPC error projection.
//!
//! Internal errors may contain credentials, filesystem paths, URLs, process
//! details, or upstream response text. Transports must project them through
//! [`PublicError`] instead of serializing `Display` or `Debug` output.

use pumas_library::{
    conversion::{
        BackendStatus, ConversionDirection, ConversionProgress, ConversionRequest,
        ConversionSetupSnapshot, ConversionSetupStatus, ConversionStatus, QuantBackend,
        QuantOption,
    },
    model_library::{
        issue_download_recovery_ticket, DownloadRecoveryModelId, DownloadRecoveryTicket,
        DownloadRecoveryToken,
    },
    models::{
        BaseResponse, CleanBrokenLinksResponse, DeleteModelResponse, DiskSpaceResponse,
        DownloadStatus, LibraryStatusResponse, LinkExclusionsResponse, LinkHealthResponse,
        LinksForModelResponse, ModelDownloadProgress, NetworkStatusResponse, PartialDownloadAction,
        StatusResponse, StatusTelemetrySnapshot, SystemResourcesResponse,
    },
    DownloadRequest, HfAuthStatus, ModelRecord, PumasError, SystemCheckResult, UpdateApplyResult,
    UpdateCheckResult,
};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::path::Path;

const MAX_METHOD_BYTES: usize = 128;
const MAX_IDENTIFIER_BYTES: usize = 4 * 1024;
const MAX_CREDENTIAL_BYTES: usize = 4 * 1024;
const MAX_COLLECTION_ITEMS: usize = 512;
const MAX_METADATA_JSON_BYTES: usize = 1024 * 1024;
const MAX_JS_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[cfg(feature = "export-contract")]
mod export;
#[cfg(feature = "export-contract")]
pub(crate) use export::{desktop_contract_fixtures, desktop_contract_schema};

/// Stable public failure categories shared by RPC transports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum PublicErrorClass {
    InvalidRequest,
    NotFound,
    Conflict,
    Cancelled,
    Unavailable,
    OperationFailed,
    Internal,
}

impl PublicErrorClass {
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRequest => "invalid_request",
            Self::NotFound => "not_found",
            Self::Conflict => "conflict",
            Self::Cancelled => "cancelled",
            Self::Unavailable => "unavailable",
            Self::OperationFailed => "operation_failed",
            Self::Internal => "internal",
        }
    }
}

/// Bounded, deny-by-default representation safe to expose outside the process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct PublicError {
    pub(crate) code: i32,
    pub(crate) class: PublicErrorClass,
    pub(crate) message: &'static str,
}

impl PublicError {
    pub(crate) const fn parse_error() -> Self {
        Self {
            code: -32700,
            class: PublicErrorClass::InvalidRequest,
            message: "The request body is not valid JSON.",
        }
    }

    pub(crate) const fn invalid_request() -> Self {
        Self {
            code: -32600,
            class: PublicErrorClass::InvalidRequest,
            message: "The JSON-RPC request is invalid.",
        }
    }

    pub(crate) const fn method_not_found() -> Self {
        Self {
            code: -32601,
            class: PublicErrorClass::NotFound,
            message: "The requested method is not supported.",
        }
    }

    pub(crate) const fn invalid_params() -> Self {
        Self {
            code: -32602,
            class: PublicErrorClass::InvalidRequest,
            message: "Request parameters are invalid.",
        }
    }

    pub(crate) const fn internal() -> Self {
        Self {
            code: -32603,
            class: PublicErrorClass::Internal,
            message: "The request could not be completed due to an internal error.",
        }
    }

    pub(crate) const fn unavailable() -> Self {
        Self {
            code: -32000,
            class: PublicErrorClass::Unavailable,
            message: "A required operation is currently unavailable.",
        }
    }

    pub(crate) fn from_pumas(error: &PumasError) -> Self {
        if matches!(
            error,
            PumasError::Config { .. } | PumasError::DownloadLifecycleClosed
        ) {
            return Self::unavailable();
        }
        if matches!(error, PumasError::DownloadShutdownFailed { .. }) {
            return Self::internal();
        }

        match error.to_rpc_error_code() {
            -32602 | -32005 => Self {
                code: error.to_rpc_error_code(),
                class: PublicErrorClass::InvalidRequest,
                message: "Request parameters are invalid.",
            },
            -32001 | -32002 | -32009 => Self {
                code: error.to_rpc_error_code(),
                class: PublicErrorClass::NotFound,
                message: "The requested resource was not found.",
            },
            -32004 => Self {
                code: error.to_rpc_error_code(),
                class: PublicErrorClass::Cancelled,
                message: "The requested operation did not complete.",
            },
            -32011 => Self {
                code: error.to_rpc_error_code(),
                class: PublicErrorClass::Conflict,
                message: "The request conflicts with the current library state.",
            },
            -32000 | -32006 | -32007 | -32010 | -32012 => Self {
                code: error.to_rpc_error_code(),
                class: PublicErrorClass::Unavailable,
                message: "A required operation is currently unavailable.",
            },
            -32003 | -32008 => Self {
                code: error.to_rpc_error_code(),
                class: PublicErrorClass::OperationFailed,
                message: "The requested operation failed.",
            },
            _ => Self::internal(),
        }
    }
}

/// A JSON-RPC request admitted at the producer boundary.
///
/// This type intentionally does not implement `Debug`: one command contains a
/// credential. Callers can log [`RpcCommand::method`] instead.
pub(crate) struct AdmittedRpcRequest {
    pub(crate) id: Option<Value>,
    pub(crate) command: RpcCommand,
}

/// Stable public admission failure with the request ID when it was safe to
/// recover from a structurally valid envelope.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RpcAdmissionError {
    pub(crate) id: Option<Value>,
    pub(crate) error: PublicError,
}

/// Closed typed commands migrated into the desktop producer contract.
///
/// `Legacy` is an explicit migration state, not a supported extension point.
/// Its method is still resolved by the producer dispatcher, where unknown
/// names become method-not-found without reaching a domain handler.
pub(crate) enum RpcCommand {
    HealthCheck,
    Shutdown,
    GetStatus,
    GetDiskSpace,
    GetSystemResources,
    GetStatusTelemetrySnapshot,
    GetLauncherVersion,
    CheckLauncherUpdates {
        force_refresh: bool,
    },
    ApplyLauncherUpdate,
    RestartLauncher,
    GetSandboxInfo,
    CheckGit,
    GetNetworkStatus,
    GetLibraryStatus,
    #[cfg(feature = "inference-plugins")]
    GetAppStatus {
        app_id: String,
    },
    SetHfToken {
        token: SecretToken,
    },
    ClearHfToken,
    GetHfAuthStatus,
    GetLinkHealth {
        version_tag: Option<String>,
    },
    CleanBrokenLinks,
    RemoveOrphanedLinks {
        version_tag: String,
    },
    GetLinksForModel {
        model_id: String,
    },
    DeleteModelWithCascade {
        model_id: String,
    },
    GetFileLinkCount {
        file_path: String,
    },
    CheckFilesWritable {
        file_paths: Vec<String>,
    },
    SetModelLinkExclusion {
        model_id: String,
        app_id: String,
        excluded: bool,
    },
    GetLinkExclusions {
        app_id: String,
    },
    StartModelConversion {
        request: ConversionRequest,
    },
    GetConversionProgress {
        conversion_id: String,
    },
    CancelModelConversion {
        conversion_id: String,
    },
    ListModelConversions,
    CheckConversionEnvironment,
    SetupConversionEnvironment,
    StartConversionSetup {
        expected_previous_operation_id: Option<String>,
    },
    GetConversionSetup,
    StartBackendSetup {
        backend: QuantBackend,
        expected_previous_operation_id: Option<String>,
    },
    GetBackendSetup {
        backend: QuantBackend,
    },
    GetSupportedQuantTypes,
    GetBackendStatus,
    SetupQuantizationBackend {
        backend: QuantBackend,
    },
    OpenPath {
        path: String,
    },
    OpenUrl {
        url: String,
    },
    DownloadModelFromHf {
        request: DownloadRequest,
    },
    StartModelDownloadFromHf {
        request: DownloadRequest,
    },
    GetModelDownloadStatus {
        download_id: String,
    },
    CancelModelDownload {
        download_id: String,
    },
    PauseModelDownload {
        download_id: String,
    },
    ResumeModelDownload {
        download_id: String,
    },
    ListModelDownloads,
    ResumePartialDownload {
        model_id: DownloadRecoveryModelId,
        recovery_token: DownloadRecoveryToken,
    },
    GetModels,
    SearchCatalog {
        query: String,
        limit: usize,
        offset: usize,
    },
    RefreshModelIndex,
    Legacy {
        method: String,
        params: Value,
    },
}

impl RpcCommand {
    pub(crate) fn method(&self) -> &str {
        match self {
            Self::HealthCheck => "health_check",
            Self::Shutdown => "shutdown",
            Self::GetStatus => "get_status",
            Self::GetDiskSpace => "get_disk_space",
            Self::GetSystemResources => "get_system_resources",
            Self::GetStatusTelemetrySnapshot => "get_status_telemetry_snapshot",
            Self::GetLauncherVersion => "get_launcher_version",
            Self::CheckLauncherUpdates { .. } => "check_launcher_updates",
            Self::ApplyLauncherUpdate => "apply_launcher_update",
            Self::RestartLauncher => "restart_launcher",
            Self::GetSandboxInfo => "get_sandbox_info",
            Self::CheckGit => "check_git",
            Self::GetNetworkStatus => "get_network_status",
            Self::GetLibraryStatus => "get_library_status",
            #[cfg(feature = "inference-plugins")]
            Self::GetAppStatus { .. } => "get_app_status",
            Self::SetHfToken { .. } => "set_hf_token",
            Self::ClearHfToken => "clear_hf_token",
            Self::GetHfAuthStatus => "get_hf_auth_status",
            Self::GetLinkHealth { .. } => "get_link_health",
            Self::CleanBrokenLinks => "clean_broken_links",
            Self::RemoveOrphanedLinks { .. } => "remove_orphaned_links",
            Self::GetLinksForModel { .. } => "get_links_for_model",
            Self::DeleteModelWithCascade { .. } => "delete_model_with_cascade",
            Self::GetFileLinkCount { .. } => "get_file_link_count",
            Self::CheckFilesWritable { .. } => "check_files_writable",
            Self::SetModelLinkExclusion { .. } => "set_model_link_exclusion",
            Self::GetLinkExclusions { .. } => "get_link_exclusions",
            Self::StartModelConversion { .. } => "start_model_conversion",
            Self::GetConversionProgress { .. } => "get_conversion_progress",
            Self::CancelModelConversion { .. } => "cancel_model_conversion",
            Self::ListModelConversions => "list_model_conversions",
            Self::CheckConversionEnvironment => "check_conversion_environment",
            Self::SetupConversionEnvironment => "setup_conversion_environment",
            Self::StartConversionSetup { .. } => "start_conversion_setup",
            Self::GetConversionSetup => "get_conversion_setup",
            Self::StartBackendSetup { .. } => "start_backend_setup",
            Self::GetBackendSetup { .. } => "get_backend_setup",
            Self::GetSupportedQuantTypes => "get_supported_quant_types",
            Self::GetBackendStatus => "get_backend_status",
            Self::SetupQuantizationBackend { .. } => "setup_quantization_backend",
            Self::OpenPath { .. } => "open_path",
            Self::OpenUrl { .. } => "open_url",
            Self::DownloadModelFromHf { .. } => "download_model_from_hf",
            Self::StartModelDownloadFromHf { .. } => "start_model_download_from_hf",
            Self::GetModelDownloadStatus { .. } => "get_model_download_status",
            Self::CancelModelDownload { .. } => "cancel_model_download",
            Self::PauseModelDownload { .. } => "pause_model_download",
            Self::ResumeModelDownload { .. } => "resume_model_download",
            Self::ListModelDownloads => "list_model_downloads",
            Self::ResumePartialDownload { .. } => "resume_partial_download",
            Self::GetModels => "get_models",
            Self::SearchCatalog { .. } => "search_models_fts",
            Self::RefreshModelIndex => "refresh_model_index",
            Self::Legacy { method, .. } => method,
        }
    }
}

/// Credential value that cannot be formatted through `Debug` or `Display`.
pub(crate) struct SecretToken(String);

impl SecretToken {
    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

/// Closed outcomes for commands migrated into the producer contract.
///
/// Only the temporary `Legacy` branch may carry arbitrary JSON. It is removed
/// one domain group at a time as typed commands move into this module.
pub(crate) enum RpcOutcome {
    Health(HealthOutcome),
    Shutdown(ShutdownOutcome),
    Status(Box<StatusResponse>),
    DiskSpace(Box<DiskSpaceResponse>),
    SystemResources(Box<SystemResourcesResponse>),
    StatusTelemetry(Box<StatusTelemetrySnapshot>),
    LauncherVersion(Box<LauncherVersionOutcome>),
    LauncherUpdateCheck(Box<UpdateCheckResult>),
    LauncherUpdateApply(Box<UpdateApplyResult>),
    OperationStatus(OperationStatusOutcome),
    Sandbox(SandboxOutcome),
    Git(Box<SystemCheckResult>),
    Network(Box<NetworkStatusResponse>),
    Library(Box<LibraryStatusResponse>),
    #[cfg(feature = "inference-plugins")]
    AppStatus(AppStatusOutcome),
    HfTokenMutation(SuccessOutcome),
    HfAuth(Box<HfAuthOutcome>),
    LinkHealth(Box<LinkHealthOutcome>),
    CleanBrokenLinks(CleanBrokenLinksResponse),
    RemoveOrphanedLinks(RemoveOrphanedLinksOutcome),
    LinksForModel(Box<LinksForModelResponse>),
    DeleteModel(DeleteModelResponse),
    FileLinkCount(FileLinkCountOutcome),
    FilesWritable(Box<FilesWritableOutcome>),
    LinkExclusionMutation(BaseResponse),
    LinkExclusions(Box<LinkExclusionsResponse>),
    ConversionStarted(ConversionStartedOutcome),
    ConversionProgress(Box<ConversionProgressResponse>),
    ConversionCancelled(ConversionCancelledOutcome),
    ConversionList(Box<ConversionListOutcome>),
    ConversionEnvironment(ConversionEnvironmentOutcome),
    ConversionMutation(SuccessOutcome),
    ConversionSetupStarted(ConversionSetupStartedOutcome),
    ConversionSetupStatus(ConversionSetupStatusOutcome),
    SupportedQuantTypes(Box<SupportedQuantTypesOutcome>),
    BackendStatus(Box<BackendStatusOutcome>),
    DownloadStarted(DownloadStartedOutcome),
    DownloadStatus(Box<DownloadStatusOutcome>),
    DownloadMutation(DownloadMutationOutcome),
    DownloadList(Box<DownloadListOutcome>),
    PartialDownload(Box<PartialDownloadOutcome>),
    Models(Box<ModelsOutcome>),
    CatalogSearch(Box<CatalogSearchOutcome>),
    ModelIndexRefresh(ModelIndexRefreshOutcome),
    Legacy(Value),
}

impl RpcOutcome {
    pub(crate) const fn uses_response_wrapper(&self) -> bool {
        matches!(self, Self::Legacy(_))
    }

    pub(crate) fn into_value(self) -> Result<Value, PublicError> {
        let result = match self {
            Self::Health(value) => serde_json::to_value(value),
            Self::Shutdown(value) => serde_json::to_value(value),
            Self::Status(value) => serde_json::to_value(value),
            Self::DiskSpace(value) => serde_json::to_value(value),
            Self::SystemResources(value) => serde_json::to_value(value),
            Self::StatusTelemetry(value) => serde_json::to_value(value),
            Self::LauncherVersion(value) => serde_json::to_value(value),
            Self::LauncherUpdateCheck(value) => serde_json::to_value(value),
            Self::LauncherUpdateApply(value) => serde_json::to_value(value),
            Self::OperationStatus(value) => serde_json::to_value(value),
            Self::Sandbox(value) => serde_json::to_value(value),
            Self::Git(value) => serde_json::to_value(value),
            Self::Network(value) => serde_json::to_value(value),
            Self::Library(value) => serde_json::to_value(value),
            #[cfg(feature = "inference-plugins")]
            Self::AppStatus(value) => serde_json::to_value(value),
            Self::HfTokenMutation(value) => serde_json::to_value(value),
            Self::HfAuth(value) => serde_json::to_value(value),
            Self::LinkHealth(value) => serde_json::to_value(value),
            Self::CleanBrokenLinks(value) => serde_json::to_value(value),
            Self::RemoveOrphanedLinks(value) => serde_json::to_value(value),
            Self::LinksForModel(value) => serde_json::to_value(value),
            Self::DeleteModel(value) => serde_json::to_value(value),
            Self::FileLinkCount(value) => serde_json::to_value(value),
            Self::FilesWritable(value) => serde_json::to_value(value),
            Self::LinkExclusionMutation(value) => serde_json::to_value(value),
            Self::LinkExclusions(value) => serde_json::to_value(value),
            Self::ConversionStarted(value) => serde_json::to_value(value),
            Self::ConversionProgress(value) => serde_json::to_value(value),
            Self::ConversionCancelled(value) => serde_json::to_value(value),
            Self::ConversionList(value) => serde_json::to_value(value),
            Self::ConversionEnvironment(value) => serde_json::to_value(value),
            Self::ConversionMutation(value) => serde_json::to_value(value),
            Self::ConversionSetupStarted(value) => serde_json::to_value(value),
            Self::ConversionSetupStatus(value) => serde_json::to_value(value),
            Self::SupportedQuantTypes(value) => serde_json::to_value(value),
            Self::BackendStatus(value) => serde_json::to_value(value),
            Self::DownloadStarted(value) => serde_json::to_value(value),
            Self::DownloadStatus(value) => serde_json::to_value(value),
            Self::DownloadMutation(value) => serde_json::to_value(value),
            Self::DownloadList(value) => serde_json::to_value(value),
            Self::PartialDownload(value) => serde_json::to_value(value),
            Self::Models(value) => serde_json::to_value(value),
            Self::CatalogSearch(value) => serde_json::to_value(value),
            Self::ModelIndexRefresh(value) => serde_json::to_value(value),
            Self::Legacy(value) => return Ok(value),
        };
        result.map_err(|_| PublicError::internal())
    }
}

#[derive(Serialize)]
pub(crate) struct HealthOutcome {
    status: &'static str,
}

impl HealthOutcome {
    pub(crate) const fn ok() -> Self {
        Self { status: "ok" }
    }
}

#[derive(Serialize)]
pub(crate) struct ShutdownOutcome {
    status: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    managed_profiles_processed: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    managed_processes_stopped: Option<usize>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    errors: Vec<&'static str>,
}

impl ShutdownOutcome {
    #[cfg(not(feature = "inference-plugins"))]
    pub(crate) const fn core_only() -> Self {
        Self {
            status: "shutting_down",
            managed_profiles_processed: None,
            managed_processes_stopped: None,
            errors: Vec::new(),
        }
    }

    #[cfg(feature = "inference-plugins")]
    pub(crate) fn managed(
        profiles_processed: usize,
        processes_stopped: usize,
        error_count: usize,
    ) -> Self {
        Self {
            status: "shutting_down",
            managed_profiles_processed: Some(profiles_processed),
            managed_processes_stopped: Some(processes_stopped),
            errors: vec!["A managed runtime could not be stopped."; error_count],
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct LauncherVersionOutcome {
    success: bool,
    version: String,
    current_commit: String,
    branch: String,
    is_git_repo: bool,
}

impl LauncherVersionOutcome {
    pub(crate) fn decode(value: Value) -> pumas_library::Result<Self> {
        Ok(serde_json::from_value(value)?)
    }
}

#[derive(Serialize)]
pub(crate) struct OperationStatusOutcome {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<&'static str>,
}

impl OperationStatusOutcome {
    pub(crate) const fn success() -> Self {
        Self {
            success: true,
            error: None,
        }
    }

    pub(crate) const fn failed() -> Self {
        Self {
            success: false,
            error: Some("The requested operation failed."),
        }
    }
}

#[derive(Serialize)]
pub(crate) struct SandboxOutcome {
    success: bool,
    is_sandboxed: bool,
    sandbox_type: &'static str,
    limitations: Vec<&'static str>,
}

impl SandboxOutcome {
    pub(crate) fn new(
        is_sandboxed: bool,
        sandbox_type: &'static str,
        limitations: Vec<&'static str>,
    ) -> Self {
        Self {
            success: true,
            is_sandboxed,
            sandbox_type,
            limitations,
        }
    }
}

#[cfg(feature = "inference-plugins")]
#[derive(Serialize)]
pub(crate) struct AppStatusOutcome {
    success: bool,
    running: bool,
}

#[cfg(feature = "inference-plugins")]
impl AppStatusOutcome {
    pub(crate) const fn new(running: bool) -> Self {
        Self {
            success: true,
            running,
        }
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct SuccessOutcome {
    success: bool,
}

impl SuccessOutcome {
    pub(crate) const fn new() -> Self {
        Self { success: true }
    }
}

#[derive(Serialize)]
pub(crate) struct HfAuthOutcome {
    success: bool,
    authenticated: bool,
    username: Option<String>,
    token_source: Option<String>,
}

impl From<HfAuthStatus> for HfAuthOutcome {
    fn from(status: HfAuthStatus) -> Self {
        Self {
            success: true,
            authenticated: status.authenticated,
            username: status.username,
            token_source: status.token_source,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct RemoveOrphanedLinksOutcome {
    success: bool,
    removed: usize,
}

/// Validated desktop projection retaining the standalone core report shape.
#[derive(Serialize)]
#[serde(transparent)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct LinkHealthOutcome(LinkHealthResponse);

impl TryFrom<LinkHealthResponse> for LinkHealthOutcome {
    type Error = PumasError;

    fn try_from(report: LinkHealthResponse) -> Result<Self, Self::Error> {
        let expected_status = if report.broken_links.is_empty() {
            "healthy"
        } else {
            "degraded"
        };
        if !report.success
            || report.error.is_some()
            || report.status != expected_status
            || report.total_links as u128 > MAX_JS_SAFE_INTEGER as u128
            || report.healthy_links as u128 > MAX_JS_SAFE_INTEGER as u128
            || report.healthy_links.checked_add(report.broken_links.len())
                != Some(report.total_links)
        {
            return Err(invalid_domain_outcome("link health report"));
        }
        Ok(Self(report))
    }
}

impl RemoveOrphanedLinksOutcome {
    pub(crate) const fn new(removed: usize) -> Self {
        Self {
            success: true,
            removed,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct FileLinkCountOutcome {
    success: bool,
    count: u64,
}

impl FileLinkCountOutcome {
    pub(crate) const fn new(count: u64) -> Self {
        Self {
            success: true,
            count,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct FilesWritableOutcome {
    success: bool,
    results: Vec<FileWritableOutcome>,
}

impl FilesWritableOutcome {
    pub(crate) fn new(results: Vec<FileWritableOutcome>) -> Self {
        Self {
            success: true,
            results,
        }
    }
}

#[derive(Serialize)]
pub(crate) struct FileWritableOutcome {
    path: String,
    writable: bool,
}

impl FileWritableOutcome {
    pub(crate) fn new(path: String, writable: bool) -> Self {
        Self { path, writable }
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct ConversionStartedOutcome {
    success: bool,
    conversion_id: String,
}

impl ConversionStartedOutcome {
    pub(crate) fn new(conversion_id: String) -> Result<Self, PumasError> {
        if conversion_id.trim().is_empty() || conversion_id.len() > MAX_IDENTIFIER_BYTES {
            return Err(invalid_domain_outcome("started conversion ID"));
        }
        Ok(Self {
            success: true,
            conversion_id,
        })
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct ConversionProgressResponse {
    success: bool,
    progress: Option<ConversionProgressOutcome>,
}

const SETUP_FAILURE_MESSAGE: &str = "Conversion environment setup did not complete successfully.";

// Lexical projection of the core's canonical hyphenated UUID identity. No UUID
// version or provenance is inferred here; operation ownership stays in core.
fn canonical_setup_id(value: &str) -> bool {
    value.len() == 36
        && value.bytes().enumerate().all(|(index, byte)| {
            if matches!(index, 8 | 13 | 18 | 23) {
                byte == b'-'
            } else {
                byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
            }
        })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
struct ConversionSetupSnapshotOutcome {
    operation_id: String,
    status: ConversionSetupStatus,
    error: Option<&'static str>,
}

impl TryFrom<ConversionSetupSnapshot> for ConversionSetupSnapshotOutcome {
    type Error = PumasError;

    fn try_from(snapshot: ConversionSetupSnapshot) -> Result<Self, Self::Error> {
        if !canonical_setup_id(&snapshot.operation_id)
            || (snapshot.status == ConversionSetupStatus::Failed) != snapshot.error.is_some()
        {
            return Err(invalid_domain_outcome("conversion setup snapshot"));
        }
        Ok(Self {
            operation_id: snapshot.operation_id,
            status: snapshot.status,
            error: snapshot.error.map(|_| SETUP_FAILURE_MESSAGE),
        })
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct ConversionSetupStartedOutcome {
    success: bool,
    setup: ConversionSetupSnapshotOutcome,
}

impl ConversionSetupStartedOutcome {
    pub(crate) fn new(setup: ConversionSetupSnapshot) -> Result<Self, PumasError> {
        Ok(Self {
            success: true,
            setup: setup.try_into()?,
        })
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct ConversionSetupStatusOutcome {
    success: bool,
    setup: Option<ConversionSetupSnapshotOutcome>,
}

impl ConversionSetupStatusOutcome {
    pub(crate) fn new(setup: Option<ConversionSetupSnapshot>) -> Result<Self, PumasError> {
        Ok(Self {
            success: true,
            setup: setup
                .map(ConversionSetupSnapshotOutcome::try_from)
                .transpose()?,
        })
    }
}

impl ConversionProgressResponse {
    pub(crate) fn new(progress: Option<ConversionProgress>) -> Result<Self, PumasError> {
        Ok(Self {
            success: true,
            progress: progress
                .map(ConversionProgressOutcome::try_from)
                .transpose()?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
struct ConversionProgressOutcome {
    conversion_id: String,
    source_model_id: String,
    direction: ConversionDirection,
    status: ConversionStatus,
    progress: Option<f32>,
    current_tensor: Option<String>,
    tensors_completed: Option<u32>,
    tensors_total: Option<u32>,
    bytes_written: Option<u64>,
    estimated_output_size: Option<u64>,
    target_quant: Option<String>,
    error: Option<&'static str>,
    output_model_id: Option<String>,
    pipeline_step: Option<u32>,
    pipeline_steps_total: Option<u32>,
    pipeline_step_label: Option<String>,
}

impl TryFrom<ConversionProgress> for ConversionProgressOutcome {
    type Error = PumasError;

    fn try_from(progress: ConversionProgress) -> Result<Self, Self::Error> {
        if progress
            .progress
            .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || [progress.bytes_written, progress.estimated_output_size]
                .into_iter()
                .flatten()
                .any(|value| value > MAX_JS_SAFE_INTEGER)
        {
            return Err(invalid_domain_outcome(
                "conversion progress numeric evidence",
            ));
        }
        Ok(Self {
            conversion_id: progress.conversion_id,
            source_model_id: progress.source_model_id,
            direction: progress.direction,
            status: progress.status,
            progress: progress.progress,
            current_tensor: progress.current_tensor,
            tensors_completed: progress.tensors_completed,
            tensors_total: progress.tensors_total,
            bytes_written: progress.bytes_written,
            estimated_output_size: progress.estimated_output_size,
            target_quant: progress.target_quant,
            error: progress
                .error
                .map(|_| "The model conversion did not complete successfully."),
            output_model_id: progress.output_model_id,
            pipeline_step: progress.pipeline_step,
            pipeline_steps_total: progress.pipeline_steps_total,
            pipeline_step_label: progress.pipeline_step_label,
        })
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct ConversionCancelledOutcome {
    success: bool,
    cancelled: bool,
}

impl ConversionCancelledOutcome {
    pub(crate) const fn new(cancelled: bool) -> Self {
        Self {
            success: true,
            cancelled,
        }
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct ConversionListOutcome {
    success: bool,
    conversions: Vec<ConversionProgressOutcome>,
}

impl ConversionListOutcome {
    pub(crate) fn new(conversions: Vec<ConversionProgress>) -> Result<Self, PumasError> {
        Ok(Self {
            success: true,
            conversions: conversions
                .into_iter()
                .map(ConversionProgressOutcome::try_from)
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct ConversionEnvironmentOutcome {
    success: bool,
    ready: bool,
}

impl ConversionEnvironmentOutcome {
    pub(crate) const fn new(ready: bool) -> Self {
        Self {
            success: true,
            ready,
        }
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct SupportedQuantTypesOutcome {
    success: bool,
    quant_types: Vec<QuantOption>,
}

impl SupportedQuantTypesOutcome {
    pub(crate) fn new(quant_types: Vec<QuantOption>) -> Result<Self, PumasError> {
        if quant_types
            .iter()
            .any(|option| !option.bits_per_weight.is_finite() || option.bits_per_weight < 0.0)
        {
            return Err(invalid_domain_outcome("quantization bits per weight"));
        }
        Ok(Self {
            success: true,
            quant_types,
        })
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct BackendStatusOutcome {
    success: bool,
    backends: Vec<BackendStatus>,
}

impl BackendStatusOutcome {
    pub(crate) const fn new(backends: Vec<BackendStatus>) -> Self {
        Self {
            success: true,
            backends,
        }
    }
}

#[derive(Serialize)]
#[serde(untagged)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum DownloadStartedOutcome {
    Started(DownloadStartedSuccess),
    Failed(DownloadStartedFailure),
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct DownloadStartedSuccess {
    success: bool,
    download_id: String,
    #[serde(rename = "selectedArtifactId")]
    selected_artifact_id: Option<String>,
    #[serde(rename = "artifactId")]
    artifact_id: Option<String>,
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct DownloadStartedFailure {
    success: bool,
    error: &'static str,
}

impl DownloadStartedOutcome {
    pub(crate) fn started(download_id: String, selected_artifact_id: Option<String>) -> Self {
        Self::Started(DownloadStartedSuccess {
            success: true,
            download_id,
            selected_artifact_id: selected_artifact_id.clone(),
            artifact_id: selected_artifact_id,
        })
    }

    pub(crate) fn failed(error: &PumasError) -> Self {
        Self::Failed(DownloadStartedFailure {
            success: false,
            error: PublicError::from(error).message,
        })
    }
}

#[derive(Serialize)]
#[serde(untagged)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) enum DownloadStatusOutcome {
    Found(Box<DownloadStatusFoundOutcome>),
    Missing(DownloadStatusMissingOutcome),
}

impl DownloadStatusOutcome {
    pub(crate) fn new(progress: Option<ModelDownloadProgress>) -> Result<Self, PumasError> {
        Ok(match progress {
            Some(progress) => Self::Found(Box::new(DownloadStatusFoundOutcome {
                success: true,
                progress: DownloadProgressOutcome::try_from(progress)?,
            })),
            None => Self::Missing(DownloadStatusMissingOutcome {
                success: false,
                error: "Download not found",
            }),
        })
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct DownloadStatusFoundOutcome {
    success: bool,
    #[serde(flatten)]
    progress: DownloadProgressOutcome,
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct DownloadStatusMissingOutcome {
    success: bool,
    error: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
struct DownloadProgressOutcome {
    download_id: String,
    library_model_id: Option<String>,
    repo_id: Option<String>,
    selected_artifact_id: Option<String>,
    model_name: Option<String>,
    model_type: Option<String>,
    status: DownloadStatus,
    progress: Option<f32>,
    downloaded_bytes: Option<u64>,
    total_bytes: Option<u64>,
    speed: Option<f64>,
    eta_seconds: Option<f64>,
    retry_attempt: Option<u32>,
    retry_limit: Option<u32>,
    retrying: Option<bool>,
    next_retry_delay_seconds: Option<f64>,
    error: Option<&'static str>,
}

impl TryFrom<ModelDownloadProgress> for DownloadProgressOutcome {
    type Error = PumasError;

    fn try_from(progress: ModelDownloadProgress) -> Result<Self, Self::Error> {
        if progress
            .library_model_id
            .as_deref()
            .is_some_and(|model_id| DownloadRecoveryModelId::parse(model_id).is_none())
        {
            return Err(invalid_domain_outcome("download library model ID"));
        }
        if [progress.downloaded_bytes, progress.total_bytes]
            .into_iter()
            .flatten()
            .any(|value| value > MAX_JS_SAFE_INTEGER)
            || progress
                .progress
                .is_some_and(|value| !value.is_finite() || !(0.0..=1.0).contains(&value))
            || [
                progress.speed,
                progress.eta_seconds,
                progress.next_retry_delay_seconds,
            ]
            .into_iter()
            .flatten()
            .any(|value| !value.is_finite() || value < 0.0)
        {
            return Err(invalid_domain_outcome("download progress numeric evidence"));
        }
        Ok(Self {
            download_id: progress.download_id,
            library_model_id: progress.library_model_id,
            repo_id: progress.repo_id,
            selected_artifact_id: progress.selected_artifact_id,
            model_name: progress.model_name,
            model_type: progress.model_type,
            status: progress.status,
            progress: progress.progress,
            downloaded_bytes: progress.downloaded_bytes,
            total_bytes: progress.total_bytes,
            speed: progress.speed,
            eta_seconds: progress.eta_seconds,
            retry_attempt: progress.retry_attempt,
            retry_limit: progress.retry_limit,
            retrying: progress.retrying,
            next_retry_delay_seconds: progress.next_retry_delay_seconds,
            error: progress
                .error
                .map(|_| "The model download did not complete successfully."),
        })
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct DownloadMutationOutcome {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-contract", schemars(with = "String"))]
    error: Option<&'static str>,
}

impl DownloadMutationOutcome {
    pub(crate) const fn completed(success: bool) -> Self {
        Self {
            success,
            error: if success {
                None
            } else {
                Some("Download not found")
            },
        }
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct DownloadListOutcome {
    success: bool,
    downloads: Vec<DownloadProgressOutcome>,
}

/// Push delivery uses the same validated and redacted progress as polling.
pub(crate) fn project_download_notification(
    notification: &pumas_library::models::ModelDownloadUpdateNotification,
) -> Result<Value, PumasError> {
    let downloads = DownloadListOutcome::new(notification.snapshot.downloads.clone())?;
    Ok(serde_json::json!({
        "cursor": notification.cursor,
        "snapshot": {
            "cursor": notification.snapshot.cursor,
            "revision": notification.snapshot.revision,
            "downloads": downloads.downloads,
        },
        "stale_cursor": notification.stale_cursor,
        "snapshot_required": notification.snapshot_required,
    }))
}

impl DownloadListOutcome {
    pub(crate) fn new(downloads: Vec<ModelDownloadProgress>) -> Result<Self, PumasError> {
        Ok(Self {
            success: true,
            downloads: downloads
                .into_iter()
                .map(DownloadProgressOutcome::try_from)
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct ModelsOutcome {
    success: bool,
    models: BTreeMap<String, CatalogModel>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
struct CatalogModel {
    id: String,
    model_dir: String,
    display_name: String,
    model_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-contract", schemars(with = "String"))]
    format: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-contract", schemars(with = "String"))]
    quantization: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-contract", schemars(with = "u64"))]
    size_bytes: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-contract", schemars(with = "String"))]
    display_date: Option<String>,
    dependency_count: u32,
    related_available: bool,
    artifact: CatalogArtifactState,
    integrity: CatalogIntegrityState,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
enum CatalogArtifactState {
    Complete,
    Partial {
        #[serde(
            rename = "downloadProgressFraction",
            skip_serializing_if = "Option::is_none"
        )]
        #[cfg_attr(feature = "export-contract", schemars(with = "f64"))]
        download_progress_fraction: Option<f64>,
        reasons: Vec<CatalogPartialReason>,
        #[serde(skip_serializing_if = "Option::is_none")]
        #[cfg_attr(
            feature = "export-contract",
            schemars(with = "CatalogRecoveryIdentity")
        )]
        recovery: Option<CatalogRecoveryIdentity>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
struct CatalogRecoveryIdentity {
    recovery_token: String,
    repo_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-contract", schemars(with = "String"))]
    selected_artifact_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    selected_artifact_files: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[cfg_attr(feature = "export-contract", schemars(with = "String"))]
    selected_artifact_quant: Option<String>,
}

impl From<DownloadRecoveryTicket> for CatalogRecoveryIdentity {
    fn from(ticket: DownloadRecoveryTicket) -> Self {
        Self {
            recovery_token: ticket.token().to_string(),
            repo_id: ticket.repo_id().to_string(),
            selected_artifact_id: ticket.selected_artifact_id().map(str::to_string),
            selected_artifact_files: ticket.selected_artifact_files().to_vec(),
            selected_artifact_quant: ticket.selected_artifact_quant().map(str::to_string),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
enum CatalogPartialReason {
    PartFilePresent,
    ExpectedFilesMissing,
}

#[derive(Serialize)]
#[serde(tag = "state", rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
enum CatalogIntegrityState {
    Clean,
    Duplicate {
        count: u32,
        #[serde(rename = "otherModelIds")]
        other_model_ids: Vec<String>,
    },
}

impl CatalogModel {
    fn from_record(
        record: ModelRecord,
        recovery: Option<DownloadRecoveryTicket>,
    ) -> Result<Self, PumasError> {
        let model_id = required_record_text(&record.id, "id")?;
        if model_id != record.id {
            return Err(invalid_catalog_record(&model_id, "id is not canonical"));
        }
        let model_dir = required_record_text(&record.path, "path")?;
        let model_type = required_record_text(&record.model_type, "model_type")?;
        let display_name = if !record.official_name.trim().is_empty() {
            bounded_nonblank_text(&record.official_name).ok_or_else(|| {
                invalid_catalog_record(&model_id, "official_name is blank or oversized")
            })?
        } else if !record.cleaned_name.trim().is_empty() {
            bounded_nonblank_text(&record.cleaned_name).ok_or_else(|| {
                invalid_catalog_record(&model_id, "cleaned_name is blank or oversized")
            })?
        } else {
            model_id
                .rsplit('/')
                .find_map(bounded_nonblank_text)
                .ok_or_else(|| invalid_catalog_record(&model_id, "display name is missing"))?
        };
        let metadata = record
            .metadata
            .as_object()
            .ok_or_else(|| invalid_catalog_record(&model_id, "metadata is not an object"))?;

        let format = optional_metadata_text(metadata, "primary_format", &model_id)?
            .map(|value| value.to_ascii_lowercase());
        let quantization = optional_metadata_text(metadata, "quantization", &model_id)?
            .map(|value| value.to_ascii_uppercase());
        let size_bytes = optional_js_safe_u64(metadata, "size_bytes", &model_id)?;
        let display_date = optional_metadata_text(metadata, "added_date", &model_id)?;
        let dependency_count = dependency_count(metadata, &model_id)?;
        let related_available = match metadata.get("related_available") {
            // Core ModelMetadata owns this as Option<bool>, including JSON null.
            None | Some(Value::Null) => false,
            Some(value) => value.as_bool().ok_or_else(|| {
                invalid_catalog_record(&model_id, "related availability is not boolean")
            })?,
        };
        let artifact = catalog_artifact_state(
            metadata,
            &model_id,
            recovery.map(CatalogRecoveryIdentity::from),
        )?;
        let integrity = catalog_integrity_state(metadata, &model_id)?;

        Ok(Self {
            id: model_id,
            model_dir,
            display_name,
            model_type,
            format,
            quantization,
            size_bytes,
            display_date,
            dependency_count,
            related_available,
            artifact,
            integrity,
        })
    }
}

fn required_record_text(value: &str, field: &str) -> Result<String, PumasError> {
    bounded_nonblank_text(value)
        .ok_or_else(|| invalid_catalog_record("unknown", &format!("{field} is blank or oversized")))
}

fn bounded_nonblank_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty() && trimmed.len() <= MAX_IDENTIFIER_BYTES).then(|| trimmed.to_string())
}

fn optional_metadata_text(
    metadata: &Map<String, Value>,
    field: &str,
    model_id: &str,
) -> Result<Option<String>, PumasError> {
    match metadata.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => bounded_nonblank_text(value).map(Some).ok_or_else(|| {
            invalid_catalog_record(model_id, &format!("{field} is blank or oversized"))
        }),
        Some(_) => Err(invalid_catalog_record(
            model_id,
            &format!("{field} has the wrong type"),
        )),
    }
}

fn optional_js_safe_u64(
    metadata: &Map<String, Value>,
    field: &str,
    model_id: &str,
) -> Result<Option<u64>, PumasError> {
    match metadata.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_u64()
            .filter(|value| *value <= MAX_JS_SAFE_INTEGER)
            .map(Some)
            .ok_or_else(|| {
                invalid_catalog_record(model_id, &format!("{field} is not a JS-safe integer"))
            }),
        Some(_) => Err(invalid_catalog_record(
            model_id,
            &format!("{field} has the wrong type"),
        )),
    }
}

fn required_metadata_bool(
    metadata: &Map<String, Value>,
    field: &str,
    model_id: &str,
) -> Result<bool, PumasError> {
    metadata
        .get(field)
        .and_then(Value::as_bool)
        .ok_or_else(|| invalid_catalog_record(model_id, &format!("{field} is not a boolean")))
}

fn required_metadata_u32(
    metadata: &Map<String, Value>,
    field: &str,
    model_id: &str,
) -> Result<u32, PumasError> {
    metadata
        .get(field)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
        .ok_or_else(|| invalid_catalog_record(model_id, &format!("{field} is not a valid u32")))
}

fn optional_progress_fraction(
    metadata: &Map<String, Value>,
    model_id: &str,
) -> Result<Option<f64>, PumasError> {
    match metadata.get("download_progress") {
        Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_f64()
            .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
            .map(Some)
            .ok_or_else(|| {
                invalid_catalog_record(model_id, "download_progress is outside 0.0..=1.0")
            }),
        Some(_) => Err(invalid_catalog_record(
            model_id,
            "download_progress has the wrong type",
        )),
        None => Err(invalid_catalog_record(
            model_id,
            "download_progress is missing",
        )),
    }
}

fn dependency_count(metadata: &Map<String, Value>, model_id: &str) -> Result<u32, PumasError> {
    let bindings = metadata
        .get("dependency_bindings")
        .and_then(Value::as_array)
        .ok_or_else(|| {
            invalid_catalog_record(model_id, "dependency_bindings is missing or not an array")
        })?;
    if bindings.len() > MAX_COLLECTION_ITEMS {
        return Err(invalid_catalog_record(
            model_id,
            "dependency_bindings is too large",
        ));
    }
    if bindings.iter().any(|binding| !binding.is_object()) {
        return Err(invalid_catalog_record(
            model_id,
            "dependency_bindings contains a non-object",
        ));
    }
    u32::try_from(bindings.len())
        .map_err(|_| invalid_catalog_record(model_id, "dependency_bindings is too large"))
}

fn catalog_artifact_state(
    metadata: &Map<String, Value>,
    model_id: &str,
    recovery: Option<CatalogRecoveryIdentity>,
) -> Result<CatalogArtifactState, PumasError> {
    let incomplete = required_metadata_bool(metadata, "download_incomplete", model_id)?;
    let has_part_files = required_metadata_bool(metadata, "download_has_part_files", model_id)?;
    let missing_expected_files =
        required_metadata_u32(metadata, "download_missing_expected_files", model_id)?;
    let progress = optional_progress_fraction(metadata, model_id)?;

    if incomplete {
        if progress.is_some_and(|value| value >= 1.0) {
            return Err(invalid_catalog_record(
                model_id,
                "partial artifact progress must be below 1.0",
            ));
        }
        let mut reasons = Vec::new();
        if has_part_files {
            reasons.push(CatalogPartialReason::PartFilePresent);
        }
        if missing_expected_files > 0 {
            reasons.push(CatalogPartialReason::ExpectedFilesMissing);
        }
        if reasons.is_empty() {
            return Err(invalid_catalog_record(
                model_id,
                "partial artifact has no partial reason",
            ));
        }
        Ok(CatalogArtifactState::Partial {
            download_progress_fraction: progress,
            reasons,
            recovery,
        })
    } else {
        Ok(CatalogArtifactState::Complete)
    }
}

fn catalog_integrity_state(
    metadata: &Map<String, Value>,
    model_id: &str,
) -> Result<CatalogIntegrityState, PumasError> {
    let duplicate = match metadata.get("integrity_issue_duplicate_repo_id") {
        None | Some(Value::Null) => false,
        Some(Value::Bool(value)) => *value,
        Some(_) => {
            return Err(invalid_catalog_record(
                model_id,
                "duplicate integrity flag has the wrong type",
            ));
        }
    };
    let count_value = metadata.get("integrity_issue_duplicate_repo_id_count");
    let others_value = metadata.get("integrity_issue_duplicate_repo_id_others");
    if !duplicate {
        if count_value.is_some_and(|value| !value.is_null())
            || others_value.is_some_and(|value| !value.is_null())
        {
            return Err(invalid_catalog_record(
                model_id,
                "clean integrity state contains duplicate details",
            ));
        }
        return Ok(CatalogIntegrityState::Clean);
    }

    let count = required_metadata_u32(
        metadata,
        "integrity_issue_duplicate_repo_id_count",
        model_id,
    )?;
    if count < 2 {
        return Err(invalid_catalog_record(
            model_id,
            "duplicate integrity count is below two",
        ));
    }
    let values = others_value
        .and_then(Value::as_array)
        .ok_or_else(|| invalid_catalog_record(model_id, "duplicate integrity peers are missing"))?;
    if values.len() > MAX_COLLECTION_ITEMS {
        return Err(invalid_catalog_record(
            model_id,
            "duplicate integrity peers are too large",
        ));
    }
    let mut other_model_ids = Vec::with_capacity(values.len());
    for value in values {
        let other_id = value
            .as_str()
            .and_then(bounded_nonblank_text)
            .ok_or_else(|| {
                invalid_catalog_record(model_id, "duplicate integrity peers are malformed")
            })?;
        if other_id == model_id {
            return Err(invalid_catalog_record(
                model_id,
                "duplicate integrity peers contain the model itself",
            ));
        }
        other_model_ids.push(other_id);
    }
    other_model_ids.sort();
    if other_model_ids.windows(2).any(|pair| pair[0] == pair[1])
        || other_model_ids.len().saturating_add(1) != count as usize
    {
        return Err(invalid_catalog_record(
            model_id,
            "duplicate integrity details disagree",
        ));
    }
    Ok(CatalogIntegrityState::Duplicate {
        count,
        other_model_ids,
    })
}

fn invalid_catalog_record(model_id: &str, reason: &str) -> PumasError {
    PumasError::Other(format!("Invalid model catalog record {model_id}: {reason}"))
}

impl ModelsOutcome {
    pub(crate) fn from_records(
        records: Vec<ModelRecord>,
        library_root: &Path,
    ) -> Result<Self, PumasError> {
        let mut models = BTreeMap::new();
        for record in records {
            let model_id = record.id.clone();
            let recovery = issue_download_recovery_ticket(library_root, &record)?;
            let catalog_model = CatalogModel::from_record(record, recovery)?;
            if catalog_model.id != model_id {
                return Err(invalid_catalog_record(
                    &model_id,
                    "map key does not match record id",
                ));
            }
            if models.insert(model_id.clone(), catalog_model).is_some() {
                return Err(PumasError::Other(format!(
                    "Model catalog contains duplicate ID: {model_id}"
                )));
            }
        }
        Ok(Self {
            success: true,
            models,
        })
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct ModelIndexRefreshOutcome {
    success: bool,
    indexed_count: u32,
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct CatalogSearchOutcome {
    success: bool,
    models: Vec<CatalogModel>,
    total_count: u64,
    query_time_ms: f64,
    query: String,
}

impl CatalogSearchOutcome {
    pub(crate) fn from_search(
        search: pumas_library::index::SearchResult,
        root: &Path,
    ) -> Result<Self, PumasError> {
        let total_count = u64::try_from(search.total_count)
            .map_err(|_| invalid_domain_outcome("catalog search count"))?;
        if total_count > MAX_JS_SAFE_INTEGER
            || search.total_count < search.models.len()
            || !search.query_time_ms.is_finite()
            || search.query_time_ms < 0.0
        {
            return Err(invalid_domain_outcome("catalog search result"));
        }
        let order = search
            .models
            .iter()
            .map(|record| record.id.clone())
            .collect::<Vec<_>>();
        let mut projected = ModelsOutcome::from_records(search.models, root)?.models;
        let models = order
            .into_iter()
            .map(|id| {
                projected
                    .remove(&id)
                    .ok_or_else(|| invalid_domain_outcome("catalog search identity"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            success: true,
            models,
            total_count,
            query_time_ms: search.query_time_ms,
            query: search.query,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct SearchCatalogParams {
    query: String,
    limit: Option<u32>,
    offset: Option<u32>,
}

impl ModelIndexRefreshOutcome {
    pub(crate) const fn new(indexed_count: u32) -> Self {
        Self {
            success: true,
            indexed_count,
        }
    }
}

impl TryFrom<usize> for ModelIndexRefreshOutcome {
    type Error = PumasError;

    fn try_from(indexed_count: usize) -> Result<Self, Self::Error> {
        let indexed_count = u32::try_from(indexed_count).map_err(|_| {
            PumasError::Other("Model index count exceeds the RPC representation".to_string())
        })?;
        Ok(Self::new(indexed_count))
    }
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
pub(crate) struct PartialDownloadOutcome {
    success: bool,
    action: PartialDownloadActionName,
    download_id: Option<String>,
    status: Option<DownloadStatus>,
    reason_code: Option<PartialDownloadReason>,
    error: Option<&'static str>,
}

impl TryFrom<PartialDownloadAction> for PartialDownloadOutcome {
    type Error = PumasError;

    fn try_from(action: PartialDownloadAction) -> Result<Self, Self::Error> {
        let (action_name, success) = match action.action.as_str() {
            "resume" => (PartialDownloadActionName::Resume, true),
            "recover" => (PartialDownloadActionName::Recover, true),
            "attach" => (PartialDownloadActionName::Attach, true),
            "none" => (PartialDownloadActionName::None, false),
            _ => return Err(invalid_domain_outcome("partial download action")),
        };
        let reason_code = action
            .reason_code
            .as_deref()
            .map(PartialDownloadReason::parse)
            .transpose()?;
        let download_id = action
            .download_id
            .map(|download_id| {
                if download_id.trim().is_empty() || download_id.len() > MAX_IDENTIFIER_BYTES {
                    Err(invalid_domain_outcome("partial download ID"))
                } else {
                    Ok(download_id)
                }
            })
            .transpose()?;
        let valid = match (
            &action_name,
            download_id.as_ref(),
            action.status,
            reason_code.as_ref(),
        ) {
            (
                PartialDownloadActionName::Resume | PartialDownloadActionName::Recover,
                Some(_),
                Some(DownloadStatus::Queued),
                None,
            ) => true,
            (
                PartialDownloadActionName::Attach,
                Some(_),
                Some(
                    DownloadStatus::Queued
                    | DownloadStatus::Downloading
                    | DownloadStatus::Pausing
                    | DownloadStatus::Cancelling,
                ),
                None,
            ) => true,
            (
                PartialDownloadActionName::None,
                Some(_),
                Some(DownloadStatus::Completed),
                Some(PartialDownloadReason::AlreadyCompleted),
            ) => true,
            (
                PartialDownloadActionName::None,
                Some(_),
                Some(DownloadStatus::Cancelled),
                Some(PartialDownloadReason::AlreadyCancelled),
            ) => true,
            (
                PartialDownloadActionName::None,
                Some(_),
                Some(DownloadStatus::Paused | DownloadStatus::Error),
                Some(PartialDownloadReason::ResumeRejected),
            ) => true,
            (PartialDownloadActionName::None, None, None, Some(reason)) => {
                reason.is_untracked_failure()
            }
            _ => false,
        };
        if !valid || success != !matches!(action_name, PartialDownloadActionName::None) {
            return Err(invalid_domain_outcome("partial download outcome"));
        }

        Ok(Self {
            success,
            action: action_name,
            download_id,
            status: action.status,
            reason_code,
            error: (!success).then_some("The partial download could not be resumed."),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
enum PartialDownloadActionName {
    Resume,
    Recover,
    Attach,
    None,
}

#[derive(Serialize)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
enum PartialDownloadReason {
    HfClientUnavailable,
    DownloadRootBusy,
    ModelNotFound,
    ModelNotPartial,
    RecoveryUnavailable,
    RecoveryContextStale,
    ResumeRejected,
    AlreadyCompleted,
    AlreadyCancelled,
    InvalidRepoId,
    RepoNotFound,
    RateLimited,
    PermissionDenied,
    NetworkError,
    RecoverFailed,
}

impl PartialDownloadReason {
    const fn is_untracked_failure(&self) -> bool {
        matches!(
            self,
            Self::HfClientUnavailable
                | Self::DownloadRootBusy
                | Self::ModelNotFound
                | Self::ModelNotPartial
                | Self::RecoveryUnavailable
                | Self::RecoveryContextStale
                | Self::InvalidRepoId
                | Self::RepoNotFound
                | Self::RateLimited
                | Self::PermissionDenied
                | Self::NetworkError
                | Self::RecoverFailed
        )
    }

    fn parse(value: &str) -> Result<Self, PumasError> {
        match value {
            "hf_client_unavailable" => Ok(Self::HfClientUnavailable),
            "download_root_busy" => Ok(Self::DownloadRootBusy),
            "model_not_found" => Ok(Self::ModelNotFound),
            "model_not_partial" => Ok(Self::ModelNotPartial),
            "recovery_unavailable" => Ok(Self::RecoveryUnavailable),
            "recovery_context_stale" => Ok(Self::RecoveryContextStale),
            "resume_rejected" => Ok(Self::ResumeRejected),
            "already_completed" => Ok(Self::AlreadyCompleted),
            "already_cancelled" => Ok(Self::AlreadyCancelled),
            "invalid_repo_id" => Ok(Self::InvalidRepoId),
            "repo_not_found" => Ok(Self::RepoNotFound),
            "rate_limited" => Ok(Self::RateLimited),
            "permission_denied" => Ok(Self::PermissionDenied),
            "network_error" => Ok(Self::NetworkError),
            "recover_failed" => Ok(Self::RecoverFailed),
            _ => Err(invalid_domain_outcome("partial download reason")),
        }
    }
}

fn invalid_domain_outcome(name: &str) -> PumasError {
    PumasError::Other(format!("invalid {name} returned by domain"))
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EmptyParams {}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CheckLauncherUpdatesParams {
    #[serde(default, alias = "forceRefresh")]
    force_refresh: bool,
}

#[cfg(feature = "inference-plugins")]
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetAppStatusParams {
    #[serde(alias = "appId")]
    app_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetHfTokenParams {
    token: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GetLinkHealthParams {
    #[serde(default, alias = "versionTag")]
    version_tag: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct VersionTagParams {
    #[serde(alias = "versionTag")]
    version_tag: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ModelIdParams {
    #[serde(alias = "modelId")]
    model_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FilePathParams {
    #[serde(alias = "filePath")]
    file_path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FilePathsParams {
    #[serde(alias = "filePaths")]
    file_paths: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetModelLinkExclusionParams {
    #[serde(alias = "modelId")]
    model_id: String,
    #[serde(alias = "appId")]
    app_id: String,
    #[serde(default = "default_true")]
    excluded: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppIdParams {
    #[serde(alias = "appId")]
    app_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct StartModelConversionParams {
    #[serde(alias = "modelId")]
    model_id: String,
    direction: String,
    #[serde(default, alias = "targetQuant")]
    target_quant: Option<String>,
    #[serde(default, alias = "outputName")]
    output_name: Option<String>,
    #[serde(default, alias = "imatrixCalibrationFile")]
    imatrix_calibration_file: Option<String>,
    #[serde(default, alias = "forceImatrix")]
    force_imatrix: Option<bool>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConversionIdParams {
    #[serde(alias = "conversionId")]
    conversion_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
struct StartConversionSetupParams {
    #[serde(default)]
    expected_previous_operation_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
struct StartBackendSetupParams {
    backend: QuantBackend,
    #[serde(default)]
    expected_previous_operation_id: Option<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
struct GetBackendSetupParams {
    backend: QuantBackend,
}

#[cfg(any(test, feature = "export-contract"))]
fn backend_setup_requests() -> Vec<(&'static str, Value, bool)> {
    let mut requests = Vec::new();
    for backend in ["python_conversion", "llama_cpp", "nvfp4", "sherry"] {
        requests.push((
            "get_backend_setup",
            serde_json::json!({"backend":backend}),
            true,
        ));
        requests.push((
            "start_backend_setup",
            serde_json::json!({"backend":backend}),
            true,
        ));
        requests.push((
            "start_backend_setup",
            serde_json::json!({"backend":backend,"expected_previous_operation_id":null}),
            true,
        ));
        requests.push(("start_backend_setup", serde_json::json!({"backend":backend,"expected_previous_operation_id":"00112233-4455-4677-8899-aabbccddeeff"}), true));
    }
    for method in ["start_backend_setup", "get_backend_setup"] {
        for params in [
            serde_json::json!({}),
            serde_json::json!(null),
            serde_json::json!([]),
            serde_json::json!({"backend":"LlamaCpp"}),
            serde_json::json!({"backend":"unknown"}),
            serde_json::json!({"backend":null}),
            serde_json::json!({"backend":7}),
            serde_json::json!({"backend":"llama_cpp","extra":true}),
        ] {
            requests.push((method, params, false));
        }
    }
    for token in [
        serde_json::json!("bad"),
        serde_json::json!("00112233-4455-4677-8899-AABBCCDDEEFF"),
        serde_json::json!(7),
        serde_json::json!({}),
    ] {
        requests.push((
            "start_backend_setup",
            serde_json::json!({"backend":"llama_cpp","expected_previous_operation_id":token}),
            false,
        ));
    }
    requests.push((
        "get_backend_setup",
        serde_json::json!({"backend":"llama_cpp","expected_previous_operation_id":null}),
        false,
    ));
    requests
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SetupQuantizationBackendParams {
    backend: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenPathParams {
    path: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct OpenUrlParams {
    url: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct DownloadModelFromHfParams {
    #[serde(alias = "repoId")]
    repo_id: String,
    family: String,
    #[serde(alias = "officialName")]
    official_name: String,
    #[serde(default, alias = "modelType")]
    model_type: Option<String>,
    #[serde(default)]
    quant: Option<String>,
    #[serde(default)]
    filename: Option<String>,
    #[serde(default)]
    filenames: Option<Vec<String>>,
    #[serde(default, alias = "pipelineTag")]
    pipeline_tag: Option<String>,
    #[serde(default)]
    subtype: Option<String>,
    #[serde(default, alias = "releaseDate")]
    release_date: Option<String>,
    #[serde(default, alias = "downloadUrl")]
    download_url: Option<String>,
    #[serde(default, alias = "modelCardJson")]
    model_card_json: Option<String>,
    #[serde(default, alias = "licenseStatus")]
    license_status: Option<String>,
}

impl DownloadModelFromHfParams {
    fn into_request(self) -> Result<DownloadRequest, PublicError> {
        Ok(DownloadRequest {
            repo_id: validate_bounded_non_empty(self.repo_id, MAX_IDENTIFIER_BYTES)?,
            family: validate_bounded_non_empty(self.family, MAX_IDENTIFIER_BYTES)?,
            official_name: validate_bounded_non_empty(self.official_name, MAX_IDENTIFIER_BYTES)?,
            model_type: validate_optional_bounded(self.model_type)?,
            quant: validate_optional_bounded(self.quant)?,
            filename: validate_optional_bounded(self.filename)?,
            filenames: self
                .filenames
                .map(validate_bounded_collection)
                .transpose()?,
            pipeline_tag: validate_optional_bounded(self.pipeline_tag.or(self.subtype))?,
            bundle_format: None,
            pipeline_class: None,
            release_date: validate_optional_bounded(self.release_date)?,
            download_url: validate_optional_bounded(self.download_url)?,
            model_card_json: validate_optional_bounded_with_limit(
                self.model_card_json,
                MAX_METADATA_JSON_BYTES,
            )?,
            license_status: validate_optional_bounded(self.license_status)?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
struct DownloadIdParams {
    #[serde(alias = "downloadId")]
    download_id: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "export-contract", derive(schemars::JsonSchema))]
struct RecoverDownloadParams {
    #[serde(rename = "modelId")]
    model_id: String,
    #[serde(rename = "recoveryToken")]
    recovery_token: String,
}

const fn default_true() -> bool {
    true
}

impl AdmittedRpcRequest {
    pub(crate) fn decode(body: &[u8]) -> Result<Self, RpcAdmissionError> {
        let value = serde_json::from_slice::<Value>(body).map_err(|_| RpcAdmissionError {
            id: None,
            error: PublicError::parse_error(),
        })?;
        let object = value.as_object().ok_or_else(invalid_request_without_id)?;
        let id = parse_request_id(object)?;

        if object
            .keys()
            .any(|key| !matches!(key.as_str(), "jsonrpc" | "method" | "params" | "id"))
        {
            return Err(RpcAdmissionError {
                id,
                error: PublicError::invalid_request(),
            });
        }

        if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
            return Err(RpcAdmissionError {
                id,
                error: PublicError::invalid_request(),
            });
        }

        let method = object
            .get("method")
            .and_then(Value::as_str)
            .filter(|method| !method.is_empty() && method.len() <= MAX_METHOD_BYTES)
            .ok_or_else(|| RpcAdmissionError {
                id: id.clone(),
                error: PublicError::invalid_request(),
            })?;
        let params = object.get("params");
        let command = parse_command(method, params).map_err(|error| RpcAdmissionError {
            id: id.clone(),
            error,
        })?;

        Ok(Self { id, command })
    }
}

fn invalid_request_without_id() -> RpcAdmissionError {
    RpcAdmissionError {
        id: None,
        error: PublicError::invalid_request(),
    }
}

fn parse_request_id(object: &Map<String, Value>) -> Result<Option<Value>, RpcAdmissionError> {
    let Some(id) = object.get("id") else {
        return Ok(None);
    };
    let valid = match id {
        Value::Null => true,
        Value::String(value) => value.len() <= MAX_IDENTIFIER_BYTES,
        Value::Number(value) => value.as_i64().is_some() || value.as_u64().is_some(),
        Value::Bool(_) | Value::Array(_) | Value::Object(_) => false,
    };
    if valid {
        Ok((!id.is_null()).then(|| id.clone()))
    } else {
        Err(invalid_request_without_id())
    }
}

fn parse_command(method: &str, params: Option<&Value>) -> Result<RpcCommand, PublicError> {
    let empty = || parse_params::<EmptyParams>(params).map(|_| ());
    let download_request = || {
        parse_params::<DownloadModelFromHfParams>(params)
            .and_then(DownloadModelFromHfParams::into_request)
    };
    match method {
        "health_check" => empty().map(|()| RpcCommand::HealthCheck),
        "shutdown" => empty().map(|()| RpcCommand::Shutdown),
        "get_status" => empty().map(|()| RpcCommand::GetStatus),
        "get_disk_space" => empty().map(|()| RpcCommand::GetDiskSpace),
        "get_system_resources" => empty().map(|()| RpcCommand::GetSystemResources),
        "get_status_telemetry_snapshot" => empty().map(|()| RpcCommand::GetStatusTelemetrySnapshot),
        "get_launcher_version" => empty().map(|()| RpcCommand::GetLauncherVersion),
        "check_launcher_updates" => {
            parse_params::<CheckLauncherUpdatesParams>(params).map(|params| {
                RpcCommand::CheckLauncherUpdates {
                    force_refresh: params.force_refresh,
                }
            })
        }
        "apply_launcher_update" => empty().map(|()| RpcCommand::ApplyLauncherUpdate),
        "restart_launcher" => empty().map(|()| RpcCommand::RestartLauncher),
        "get_sandbox_info" => empty().map(|()| RpcCommand::GetSandboxInfo),
        "check_git" => empty().map(|()| RpcCommand::CheckGit),
        "get_network_status" => empty().map(|()| RpcCommand::GetNetworkStatus),
        "get_library_status" => empty().map(|()| RpcCommand::GetLibraryStatus),
        #[cfg(feature = "inference-plugins")]
        "get_app_status" => parse_params::<GetAppStatusParams>(params).and_then(|params| {
            validate_bounded_non_empty(params.app_id, MAX_IDENTIFIER_BYTES)
                .map(|app_id| RpcCommand::GetAppStatus { app_id })
        }),
        "set_hf_token" => parse_params::<SetHfTokenParams>(params).and_then(|params| {
            validate_bounded_non_empty(params.token, MAX_CREDENTIAL_BYTES)
                .map(SecretToken)
                .map(|token| RpcCommand::SetHfToken { token })
        }),
        "clear_hf_token" => empty().map(|()| RpcCommand::ClearHfToken),
        "get_hf_auth_status" => empty().map(|()| RpcCommand::GetHfAuthStatus),
        "get_link_health" => parse_params::<GetLinkHealthParams>(params).and_then(|params| {
            params
                .version_tag
                .map(|value| validate_bounded_non_empty(value, MAX_IDENTIFIER_BYTES))
                .transpose()
                .map(|version_tag| RpcCommand::GetLinkHealth { version_tag })
        }),
        "clean_broken_links" => empty().map(|()| RpcCommand::CleanBrokenLinks),
        "remove_orphaned_links" => parse_params::<VersionTagParams>(params).and_then(|params| {
            validate_bounded_non_empty(params.version_tag, MAX_IDENTIFIER_BYTES)
                .map(|version_tag| RpcCommand::RemoveOrphanedLinks { version_tag })
        }),
        "get_links_for_model" => parse_params::<ModelIdParams>(params).and_then(|params| {
            validate_bounded_non_empty(params.model_id, MAX_IDENTIFIER_BYTES)
                .map(|model_id| RpcCommand::GetLinksForModel { model_id })
        }),
        "delete_model_with_cascade" => parse_params::<ModelIdParams>(params).and_then(|params| {
            validate_bounded_non_empty(params.model_id, MAX_IDENTIFIER_BYTES)
                .map(|model_id| RpcCommand::DeleteModelWithCascade { model_id })
        }),
        "get_file_link_count" => parse_params::<FilePathParams>(params).and_then(|params| {
            validate_bounded_non_empty(params.file_path, MAX_IDENTIFIER_BYTES)
                .map(|file_path| RpcCommand::GetFileLinkCount { file_path })
        }),
        "check_files_writable" => parse_params::<FilePathsParams>(params).and_then(|params| {
            validate_bounded_collection(params.file_paths)
                .map(|file_paths| RpcCommand::CheckFilesWritable { file_paths })
        }),
        "set_model_link_exclusion" => {
            parse_params::<SetModelLinkExclusionParams>(params).and_then(|params| {
                let model_id = validate_bounded_non_empty(params.model_id, MAX_IDENTIFIER_BYTES)?;
                let app_id = validate_bounded_non_empty(params.app_id, MAX_IDENTIFIER_BYTES)?;
                Ok(RpcCommand::SetModelLinkExclusion {
                    model_id,
                    app_id,
                    excluded: params.excluded,
                })
            })
        }
        "get_link_exclusions" => parse_params::<AppIdParams>(params).and_then(|params| {
            validate_bounded_non_empty(params.app_id, MAX_IDENTIFIER_BYTES)
                .map(|app_id| RpcCommand::GetLinkExclusions { app_id })
        }),
        "start_model_conversion" => {
            parse_params::<StartModelConversionParams>(params).and_then(|params| {
                let model_id = validate_bounded_non_empty(params.model_id, MAX_IDENTIFIER_BYTES)?;
                let direction = parse_conversion_direction(&params.direction)?;
                let target_quant = validate_optional_bounded(params.target_quant)?;
                let output_name = validate_optional_bounded(params.output_name)?;
                let imatrix_calibration_file =
                    validate_optional_bounded(params.imatrix_calibration_file)?;
                Ok(RpcCommand::StartModelConversion {
                    request: ConversionRequest {
                        model_id,
                        direction,
                        target_quant,
                        output_name,
                        imatrix_calibration_file,
                        force_imatrix: params.force_imatrix,
                    },
                })
            })
        }
        "get_conversion_progress" => {
            parse_params::<ConversionIdParams>(params).and_then(|params| {
                validate_bounded_non_empty(params.conversion_id, MAX_IDENTIFIER_BYTES)
                    .map(|conversion_id| RpcCommand::GetConversionProgress { conversion_id })
            })
        }
        "cancel_model_conversion" => {
            parse_params::<ConversionIdParams>(params).and_then(|params| {
                validate_bounded_non_empty(params.conversion_id, MAX_IDENTIFIER_BYTES)
                    .map(|conversion_id| RpcCommand::CancelModelConversion { conversion_id })
            })
        }
        "list_model_conversions" => empty().map(|()| RpcCommand::ListModelConversions),
        "check_conversion_environment" => empty().map(|()| RpcCommand::CheckConversionEnvironment),
        "setup_conversion_environment" => empty().map(|()| RpcCommand::SetupConversionEnvironment),
        "get_conversion_setup" => empty().map(|()| RpcCommand::GetConversionSetup),
        "get_backend_setup" => parse_params::<GetBackendSetupParams>(params).map(|params| {
            RpcCommand::GetBackendSetup {
                backend: params.backend,
            }
        }),
        "start_backend_setup" => {
            parse_params::<StartBackendSetupParams>(params).and_then(|params| {
                if params
                    .expected_previous_operation_id
                    .as_deref()
                    .is_some_and(|id| !canonical_setup_id(id))
                {
                    return Err(PublicError::invalid_params());
                }
                Ok(RpcCommand::StartBackendSetup {
                    backend: params.backend,
                    expected_previous_operation_id: params.expected_previous_operation_id,
                })
            })
        }
        "start_conversion_setup" => {
            parse_params::<StartConversionSetupParams>(params).and_then(|params| {
                if params
                    .expected_previous_operation_id
                    .as_deref()
                    .is_some_and(|id| !canonical_setup_id(id))
                {
                    return Err(PublicError::invalid_params());
                }
                Ok(RpcCommand::StartConversionSetup {
                    expected_previous_operation_id: params.expected_previous_operation_id,
                })
            })
        }
        "get_supported_quant_types" => empty().map(|()| RpcCommand::GetSupportedQuantTypes),
        "get_backend_status" => empty().map(|()| RpcCommand::GetBackendStatus),
        "setup_quantization_backend" => parse_params::<SetupQuantizationBackendParams>(params)
            .and_then(|params| {
                parse_quant_backend(&params.backend)
                    .map(|backend| RpcCommand::SetupQuantizationBackend { backend })
            }),
        "open_path" => parse_params::<OpenPathParams>(params).and_then(|params| {
            validate_bounded_non_empty(params.path, MAX_IDENTIFIER_BYTES)
                .map(|path| RpcCommand::OpenPath { path })
        }),
        "open_url" => parse_params::<OpenUrlParams>(params).and_then(|params| {
            validate_bounded_non_empty(params.url, MAX_IDENTIFIER_BYTES)
                .map(|url| RpcCommand::OpenUrl { url })
        }),
        "download_model_from_hf" => {
            download_request().map(|request| RpcCommand::DownloadModelFromHf { request })
        }
        "start_model_download_from_hf" => {
            download_request().map(|request| RpcCommand::StartModelDownloadFromHf { request })
        }
        "get_model_download_status" => {
            parse_params::<DownloadIdParams>(params).and_then(|params| {
                validate_bounded_non_empty(params.download_id, MAX_IDENTIFIER_BYTES)
                    .map(|download_id| RpcCommand::GetModelDownloadStatus { download_id })
            })
        }
        "cancel_model_download" => parse_params::<DownloadIdParams>(params).and_then(|params| {
            validate_bounded_non_empty(params.download_id, MAX_IDENTIFIER_BYTES)
                .map(|download_id| RpcCommand::CancelModelDownload { download_id })
        }),
        "pause_model_download" => parse_params::<DownloadIdParams>(params).and_then(|params| {
            validate_bounded_non_empty(params.download_id, MAX_IDENTIFIER_BYTES)
                .map(|download_id| RpcCommand::PauseModelDownload { download_id })
        }),
        "resume_model_download" => parse_params::<DownloadIdParams>(params).and_then(|params| {
            validate_bounded_non_empty(params.download_id, MAX_IDENTIFIER_BYTES)
                .map(|download_id| RpcCommand::ResumeModelDownload { download_id })
        }),
        "list_model_downloads" => empty().map(|()| RpcCommand::ListModelDownloads),
        "list_interrupted_downloads" | "recover_download" => Err(PublicError::method_not_found()),
        "resume_partial_download" => {
            parse_params::<RecoverDownloadParams>(params).and_then(|params| {
                let model_id = DownloadRecoveryModelId::parse(&params.model_id)
                    .ok_or_else(PublicError::invalid_params)?;
                let recovery_token = DownloadRecoveryToken::parse(&params.recovery_token)
                    .ok_or_else(PublicError::invalid_params)?;
                Ok(RpcCommand::ResumePartialDownload {
                    model_id,
                    recovery_token,
                })
            })
        }
        "get_models" => empty().map(|()| RpcCommand::GetModels),
        "search_models_fts" => parse_params::<SearchCatalogParams>(params).and_then(|params| {
            // An empty query is the core's supported paginated catalog listing.
            if params.query.len() > MAX_IDENTIFIER_BYTES {
                return Err(PublicError::invalid_params());
            }
            let query = params.query;
            let limit = usize::try_from(params.limit.unwrap_or(100))
                .map_err(|_| PublicError::invalid_params())?;
            let offset = usize::try_from(params.offset.unwrap_or(0))
                .map_err(|_| PublicError::invalid_params())?;
            if limit == 0 || limit > MAX_COLLECTION_ITEMS {
                return Err(PublicError::invalid_params());
            }
            Ok(RpcCommand::SearchCatalog {
                query,
                limit,
                offset,
            })
        }),
        "refresh_model_index" => empty().map(|()| RpcCommand::RefreshModelIndex),
        _ => Ok(RpcCommand::Legacy {
            method: method.to_string(),
            params: params.cloned().unwrap_or_else(|| Value::Object(Map::new())),
        }),
    }
}

fn parse_params<T>(params: Option<&Value>) -> Result<T, PublicError>
where
    T: for<'de> Deserialize<'de>,
{
    let value = params.cloned().unwrap_or_else(|| Value::Object(Map::new()));
    if !value.is_object() {
        return Err(PublicError::invalid_params());
    }
    serde_json::from_value(value).map_err(|_| PublicError::invalid_params())
}

fn validate_bounded_non_empty(value: String, max_bytes: usize) -> Result<String, PublicError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > max_bytes {
        return Err(PublicError::invalid_params());
    }
    Ok(trimmed.to_string())
}

fn validate_bounded_collection(values: Vec<String>) -> Result<Vec<String>, PublicError> {
    if values.is_empty() || values.len() > MAX_COLLECTION_ITEMS {
        return Err(PublicError::invalid_params());
    }
    values
        .into_iter()
        .map(|value| validate_bounded_non_empty(value, MAX_IDENTIFIER_BYTES))
        .collect()
}

fn validate_optional_bounded(value: Option<String>) -> Result<Option<String>, PublicError> {
    validate_optional_bounded_with_limit(value, MAX_IDENTIFIER_BYTES)
}

fn validate_optional_bounded_with_limit(
    value: Option<String>,
    max_bytes: usize,
) -> Result<Option<String>, PublicError> {
    value
        .map(|value| validate_bounded_non_empty(value, max_bytes))
        .transpose()
}

fn parse_conversion_direction(value: &str) -> Result<ConversionDirection, PublicError> {
    match value {
        "gguf_to_safetensors" | "GgufToSafetensors" => Ok(ConversionDirection::GgufToSafetensors),
        "safetensors_to_gguf" | "SafetensorsToGguf" => Ok(ConversionDirection::SafetensorsToGguf),
        "safetensors_to_quantized_gguf" | "SafetensorsToQuantizedGguf" => {
            Ok(ConversionDirection::SafetensorsToQuantizedGguf)
        }
        "gguf_to_quantized_gguf" | "GgufToQuantizedGguf" => {
            Ok(ConversionDirection::GgufToQuantizedGguf)
        }
        "safetensors_to_nvfp4" | "SafetensorsToNvfp4" => {
            Ok(ConversionDirection::SafetensorsToNvfp4)
        }
        "safetensors_to_sherry_qat" | "SafetensorsToSherryQat" => {
            Ok(ConversionDirection::SafetensorsToSherryQat)
        }
        _ => Err(PublicError::invalid_params()),
    }
}

fn parse_quant_backend(value: &str) -> Result<QuantBackend, PublicError> {
    match value {
        "llama_cpp" | "LlamaCpp" => Ok(QuantBackend::LlamaCpp),
        "nvfp4" | "Nvfp4" => Ok(QuantBackend::Nvfp4),
        "sherry" | "Sherry" => Ok(QuantBackend::Sherry),
        "python_conversion" | "PythonConversion" => Ok(QuantBackend::PythonConversion),
        _ => Err(PublicError::invalid_params()),
    }
}

impl From<&PumasError> for PublicError {
    fn from(error: &PumasError) -> Self {
        Self::from_pumas(error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    const SECRET: &str = "hf_test_rpc_secret_do_not_disclose";
    const PRIVATE_PATH: &str = "/private/rpc-sentinel/model.gguf";
    const PRIVATE_URL: &str = "https://example.invalid/private/rpc-sentinel/model.gguf";

    fn assert_projection_is_bounded(error: PumasError, expected_code: i32) {
        let projected = PublicError::from(&error);
        let encoded = serde_json::to_string(&projected).expect("public error must serialize");

        assert_eq!(projected.code, expected_code);
        assert!(!encoded.contains(SECRET));
        assert!(!encoded.contains(PRIVATE_PATH));
        assert!(!encoded.contains(PRIVATE_URL));
        assert!(encoded.len() < 256, "public error must remain bounded");
    }

    #[test]
    fn public_error_projection_redacts_secrets_and_private_locators() {
        assert_projection_is_bounded(
            PumasError::Config {
                message: format!("credential {SECRET} is invalid"),
            },
            -32000,
        );
        assert_projection_is_bounded(
            PumasError::Io {
                message: format!("cannot read {PRIVATE_PATH}"),
                path: Some(PathBuf::from(PRIVATE_PATH)),
                source: None,
            },
            -32603,
        );
        assert_projection_is_bounded(
            PumasError::DownloadFailed {
                url: PRIVATE_URL.to_string(),
                message: format!("authorization {SECRET} failed"),
            },
            -32003,
        );
    }

    #[test]
    fn public_error_projection_preserves_stable_failure_codes_and_classes() {
        let closed = PublicError::from(&PumasError::DownloadLifecycleClosed);
        assert_eq!(closed.code, -32000);
        assert_eq!(closed.class, PublicErrorClass::Unavailable);
        let shutdown = PublicError::from(&PumasError::DownloadShutdownFailed { failures: 17 });
        assert_eq!(shutdown.code, -32603);
        assert_eq!(shutdown.class, PublicErrorClass::Internal);
        assert!(!shutdown.message.contains("17"));
        let unavailable = PublicError::from(&PumasError::Config {
            message: format!("disabled credential {SECRET}"),
        });
        assert_eq!(unavailable.code, -32000);
        assert_eq!(unavailable.class, PublicErrorClass::Unavailable);

        let invalid = PublicError::from(&PumasError::InvalidParams {
            message: format!("missing path {PRIVATE_PATH}"),
        });
        assert_eq!(invalid.code, -32602);
        assert_eq!(invalid.class, PublicErrorClass::InvalidRequest);

        let missing = PublicError::from(&PumasError::ModelNotFound {
            model_id: SECRET.to_string(),
        });
        assert_eq!(missing.code, -32002);
        assert_eq!(missing.class, PublicErrorClass::NotFound);

        let in_progress = PublicError::from(&PumasError::ModelIndexRefreshInProgress);
        assert_eq!(in_progress.code, -32011);
        assert_eq!(in_progress.class, PublicErrorClass::Conflict);

        let root_busy = PublicError::from(&PumasError::DownloadRootBusy);
        assert_eq!(root_busy.code, -32011);
        assert_eq!(root_busy.class, PublicErrorClass::Conflict);
        assert_eq!(
            root_busy.message,
            "The request conflicts with the current library state."
        );
        assert_eq!(
            serde_json::to_value(root_busy).unwrap(),
            json!({
                "code": -32011,
                "class": "conflict",
                "message": "The request conflicts with the current library state.",
            })
        );
    }

    fn request(method: &str, params: Option<Value>) -> Vec<u8> {
        let mut value = json!({
            "jsonrpc": "2.0",
            "method": method,
            "id": 7
        });
        if let Some(params) = params {
            value["params"] = params;
        }
        serde_json::to_vec(&value).unwrap()
    }

    fn catalog_record(id: &str, metadata: Value) -> ModelRecord {
        ModelRecord {
            id: id.to_string(),
            path: format!("/models/{id}"),
            cleaned_name: id.rsplit('/').next().unwrap().to_string(),
            official_name: format!("Model {id}"),
            model_type: "llm".to_string(),
            tags: vec!["not-projected".to_string()],
            hashes: [("sha256".to_string(), format!("hash-{id}"))]
                .into_iter()
                .collect(),
            metadata,
            updated_at: "2026-09-03T00:00:00Z".to_string(),
        }
    }

    fn models_outcome(records: Vec<ModelRecord>) -> Result<ModelsOutcome, PumasError> {
        ModelsOutcome::from_records(records, Path::new("/models"))
    }

    fn managed_models_outcome(mut records: Vec<ModelRecord>) -> Result<ModelsOutcome, PumasError> {
        let temp = tempfile::TempDir::new().unwrap();
        for record in &mut records {
            if record.metadata["download_incomplete"].as_bool() == Some(true) {
                let model_dir = temp.path().join(&record.id);
                std::fs::create_dir_all(&model_dir).unwrap();
                record.path = model_dir.display().to_string();
            }
        }
        ModelsOutcome::from_records(records, temp.path())
    }

    fn admission_error(body: &[u8]) -> RpcAdmissionError {
        match AdmittedRpcRequest::decode(body) {
            Ok(_) => panic!("request unexpectedly passed admission"),
            Err(error) => error,
        }
    }

    #[test]
    fn catalog_search_rejects_invalid_pagination_before_dispatch() {
        for params in [
            json!({"query":"llama", "limit":-1}),
            json!({"query":"llama", "offset":-1}),
            json!({"query":"llama", "limit":0}),
            json!({"query":"llama", "unexpected":true}),
        ] {
            assert_eq!(
                admission_error(&request("search_models_fts", Some(params)))
                    .error
                    .code,
                -32602
            );
        }
    }

    #[test]
    fn admission_distinguishes_syntax_envelope_method_and_param_failures() {
        let syntax = admission_error(br#"{"#);
        assert_eq!(syntax.error.code, -32700);

        let envelope = admission_error(br#"{"jsonrpc":"1.0","method":"get_status","id":7}"#);
        assert_eq!(envelope.error.code, -32600);
        assert_eq!(envelope.id, Some(json!(7)));

        let params = admission_error(&request("get_status", Some(Value::Null)));
        assert_eq!(params.error.code, -32602);
        assert_eq!(params.id, Some(json!(7)));

        let command =
            AdmittedRpcRequest::decode(&request("not_a_real_method", Some(json!({})))).unwrap();
        assert_eq!(command.command.method(), "not_a_real_method");
    }

    #[test]
    fn admission_rejects_unknown_fields_and_invalid_ids() {
        let extra = admission_error(
            br#"{"jsonrpc":"2.0","method":"get_status","params":{},"id":7,"extra":true}"#,
        );
        assert_eq!(extra.error.code, -32600);

        let invalid_id =
            admission_error(br#"{"jsonrpc":"2.0","method":"get_status","params":{},"id":{}}"#);
        assert_eq!(invalid_id.error.code, -32600);
        assert_eq!(invalid_id.id, None);
    }

    #[test]
    fn typed_params_enforce_aliases_types_bounds_and_exact_fields() {
        let admitted = AdmittedRpcRequest::decode(&request(
            "check_launcher_updates",
            Some(json!({"forceRefresh": true})),
        ))
        .unwrap();
        assert!(matches!(
            admitted.command,
            RpcCommand::CheckLauncherUpdates {
                force_refresh: true
            }
        ));

        for params in [
            json!({"force_refresh": null}),
            json!({"force_refresh": -1}),
            json!({"force_refresh": true, "extra": false}),
            json!({"force_refresh": true, "forceRefresh": false}),
        ] {
            let error = admission_error(&request("check_launcher_updates", Some(params)));
            assert_eq!(error.error.code, -32602);
        }
    }

    #[test]
    fn credential_admission_is_bounded_and_never_returns_secret_text() {
        let valid =
            AdmittedRpcRequest::decode(&request("set_hf_token", Some(json!({"token": SECRET}))))
                .unwrap();
        match valid.command {
            RpcCommand::SetHfToken { token } => assert_eq!(token.expose(), SECRET),
            _ => panic!("wrong admitted command"),
        }

        for params in [
            json!({}),
            json!({"token": null}),
            json!({"token": "  "}),
            json!({"token": SECRET, "extra": SECRET}),
            json!({"token": "x".repeat(MAX_CREDENTIAL_BYTES + 1)}),
        ] {
            let error = admission_error(&request("set_hf_token", Some(params)));
            let encoded = serde_json::to_string(&error.error).unwrap();
            assert_eq!(error.error.code, -32602);
            assert!(!encoded.contains(SECRET));
        }
    }

    #[test]
    fn typed_outcomes_serialize_exact_shapes_without_legacy_wrapper_defaults() {
        let health = RpcOutcome::Health(HealthOutcome::ok());
        assert!(!health.uses_response_wrapper());
        assert_eq!(health.into_value().unwrap(), json!({"status": "ok"}));

        let mutation = RpcOutcome::HfTokenMutation(SuccessOutcome::new());
        assert!(!mutation.uses_response_wrapper());
        assert_eq!(mutation.into_value().unwrap(), json!({"success": true}));

        let auth = RpcOutcome::HfAuth(Box::new(HfAuthOutcome::from(HfAuthStatus {
            authenticated: false,
            username: None,
            token_source: None,
        })));
        assert_eq!(
            auth.into_value().unwrap(),
            json!({
                "success": true,
                "authenticated": false,
                "username": null,
                "token_source": null,
            })
        );

        let legacy = RpcOutcome::Legacy(Value::Null);
        assert!(legacy.uses_response_wrapper());
    }

    #[test]
    fn launcher_version_outcome_rejects_wrong_or_extra_result_fields() {
        let valid = LauncherVersionOutcome::decode(json!({
            "success": true,
            "version": "0.6.0",
            "currentCommit": "abc1234",
            "branch": "main",
            "isGitRepo": true,
        }))
        .unwrap();
        assert_eq!(
            RpcOutcome::LauncherVersion(Box::new(valid))
                .into_value()
                .unwrap(),
            json!({
                "success": true,
                "version": "0.6.0",
                "currentCommit": "abc1234",
                "branch": "main",
                "isGitRepo": true,
            })
        );

        assert!(LauncherVersionOutcome::decode(json!({})).is_err());
        assert!(LauncherVersionOutcome::decode(json!({
            "success": true,
            "version": "0.6.0",
            "currentCommit": "abc1234",
            "branch": "main",
            "isGitRepo": true,
            "unexpected": false,
        }))
        .is_err());
    }

    #[test]
    fn link_commands_enforce_exact_bounded_params_before_dispatch() {
        let exclusion = AdmittedRpcRequest::decode(&request(
            "set_model_link_exclusion",
            Some(json!({"modelId": "llm/acme/model", "appId": "ollama"})),
        ))
        .unwrap();
        assert!(matches!(
            exclusion.command,
            RpcCommand::SetModelLinkExclusion { excluded: true, .. }
        ));

        let files = AdmittedRpcRequest::decode(&request(
            "check_files_writable",
            Some(json!({"filePaths": ["/tmp/model.gguf"]})),
        ))
        .unwrap();
        assert!(matches!(
            files.command,
            RpcCommand::CheckFilesWritable { file_paths } if file_paths.len() == 1
        ));

        for (method, params) in [
            ("get_links_for_model", json!({})),
            ("get_links_for_model", json!({"model_id": "  "})),
            (
                "get_links_for_model",
                json!({"model_id": "valid", "unexpected": true}),
            ),
            ("set_model_link_exclusion", json!({"model_id": "model"})),
            (
                "set_model_link_exclusion",
                json!({"model_id": "model", "app_id": "app", "excluded": 1}),
            ),
            ("check_files_writable", json!({"file_paths": []})),
            (
                "check_files_writable",
                json!({"file_paths": ["x".repeat(MAX_IDENTIFIER_BYTES + 1)]}),
            ),
            (
                "check_files_writable",
                json!({"file_paths": vec!["x"; MAX_COLLECTION_ITEMS + 1]}),
            ),
        ] {
            let error = admission_error(&request(method, Some(params)));
            assert_eq!(error.error, PublicError::invalid_params());
        }
    }

    #[test]
    fn link_outcomes_have_typed_non_defaulting_shapes() {
        let removed = RpcOutcome::RemoveOrphanedLinks(RemoveOrphanedLinksOutcome::new(3));
        assert!(!removed.uses_response_wrapper());
        assert_eq!(
            removed.into_value().unwrap(),
            json!({"success": true, "removed": 3})
        );

        let writable = RpcOutcome::FilesWritable(Box::new(FilesWritableOutcome::new(vec![
            FileWritableOutcome::new("/tmp/model.gguf".to_string(), true),
        ])));
        assert_eq!(
            writable.into_value().unwrap(),
            json!({
                "success": true,
                "results": [{"path": "/tmp/model.gguf", "writable": true}],
            })
        );
    }

    #[test]
    fn conversion_commands_enforce_exact_bounded_params_before_dispatch() {
        let start = AdmittedRpcRequest::decode(&request(
            "start_model_conversion",
            Some(json!({
                "modelId": "llm/acme/model",
                "direction": "SafetensorsToQuantizedGguf",
                "targetQuant": "Q4_K_M",
                "outputName": "model-q4",
                "imatrixCalibrationFile": "/tmp/calibration.txt",
                "forceImatrix": true,
            })),
        ))
        .unwrap();
        match start.command {
            RpcCommand::StartModelConversion { request } => {
                assert_eq!(request.model_id, "llm/acme/model");
                assert_eq!(
                    request.direction,
                    ConversionDirection::SafetensorsToQuantizedGguf
                );
                assert_eq!(request.target_quant.as_deref(), Some("Q4_K_M"));
                assert_eq!(request.output_name.as_deref(), Some("model-q4"));
                assert_eq!(
                    request.imatrix_calibration_file.as_deref(),
                    Some("/tmp/calibration.txt")
                );
                assert_eq!(request.force_imatrix, Some(true));
            }
            _ => panic!("wrong admitted conversion command"),
        }

        let backend = AdmittedRpcRequest::decode(&request(
            "setup_quantization_backend",
            Some(json!({"backend": "llama_cpp"})),
        ))
        .unwrap();
        assert!(matches!(
            backend.command,
            RpcCommand::SetupQuantizationBackend {
                backend: QuantBackend::LlamaCpp
            }
        ));

        for (method, params) in [
            ("start_model_conversion", json!({})),
            (
                "start_model_conversion",
                json!({"model_id": "model", "direction": "unknown"}),
            ),
            (
                "start_model_conversion",
                json!({
                    "model_id": "model",
                    "direction": "gguf_to_safetensors",
                    "target_quant": " ",
                }),
            ),
            (
                "start_model_conversion",
                json!({
                    "model_id": "model",
                    "modelId": "duplicate",
                    "direction": "gguf_to_safetensors",
                }),
            ),
            (
                "start_model_conversion",
                json!({
                    "model_id": "model",
                    "direction": "gguf_to_safetensors",
                    "unexpected": true,
                }),
            ),
            ("get_conversion_progress", json!({"conversion_id": "  "})),
            (
                "get_conversion_progress",
                json!({"conversion_id": "one", "conversionId": "two"}),
            ),
            ("list_model_conversions", json!({"unexpected": true})),
            ("setup_quantization_backend", json!({"backend": "unknown"})),
        ] {
            let error = admission_error(&request(method, Some(params)));
            assert_eq!(error.error, PublicError::invalid_params());
        }
    }

    #[test]
    fn conversion_started_ids_and_quantization_evidence_are_validated() {
        for invalid in [
            String::new(),
            " \t\n\u{85}".into(),
            "x".repeat(MAX_IDENTIFIER_BYTES + 1),
        ] {
            assert!(ConversionStartedOutcome::new(invalid).is_err());
        }
        assert!(ConversionStartedOutcome::new("x".repeat(MAX_IDENTIFIER_BYTES)).is_ok());
        assert!(ConversionStartedOutcome::new(" conversion ".into()).is_ok());
        let option = |bits_per_weight| QuantOption {
            name: "fixture".into(),
            description: "quantization fixture".into(),
            bits_per_weight,
            recommended: false,
            backend: None,
            imatrix_recommended: true,
        };
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY, -0.1] {
            assert!(SupportedQuantTypesOutcome::new(vec![option(invalid)]).is_err());
        }
        for valid in [0.0, 16.0, f32::MAX] {
            let encoded =
                serde_json::to_value(SupportedQuantTypesOutcome::new(vec![option(valid)]).unwrap())
                    .unwrap();
            assert!(encoded["quant_types"][0]["backend"].is_null());
            assert_eq!(encoded["quant_types"][0]["imatrixRecommended"], true);
            assert!(encoded["quant_types"][0].get("bitsPerWeight").is_some());
            assert!(encoded["quant_types"][0].get("bits_per_weight").is_none());
        }
    }

    #[test]
    fn backend_setup_requests_preserve_selection_and_reject_undeclared_inputs() {
        for (method, params, expected) in backend_setup_requests() {
            assert_eq!(
                parse_command(method, Some(&params)).is_ok(),
                expected,
                "{method}: {params}"
            );
        }
        let id = "00112233-4455-4677-8899-aabbccddeeff";
        for (wire, backend) in [
            ("python_conversion", QuantBackend::PythonConversion),
            ("llama_cpp", QuantBackend::LlamaCpp),
            ("nvfp4", QuantBackend::Nvfp4),
            ("sherry", QuantBackend::Sherry),
        ] {
            let params = json!({"backend":wire,"expected_previous_operation_id":id});
            assert!(
                matches!(parse_command("start_backend_setup", Some(&params)).unwrap(), RpcCommand::StartBackendSetup { backend: selected, expected_previous_operation_id: Some(token) } if selected == backend && token == id)
            );
            assert!(
                matches!(parse_command("get_backend_setup", Some(&json!({"backend":wire}))).unwrap(), RpcCommand::GetBackendSetup { backend: selected } if selected == backend)
            );
        }
    }

    #[test]
    fn conversion_setup_projection_preserves_states_and_redacts_diagnostics() {
        let id = "2e038924-e0e3-4266-95ef-f7a02997b7b6";
        for status in [
            ConversionSetupStatus::InProgress,
            ConversionSetupStatus::Completed,
            ConversionSetupStatus::Failed,
            ConversionSetupStatus::Cancelled,
        ] {
            let snapshot = ConversionSetupSnapshot {
                operation_id: id.into(),
                status,
                error: (status == ConversionSetupStatus::Failed)
                    .then(|| "/private/installer/token".into()),
            };
            let value =
                serde_json::to_value(ConversionSetupStartedOutcome::new(snapshot.clone()).unwrap())
                    .unwrap();
            assert_eq!(value["setup"]["operationId"], id);
            assert_eq!(
                value["setup"]["status"],
                serde_json::to_value(status).unwrap()
            );
            assert!(!value.to_string().contains("/private/"));
            assert_eq!(
                value["setup"]["error"].is_null(),
                status != ConversionSetupStatus::Failed
            );
            let mut contradictory = snapshot;
            contradictory.error = if contradictory.error.is_some() {
                None
            } else {
                Some("unexpected".into())
            };
            assert!(ConversionSetupStartedOutcome::new(contradictory).is_err());
        }
        assert_eq!(
            serde_json::to_value(ConversionSetupStatusOutcome::new(None).unwrap()).unwrap(),
            json!({"success":true,"setup":null})
        );
        for operation_id in [
            "",
            "not-an-id",
            "2E038924-e0e3-4266-95ef-f7a02997b7b6",
            "2e038924-e0e3-4266-95ef-f7a02997b7b6\n",
        ] {
            assert!(ConversionSetupStartedOutcome::new(ConversionSetupSnapshot {
                operation_id: operation_id.into(),
                status: ConversionSetupStatus::InProgress,
                error: None,
            })
            .is_err());
        }
    }

    #[test]
    fn conversion_setup_admission_preserves_previous_id_and_rejects_unknown_params() {
        let id = "2e038924-e0e3-4266-95ef-f7a02997b7b6";
        for params in [json!({}), json!({"expected_previous_operation_id":null})] {
            assert!(matches!(
                parse_command("start_conversion_setup", Some(&params)).unwrap(),
                RpcCommand::StartConversionSetup {
                    expected_previous_operation_id: None
                }
            ));
        }
        let params = json!({"expected_previous_operation_id":id});
        assert!(
            matches!(parse_command("start_conversion_setup", Some(&params)).unwrap(),
            RpcCommand::StartConversionSetup { expected_previous_operation_id: Some(value) } if value == id)
        );
        for params in [
            json!({"expected_previous_operation_id":""}),
            json!({"expected_previous_operation_id":42}),
            json!({"expectedPreviousOperationId":id}),
            json!({"force":true}),
            json!({"expected_previous_operation_id":format!("{id}\n")}),
        ] {
            assert!(parse_command("start_conversion_setup", Some(&params)).is_err());
        }
        assert!(matches!(
            parse_command("get_conversion_setup", None).unwrap(),
            RpcCommand::GetConversionSetup
        ));
        assert!(parse_command("get_conversion_setup", Some(&json!({"force":true}))).is_err());
    }

    #[test]
    fn conversion_read_projection_rejects_invalid_numeric_evidence() {
        let valid = || {
            serde_json::from_value::<ConversionProgress>(json!({
                "conversionId":"fixture", "sourceModelId":"llm/example/model",
                "direction":"gguf_to_safetensors", "status":"converting",
            }))
            .unwrap()
        };
        for value in [f32::NAN, f32::INFINITY, -0.01, 1.01] {
            let mut progress = valid();
            progress.progress = Some(value);
            assert!(ConversionProgressResponse::new(Some(progress.clone())).is_err());
            assert!(ConversionListOutcome::new(vec![valid(), progress]).is_err());
        }
        for estimated in [false, true] {
            let mut progress = valid();
            if estimated {
                progress.estimated_output_size = Some(MAX_JS_SAFE_INTEGER + 1);
            } else {
                progress.bytes_written = Some(MAX_JS_SAFE_INTEGER + 1);
            }
            assert!(ConversionProgressResponse::new(Some(progress.clone())).is_err());
            assert!(ConversionListOutcome::new(vec![progress]).is_err());
        }
        let mut boundary = valid();
        boundary.progress = Some(1.0);
        boundary.bytes_written = Some(MAX_JS_SAFE_INTEGER);
        boundary.estimated_output_size = Some(MAX_JS_SAFE_INTEGER);
        assert!(ConversionProgressResponse::new(Some(boundary.clone())).is_ok());
        assert!(ConversionListOutcome::new(vec![boundary]).is_ok());
        assert_eq!(
            serde_json::to_value(ConversionProgressResponse::new(None).unwrap()).unwrap(),
            json!({"success":true,"progress":null})
        );
        let absent =
            serde_json::to_value(ConversionProgressResponse::new(Some(valid())).unwrap()).unwrap();
        for field in [
            "progress",
            "currentTensor",
            "bytesWritten",
            "error",
            "pipelineStep",
        ] {
            assert_eq!(absent["progress"].get(field), Some(&Value::Null));
        }
    }

    #[test]
    fn conversion_outcomes_are_typed_and_redact_internal_progress_errors() {
        let started = RpcOutcome::ConversionStarted(
            ConversionStartedOutcome::new("conversion-1".to_string()).unwrap(),
        );
        assert!(!started.uses_response_wrapper());
        assert_eq!(
            started.into_value().unwrap(),
            json!({"success": true, "conversion_id": "conversion-1"})
        );

        let progress = ConversionProgress {
            conversion_id: "conversion-1".to_string(),
            source_model_id: "llm/acme/model".to_string(),
            direction: ConversionDirection::GgufToSafetensors,
            status: ConversionStatus::Error,
            progress: Some(0.25),
            current_tensor: None,
            tensors_completed: Some(1),
            tensors_total: Some(4),
            bytes_written: None,
            estimated_output_size: None,
            target_quant: None,
            error: Some(format!("credential {SECRET} failed at {PRIVATE_PATH}")),
            output_model_id: None,
            pipeline_step: None,
            pipeline_steps_total: None,
            pipeline_step_label: None,
        };
        let encoded = RpcOutcome::ConversionProgress(Box::new(
            ConversionProgressResponse::new(Some(progress)).unwrap(),
        ))
        .into_value()
        .unwrap();
        assert_eq!(encoded.get("success").and_then(Value::as_bool), Some(true));
        assert_eq!(
            encoded
                .pointer("/progress/conversionId")
                .and_then(Value::as_str),
            Some("conversion-1")
        );
        assert_eq!(
            encoded.pointer("/progress/error").and_then(Value::as_str),
            Some("The model conversion did not complete successfully.")
        );
        assert!(!encoded.to_string().contains(SECRET));
        assert!(!encoded.to_string().contains(PRIVATE_PATH));
    }

    #[test]
    fn os_open_commands_enforce_exact_bounded_params_before_dispatch() {
        let path = AdmittedRpcRequest::decode(&request(
            "open_path",
            Some(json!({"path": "/tmp/model.gguf"})),
        ))
        .unwrap();
        assert!(matches!(
            path.command,
            RpcCommand::OpenPath { path } if path == "/tmp/model.gguf"
        ));

        let url = AdmittedRpcRequest::decode(&request(
            "open_url",
            Some(json!({"url": "https://example.com/model"})),
        ))
        .unwrap();
        assert!(matches!(
            url.command,
            RpcCommand::OpenUrl { url } if url == "https://example.com/model"
        ));

        for (method, params) in [
            ("open_path", json!({})),
            ("open_path", json!({"path": null})),
            ("open_path", json!({"path": "  "})),
            ("open_path", json!({"path": "/tmp", "extra": true})),
            (
                "open_path",
                json!({"path": "x".repeat(MAX_IDENTIFIER_BYTES + 1)}),
            ),
            ("open_url", json!({})),
            ("open_url", json!({"url": false})),
            ("open_url", json!({"url": "  "})),
            (
                "open_url",
                json!({"url": "https://example.com", "extra": true}),
            ),
        ] {
            let error = admission_error(&request(method, Some(params)));
            assert_eq!(error.error, PublicError::invalid_params());
        }
    }

    #[test]
    fn operation_status_outcome_is_typed_and_non_defaulting() {
        let failure = RpcOutcome::OperationStatus(OperationStatusOutcome::failed());
        assert!(!failure.uses_response_wrapper());
        assert_eq!(
            failure.into_value().unwrap(),
            json!({
                "success": false,
                "error": "The requested operation failed.",
            })
        );
    }

    #[test]
    fn download_root_busy_partial_outcome_preserves_untracked_failure() {
        let action = PartialDownloadAction {
            action: "none".into(),
            download_id: None,
            status: None,
            reason_code: Some("download_root_busy".into()),
            message: Some(format!("private root {PRIVATE_PATH}")),
        };
        let outcome = RpcOutcome::PartialDownload(Box::new(action.clone().try_into().unwrap()))
            .into_value()
            .unwrap();
        assert_eq!(
            outcome,
            json!({
                "success": false,
                "action": "none",
                "download_id": null,
                "status": null,
                "reason_code": "download_root_busy",
                "error": "The partial download could not be resumed.",
            })
        );
        let tracked = PartialDownloadAction {
            download_id: Some("unexpected-admission".into()),
            status: Some(DownloadStatus::Queued),
            ..action
        };
        assert!(PartialDownloadOutcome::try_from(tracked).is_err());
    }

    #[test]
    fn download_commands_enforce_exact_bounded_params_before_dispatch() {
        let unknown_field = admission_error(&request(
            "start_model_download_from_hf",
            Some(json!({
                "repoId": "acme/model",
                "family": "acme",
                "officialName": "Model",
                "modelType": "llm",
                "target_unused": null,
            })),
        ));
        assert_eq!(
            unknown_field.error,
            PublicError::invalid_params(),
            "unknown download fields must fail closed"
        );

        let start = AdmittedRpcRequest::decode(&request(
            "start_model_download_from_hf",
            Some(json!({
                "repoId": "acme/model",
                "family": "acme",
                "officialName": "Model",
                "modelType": "llm",
                "filenames": ["model-00001-of-00002.safetensors", "model-00002-of-00002.safetensors"],
                "pipelineTag": "text-generation",
            })),
        ))
        .unwrap();
        match start.command {
            RpcCommand::StartModelDownloadFromHf { request } => {
                assert_eq!(request.repo_id, "acme/model");
                assert_eq!(request.model_type.as_deref(), Some("llm"));
                assert_eq!(request.pipeline_tag.as_deref(), Some("text-generation"));
                assert_eq!(request.filenames.as_ref().map(Vec::len), Some(2));
            }
            _ => panic!("wrong admitted download command"),
        }

        let recovery_token = format!("v1:{}", "a".repeat(64));
        let partial = AdmittedRpcRequest::decode(&request(
            "resume_partial_download",
            Some(json!({
                "modelId": "llm/acme/model",
                "recoveryToken": recovery_token,
            })),
        ))
        .unwrap();
        assert!(matches!(
            partial.command,
            RpcCommand::ResumePartialDownload { model_id, recovery_token }
                if model_id.as_str() == "llm/acme/model"
                    && recovery_token.as_str() == format!("v1:{}", "a".repeat(64))
        ));

        for (method, params) in [
            ("download_model_from_hf", json!({})),
            (
                "download_model_from_hf",
                json!({
                    "repo_id": "acme/model",
                    "repoId": "duplicate/model",
                    "family": "acme",
                    "official_name": "Model",
                }),
            ),
            (
                "download_model_from_hf",
                json!({
                    "repo_id": "acme/model",
                    "family": " ",
                    "official_name": "Model",
                }),
            ),
            (
                "download_model_from_hf",
                json!({
                    "repo_id": "acme/model",
                    "family": "acme",
                    "official_name": "Model",
                    "filenames": [],
                }),
            ),
            (
                "download_model_from_hf",
                json!({
                    "repo_id": "acme/model",
                    "family": "acme",
                    "official_name": "Model",
                    "filenames": vec!["model.safetensors"; MAX_COLLECTION_ITEMS + 1],
                }),
            ),
            (
                "download_model_from_hf",
                json!({
                    "repo_id": "acme/model",
                    "family": "acme",
                    "official_name": "Model",
                    "model_card_json": "x".repeat(MAX_METADATA_JSON_BYTES + 1),
                }),
            ),
            ("get_model_download_status", json!({"download_id": "  "})),
            (
                "cancel_model_download",
                json!({"download_id": "one", "downloadId": "two"}),
            ),
            ("list_model_downloads", json!({"unexpected": true})),
            (
                "resume_partial_download",
                json!({"repo_id": "acme/model", "dest_dir": "/tmp/model"}),
            ),
            (
                "resume_partial_download",
                json!({
                    "model_id": "llm/acme/model",
                    "recovery_token": format!("v1:{}", "a".repeat(64)),
                }),
            ),
            (
                "resume_partial_download",
                json!({
                    "model_id": "../outside",
                    "recovery_token": format!("v1:{}", "a".repeat(64)),
                }),
            ),
            (
                "resume_partial_download",
                json!({"model_id": "llm/acme/model", "recovery_token": "v1:short"}),
            ),
        ] {
            let error = admission_error(&request(method, Some(params)));
            assert_eq!(error.error, PublicError::invalid_params());
        }
    }

    #[test]
    fn download_outcomes_are_typed_and_redact_internal_failure_text() {
        let started = RpcOutcome::DownloadStarted(DownloadStartedOutcome::started(
            "download-1".to_string(),
            None,
        ));
        assert!(!started.uses_response_wrapper());
        assert_eq!(
            started.into_value().unwrap(),
            json!({
                "success": true,
                "download_id": "download-1",
                "selectedArtifactId": null,
                "artifactId": null,
            })
        );

        let unavailable =
            RpcOutcome::DownloadStarted(DownloadStartedOutcome::failed(&PumasError::Config {
                message: format!("credential {SECRET} is disabled"),
            }));
        let unavailable = unavailable.into_value().unwrap();
        assert_eq!(
            unavailable,
            json!({
                "success": false,
                "error": "A required operation is currently unavailable.",
            })
        );
        assert!(!unavailable.to_string().contains(SECRET));

        let progress = ModelDownloadProgress {
            download_id: "download-1".to_string(),
            library_model_id: Some("llm/acme/model".to_string()),
            repo_id: Some("acme/model".to_string()),
            selected_artifact_id: None,
            model_name: Some("Model".to_string()),
            model_type: Some("llm".to_string()),
            status: DownloadStatus::Error,
            progress: Some(0.5),
            downloaded_bytes: Some(5),
            total_bytes: Some(10),
            speed: None,
            eta_seconds: None,
            retry_attempt: None,
            retry_limit: None,
            retrying: None,
            next_retry_delay_seconds: None,
            error: Some(format!("credential {SECRET} failed at {PRIVATE_PATH}")),
        };
        let encoded = RpcOutcome::DownloadStatus(Box::new(
            DownloadStatusOutcome::new(Some(progress)).unwrap(),
        ))
        .into_value()
        .unwrap();
        assert_eq!(encoded.get("success").and_then(Value::as_bool), Some(true));
        assert_eq!(
            encoded.get("downloadId").and_then(Value::as_str),
            Some("download-1")
        );
        assert_eq!(
            encoded.get("error").and_then(Value::as_str),
            Some("The model download did not complete successfully.")
        );
        assert!(!encoded.to_string().contains(SECRET));
        assert!(!encoded.to_string().contains(PRIVATE_PATH));

        let partial = PartialDownloadAction {
            action: "none".to_string(),
            download_id: None,
            status: None,
            reason_code: Some("recover_failed".to_string()),
            message: Some(format!("credential {SECRET} failed at {PRIVATE_PATH}")),
        };
        let partial = RpcOutcome::PartialDownload(Box::new(partial.try_into().unwrap()))
            .into_value()
            .unwrap();
        assert_eq!(partial.get("success").and_then(Value::as_bool), Some(false));
        assert_eq!(
            partial.get("reason_code").and_then(Value::as_str),
            Some("recover_failed")
        );
        assert!(!partial.to_string().contains(SECRET));
        assert!(!partial.to_string().contains(PRIVATE_PATH));

        let disabled = PartialDownloadAction {
            action: "none".to_string(),
            download_id: None,
            status: None,
            reason_code: Some("hf_client_unavailable".to_string()),
            message: Some(format!("credential {SECRET} failed at {PRIVATE_PATH}")),
        };
        let disabled = RpcOutcome::PartialDownload(Box::new(disabled.try_into().unwrap()))
            .into_value()
            .unwrap();
        assert_eq!(
            disabled,
            json!({
                "success": false,
                "action": "none",
                "download_id": null,
                "status": null,
                "reason_code": "hf_client_unavailable",
                "error": "The partial download could not be resumed."
            })
        );
        assert!(!disabled.to_string().contains(SECRET));
        assert!(!disabled.to_string().contains(PRIVATE_PATH));

        let missing_mutation =
            RpcOutcome::DownloadMutation(DownloadMutationOutcome::completed(false))
                .into_value()
                .unwrap();
        assert_eq!(
            missing_mutation,
            json!({"success": false, "error": "Download not found"})
        );

        for invalid in [
            PartialDownloadAction {
                action: "invented".to_string(),
                download_id: None,
                status: None,
                reason_code: Some("recover_failed".to_string()),
                message: None,
            },
            PartialDownloadAction {
                action: "resume".to_string(),
                download_id: None,
                status: Some(DownloadStatus::Queued),
                reason_code: None,
                message: None,
            },
            PartialDownloadAction {
                action: "resume".to_string(),
                download_id: Some("download-1".to_string()),
                status: Some(DownloadStatus::Paused),
                reason_code: None,
                message: None,
            },
            PartialDownloadAction {
                action: "recover".to_string(),
                download_id: Some("download-1".to_string()),
                status: None,
                reason_code: None,
                message: None,
            },
            PartialDownloadAction {
                action: "attach".to_string(),
                download_id: Some("download-1".to_string()),
                status: Some(DownloadStatus::Completed),
                reason_code: None,
                message: None,
            },
            PartialDownloadAction {
                action: "none".to_string(),
                download_id: None,
                status: None,
                reason_code: Some("already_completed".to_string()),
                message: None,
            },
            PartialDownloadAction {
                action: "none".to_string(),
                download_id: Some("download-1".to_string()),
                status: Some(DownloadStatus::Downloading),
                reason_code: Some("already_completed".to_string()),
                message: None,
            },
            PartialDownloadAction {
                action: "none".to_string(),
                download_id: Some("download-1".to_string()),
                status: Some(DownloadStatus::Paused),
                reason_code: Some("recover_failed".to_string()),
                message: None,
            },
            PartialDownloadAction {
                action: "attach".to_string(),
                download_id: Some("   ".to_string()),
                status: Some(DownloadStatus::Downloading),
                reason_code: None,
                message: None,
            },
            PartialDownloadAction {
                action: "attach".to_string(),
                download_id: Some("x".repeat(MAX_IDENTIFIER_BYTES + 1)),
                status: Some(DownloadStatus::Downloading),
                reason_code: None,
                message: None,
            },
        ] {
            assert!(PartialDownloadOutcome::try_from(invalid).is_err());
        }
    }

    #[test]
    fn link_health_projection_preserves_public_shape_and_rejects_false_reports() {
        let valid = || LinkHealthResponse {
            success: true,
            error: None,
            status: "degraded".into(),
            total_links: 2,
            healthy_links: 1,
            broken_links: vec!["/fixture/missing".into()],
            orphaned_links: vec![],
            warnings: vec![],
            errors: vec![],
        };
        let report = valid();
        let expected = serde_json::to_value(&report).unwrap();
        assert_eq!(
            RpcOutcome::LinkHealth(Box::new(report.try_into().unwrap()))
                .into_value()
                .unwrap(),
            expected
        );
        for field in ["success", "error", "status", "total", "healthy", "overflow"] {
            let mut report = valid();
            match field {
                "success" => report.success = false,
                "error" => report.error = Some("private diagnostic".into()),
                "status" => report.status = "healthy".into(),
                "total" => report.total_links = 3,
                "healthy" => report.healthy_links = 2,
                "overflow" => report.healthy_links = usize::MAX,
                _ => unreachable!(),
            }
            let error = LinkHealthOutcome::try_from(report)
                .err()
                .expect("contradictory health must fail");
            let public = PublicError::from(&error);
            assert_eq!(public.class, PublicErrorClass::Internal);
            assert!(!public.message.contains("private diagnostic"));
        }
    }

    #[test]
    fn download_progress_rejects_unrepresentable_numeric_evidence() {
        let valid = || {
            serde_json::from_value::<ModelDownloadProgress>(json!({
                "downloadId":"numeric-fixture", "status":"downloading", "progress":0.5,
                "downloadedBytes":5, "totalBytes":10
            }))
            .unwrap()
        };
        let mut invalid = valid();
        invalid.total_bytes = Some(MAX_JS_SAFE_INTEGER + 1);
        assert!(DownloadProgressOutcome::try_from(invalid).is_err());
        for progress in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            let mut invalid = valid();
            invalid.progress = Some(progress);
            assert!(DownloadProgressOutcome::try_from(invalid).is_err());
        }
        for value in [f64::NAN, f64::INFINITY, -1.0] {
            let mut invalid = valid();
            invalid.speed = Some(value);
            assert!(DownloadProgressOutcome::try_from(invalid).is_err());
        }
        let mut boundary = valid();
        boundary.total_bytes = Some(MAX_JS_SAFE_INTEGER);
        boundary.progress = Some(1.0);
        assert!(DownloadProgressOutcome::try_from(boundary).is_ok());
    }

    #[test]
    fn download_library_identity_is_exact_optional_and_validated_for_poll_and_push() {
        let progress = |model_id: Option<&str>| {
            serde_json::from_value::<ModelDownloadProgress>(json!({
                "downloadId": "identity-fixture", "status": "downloading",
                "libraryModelId": model_id, "error": "private diagnostic",
            }))
            .unwrap()
        };
        let notification = |progress| pumas_library::models::ModelDownloadUpdateNotification {
            cursor: "download:1".into(),
            snapshot: pumas_library::models::ModelDownloadSnapshot {
                cursor: "download:1".into(),
                revision: 1,
                downloads: vec![progress],
            },
            stale_cursor: false,
            snapshot_required: true,
        };
        for model_id in [None, Some("llm/acme/model-q4"), Some("llm/other/model-q8")] {
            let item = progress(model_id);
            let poll = serde_json::to_value(DownloadListOutcome::new(vec![item.clone()]).unwrap())
                .unwrap();
            let push = project_download_notification(&notification(item)).unwrap();
            assert_eq!(poll["downloads"], push["snapshot"]["downloads"]);
            assert_eq!(
                push["snapshot"]["downloads"][0]["libraryModelId"],
                json!(model_id)
            );
            assert!(!push.to_string().contains("private diagnostic"));
        }
        for invalid in ["", "../model", "/llm/model", "llm\\model", "llm/CON/model"] {
            assert!(DownloadListOutcome::new(vec![progress(Some(invalid))]).is_err());
            assert!(project_download_notification(&notification(progress(Some(invalid)))).is_err());
        }
    }

    #[test]
    fn model_catalog_commands_and_outcomes_are_exact_and_non_defaulting() {
        let get_models = AdmittedRpcRequest::decode(&request("get_models", None)).unwrap();
        assert!(matches!(get_models.command, RpcCommand::GetModels));

        let refresh =
            AdmittedRpcRequest::decode(&request("refresh_model_index", Some(json!({})))).unwrap();
        assert!(matches!(refresh.command, RpcCommand::RefreshModelIndex));

        for (method, params) in [
            ("get_models", json!(null)),
            ("get_models", json!({"unexpected": true})),
            ("refresh_model_index", json!([])),
            ("refresh_model_index", json!({"unexpected": true})),
        ] {
            let error = admission_error(&request(method, Some(params)));
            assert_eq!(error.error, PublicError::invalid_params());
        }

        let model = pumas_library::ModelRecord {
            id: "llm/acme/model".to_string(),
            path: "/models/llm/acme/model".to_string(),
            cleaned_name: "model".to_string(),
            official_name: "Model".to_string(),
            model_type: "llm".to_string(),
            tags: vec!["text-generation".to_string()],
            hashes: Default::default(),
            metadata: json!({
                "family": "acme",
                "size_bytes": 4096,
                "added_date": "2026-09-02",
                "dependency_bindings": [{"profile_id": "llama-cpp-runtime"}],
                "related_available": true,
                "primary_format": "GGUF",
                "quantization": " q4_k_m ",
                "download_incomplete": true,
                "download_has_part_files": true,
                "download_missing_expected_files": 1,
                "download_progress": 0.42,
                "repo_id": "acme/model",
                "selected_artifact_id": "acme/model::Q4_K_M",
                "selected_artifact_files": ["weights-2.gguf", "weights-1.gguf"],
                "selected_artifact_quant": "Q4_K_M",
                "integrity_issue_duplicate_repo_id": true,
                "integrity_issue_duplicate_repo_id_count": 2,
                "integrity_issue_duplicate_repo_id_others": ["llm/acme/model-copy"]
            }),
            updated_at: "2026-09-03T00:00:00Z".to_string(),
        };
        let temp = tempfile::TempDir::new().unwrap();
        let model_dir = temp.path().join(&model.id);
        std::fs::create_dir_all(&model_dir).unwrap();
        let mut model = model;
        model.path = model_dir.display().to_string();
        let models = RpcOutcome::Models(Box::new(
            ModelsOutcome::from_records(vec![model.clone()], temp.path()).unwrap(),
        ))
        .into_value()
        .unwrap();
        let recovery_token = models["models"]["llm/acme/model"]["artifact"]["recovery"]
            ["recoveryToken"]
            .as_str()
            .unwrap()
            .to_string();
        assert!(DownloadRecoveryToken::parse(&recovery_token).is_some());
        assert_eq!(
            models,
            json!({
                "success": true,
                "models": {
                    "llm/acme/model": {
                        "id": "llm/acme/model",
                        "modelDir": model_dir.display().to_string(),
                        "displayName": "Model",
                        "modelType": "llm",
                        "format": "gguf",
                        "quantization": "Q4_K_M",
                        "sizeBytes": 4096,
                        "displayDate": "2026-09-02",
                        "dependencyCount": 1,
                        "relatedAvailable": true,
                        "artifact": {
                            "state": "partial",
                            "downloadProgressFraction": 0.42,
                            "reasons": ["part_file_present", "expected_files_missing"],
                            "recovery": {
                                "recoveryToken": recovery_token,
                                "repoId": "acme/model",
                                "selectedArtifactId": "acme/model::Q4_K_M",
                                "selectedArtifactFiles": ["weights-1.gguf", "weights-2.gguf"],
                                "selectedArtifactQuant": "Q4_K_M"
                            }
                        },
                        "integrity": {
                            "state": "duplicate",
                            "count": 2,
                            "otherModelIds": ["llm/acme/model-copy"]
                        }
                    }
                }
            })
        );
        assert!(ModelsOutcome::from_records(vec![model.clone(), model], temp.path()).is_err());

        let refreshed = RpcOutcome::ModelIndexRefresh(ModelIndexRefreshOutcome::new(48))
            .into_value()
            .unwrap();
        assert_eq!(refreshed, json!({"success": true, "indexed_count": 48}));
    }

    #[test]
    fn catalog_related_availability_preserves_core_optional_boolean() {
        let valid_metadata = json!({
            "dependency_bindings": [],
            "download_incomplete": false,
            "download_has_part_files": false,
            "download_missing_expected_files": 0,
            "download_progress": null
        });
        for (flag, expected) in [
            (None, false),
            (Some(Value::Null), false),
            (Some(json!(false)), false),
            (Some(json!(true)), true),
        ] {
            let mut metadata = valid_metadata.clone();
            if let Some(flag) = flag {
                metadata["related_available"] = flag;
            }
            let outcome = models_outcome(vec![catalog_record("llm/acme/model", metadata)]).unwrap();
            assert_eq!(
                serde_json::to_value(outcome).unwrap()["models"]["llm/acme/model"]
                    ["relatedAvailable"],
                expected
            );
        }
        let mut invalid_metadata = valid_metadata;
        invalid_metadata["related_available"] = json!("yes");
        let error = models_outcome(vec![catalog_record("llm/acme/model", invalid_metadata)])
            .err()
            .expect("malformed related availability must be rejected");
        assert!(matches!(
            error,
            PumasError::Other(message)
                if message == "Invalid model catalog record llm/acme/model: related availability is not boolean"
        ));
    }

    #[test]
    fn model_catalog_projection_rejects_malformed_or_inconsistent_records() {
        let valid_metadata = json!({
            "dependency_bindings": [],
            "download_incomplete": false,
            "download_has_part_files": false,
            "download_missing_expected_files": 0,
            "download_progress": null
        });
        let mut invalid_records = vec![catalog_record("llm/acme/non-object", json!(null))];

        for (suffix, field, value) in [
            ("missing-state", "download_incomplete", None),
            ("wrong-state", "download_incomplete", Some(json!("false"))),
            (
                "unsafe-size",
                "size_bytes",
                Some(json!(9_007_199_254_740_992_u64)),
            ),
            (
                "wrong-bindings",
                "dependency_bindings",
                Some(json!(["bad"])),
            ),
        ] {
            let mut metadata = valid_metadata.clone();
            match value {
                Some(value) => metadata[field] = value,
                None => {
                    metadata.as_object_mut().unwrap().remove(field);
                }
            }
            invalid_records.push(catalog_record(&format!("llm/acme/{suffix}"), metadata));
        }

        for (suffix, metadata) in [
            (
                "partial-no-reason",
                json!({
                    "dependency_bindings": [],
                    "download_incomplete": true,
                    "download_has_part_files": false,
                    "download_missing_expected_files": 0,
                    "download_progress": 0.5
                }),
            ),
            (
                "partial-complete-progress",
                json!({
                    "dependency_bindings": [],
                    "download_incomplete": true,
                    "download_has_part_files": true,
                    "download_missing_expected_files": 0,
                    "download_progress": 1.0
                }),
            ),
            (
                "progress-out-of-range",
                json!({
                    "dependency_bindings": [],
                    "download_incomplete": true,
                    "download_has_part_files": true,
                    "download_missing_expected_files": 0,
                    "download_progress": 1.1
                }),
            ),
            (
                "malformed-selected-files",
                json!({
                    "dependency_bindings": [],
                    "download_incomplete": true,
                    "download_has_part_files": true,
                    "download_missing_expected_files": 0,
                    "download_progress": 0.5,
                    "repo_id": "acme/model",
                    "selected_artifact_files": ["model.gguf", 7]
                }),
            ),
            (
                "unsafe-selected-file",
                json!({
                    "dependency_bindings": [],
                    "download_incomplete": true,
                    "download_has_part_files": true,
                    "download_missing_expected_files": 0,
                    "download_progress": 0.5,
                    "repo_id": "acme/model",
                    "selected_artifact_files": ["../model.gguf"]
                }),
            ),
            (
                "invalid-integrity",
                json!({
                    "dependency_bindings": [],
                    "download_incomplete": false,
                    "download_has_part_files": false,
                    "download_missing_expected_files": 0,
                    "download_progress": null,
                    "integrity_issue_duplicate_repo_id": true,
                    "integrity_issue_duplicate_repo_id_count": 1,
                    "integrity_issue_duplicate_repo_id_others": []
                }),
            ),
        ] {
            invalid_records.push(catalog_record(&format!("llm/acme/{suffix}"), metadata));
        }

        for record in invalid_records {
            assert!(
                managed_models_outcome(vec![record]).is_err(),
                "invalid catalog record must fail the whole outcome"
            );
        }
    }

    #[test]
    fn model_catalog_keeps_complete_variant_ready_when_another_quant_is_partial() {
        // This is the metadata shape produced by ModelLibrary when the selected
        // Q4 artifact is partial but a complete Q5 GGUF remains displayable.
        let model_id = "vlm/test/mixed-gguf-partial";
        let metadata = json!({
            "dependency_bindings": [],
            "primary_format": "gguf",
            "quantization": "Q5_K_M",
            "download_incomplete": false,
            "download_has_part_files": true,
            "download_missing_expected_files": 1,
            "download_progress": null,
            "repo_id": "owner/model",
            "selected_artifact_id": "owner/model::Q4_K_M",
            "selected_artifact_files": ["model-Q4_K_M.gguf"],
            "selected_artifact_quant": "Q4_K_M"
        });
        let mut record = catalog_record(model_id, metadata.clone());
        record.model_type = "vlm".to_string();

        let value = RpcOutcome::Models(Box::new(models_outcome(vec![record]).unwrap()))
            .into_value()
            .unwrap();

        assert_eq!(
            value["models"][model_id]["artifact"],
            json!({"state": "complete"})
        );
        assert_eq!(value["models"][model_id]["quantization"], "Q5_K_M");

        // Complete rows do not own a recovery action. Legacy subordinate
        // selection data therefore cannot invalidate an otherwise ready row.
        let legacy_model_id = "vlm/test/mixed-gguf-legacy-selection";
        let mut legacy_metadata = metadata;
        legacy_metadata["repo_id"] = json!(7);
        legacy_metadata["selected_artifact_files"] = json!(["../model-Q4_K_M.gguf"]);
        let mut legacy_record = catalog_record(legacy_model_id, legacy_metadata.clone());
        legacy_record.model_type = "vlm".to_string();
        let legacy_value =
            RpcOutcome::Models(Box::new(models_outcome(vec![legacy_record]).unwrap()))
                .into_value()
                .unwrap();
        assert_eq!(
            legacy_value["models"][legacy_model_id]["artifact"],
            json!({"state": "complete"})
        );

        // The same malformed recovery data remains invalid when the row is
        // genuinely partial and recovery becomes part of its closed state.
        legacy_metadata["download_incomplete"] = json!(true);
        assert!(managed_models_outcome(vec![catalog_record(
            "vlm/test/partial-malformed-recovery",
            legacy_metadata,
        )])
        .is_err());
    }

    #[test]
    fn model_catalog_keeps_filesystem_ineligible_partial_rows_without_recovery() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path().join("library");
        let outside = temp.path().join("outside/llm/acme/model");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::create_dir_all(&outside).unwrap();
        let mut record = catalog_record(
            "llm/acme/model",
            json!({
                "dependency_bindings": [],
                "download_incomplete": true,
                "download_has_part_files": true,
                "download_missing_expected_files": 1,
                "download_progress": 0.5,
                "repo_id": "acme/model",
                "selected_artifact_files": ["weights.gguf"]
            }),
        );
        record.path = outside.display().to_string();

        let value = RpcOutcome::Models(Box::new(
            ModelsOutcome::from_records(vec![record], &root).unwrap(),
        ))
        .into_value()
        .unwrap();
        assert_eq!(
            value["models"]["llm/acme/model"]["artifact"],
            json!({
                "state": "partial",
                "downloadProgressFraction": 0.5,
                "reasons": ["part_file_present", "expected_files_missing"]
            })
        );

        let managed_dir = root.join("llm/acme/no-provenance");
        std::fs::create_dir_all(&managed_dir).unwrap();
        let mut no_provenance = catalog_record(
            "llm/acme/no-provenance",
            json!({
                "dependency_bindings": [],
                "download_incomplete": true,
                "download_has_part_files": true,
                "download_missing_expected_files": 1,
                "download_progress": 0.5,
                "selected_artifact_id": "acme/model::Q4",
                "selected_artifact_files": ["weights.gguf"]
            }),
        );
        no_provenance.path = managed_dir.display().to_string();
        let value = RpcOutcome::Models(Box::new(
            ModelsOutcome::from_records(vec![no_provenance], &root).unwrap(),
        ))
        .into_value()
        .unwrap();
        assert_eq!(
            value["models"]["llm/acme/no-provenance"]["artifact"],
            json!({
                "state": "partial",
                "downloadProgressFraction": 0.5,
                "reasons": ["part_file_present", "expected_files_missing"]
            })
        );
    }

    #[test]
    fn model_catalog_projection_rejects_oversized_record_fields_and_lists() {
        let complete = json!({
            "dependency_bindings": [],
            "download_incomplete": false,
            "download_has_part_files": false,
            "download_missing_expected_files": 0,
            "download_progress": null
        });
        let partial = json!({
            "dependency_bindings": [],
            "download_incomplete": true,
            "download_has_part_files": true,
            "download_missing_expected_files": 0,
            "download_progress": 0.5
        });
        let oversized = "x".repeat(MAX_IDENTIFIER_BYTES + 1);

        let oversized_id = catalog_record(&oversized, complete.clone());
        let mut oversized_path = catalog_record("llm/acme/path", complete.clone());
        oversized_path.path = oversized.clone();
        let mut oversized_display = catalog_record("llm/acme/display", complete.clone());
        oversized_display.official_name = oversized.clone();
        let mut oversized_type = catalog_record("llm/acme/type", complete.clone());
        oversized_type.model_type = oversized.clone();

        let mut oversized_optional = partial.clone();
        oversized_optional["repo_id"] = json!(oversized.clone());

        let mut too_many_files = partial.clone();
        too_many_files["repo_id"] = json!("acme/model");
        too_many_files["selected_artifact_files"] = Value::Array(
            (0..=MAX_COLLECTION_ITEMS)
                .map(|index| json!(format!("file-{index}.gguf")))
                .collect(),
        );

        let mut oversized_file = partial;
        oversized_file["repo_id"] = json!("acme/model");
        oversized_file["selected_artifact_files"] = json!([oversized.clone()]);

        let mut too_many_bindings = complete.clone();
        too_many_bindings["dependency_bindings"] =
            Value::Array((0..=MAX_COLLECTION_ITEMS).map(|_| json!({})).collect());

        for record in [
            oversized_id,
            oversized_path,
            oversized_display,
            oversized_type,
            catalog_record("llm/acme/optional", oversized_optional),
            catalog_record("llm/acme/files", too_many_files),
            catalog_record("llm/acme/file", oversized_file),
            catalog_record("llm/acme/bindings", too_many_bindings),
        ] {
            assert!(managed_models_outcome(vec![record]).is_err());
        }
    }

    #[test]
    fn model_catalog_recovery_repo_id_requires_exact_owner_and_name() {
        let base = json!({
            "dependency_bindings": [],
            "download_incomplete": true,
            "download_has_part_files": true,
            "download_missing_expected_files": 0,
            "download_progress": 0.5
        });

        for (index, repo_id) in [
            "owner",
            "/name",
            "owner/",
            "owner/name/extra",
            "owner/bad name",
            "owner/..",
            ".owner/name",
            "owner/name.",
            "owner/name--variant",
            "owner/name..variant",
            "owner/name.git",
            "owner/na!me",
        ]
        .into_iter()
        .enumerate()
        {
            let mut metadata = base.clone();
            metadata["repo_id"] = json!(repo_id);
            assert!(managed_models_outcome(vec![catalog_record(
                &format!("llm/acme/invalid-repo-{index}"),
                metadata,
            )])
            .is_err());
        }

        let mut oversized_repo = base.clone();
        oversized_repo["repo_id"] = json!(format!("a/{}", "b".repeat(95)));
        assert!(managed_models_outcome(vec![catalog_record(
            "llm/acme/oversized-repo",
            oversized_repo,
        )])
        .is_err());

        let mut valid = base;
        valid["repo_id"] = json!("Owner_1/model.name-2");
        assert!(
            managed_models_outcome(vec![catalog_record("llm/acme/valid-repo", valid,)]).is_ok()
        );
    }

    #[test]
    fn model_catalog_recovery_paths_are_platform_neutral() {
        let base = json!({
            "dependency_bindings": [],
            "download_incomplete": true,
            "download_has_part_files": true,
            "download_missing_expected_files": 0,
            "download_progress": 0.5,
            "repo_id": "owner/name"
        });

        for (index, path) in [
            r"C:\models\file.gguf",
            r"..\file.gguf",
            r"\\server\share\file.gguf",
            "C:/models/file.gguf",
            "folder//file.gguf",
            "folder/./file.gguf",
            "folder/CON.gguf",
            "folder/file.gguf.",
            "folder/file?.gguf",
        ]
        .into_iter()
        .enumerate()
        {
            let mut metadata = base.clone();
            metadata["selected_artifact_files"] = json!([path]);
            assert!(
                managed_models_outcome(vec![catalog_record(
                    &format!("llm/acme/invalid-path-{index}"),
                    metadata,
                )])
                .is_err(),
                "path must be rejected on every host: {path:?}"
            );
        }

        let mut oversized_component = base.clone();
        oversized_component["selected_artifact_files"] = json!(["a".repeat(256)]);
        assert!(managed_models_outcome(vec![catalog_record(
            "llm/acme/oversized-path-component",
            oversized_component,
        )])
        .is_err());

        let mut valid = base;
        valid["selected_artifact_files"] = json!(["weights/model.gguf"]);
        assert!(managed_models_outcome(vec![catalog_record("llm/acme/valid-path", valid)]).is_ok());
    }

    #[test]
    fn model_catalog_serialization_is_stable_and_omits_open_core_fields() {
        let metadata = json!({
            "dependency_bindings": [],
            "download_incomplete": false,
            "download_has_part_files": false,
            "download_missing_expected_files": 0,
            "download_progress": null,
            "unconsumed_open_value": {"must": "not leak"}
        });
        let first = catalog_record("llm/acme/a", metadata.clone());
        let second = catalog_record("llm/acme/b", metadata);

        let forward =
            serde_json::to_vec(&models_outcome(vec![first.clone(), second.clone()]).unwrap())
                .unwrap();
        let reverse = serde_json::to_vec(&models_outcome(vec![second, first]).unwrap()).unwrap();

        assert_eq!(forward, reverse);
        let encoded = String::from_utf8(forward).unwrap();
        for omitted in [
            "metadata",
            "hashes",
            "tags",
            "updatedAt",
            "unconsumed_open_value",
        ] {
            assert!(!encoded.contains(omitted));
        }
    }

    #[test]
    fn model_index_refresh_count_is_checked_to_u32() {
        assert!(ModelIndexRefreshOutcome::try_from(48_usize).is_ok());
        if usize::BITS > u32::BITS {
            assert!(ModelIndexRefreshOutcome::try_from(usize::MAX).is_err());
        }
    }
}
