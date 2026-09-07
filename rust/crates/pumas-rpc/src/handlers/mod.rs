//! JSON-RPC request handlers, split by domain.

mod conversion;
mod links;
mod models;
#[cfg(feature = "inference-plugins")]
mod ollama;
#[cfg(feature = "inference-plugins")]
mod openai_gateway;
#[cfg(feature = "inference-plugins")]
mod openai_gateway_onnx;
#[cfg(feature = "inference-plugins")]
mod plugins;
mod process;
#[cfg(feature = "inference-plugins")]
mod runtime_profiles;
#[cfg(feature = "inference-plugins")]
mod serving;
#[cfg(feature = "inference-plugins")]
mod serving_llama_cpp;
#[cfg(feature = "inference-plugins")]
mod serving_llama_cpp_router;
#[cfg(feature = "inference-plugins")]
mod serving_llama_cpp_shared;
#[cfg(feature = "inference-plugins")]
mod serving_ollama;
#[cfg(feature = "inference-plugins")]
mod serving_onnx;
mod shared;
mod status;
#[cfg(test)]
pub(crate) mod test_support;
#[cfg(feature = "inference-plugins")]
mod torch;
#[cfg(feature = "inference-plugins")]
mod versions;

use crate::contract::{
    AdmittedRpcRequest, HealthOutcome, HfAuthOutcome, PublicError, RpcCommand, RpcOutcome,
    ShutdownOutcome, SuccessOutcome,
};
use crate::server::AppState;
use crate::wrapper::wrap_response;
use axum::{
    body::Bytes,
    extract::{Query, State},
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    Json,
};
use futures::{
    stream::{self, BoxStream},
    StreamExt,
};
use pumas_library::models::{
    ModelDownloadUpdateNotification, ModelLibraryUpdateNotification,
    StatusTelemetryUpdateNotification,
};
#[cfg(feature = "inference-plugins")]
use pumas_library::models::{RuntimeProfileUpdateFeed, ServingStatusUpdateFeed};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::convert::Infallible;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, warn};

const MODEL_LIBRARY_UPDATE_STREAM_LIMIT: usize = 250;

#[cfg(feature = "inference-plugins")]
pub use openai_gateway::{handle_openai_models, handle_openai_proxy};

pub(crate) use shared::{
    detect_sandbox_environment, extract_safetensors_header, get_bool_param, get_i64_param,
    get_str_param, parse_params, path_exists, require_str_param,
    validate_existing_local_directory_path, validate_existing_local_file_path,
    validate_existing_local_path, validate_external_url, validate_local_write_target_path,
    validate_non_empty,
};
#[cfg(feature = "inference-plugins")]
pub(crate) use shared::{get_version_manager, read_utf8_file, require_version_manager};

// ============================================================================
// JSON-RPC types
// ============================================================================

/// JSON-RPC 2.0 response structure.
#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
    pub id: Option<Value>,
}

/// JSON-RPC 2.0 error structure.
#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<Value>, result: Value) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: Some(result),
            error: None,
            id,
        }
    }

    pub fn error(id: Option<Value>, error: PublicError) -> Self {
        Self {
            jsonrpc: "2.0".to_string(),
            result: None,
            error: Some(JsonRpcError {
                code: error.code,
                message: error.message.to_string(),
                data: Some(json!({ "class": error.class })),
            }),
            id,
        }
    }
}

// ============================================================================
// HTTP endpoints
// ============================================================================

/// Health check endpoint.
pub async fn handle_health() -> impl IntoResponse {
    Json(json!({"status": "ok"}))
}

/// Server-sent model-library update notification stream.
pub async fn handle_model_library_update_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ModelLibraryUpdateStreamQuery>,
) -> Sse<BoxStream<'static, Result<Event, Infallible>>> {
    let stream: BoxStream<'static, Result<Event, Infallible>> =
        match build_model_library_update_stream_state(state, query.cursor).await {
            Ok(stream_state) => {
                stream::unfold(stream_state, next_model_library_update_event).boxed()
            }
            Err(error) => {
                let public_error = PublicError::from(&error);
                warn!(
                    error_code = public_error.code,
                    error_class = public_error.class.as_str(),
                    "model-library update stream startup failed"
                );
                stream::once(async move {
                    Ok(Event::default()
                        .event("model-library-error")
                        .data(public_error_event(public_error)))
                })
                .boxed()
            }
        };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[derive(Debug, Default, Deserialize)]
pub struct ModelLibraryUpdateStreamQuery {
    pub cursor: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct ModelDownloadUpdateStreamQuery {
    pub cursor: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[cfg(feature = "inference-plugins")]
pub struct RuntimeProfileUpdateStreamQuery {
    pub cursor: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[cfg(feature = "inference-plugins")]
pub struct ServingStatusUpdateStreamQuery {
    pub cursor: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
pub struct StatusTelemetryUpdateStreamQuery {
    pub cursor: Option<String>,
}

async fn build_model_library_update_stream_state(
    state: Arc<AppState>,
    cursor: Option<String>,
) -> pumas_library::Result<ModelLibraryUpdateStreamState> {
    let requested_cursor = match cursor {
        Some(cursor) if !cursor.trim().is_empty() => cursor,
        _ => {
            state
                .api
                .list_model_library_updates_since(None, MODEL_LIBRARY_UPDATE_STREAM_LIMIT)
                .await?
                .cursor
        }
    };
    let subscriber = state
        .api
        .subscribe_model_library_update_stream_since(&requested_cursor)
        .await?;
    let handshake = subscriber.handshake().clone();
    let cursor = handshake.cursor_after_recovery.clone();
    let pending_notification = if handshake.recovered_events.is_empty()
        && !handshake.stale_cursor
        && !handshake.snapshot_required
    {
        None
    } else {
        Some(ModelLibraryUpdateNotification {
            cursor: handshake.cursor_after_recovery,
            events: handshake.recovered_events,
            stale_cursor: handshake.stale_cursor,
            snapshot_required: handshake.snapshot_required,
        })
    };

    Ok(ModelLibraryUpdateStreamState {
        subscriber,
        cursor,
        pending_notification,
    })
}

/// Server-sent model download update notification stream.
pub async fn handle_model_download_update_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ModelDownloadUpdateStreamQuery>,
) -> Sse<BoxStream<'static, Result<Event, Infallible>>> {
    let stream: BoxStream<'static, Result<Event, Infallible>> =
        match build_model_download_update_stream_state(state, query.cursor).await {
            Ok(stream_state) => {
                stream::unfold(stream_state, next_model_download_update_event).boxed()
            }
            Err(error) => {
                let public_error = PublicError::from(&error);
                warn!(
                    error_code = public_error.code,
                    error_class = public_error.class.as_str(),
                    "model download update stream startup failed"
                );
                stream::once(async move {
                    Ok(Event::default()
                        .event("model-download-error")
                        .data(public_error_event(public_error)))
                })
                .boxed()
            }
        };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

async fn build_model_download_update_stream_state(
    state: Arc<AppState>,
    cursor: Option<String>,
) -> pumas_library::Result<ModelDownloadUpdateStreamState> {
    let receiver = state.api.subscribe_hf_download_updates().ok_or_else(|| {
        pumas_library::PumasError::Config {
            message: "HuggingFace client not initialized".to_string(),
        }
    })?;
    let pending_notification = Some(
        state
            .api
            .hf_download_notification_since(cursor.as_deref())
            .await,
    );

    Ok(ModelDownloadUpdateStreamState {
        state,
        receiver,
        pending_notification,
    })
}

/// Server-sent runtime-profile update notification stream.
#[cfg(feature = "inference-plugins")]
pub async fn handle_runtime_profile_update_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<RuntimeProfileUpdateStreamQuery>,
) -> Sse<BoxStream<'static, Result<Event, Infallible>>> {
    let stream_state = build_runtime_profile_update_stream_state(state, query.cursor).await;
    let stream = stream::unfold(stream_state, next_runtime_profile_update_event).boxed();

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[cfg(feature = "inference-plugins")]
async fn build_runtime_profile_update_stream_state(
    state: Arc<AppState>,
    cursor: Option<String>,
) -> RuntimeProfileUpdateStreamState {
    let receiver = state.api.subscribe_runtime_profile_updates();
    let pending_feed = match state
        .api
        .list_runtime_profile_updates_since(cursor.as_deref())
        .await
    {
        Ok(response)
            if !response.feed.events.is_empty()
                || response.feed.stale_cursor
                || response.feed.snapshot_required =>
        {
            Some(response.feed)
        }
        Ok(_) => None,
        Err(error) => {
            let public_error = PublicError::from(&error);
            warn!(
                error_code = public_error.code,
                error_class = public_error.class.as_str(),
                "runtime-profile update stream startup recovery failed"
            );
            Some(RuntimeProfileUpdateFeed::snapshot_required(
                "runtime-profiles:0".to_string(),
            ))
        }
    };

    RuntimeProfileUpdateStreamState {
        receiver,
        pending_feed,
    }
}

/// Server-sent serving-status update notification stream.
#[cfg(feature = "inference-plugins")]
pub async fn handle_serving_status_update_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<ServingStatusUpdateStreamQuery>,
) -> Sse<BoxStream<'static, Result<Event, Infallible>>> {
    let stream_state = build_serving_status_update_stream_state(state, query.cursor).await;
    let stream = stream::unfold(stream_state, next_serving_status_update_event).boxed();

    Sse::new(stream).keep_alive(KeepAlive::default())
}

#[cfg(feature = "inference-plugins")]
async fn build_serving_status_update_stream_state(
    state: Arc<AppState>,
    cursor: Option<String>,
) -> ServingStatusUpdateStreamState {
    let receiver = state.api.subscribe_serving_status_updates();
    let pending_feed = match state
        .api
        .list_serving_status_updates_since(cursor.as_deref())
        .await
    {
        Ok(response)
            if !response.feed.events.is_empty()
                || response.feed.stale_cursor
                || response.feed.snapshot_required =>
        {
            Some(response.feed)
        }
        Ok(_) => None,
        Err(error) => {
            let public_error = PublicError::from(&error);
            warn!(
                error_code = public_error.code,
                error_class = public_error.class.as_str(),
                "serving-status update stream startup recovery failed"
            );
            Some(ServingStatusUpdateFeed::snapshot_required(
                "serving:0".to_string(),
            ))
        }
    };

    ServingStatusUpdateStreamState {
        receiver,
        pending_feed,
    }
}

/// Server-sent status/resource telemetry update stream.
pub async fn handle_status_telemetry_update_events(
    State(state): State<Arc<AppState>>,
    Query(query): Query<StatusTelemetryUpdateStreamQuery>,
) -> Sse<BoxStream<'static, Result<Event, Infallible>>> {
    let stream: BoxStream<'static, Result<Event, Infallible>> =
        match build_status_telemetry_update_stream_state(state, query.cursor).await {
            Ok(stream_state) => {
                stream::unfold(stream_state, next_status_telemetry_update_event).boxed()
            }
            Err(error) => {
                let public_error = PublicError::from(&error);
                warn!(
                    error_code = public_error.code,
                    error_class = public_error.class.as_str(),
                    "status telemetry update stream startup failed"
                );
                stream::once(async move {
                    Ok(Event::default()
                        .event("status-telemetry-error")
                        .data(public_error_event(public_error)))
                })
                .boxed()
            }
        };

    Sse::new(stream).keep_alive(KeepAlive::default())
}

/// Main JSON-RPC handler.
pub async fn handle_rpc(State(state): State<Arc<AppState>>, body: Bytes) -> impl IntoResponse {
    let request = match AdmittedRpcRequest::decode(&body) {
        Ok(request) => request,
        Err(admission) => {
            warn!(
                error_code = admission.error.code,
                error_class = admission.error.class.as_str(),
                "RPC request admission failed"
            );
            return (
                StatusCode::OK,
                Json(JsonRpcResponse::error(admission.id, admission.error)),
            );
        }
    };
    let id = request.id;
    let method = request.command.method().to_string();

    debug!(
        rpc_method = diagnostic_method(&method),
        request_id = ?diagnostic_request_id(id.as_ref()),
        "RPC call received"
    );

    let result = dispatch_admitted_command(&state, request.command).await;

    match result {
        Ok(outcome) => {
            let uses_response_wrapper = outcome.uses_response_wrapper();
            let value = match outcome.into_value() {
                Ok(value) => value,
                Err(public_error) => {
                    error!(
                        rpc_method = diagnostic_method(&method),
                        request_id = ?diagnostic_request_id(id.as_ref()),
                        error_code = public_error.code,
                        error_class = public_error.class.as_str(),
                        "RPC outcome serialization failed"
                    );
                    return (
                        StatusCode::OK,
                        Json(JsonRpcResponse::error(id, public_error)),
                    );
                }
            };
            let value = if uses_response_wrapper {
                wrap_response(&method, value)
            } else {
                value
            };
            (StatusCode::OK, Json(JsonRpcResponse::success(id, value)))
        }
        Err(error) => {
            let public_error = match error {
                RpcDispatchError::Domain(error) => PublicError::from(&error),
                RpcDispatchError::MethodNotFound => PublicError::method_not_found(),
            };
            error!(
                rpc_method = diagnostic_method(&method),
                request_id = ?diagnostic_request_id(id.as_ref()),
                error_code = public_error.code,
                error_class = public_error.class.as_str(),
                "RPC call failed"
            );
            (
                StatusCode::OK,
                Json(JsonRpcResponse::error(id, public_error)),
            )
        }
    }
}

enum RpcDispatchError {
    Domain(pumas_library::PumasError),
    MethodNotFound,
}

impl From<pumas_library::PumasError> for RpcDispatchError {
    fn from(error: pumas_library::PumasError) -> Self {
        Self::Domain(error)
    }
}

async fn dispatch_admitted_command(
    state: &AppState,
    command: RpcCommand,
) -> Result<RpcOutcome, RpcDispatchError> {
    let result: pumas_library::Result<RpcOutcome> = match command {
        RpcCommand::HealthCheck => Ok(RpcOutcome::Health(HealthOutcome::ok())),
        RpcCommand::Shutdown => shutdown_result(state).await,
        RpcCommand::GetStatus => status::get_status(state)
            .await
            .map(Box::new)
            .map(RpcOutcome::Status),
        RpcCommand::GetDiskSpace => status::get_disk_space(state)
            .await
            .map(Box::new)
            .map(RpcOutcome::DiskSpace),
        RpcCommand::GetSystemResources => status::get_system_resources(state)
            .await
            .map(Box::new)
            .map(RpcOutcome::SystemResources),
        RpcCommand::GetStatusTelemetrySnapshot => status::get_status_telemetry_snapshot(state)
            .await
            .map(Box::new)
            .map(RpcOutcome::StatusTelemetry),
        RpcCommand::GetLauncherVersion => status::get_launcher_version(state)
            .await
            .map(Box::new)
            .map(RpcOutcome::LauncherVersion),
        RpcCommand::CheckLauncherUpdates { force_refresh } => {
            status::check_launcher_updates(state, force_refresh)
                .await
                .map(Box::new)
                .map(RpcOutcome::LauncherUpdateCheck)
        }
        RpcCommand::ApplyLauncherUpdate => status::apply_launcher_update(state)
            .await
            .map(Box::new)
            .map(RpcOutcome::LauncherUpdateApply),
        RpcCommand::RestartLauncher => Ok(RpcOutcome::OperationStatus(
            status::restart_launcher(state).await,
        )),
        RpcCommand::GetSandboxInfo => Ok(RpcOutcome::Sandbox(status::get_sandbox_info().await)),
        RpcCommand::CheckGit => Ok(RpcOutcome::Git(Box::new(status::check_git(state).await))),
        RpcCommand::GetNetworkStatus => Ok(RpcOutcome::Network(Box::new(
            status::get_network_status(state).await,
        ))),
        RpcCommand::GetLibraryStatus => status::get_library_status(state)
            .await
            .map(Box::new)
            .map(RpcOutcome::Library),
        #[cfg(feature = "inference-plugins")]
        RpcCommand::GetAppStatus { app_id } => Ok(RpcOutcome::AppStatus(
            status::get_app_status(state, &app_id).await,
        )),
        RpcCommand::SetHfToken { token } => {
            state.api.set_hf_token(token.expose()).await?;
            Ok(RpcOutcome::HfTokenMutation(SuccessOutcome::new()))
        }
        RpcCommand::ClearHfToken => {
            state.api.clear_hf_token().await?;
            Ok(RpcOutcome::HfTokenMutation(SuccessOutcome::new()))
        }
        RpcCommand::GetHfAuthStatus => {
            let status = state.api.get_hf_auth_status().await?;
            Ok(RpcOutcome::HfAuth(Box::new(HfAuthOutcome::from(status))))
        }
        RpcCommand::GetLinkHealth { version_tag } => {
            links::get_link_health(state, version_tag.as_deref())
                .await
                .map(Box::new)
                .map(RpcOutcome::LinkHealth)
        }
        RpcCommand::CleanBrokenLinks => links::clean_broken_links(state)
            .await
            .map(RpcOutcome::CleanBrokenLinks),
        RpcCommand::RemoveOrphanedLinks { version_tag } => {
            links::remove_orphaned_links(state, &version_tag)
                .await
                .map(RpcOutcome::RemoveOrphanedLinks)
        }
        RpcCommand::GetLinksForModel { model_id } => links::get_links_for_model(state, &model_id)
            .await
            .map(Box::new)
            .map(RpcOutcome::LinksForModel),
        RpcCommand::DeleteModelWithCascade { model_id } => {
            links::delete_model_with_cascade(state, &model_id)
                .await
                .map(RpcOutcome::DeleteModel)
        }
        RpcCommand::GetFileLinkCount { file_path } => links::get_file_link_count(file_path)
            .await
            .map(RpcOutcome::FileLinkCount),
        RpcCommand::CheckFilesWritable { file_paths } => links::check_files_writable(file_paths)
            .await
            .map(Box::new)
            .map(RpcOutcome::FilesWritable),
        RpcCommand::SetModelLinkExclusion {
            model_id,
            app_id,
            excluded,
        } => links::set_model_link_exclusion(state, &model_id, &app_id, excluded)
            .await
            .map(RpcOutcome::LinkExclusionMutation),
        RpcCommand::GetLinkExclusions { app_id } => links::get_link_exclusions(state, &app_id)
            .await
            .map(Box::new)
            .map(RpcOutcome::LinkExclusions),
        RpcCommand::StartModelConversion { request } => {
            conversion::start_model_conversion(state, request)
                .await
                .map(RpcOutcome::ConversionStarted)
        }
        RpcCommand::GetConversionProgress { conversion_id } => {
            conversion::get_conversion_progress(state, &conversion_id)
                .map(Box::new)
                .map(RpcOutcome::ConversionProgress)
        }
        RpcCommand::CancelModelConversion { conversion_id } => {
            conversion::cancel_model_conversion(state, &conversion_id)
                .await
                .map(RpcOutcome::ConversionCancelled)
        }
        RpcCommand::ListModelConversions => conversion::list_model_conversions(state)
            .map(Box::new)
            .map(RpcOutcome::ConversionList),
        RpcCommand::CheckConversionEnvironment => conversion::check_conversion_environment(state)
            .await
            .map(RpcOutcome::ConversionEnvironment),
        RpcCommand::SetupConversionEnvironment => {
            conversion::setup_conversion_environment(state).await?;
            Ok(RpcOutcome::ConversionMutation(SuccessOutcome::new()))
        }
        RpcCommand::GetSupportedQuantTypes => conversion::get_supported_quant_types(state)
            .await
            .map(Box::new)
            .map(RpcOutcome::SupportedQuantTypes),
        RpcCommand::GetBackendStatus => conversion::get_backend_status(state)
            .await
            .map(Box::new)
            .map(RpcOutcome::BackendStatus),
        RpcCommand::SetupQuantizationBackend { backend } => {
            conversion::setup_quantization_backend(state, backend).await?;
            Ok(RpcOutcome::ConversionMutation(SuccessOutcome::new()))
        }
        RpcCommand::OpenPath { path } => process::open_path(state, path)
            .await
            .map(RpcOutcome::OperationStatus),
        RpcCommand::OpenUrl { url } => process::open_url(state, url)
            .await
            .map(RpcOutcome::OperationStatus),
        RpcCommand::DownloadModelFromHf { request } => {
            models::download_model_from_hf(state, request)
                .await
                .map(RpcOutcome::DownloadStarted)
        }
        RpcCommand::StartModelDownloadFromHf { request } => {
            models::start_model_download_from_hf(state, request)
                .await
                .map(RpcOutcome::DownloadStarted)
        }
        RpcCommand::GetModelDownloadStatus { download_id } => {
            models::get_model_download_status(state, &download_id)
                .await
                .map(Box::new)
                .map(RpcOutcome::DownloadStatus)
        }
        RpcCommand::CancelModelDownload { download_id } => {
            models::cancel_model_download(state, &download_id)
                .await
                .map(RpcOutcome::DownloadMutation)
        }
        RpcCommand::PauseModelDownload { download_id } => {
            models::pause_model_download(state, &download_id)
                .await
                .map(RpcOutcome::DownloadMutation)
        }
        RpcCommand::ResumeModelDownload { download_id } => {
            models::resume_model_download(state, &download_id)
                .await
                .map(RpcOutcome::DownloadMutation)
        }
        RpcCommand::ListModelDownloads => models::list_model_downloads(state)
            .await
            .map(Box::new)
            .map(RpcOutcome::DownloadList),
        RpcCommand::ResumePartialDownload {
            model_id,
            recovery_token,
        } => models::resume_partial_download(state, &model_id, &recovery_token)
            .await
            .map(Box::new)
            .map(RpcOutcome::PartialDownload),
        RpcCommand::GetModels => models::get_models(state)
            .await
            .map(Box::new)
            .map(RpcOutcome::Models),
        RpcCommand::SearchCatalog {
            query,
            limit,
            offset,
        } => models::search_models_fts(state, &query, limit, offset)
            .await
            .map(Box::new)
            .map(RpcOutcome::CatalogSearch),
        RpcCommand::RefreshModelIndex => models::refresh_model_index(state)
            .await
            .map(RpcOutcome::ModelIndexRefresh),
        RpcCommand::Legacy { method, params } => {
            return dispatch_method(state, &method, &params)
                .await
                .map(RpcOutcome::Legacy);
        }
    };
    result.map_err(RpcDispatchError::Domain)
}

async fn shutdown_result(state: &AppState) -> pumas_library::Result<RpcOutcome> {
    #[cfg(not(feature = "inference-plugins"))]
    {
        let _ = state;
        Ok(RpcOutcome::Shutdown(ShutdownOutcome::core_only()))
    }

    #[cfg(feature = "inference-plugins")]
    {
        let shutdown_summary = match state.api.stop_all_managed_runtime_profiles().await {
            Ok(summary) => summary,
            Err(error) => {
                let public_error = PublicError::from(&error);
                warn!(
                    error_code = public_error.code,
                    error_class = public_error.class.as_str(),
                    "managed runtime shutdown failed before backend exit"
                );
                return Ok(RpcOutcome::Shutdown(ShutdownOutcome::managed(0, 0, 1)));
            }
        };
        Ok(RpcOutcome::Shutdown(ShutdownOutcome::managed(
            shutdown_summary.profiles_processed,
            shutdown_summary.processes_stopped,
            shutdown_summary.errors.len(),
        )))
    }
}

fn diagnostic_method(method: &str) -> &'static str {
    match method {
        "health_check" => "health_check",
        "shutdown" => "shutdown",
        "set_hf_token" => "set_hf_token",
        "open_path" => "open_path",
        "open_url" => "open_url",
        _ => "other_or_unsupported",
    }
}

fn diagnostic_request_id(id: Option<&Value>) -> Option<u64> {
    id.and_then(Value::as_u64)
}

fn public_error_event(error: PublicError) -> String {
    json!({
        "error": error.message,
        "error_code": error.code,
        "error_class": error.class,
    })
    .to_string()
}

struct ModelLibraryUpdateStreamState {
    subscriber: pumas_library::model_library::ModelLibraryUpdateSubscriber,
    cursor: String,
    pending_notification: Option<ModelLibraryUpdateNotification>,
}

struct ModelDownloadUpdateStreamState {
    state: Arc<AppState>,
    receiver: broadcast::Receiver<ModelDownloadUpdateNotification>,
    pending_notification: Option<ModelDownloadUpdateNotification>,
}

async fn next_model_library_update_event(
    mut state: ModelLibraryUpdateStreamState,
) -> Option<(Result<Event, Infallible>, ModelLibraryUpdateStreamState)> {
    if let Some(notification) = state.pending_notification.take() {
        let event = model_library_update_sse_event(&notification);
        return Some((Ok(event), state));
    }

    match state.subscriber.next_event().await {
        Ok(update) => {
            state.cursor = update.cursor.clone();
            let notification = ModelLibraryUpdateNotification {
                cursor: update.cursor.clone(),
                events: vec![update],
                stale_cursor: false,
                snapshot_required: false,
            };
            let event = model_library_update_sse_event(&notification);
            Some((Ok(event), state))
        }
        Err(error) => {
            let public_error = PublicError::from(&error);
            warn!(
                cursor = %state.cursor,
                error_code = public_error.code,
                error_class = public_error.class.as_str(),
                "model-library update stream ended"
            );
            None
        }
    }
}

fn model_library_update_sse_event(notification: &ModelLibraryUpdateNotification) -> Event {
    match serde_json::to_string(notification) {
        Ok(payload) => Event::default().event("model-library-update").data(payload),
        Err(_) => Event::default()
            .event("model-library-error")
            .data(public_error_event(PublicError::internal())),
    }
}

async fn next_model_download_update_event(
    mut state: ModelDownloadUpdateStreamState,
) -> Option<(Result<Event, Infallible>, ModelDownloadUpdateStreamState)> {
    if let Some(notification) = state.pending_notification.take() {
        let event = model_download_update_sse_event(&notification);
        return Some((Ok(event), state));
    }

    match state.receiver.recv().await {
        Ok(notification) => {
            let event = model_download_update_sse_event(&notification);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
            let notification = state.state.api.hf_download_notification_since(None).await;
            let event = model_download_update_sse_event(&notification);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

fn model_download_update_sse_event(notification: &ModelDownloadUpdateNotification) -> Event {
    match crate::contract::project_download_notification(notification)
        .and_then(|value| serde_json::to_string(&value).map_err(Into::into))
    {
        Ok(payload) => Event::default()
            .event("model-download-update")
            .data(payload),
        Err(_) => Event::default()
            .event("model-download-error")
            .data(public_error_event(PublicError::internal())),
    }
}

#[cfg(feature = "inference-plugins")]
struct RuntimeProfileUpdateStreamState {
    receiver: broadcast::Receiver<RuntimeProfileUpdateFeed>,
    pending_feed: Option<RuntimeProfileUpdateFeed>,
}

#[cfg(feature = "inference-plugins")]
struct ServingStatusUpdateStreamState {
    receiver: broadcast::Receiver<ServingStatusUpdateFeed>,
    pending_feed: Option<ServingStatusUpdateFeed>,
}

struct StatusTelemetryUpdateStreamState {
    state: Arc<AppState>,
    receiver: broadcast::Receiver<StatusTelemetryUpdateNotification>,
    pending_notification: Option<StatusTelemetryUpdateNotification>,
}

async fn build_status_telemetry_update_stream_state(
    state: Arc<AppState>,
    cursor: Option<String>,
) -> pumas_library::Result<StatusTelemetryUpdateStreamState> {
    let receiver = state.api.subscribe_status_telemetry_updates();
    let snapshot = state.api.get_status_telemetry_snapshot().await?;
    let pending_notification = state
        .api
        .status_telemetry_notification_since(cursor.as_deref(), snapshot);

    Ok(StatusTelemetryUpdateStreamState {
        state,
        receiver,
        pending_notification,
    })
}

async fn next_status_telemetry_update_event(
    mut state: StatusTelemetryUpdateStreamState,
) -> Option<(Result<Event, Infallible>, StatusTelemetryUpdateStreamState)> {
    if let Some(notification) = state.pending_notification.take() {
        let event = status_telemetry_update_sse_event(&notification);
        return Some((Ok(event), state));
    }

    match state.receiver.recv().await {
        Ok(notification) => {
            let event = status_telemetry_update_sse_event(&notification);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
            match state.state.api.get_status_telemetry_snapshot().await {
                Ok(snapshot) => {
                    let notification = StatusTelemetryUpdateNotification {
                        cursor: snapshot.cursor.clone(),
                        snapshot,
                        stale_cursor: true,
                        snapshot_required: true,
                    };
                    let event = status_telemetry_update_sse_event(&notification);
                    Some((Ok(event), state))
                }
                Err(error) => {
                    let public_error = PublicError::from(&error);
                    warn!(
                        error_code = public_error.code,
                        error_class = public_error.class.as_str(),
                        "status telemetry refresh after lag failed"
                    );
                    None
                }
            }
        }
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

fn status_telemetry_update_sse_event(notification: &StatusTelemetryUpdateNotification) -> Event {
    match serde_json::to_string(notification) {
        Ok(payload) => Event::default()
            .event("status-telemetry-update")
            .data(payload),
        Err(_) => Event::default()
            .event("status-telemetry-error")
            .data(public_error_event(PublicError::internal())),
    }
}

#[cfg(feature = "inference-plugins")]
async fn next_runtime_profile_update_event(
    mut state: RuntimeProfileUpdateStreamState,
) -> Option<(Result<Event, Infallible>, RuntimeProfileUpdateStreamState)> {
    if let Some(feed) = state.pending_feed.take() {
        let event = runtime_profile_update_sse_event(&feed);
        return Some((Ok(event), state));
    }

    match state.receiver.recv().await {
        Ok(feed) => {
            let event = runtime_profile_update_sse_event(&feed);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
            let feed =
                RuntimeProfileUpdateFeed::snapshot_required("runtime-profiles:0".to_string());
            let event = runtime_profile_update_sse_event(&feed);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

#[cfg(feature = "inference-plugins")]
fn runtime_profile_update_sse_event(feed: &RuntimeProfileUpdateFeed) -> Event {
    match serde_json::to_string(feed) {
        Ok(payload) => Event::default()
            .event("runtime-profile-update")
            .data(payload),
        Err(_) => Event::default()
            .event("runtime-profile-error")
            .data(public_error_event(PublicError::internal())),
    }
}

#[cfg(feature = "inference-plugins")]
async fn next_serving_status_update_event(
    mut state: ServingStatusUpdateStreamState,
) -> Option<(Result<Event, Infallible>, ServingStatusUpdateStreamState)> {
    if let Some(feed) = state.pending_feed.take() {
        let event = serving_status_update_sse_event(&feed);
        return Some((Ok(event), state));
    }

    match state.receiver.recv().await {
        Ok(feed) => {
            let event = serving_status_update_sse_event(&feed);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Lagged(_)) => {
            let feed = ServingStatusUpdateFeed::snapshot_required("serving:0".to_string());
            let event = serving_status_update_sse_event(&feed);
            Some((Ok(event), state))
        }
        Err(broadcast::error::RecvError::Closed) => None,
    }
}

#[cfg(feature = "inference-plugins")]
fn serving_status_update_sse_event(feed: &ServingStatusUpdateFeed) -> Event {
    match serde_json::to_string(feed) {
        Ok(payload) => Event::default()
            .event("serving-status-update")
            .data(payload),
        Err(_) => Event::default()
            .event("serving-status-error")
            .data(public_error_event(PublicError::internal())),
    }
}

// ============================================================================
// Method dispatcher
// ============================================================================

/// Dispatch a method call to the appropriate domain handler.
async fn dispatch_method(
    state: &AppState,
    method: &str,
    params: &Value,
) -> Result<Value, RpcDispatchError> {
    let result: pumas_library::Result<Value> = match method {
        // Local Runtime Profiles
        #[cfg(feature = "inference-plugins")]
        "get_runtime_profiles_snapshot" => {
            runtime_profiles::get_runtime_profiles_snapshot(state, params).await
        }
        #[cfg(feature = "inference-plugins")]
        "list_runtime_profile_updates_since" => {
            runtime_profiles::list_runtime_profile_updates_since(state, params).await
        }
        #[cfg(feature = "inference-plugins")]
        "upsert_runtime_profile" => runtime_profiles::upsert_runtime_profile(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "delete_runtime_profile" => runtime_profiles::delete_runtime_profile(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "set_model_runtime_route" => runtime_profiles::set_model_runtime_route(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "clear_model_runtime_route" => {
            runtime_profiles::clear_model_runtime_route(state, params).await
        }
        #[cfg(feature = "inference-plugins")]
        "launch_runtime_profile" => runtime_profiles::launch_runtime_profile(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "stop_runtime_profile" => runtime_profiles::stop_runtime_profile(state, params).await,

        // User-Directed Serving
        #[cfg(feature = "inference-plugins")]
        "get_serving_status" => serving::get_serving_status(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "list_serving_status_updates_since" => {
            serving::list_serving_status_updates_since(state, params).await
        }
        #[cfg(feature = "inference-plugins")]
        "validate_model_serving_config" => {
            serving::validate_model_serving_config(state, params).await
        }
        #[cfg(feature = "inference-plugins")]
        "serve_model" => serving::serve_model(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "unserve_model" => serving::unserve_model(state, params).await,

        // Version Management
        #[cfg(feature = "inference-plugins")]
        "get_available_versions" => versions::get_available_versions(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "get_installed_versions" => versions::get_installed_versions(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "get_active_version" => versions::get_active_version(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "get_default_version" => versions::get_default_version(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "set_default_version" => versions::set_default_version(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "switch_version" => versions::switch_version(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "install_version" => versions::install_version(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "remove_version" => versions::remove_version(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "cancel_installation" => versions::cancel_installation(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "get_installation_progress" => versions::get_installation_progress(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "validate_installations" => versions::validate_installations(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "get_version_status" => versions::get_version_status(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "get_version_info" => versions::get_version_info(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "get_release_size_info" => versions::get_release_size_info(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "get_release_size_breakdown" => versions::get_release_size_breakdown(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "calculate_release_size" => versions::calculate_release_size(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "calculate_all_release_sizes" => versions::calculate_all_release_sizes(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "has_background_fetch_completed" => {
            versions::has_background_fetch_completed(state, params).await
        }
        #[cfg(feature = "inference-plugins")]
        "reset_background_fetch_flag" => versions::reset_background_fetch_flag(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "get_github_cache_status" => versions::get_github_cache_status(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "check_version_dependencies" => versions::check_version_dependencies(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "install_version_dependencies" => {
            versions::install_version_dependencies(state, params).await
        }
        #[cfg(feature = "inference-plugins")]
        "get_release_dependencies" => versions::get_release_dependencies(state, params).await,

        // Model Library
        "import_model" => models::import_model(state, params).await,
        "search_hf_models" => models::search_hf_models(state, params).await,
        "get_hf_download_details" => models::get_hf_download_details(state, params).await,
        "get_related_models" => models::get_related_models(state, params).await,
        "import_batch" => models::import_batch(state, params).await,
        "import_external_diffusers_directory" => {
            models::import_external_diffusers_directory(state, params).await
        }
        "classify_model_import_paths" => models::classify_model_import_paths(state, params).await,
        "lookup_hf_metadata_for_file" => models::lookup_hf_metadata_for_file(state, params).await,
        "lookup_hf_metadata_for_bundle_directory" => {
            models::lookup_hf_metadata_for_bundle_directory(state, params).await
        }
        "detect_sharded_sets" => models::detect_sharded_sets(state, params).await,
        "validate_file_type" => models::validate_file_type(state, params).await,
        "get_embedded_metadata" => models::get_embedded_metadata(state, params).await,
        "get_library_model_metadata" => models::get_library_model_metadata(state, params).await,
        "resolve_model_execution_descriptor" => {
            models::resolve_model_execution_descriptor(state, params).await
        }
        "resolve_model_artifact_load_target" => {
            models::resolve_model_artifact_load_target(state, params).await
        }
        "resolve_model_package_facts" => models::resolve_model_package_facts(state, params).await,
        "list_model_library_updates_since" => {
            models::list_model_library_updates_since(state, params).await
        }
        "resolve_model_package_facts_summary" => {
            models::resolve_model_package_facts_summary(state, params).await
        }
        "model_package_facts_summary_snapshot" => {
            models::model_package_facts_summary_snapshot(state, params).await
        }
        "refetch_model_metadata_from_hf" => {
            models::refetch_model_metadata_from_hf(state, params).await
        }
        "adopt_orphan_models" => models::adopt_orphan_models(state, params).await,
        "import_model_in_place" => models::import_model_in_place(state, params).await,
        "scan_shared_storage" => models::scan_shared_storage(state, params).await,

        // Inference Settings
        "get_inference_settings" => models::get_inference_settings(state, params).await,
        "update_inference_settings" => models::update_inference_settings(state, params).await,
        "update_model_notes" => models::update_model_notes(state, params).await,
        "resolve_model_dependency_requirements" => {
            models::resolve_model_dependency_requirements(state, params).await
        }
        "audit_dependency_pin_compliance" => {
            models::audit_dependency_pin_compliance(state, params).await
        }
        "list_models_needing_review" => models::list_models_needing_review(state, params).await,
        "submit_model_review" => models::submit_model_review(state, params).await,
        "reset_model_review" => models::reset_model_review(state, params).await,
        "generate_model_migration_dry_run_report" => {
            models::generate_model_migration_dry_run_report(state, params).await
        }
        "execute_model_migration" => models::execute_model_migration(state, params).await,
        "list_model_migration_reports" => models::list_model_migration_reports(state, params).await,
        "delete_model_migration_report" => {
            models::delete_model_migration_report(state, params).await
        }
        "prune_model_migration_reports" => {
            models::prune_model_migration_reports(state, params).await
        }

        // Process Management
        #[cfg(feature = "inference-plugins")]
        "launch_ollama" => process::launch_ollama(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "stop_ollama" => process::stop_ollama(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "is_ollama_running" => process::is_ollama_running(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "launch_torch" => process::launch_torch(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "stop_torch" => process::stop_torch(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "is_torch_running" => process::is_torch_running(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "open_active_install" => process::open_active_install(state, params).await,

        // Ollama Model Management
        #[cfg(feature = "inference-plugins")]
        "ollama_list_models" => ollama::ollama_list_models(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "ollama_list_models_for_profile" => {
            ollama::ollama_list_models_for_profile(state, params).await
        }
        #[cfg(feature = "inference-plugins")]
        "ollama_create_model" => ollama::ollama_create_model(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "ollama_create_model_for_profile" => {
            ollama::ollama_create_model_for_profile(state, params).await
        }
        #[cfg(feature = "inference-plugins")]
        "ollama_delete_model" => ollama::ollama_delete_model(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "ollama_delete_model_for_profile" => {
            ollama::ollama_delete_model_for_profile(state, params).await
        }
        #[cfg(feature = "inference-plugins")]
        "ollama_load_model" => ollama::ollama_load_model(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "ollama_load_model_for_profile" => {
            ollama::ollama_load_model_for_profile(state, params).await
        }
        #[cfg(feature = "inference-plugins")]
        "ollama_unload_model" => ollama::ollama_unload_model(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "ollama_unload_model_for_profile" => {
            ollama::ollama_unload_model_for_profile(state, params).await
        }
        #[cfg(feature = "inference-plugins")]
        "ollama_list_running" => ollama::ollama_list_running(state, params).await,

        // Torch Inference Server
        #[cfg(feature = "inference-plugins")]
        "torch_list_slots" => torch::torch_list_slots(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "torch_load_model" => torch::torch_load_model(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "torch_unload_model" => torch::torch_unload_model(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "torch_get_status" => torch::torch_get_status(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "torch_list_devices" => torch::torch_list_devices(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "torch_configure" => torch::torch_configure(state, params).await,

        // Plugins
        #[cfg(feature = "inference-plugins")]
        "get_plugins" => plugins::get_plugins(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "get_plugin" => plugins::get_plugin(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "call_plugin_endpoint" => plugins::call_plugin_endpoint(state, params).await,
        #[cfg(feature = "inference-plugins")]
        "check_plugin_health" => plugins::check_plugin_health(state, params).await,

        // Unknown method
        _ => {
            warn!("Unsupported RPC method requested");
            return Err(RpcDispatchError::MethodNotFound);
        }
    };
    result.map_err(RpcDispatchError::Domain)
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn conversion_remaining_rpc_outcomes_need_no_native_setup() {
        let temp = TempDir::new().unwrap();
        let state = Arc::new(test_support::build_test_app_state(temp.path()).await);
        for (method, params) in [
            (
                "cancel_model_conversion",
                json!({"conversion_id":"missing"}),
            ),
            ("get_supported_quant_types", json!({})),
            ("get_backend_status", json!({})),
        ] {
            let body = Bytes::from(
                serde_json::to_vec(
                    &json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}),
                )
                .unwrap(),
            );
            let response = handle_rpc(State(state.clone()), body).await.into_response();
            let bytes = axum::body::to_bytes(response.into_body(), 65_536)
                .await
                .unwrap();
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["result"]["success"], true, "{method}: {value}");
            match method {
                "cancel_model_conversion" => assert_eq!(value["result"]["cancelled"], false),
                "get_supported_quant_types" => {
                    let options = value["result"]["quant_types"].as_array().unwrap();
                    assert_eq!(options.len(), 1);
                    assert_eq!(options[0]["name"], "F16");
                    assert_eq!(options[0]["bitsPerWeight"], 16.0);
                    assert_eq!(options[0]["backend"], "python_conversion");
                    assert_eq!(options[0]["imatrixRecommended"], false);
                }
                "get_backend_status" => {
                    let backends = value["result"]["backends"].as_array().unwrap();
                    assert_eq!(backends.len(), 3);
                    assert!(backends.iter().all(|backend| backend["ready"] == false));
                }
                _ => unreachable!(),
            }
        }
        assert!(!temp.path().join("launcher-data/llama-cpp").exists());
    }

    #[tokio::test]
    async fn conversion_read_rpc_preserves_missing_progress_and_empty_list() {
        let temp = TempDir::new().unwrap();
        let state = Arc::new(test_support::build_test_app_state(temp.path()).await);
        for (method, params, expected) in [
            (
                "get_conversion_progress",
                json!({"conversion_id":"missing"}),
                json!({"success":true,"progress":null}),
            ),
            (
                "list_model_conversions",
                json!({}),
                json!({"success":true,"conversions":[]}),
            ),
        ] {
            let body = Bytes::from(
                serde_json::to_vec(
                    &json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}),
                )
                .unwrap(),
            );
            let response = handle_rpc(State(state.clone()), body).await.into_response();
            let bytes = axum::body::to_bytes(response.into_body(), 16_384)
                .await
                .unwrap();
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["result"], expected);
            assert!(value.get("error").is_none());
        }
    }

    #[tokio::test]
    async fn link_health_rpc_preserves_headless_read_and_failure_contracts() {
        use pumas_library::model_library::{LinkEntry, LinkType};
        let temp = TempDir::new().unwrap();
        let state = Arc::new(test_support::build_test_app_state(temp.path()).await);
        let request = |params| {
            Bytes::from(
                serde_json::to_vec(&json!({
                    "jsonrpc": "2.0", "id": 1, "method": "get_link_health", "params": params,
                }))
                .unwrap(),
            )
        };
        for params in [json!({}), json!({"version_tag": "currently-unfiltered"})] {
            let response = handle_rpc(State(state.clone()), request(params))
                .await
                .into_response();
            let bytes = axum::body::to_bytes(response.into_body(), 16_384)
                .await
                .unwrap();
            let value: Value = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(value["result"]["success"], true);
            assert_eq!(value["result"]["status"], "healthy");
            assert_eq!(value["result"]["total_links"], 0);
            assert!(value.get("error").is_none());
        }
        let response = handle_rpc(State(state.clone()), request(json!({"unexpected": true})))
            .await
            .into_response();
        let bytes = axum::body::to_bytes(response.into_body(), 16_384)
            .await
            .unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["error"]["code"], -32602);

        let blocker = temp.path().join("private-inspection-blocker");
        std::fs::write(&blocker, b"preserve").unwrap();
        state
            .api
            .model_library()
            .link_registry()
            .read()
            .await
            .register(LinkEntry {
                model_id: "llm/example/model".into(),
                source: blocker.clone(),
                target: blocker.join("child"),
                link_type: LinkType::Copy,
                created_at: "2026-09-06T00:00:00Z".into(),
                app_id: "embedded-consumer".into(),
                app_version: None,
            })
            .await
            .unwrap();
        let response = handle_rpc(State(state.clone()), request(json!({})))
            .await
            .into_response();
        let bytes = axum::body::to_bytes(response.into_body(), 16_384)
            .await
            .unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(value["error"]["code"], -32603);
        assert!(value.get("result").is_none());
        assert!(!String::from_utf8(bytes.to_vec())
            .unwrap()
            .contains("private-inspection-blocker"));
        assert_eq!(std::fs::read(blocker).unwrap(), b"preserve");
    }

    #[tokio::test]
    async fn download_push_validates_library_identity_and_redacts_diagnostics() {
        for (model_id, event_name) in [
            ("llm/acme/model", "model-download-update"),
            ("../outside", "model-download-error"),
        ] {
            let notification: ModelDownloadUpdateNotification = serde_json::from_value(json!({
                "cursor": "download:1",
                "snapshot": {"cursor": "download:1", "revision": 1, "downloads": [{
                    "downloadId": "push-fixture", "libraryModelId": model_id,
                    "status": "downloading", "error": "private diagnostic",
                }]},
                "stale_cursor": false, "snapshot_required": true,
            }))
            .unwrap();
            let event = model_download_update_sse_event(&notification);
            let response = Sse::new(stream::iter([Ok::<_, Infallible>(event)])).into_response();
            let body = axum::body::to_bytes(response.into_body(), 16_384)
                .await
                .unwrap();
            let wire = std::str::from_utf8(&body).unwrap();
            assert!(wire.contains(event_name));
            assert!(!wire.contains("private diagnostic"));
            if event_name == "model-download-update" {
                assert!(wire.contains("\"libraryModelId\":\"llm/acme/model\""));
            } else {
                assert!(!wire.contains("../outside"));
                assert!(!wire.contains("model-download-update"));
            }
        }
    }

    #[test]
    fn test_json_rpc_response_success() {
        let response = JsonRpcResponse::success(Some(json!(1)), json!({"data": "test"}));
        assert!(response.error.is_none());
        assert!(response.result.is_some());
    }

    #[test]
    fn test_json_rpc_response_error() {
        let response = JsonRpcResponse::error(Some(json!(1)), PublicError::internal());
        assert!(response.error.is_some());
        assert!(response.result.is_none());
        assert_eq!(response.error.unwrap().code, -32603);
    }

    #[tokio::test]
    async fn test_detect_sandbox() {
        let (is_sandboxed, sandbox_type, _) = detect_sandbox_environment().await;
        // In normal development, we're not sandboxed
        // This test verifies the function runs without error
        assert!(!is_sandboxed || ["flatpak", "snap", "docker", "appimage"].contains(&sandbox_type));
    }

    #[tokio::test]
    async fn disabled_hf_download_commands_return_unavailable() {
        let temp_dir = TempDir::new().unwrap();
        let state = test_support::build_test_app_state(temp_dir.path()).await;

        for command in [
            RpcCommand::GetModelDownloadStatus {
                download_id: "missing".to_string(),
            },
            RpcCommand::CancelModelDownload {
                download_id: "missing".to_string(),
            },
            RpcCommand::PauseModelDownload {
                download_id: "missing".to_string(),
            },
            RpcCommand::ResumeModelDownload {
                download_id: "missing".to_string(),
            },
            RpcCommand::ListModelDownloads,
        ] {
            let error = match dispatch_admitted_command(&state, command).await {
                Err(RpcDispatchError::Domain(error)) => PublicError::from(&error),
                _ => panic!("disabled HF command did not return a domain error"),
            };
            assert_eq!(error.code, -32000);
            assert_eq!(error.class, crate::contract::PublicErrorClass::Unavailable);
            assert_eq!(
                error.message,
                "A required operation is currently unavailable."
            );
        }

        let partial = match dispatch_admitted_command(
            &state,
            RpcCommand::ResumePartialDownload {
                model_id: pumas_library::model_library::DownloadRecoveryModelId::parse(
                    "llm/acme/model",
                )
                .unwrap(),
                recovery_token: pumas_library::model_library::DownloadRecoveryToken::parse(
                    &format!("v1:{}", "a".repeat(64)),
                )
                .unwrap(),
            },
        )
        .await
        {
            Ok(outcome) => outcome.into_value().unwrap(),
            Err(_) => panic!("disabled partial recovery did not return its typed outcome"),
        };
        assert_eq!(
            partial,
            serde_json::json!({
                "success": false,
                "action": "none",
                "download_id": null,
                "status": null,
                "reason_code": "hf_client_unavailable",
                "error": "The partial download could not be resumed."
            })
        );
    }
}
