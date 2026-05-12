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
    let onnx_session_manager = OnnxSessionManager::new(FakeOnnxEmbeddingBackend::new(), 2).unwrap();
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
