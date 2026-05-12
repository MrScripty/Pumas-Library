//! ONNX Runtime serving adapter used by the serving RPC boundary.

use super::serving::{
    effective_gateway_alias_from_config, non_critical_failure_response, serving_error,
};
use crate::server::AppState;
use pumas_library::models::{
    ModelServeErrorCode, RuntimeProfileId, RuntimeProviderId, ServeModelRequest,
    ServeModelResponse, ServedModelLoadState, ServedModelStatus, UnserveModelRequest,
    UnserveModelResponse,
};
use pumas_library::{
    ExecutableArtifactFormat, OnnxLoadOptions, OnnxLoadRequest, OnnxModelId, ProviderRegistry,
};
use serde_json::Value;
use tracing::warn;

pub(super) async fn serve_onnx_model(
    state: &AppState,
    request: ServeModelRequest,
) -> pumas_library::Result<Value> {
    let Some(onnx_path) = resolve_onnx_model_path(state, &request).await? else {
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::ModelNotExecutable,
                "model has no executable ONNX artifact",
                &request,
            ),
        )
        .await;
    };
    let library_root = state.api.model_library().library_root().to_path_buf();
    let provider_model_id = onnx_provider_request_model_id(&request, &state.provider_registry);
    if let Some(response) =
        existing_loaded_onnx_response(state, &request, provider_model_id.as_str()).await?
    {
        return Ok(serde_json::to_value(response)?);
    }
    let load_request = match OnnxLoadRequest::parse(
        library_root,
        &onnx_path,
        provider_model_id.as_str(),
        OnnxLoadOptions::default(),
    ) {
        Ok(load_request) => load_request,
        Err(error) => {
            warn!("ONNX serving load request validation failed: {}", error);
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::InvalidRequest,
                    "ONNX Runtime rejected the selected model load request",
                    &request,
                ),
            )
            .await;
        }
    };

    let onnx_model_id = load_request.model_id.clone();
    let session = match state.onnx_session_manager.load(load_request).await {
        Ok(session) => session,
        Err(error) => {
            warn!("ONNX fake session load failed: {}", error);
            return non_critical_failure_response(
                state,
                serving_error(
                    ModelServeErrorCode::ProviderLoadFailed,
                    "ONNX Runtime could not load the selected model",
                    &request,
                ),
            )
            .await;
        }
    };
    if let Err(error) = confirm_onnx_session_loaded(state, &onnx_model_id).await {
        warn!("ONNX fake session status confirmation failed: {}", error);
        return non_critical_failure_response(
            state,
            serving_error(
                ModelServeErrorCode::ProviderLoadFailed,
                "ONNX Runtime loaded the selected model but did not report it as available",
                &request,
            ),
        )
        .await;
    }

    let status = ServedModelStatus {
        model_id: request.model_id.clone(),
        model_alias: Some(effective_gateway_alias_from_config(&request)),
        provider: RuntimeProviderId::OnnxRuntime,
        profile_id: request.config.profile_id.clone(),
        load_state: ServedModelLoadState::Loaded,
        device_mode: request.config.device_mode,
        device_id: request.config.device_id.clone(),
        gpu_layers: request.config.gpu_layers,
        tensor_split: request.config.tensor_split.clone(),
        context_size: Some(session.embedding_dimensions as u32),
        keep_loaded: request.config.keep_loaded,
        endpoint_url: None,
        memory_bytes: None,
        loaded_at: None,
        last_error: None,
    };
    let snapshot = state.api.record_served_model(status.clone()).await?;

    Ok(serde_json::to_value(ServeModelResponse {
        success: true,
        error: None,
        loaded: true,
        loaded_models_unchanged: false,
        status: Some(status),
        load_error: None,
        snapshot: Some(snapshot),
    })?)
}

pub(super) async fn unserve_onnx_model(
    state: &AppState,
    request: UnserveModelRequest,
    profile_id: RuntimeProfileId,
    model_alias: String,
) -> pumas_library::Result<Value> {
    let model_id = match OnnxModelId::parse(&request.model_id) {
        Ok(model_id) => model_id,
        Err(error) => {
            warn!("ONNX serving unload request validation failed: {}", error);
            return Ok(serde_json::to_value(UnserveModelResponse {
                success: true,
                error: Some("ONNX Runtime rejected the selected model unload request".to_string()),
                unloaded: false,
                snapshot: Some(state.api.get_serving_status().await?.snapshot),
            })?);
        }
    };

    match state.onnx_session_manager.unload(&model_id).await {
        Ok(Some(_)) => {}
        Ok(None) => {
            let snapshot = state
                .api
                .record_unserved_model(
                    &request.model_id,
                    Some(RuntimeProviderId::OnnxRuntime),
                    Some(&profile_id),
                    Some(model_alias.as_str()),
                )
                .await?;
            return Ok(serde_json::to_value(UnserveModelResponse {
                success: true,
                error: None,
                unloaded: true,
                snapshot: Some(snapshot),
            })?);
        }
        Err(error) => {
            warn!("ONNX fake session unload failed: {}", error);
            return Ok(serde_json::to_value(UnserveModelResponse {
                success: true,
                error: Some("ONNX Runtime could not unload the selected model".to_string()),
                unloaded: false,
                snapshot: Some(state.api.get_serving_status().await?.snapshot),
            })?);
        }
    }

    let snapshot = state
        .api
        .record_unserved_model(
            &request.model_id,
            Some(RuntimeProviderId::OnnxRuntime),
            Some(&profile_id),
            Some(model_alias.as_str()),
        )
        .await?;
    Ok(serde_json::to_value(UnserveModelResponse {
        success: true,
        error: None,
        unloaded: true,
        snapshot: Some(snapshot),
    })?)
}

async fn resolve_onnx_model_path(
    state: &AppState,
    request: &ServeModelRequest,
) -> pumas_library::Result<Option<std::path::PathBuf>> {
    let library = state.api.model_library().clone();
    let model_id = request.model_id.clone();
    let primary_file =
        tokio::task::spawn_blocking(move || library.get_primary_model_file(&model_id))
            .await
            .map_err(|err| {
                pumas_library::PumasError::Other(format!(
                    "Failed to join primary ONNX model lookup task: {}",
                    err
                ))
            })?;
    let Some(onnx_path) = primary_file else {
        return Ok(None);
    };
    if ExecutableArtifactFormat::from_path(&onnx_path) != Some(ExecutableArtifactFormat::Onnx) {
        return Ok(None);
    }
    Ok(Some(onnx_path))
}

async fn confirm_onnx_session_loaded(
    state: &AppState,
    model_id: &OnnxModelId,
) -> Result<(), String> {
    let sessions = state
        .onnx_session_manager
        .list()
        .await
        .map_err(|error| error.to_string())?;
    if sessions
        .iter()
        .any(|session| session.model_id.as_str() == model_id.as_str())
    {
        return Ok(());
    }
    Err(format!(
        "ONNX model '{}' was absent from session list after load",
        model_id.as_str()
    ))
}

async fn existing_loaded_onnx_response(
    state: &AppState,
    request: &ServeModelRequest,
    provider_model_id: &str,
) -> pumas_library::Result<Option<ServeModelResponse>> {
    let Ok(provider_model_id) = OnnxModelId::parse(provider_model_id) else {
        return Ok(None);
    };
    let Some(status) = state
        .api
        .find_served_model(
            &request.model_id,
            Some(RuntimeProviderId::OnnxRuntime),
            Some(&request.config.profile_id),
        )
        .await?
    else {
        return Ok(None);
    };
    if status.load_state != ServedModelLoadState::Loaded
        || status.model_alias.as_deref()
            != Some(effective_gateway_alias_from_config(request).as_str())
        || confirm_onnx_session_loaded(state, &provider_model_id)
            .await
            .is_err()
    {
        return Ok(None);
    }
    Ok(Some(ServeModelResponse {
        success: true,
        error: None,
        loaded: true,
        loaded_models_unchanged: true,
        status: Some(status),
        load_error: None,
        snapshot: Some(state.api.get_serving_status().await?.snapshot),
    }))
}

fn onnx_provider_request_model_id(
    request: &ServeModelRequest,
    registry: &ProviderRegistry,
) -> String {
    let library_model_id = request.model_id.trim();
    registry
        .get(RuntimeProviderId::OnnxRuntime)
        .map(|behavior| {
            behavior
                .provider_request_model_id(library_model_id, request.config.model_alias.as_deref())
        })
        .unwrap_or_else(|| library_model_id.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider_clients::{LlamaCppRouterClient, OllamaClientFactory};
    use pumas_app_manager::{CustomNodesManager, SizeCalculator};
    use pumas_library::models::{ModelServingConfig, RuntimeDeviceMode};
    use pumas_library::{FakeOnnxEmbeddingBackend, OnnxSessionManager, PluginLoader, PumasApi};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;
    use tokio::sync::{Mutex, RwLock};

    async fn serving_test_state() -> (TempDir, AppState) {
        let temp_dir = TempDir::new().unwrap();
        let launcher_root = temp_dir.path().to_path_buf();
        let api = PumasApi::builder(&launcher_root)
            .auto_create_dirs(true)
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let plugin_loader = PluginLoader::new_async(launcher_root.join("launcher-data/plugins"))
            .await
            .unwrap();
        let onnx_session_manager =
            OnnxSessionManager::new(FakeOnnxEmbeddingBackend::new(), 2).unwrap();
        (
            temp_dir,
            AppState {
                api,
                version_managers: Arc::new(RwLock::new(HashMap::new())),
                custom_nodes_manager: Arc::new(CustomNodesManager::new(
                    launcher_root.join("comfyui-versions"),
                )),
                size_calculator: Arc::new(Mutex::new(
                    SizeCalculator::new_with_cache(launcher_root.join("launcher-data/cache")).await,
                )),
                shortcut_manager: Arc::new(RwLock::new(None)),
                plugin_loader: Arc::new(plugin_loader),
                gateway_http_client: reqwest::Client::new(),
                provider_registry: ProviderRegistry::builtin(),
                llama_cpp_router_client: LlamaCppRouterClient::new(reqwest::Client::new()),
                ollama_client_factory: OllamaClientFactory::new(
                    pumas_app_manager::OllamaHttpClients::new().unwrap(),
                ),
                onnx_session_manager,
            },
        )
    }

    fn onnx_serving_request() -> ServeModelRequest {
        ServeModelRequest {
            model_id: "embeddings/nomic/model".to_string(),
            config: ModelServingConfig {
                provider: RuntimeProviderId::OnnxRuntime,
                profile_id: RuntimeProfileId::parse("onnx-cpu").unwrap(),
                device_mode: RuntimeDeviceMode::Cpu,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
                context_size: None,
                keep_loaded: true,
                model_alias: Some("public-nomic".to_string()),
            },
        }
    }

    fn create_onnx_model_fixture(state: &AppState, model_id: &str) {
        let model_dir = state.api.model_library().library_root().join(model_id);
        std::fs::create_dir_all(&model_dir).unwrap();
        std::fs::write(model_dir.join("model.onnx"), b"fake").unwrap();
    }

    #[test]
    fn onnx_provider_request_model_id_uses_provider_behavior_policy() {
        let request = onnx_serving_request();

        assert_eq!(
            onnx_provider_request_model_id(&request, &ProviderRegistry::builtin()),
            "embeddings/nomic/model"
        );
    }

    #[tokio::test]
    async fn serve_onnx_model_is_idempotent_for_loaded_session() {
        let (_temp_dir, state) = serving_test_state().await;
        let request = onnx_serving_request();
        create_onnx_model_fixture(&state, &request.model_id);

        let first = serve_onnx_model(&state, request.clone()).await.unwrap();
        assert_eq!(first["loaded"], true);
        assert_eq!(first["loaded_models_unchanged"], false);
        let first_cursor = first
            .pointer("/snapshot/cursor")
            .and_then(serde_json::Value::as_str)
            .unwrap()
            .to_string();

        let second = serve_onnx_model(&state, request).await.unwrap();
        assert_eq!(second["loaded"], true);
        assert_eq!(second["loaded_models_unchanged"], true);
        assert_eq!(
            second
                .pointer("/snapshot/cursor")
                .and_then(serde_json::Value::as_str),
            Some(first_cursor.as_str())
        );
        assert_eq!(state.onnx_session_manager.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn unserve_onnx_model_removes_stale_status_when_session_is_absent() {
        let (_temp_dir, state) = serving_test_state().await;
        let request = onnx_serving_request();
        state
            .api
            .record_served_model(ServedModelStatus {
                model_id: request.model_id.clone(),
                model_alias: request.config.model_alias.clone(),
                provider: RuntimeProviderId::OnnxRuntime,
                profile_id: request.config.profile_id.clone(),
                load_state: ServedModelLoadState::Loaded,
                device_mode: request.config.device_mode,
                device_id: None,
                gpu_layers: None,
                tensor_split: None,
                context_size: Some(8),
                keep_loaded: true,
                endpoint_url: None,
                memory_bytes: None,
                loaded_at: None,
                last_error: None,
            })
            .await
            .unwrap();

        let response = unserve_onnx_model(
            &state,
            UnserveModelRequest {
                model_id: request.model_id,
                provider: Some(RuntimeProviderId::OnnxRuntime),
                profile_id: Some(request.config.profile_id.clone()),
                model_alias: request.config.model_alias.clone(),
            },
            request.config.profile_id,
            "public-nomic".to_string(),
        )
        .await
        .unwrap();

        assert_eq!(response["success"], true);
        assert_eq!(response["unloaded"], true);
        assert!(response.get("error").is_none_or(serde_json::Value::is_null));
        assert!(state
            .api
            .get_serving_status()
            .await
            .unwrap()
            .snapshot
            .served_models
            .is_empty());
    }
}
