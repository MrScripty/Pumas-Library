//! Pumas RPC Server - JSON-RPC backend for Electron IPC.
//!
//! This binary provides a JSON-RPC 2.0 server that wraps the pumas-core library
//! for communication with the Electron main process.

mod handlers;
mod provider_clients;
mod server;
mod shortcut;
mod wrapper;

use anyhow::Result;
use clap::Parser;
use pumas_app_manager::{CustomNodesManager, SizeCalculator, VersionManager};
use pumas_library::{AppId, PluginLoader};
use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use tokio::runtime::Builder;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

const RPC_WORKER_THREADS: usize = 4;
const RPC_MAX_BLOCKING_THREADS: usize = 16;
const VERSION_MANAGED_APPS: &[AppId] =
    &[AppId::ComfyUI, AppId::Ollama, AppId::Torch, AppId::LlamaCpp];

#[derive(Parser, Debug)]
#[command(name = "pumas-rpc")]
#[command(about = "JSON-RPC server for Pumas Library")]
struct Args {
    /// Port to listen on (0 = auto-assign)
    #[arg(short, long, default_value = "0")]
    port: u16,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Allow binding the RPC listener to a non-loopback interface.
    #[arg(long)]
    allow_lan: bool,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Launcher root directory (defaults to current directory's parent)
    #[arg(long)]
    launcher_root: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    validate_rpc_host(&args.host, args.allow_lan)?;

    // Set up logging
    let log_level = if args.debug {
        Level::DEBUG
    } else {
        Level::INFO
    };
    FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .compact()
        .init();

    let runtime = Builder::new_multi_thread()
        .enable_all()
        .worker_threads(RPC_WORKER_THREADS)
        .max_blocking_threads(RPC_MAX_BLOCKING_THREADS)
        .thread_name("pumas-rpc")
        .build()?;

    runtime.block_on(run(args))
}

async fn run(args: Args) -> Result<()> {
    info!("Starting Pumas RPC Server");

    // Determine launcher root
    let launcher_root = match args.launcher_root {
        Some(path) => path,
        None => {
            // Default: assume we're in rust/target/*/pumas-rpc, go up to find project root
            let exe_path = std::env::current_exe()?;
            let mut path = exe_path.parent().unwrap().to_path_buf();

            // Navigate up from target directory to find project root
            while path.file_name().map(|n| n != "rust").unwrap_or(false) {
                if let Some(parent) = path.parent() {
                    path = parent.to_path_buf();
                } else {
                    break;
                }
            }

            // Go up one more level from rust/ to project root
            path.parent()
                .map(|p| p.to_path_buf())
                .unwrap_or_else(|| std::env::current_dir().unwrap())
        }
    };

    info!("Launcher root: {}", launcher_root.display());

    // Create the core API instance (model library, system utilities)
    // Use builder with auto_create_dirs so first-run (e.g. portable AppImage)
    // creates the directory structure automatically.
    let api = pumas_library::PumasApi::builder(&launcher_root)
        .auto_create_dirs(true)
        .build()
        .await?;

    let version_managers = initialize_version_managers(&launcher_root).await;
    info!("Initialized {} version manager(s)", version_managers.len());

    // Initialize custom nodes manager
    let versions_dir = launcher_root.join(AppId::ComfyUI.versions_dir_name());
    let custom_nodes_manager = CustomNodesManager::new(versions_dir);
    info!("Custom nodes manager initialized");

    // Initialize size calculator
    let cache_dir = launcher_root.join("launcher-data").join("cache");
    let size_calculator = SizeCalculator::new_with_cache(cache_dir).await;
    info!("Size calculator initialized");

    // Initialize plugin loader
    let plugins_dir = launcher_root.join("launcher-data").join("plugins");
    let plugin_loader = match PluginLoader::new_async(plugins_dir.clone()).await {
        Ok(loader) => {
            info!("Plugin loader initialized ({} plugins)", loader.count());
            loader
        }
        Err(e) => {
            warn!(
                "Failed to initialize plugin loader: {}, using empty loader",
                e
            );
            PluginLoader::new_async(std::env::temp_dir().join("pumas-plugins-fallback"))
                .await
                .unwrap()
        }
    };

    // Start the server
    let server = server::start_server(
        api,
        version_managers,
        custom_nodes_manager,
        size_calculator,
        plugin_loader,
        launcher_root,
        &args.host,
        args.port,
    )
    .await?;
    let addr = server.addr();

    // Print port for Electron to read (intentional stdout for IPC)
    // This format must match what python-bridge.ts expects
    println!("RPC_PORT={}", addr.port());

    info!("RPC server running on {}", addr);

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("Shutdown signal received, exiting");
    server.shutdown().await;

    Ok(())
}

async fn initialize_version_managers(launcher_root: &Path) -> HashMap<String, VersionManager> {
    let mut version_managers = HashMap::new();

    for app_id in VERSION_MANAGED_APPS {
        match VersionManager::new(launcher_root, *app_id).await {
            Ok(manager) => {
                info!("{app_id} version manager initialized successfully");
                version_managers.insert(app_id.as_str().to_string(), manager);
            }
            Err(error) => {
                warn!("Failed to initialize {app_id} version manager: {error}");
            }
        }
    }

    version_managers
}

fn validate_rpc_host(host: &str, allow_lan: bool) -> Result<()> {
    let ip_addr: IpAddr = host
        .parse()
        .map_err(|_| anyhow::anyhow!("RPC host must be an IP address, got '{host}'"))?;

    if ip_addr.is_loopback() || allow_lan {
        return Ok(());
    }

    Err(anyhow::anyhow!(
        "Refusing to bind RPC server to non-loopback host '{host}' without --allow-lan"
    ))
}

#[cfg(test)]
mod tests {
    use super::{validate_rpc_host, VERSION_MANAGED_APPS};
    use pumas_library::AppId;

    #[test]
    fn rpc_host_validation_allows_loopback_without_lan_flag() {
        assert!(validate_rpc_host("127.0.0.1", false).is_ok());
        assert!(validate_rpc_host("::1", false).is_ok());
    }

    #[test]
    fn rpc_host_validation_rejects_non_loopback_without_lan_flag() {
        let error = validate_rpc_host("0.0.0.0", false).unwrap_err();
        assert!(error.to_string().contains("without --allow-lan"), "{error}");
    }

    #[test]
    fn rpc_host_validation_allows_non_loopback_with_lan_flag() {
        assert!(validate_rpc_host("0.0.0.0", true).is_ok());
    }

    #[test]
    fn version_managed_apps_exclude_in_process_onnx_runtime() {
        assert!(!VERSION_MANAGED_APPS.contains(&AppId::OnnxRuntime));
        assert!(VERSION_MANAGED_APPS.iter().all(AppId::has_version_manager));
    }
}
