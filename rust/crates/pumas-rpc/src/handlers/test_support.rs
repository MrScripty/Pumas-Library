use crate::provider_clients::{LlamaCppRouterClient, OllamaClientFactory};
use crate::server::AppState;
use pumas_app_manager::{CustomNodesManager, SizeCalculator};
use pumas_library::PumasApi;
use pumas_library::{OnnxEmbeddingBackendKind, OnnxSessionManager, PluginLoader, ProviderRegistry};
use std::path::Path;
use std::sync::{Arc, OnceLock};
use tokio::sync::{Mutex, RwLock};

static REGISTRY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(crate) async fn build_test_api(launcher_root: &Path) -> PumasApi {
    std::fs::create_dir_all(launcher_root.join("launcher-data")).unwrap();
    let registry_path = launcher_root.join("registry-test").join("registry.db");
    let _registry_guard = REGISTRY_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    std::env::set_var("PUMAS_REGISTRY_DB_PATH", &registry_path);
    let api = PumasApi::builder(launcher_root)
        .auto_create_dirs(true)
        .with_hf_client(false)
        .with_process_manager(false)
        .build()
        .await;
    std::env::remove_var("PUMAS_REGISTRY_DB_PATH");
    api.unwrap()
}

pub(crate) async fn build_test_app_state(launcher_root: &Path) -> AppState {
    let api = build_test_api(launcher_root).await;
    let plugin_loader = PluginLoader::new_async(launcher_root.join("launcher-data/plugins"))
        .await
        .unwrap();
    let onnx_session_manager =
        OnnxSessionManager::new(OnnxEmbeddingBackendKind::fake(), 2).unwrap();

    AppState {
        api,
        version_managers: Arc::new(RwLock::new(Default::default())),
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
    }
}
