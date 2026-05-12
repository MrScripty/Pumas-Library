use pumas_library::PumasApi;
use std::path::Path;
use std::sync::OnceLock;
use tokio::sync::Mutex;

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
