#[cfg(feature = "inference-plugins")]
use crate::provider_clients::{LlamaCppRouterClient, OllamaClientFactory};
use crate::server::AppState;
#[cfg(feature = "inference-plugins")]
use pumas_app_manager::SizeCalculator;
use pumas_library::PumasApi;
#[cfg(feature = "inference-plugins")]
use pumas_library::{OnnxEmbeddingBackendKind, OnnxSessionManager, PluginLoader, ProviderRegistry};
use std::path::Path;
#[cfg(feature = "inference-plugins")]
use std::sync::Arc;
use std::sync::OnceLock;
use tokio::sync::Mutex;
#[cfg(feature = "inference-plugins")]
use tokio::sync::RwLock;

static REGISTRY_TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

pub(crate) async fn build_test_api(launcher_root: &Path) -> PumasApi {
    build_test_api_with_services(launcher_root, false, false).await
}

pub(crate) async fn build_test_api_with_hf(launcher_root: &Path) -> PumasApi {
    build_test_api_with_services(launcher_root, true, true).await
}

async fn build_test_api_with_services(
    launcher_root: &Path,
    with_hf_client: bool,
    with_process_manager: bool,
) -> PumasApi {
    std::fs::create_dir_all(launcher_root.join("launcher-data")).unwrap();
    let registry_path = launcher_root.join("registry-test").join("registry.db");
    let _registry_guard = REGISTRY_TEST_LOCK
        .get_or_init(|| Mutex::new(()))
        .lock()
        .await;
    std::env::set_var("PUMAS_REGISTRY_DB_PATH", &registry_path);
    let api = PumasApi::builder(launcher_root)
        .auto_create_dirs(true)
        .with_hf_client(with_hf_client)
        .with_process_manager(with_process_manager)
        .build()
        .await;
    std::env::remove_var("PUMAS_REGISTRY_DB_PATH");
    api.unwrap()
}

pub(crate) async fn build_test_app_state(launcher_root: &Path) -> AppState {
    let api = build_test_api(launcher_root).await;

    #[cfg(not(feature = "inference-plugins"))]
    {
        AppState {
            shutdown_request: crate::server::ShutdownRequest::default(),
            api,
            catalog_projection: crate::catalog_projection::CatalogProjection::unavailable(),
        }
    }

    #[cfg(feature = "inference-plugins")]
    {
        let plugin_loader = PluginLoader::new_async(launcher_root.join("launcher-data/plugins"))
            .await
            .unwrap();
        let onnx_session_manager =
            OnnxSessionManager::new(OnnxEmbeddingBackendKind::fake(), 2).unwrap();

        AppState {
            shutdown_request: crate::server::ShutdownRequest::default(),
            catalog_projection: crate::catalog_projection::CatalogProjection::unavailable(),
            api,
            version_managers: Arc::new(RwLock::new(Default::default())),
            size_calculator: Arc::new(Mutex::new(
                SizeCalculator::new_with_cache(launcher_root.join("launcher-data/cache")).await,
            )),
            plugin_loader: Arc::new(plugin_loader),
            gateway_http_client: reqwest::Client::new(),
            gateway_base_url: pumas_library::models::RuntimeEndpointUrl::parse(
                "http://127.0.0.1:3456/v1",
            )
            .unwrap(),
            provider_registry: ProviderRegistry::builtin(),
            llama_cpp_router_client: LlamaCppRouterClient::new(reqwest::Client::new()),
            ollama_client_factory: OllamaClientFactory::new(
                pumas_app_manager::OllamaHttpClients::new().unwrap(),
            ),
            onnx_session_manager,
        }
    }
}
