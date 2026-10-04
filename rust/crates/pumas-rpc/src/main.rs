//! Pumas RPC Server - JSON-RPC backend for Electron IPC.
//!
//! This binary provides a JSON-RPC 2.0 server that wraps the pumas-core library
//! for communication with the Electron main process.

mod catalog_projection;
mod contract;
mod handlers;
mod http_admission;
mod http_transport;
#[cfg(feature = "inference-plugins")]
mod provider_clients;
mod server;
mod wrapper;

use anyhow::Result;
use clap::Parser;
#[cfg(feature = "inference-plugins")]
use pumas_app_manager::{SizeCalculator, VersionManager};
#[cfg(feature = "inference-plugins")]
use pumas_library::{AppId, PluginLoader};
#[cfg(feature = "inference-plugins")]
use std::collections::HashMap;
#[cfg(feature = "inference-plugins")]
use std::path::Path;
use std::path::PathBuf;
use tokio::runtime::Builder;
#[cfg(feature = "inference-plugins")]
use tracing::warn;
use tracing::{info, Level};
use tracing_subscriber::FmtSubscriber;

const RPC_WORKER_THREADS: usize = 4;
const RPC_MAX_BLOCKING_THREADS: usize = 16;
#[cfg(feature = "inference-plugins")]
const VERSION_MANAGED_APPS: &[AppId] = &[AppId::Ollama, AppId::Torch, AppId::LlamaCpp];

#[derive(Parser, Debug)]
#[command(name = "pumas-rpc")]
#[command(about = "JSON-RPC server for Pumas Library")]
struct Args {
    /// Export the current desktop wire contract without starting a server.
    #[cfg(feature = "export-contract")]
    #[arg(long)]
    export_desktop_contract: bool,
    /// Produce real constructor-generated conformance fixtures in a temporary library.
    #[cfg(feature = "export-contract")]
    #[arg(long)]
    export_desktop_fixtures: bool,
    /// Port to listen on (0 = auto-assign)
    #[arg(short, long, default_value = "0")]
    port: u16,

    /// Host to bind to
    #[arg(long, default_value = "127.0.0.1")]
    host: String,

    /// Grace for accepted HTTP connections after shutdown; not a request timeout
    #[arg(long, default_value_t = http_transport::DEFAULT_HTTP_SHUTDOWN_GRACE_MS)]
    http_shutdown_grace_ms: u64,

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Launcher root directory (defaults to current directory's parent)
    #[arg(long)]
    launcher_root: Option<PathBuf>,
}

fn main() -> Result<()> {
    let args = Args::parse();
    #[cfg(feature = "export-contract")]
    if args.export_desktop_fixtures {
        serde_json::to_writer_pretty(std::io::stdout(), &contract::desktop_contract_fixtures()?)?;
        return Ok(());
    }
    #[cfg(feature = "export-contract")]
    if args.export_desktop_contract {
        serde_json::to_writer_pretty(std::io::stdout(), &contract::desktop_contract_schema()?)?;
        return Ok(());
    }
    let host = server::LoopbackHost::parse(&args.host)?;
    let http_policy = http_transport::HttpShutdownPolicy::from_millis(args.http_shutdown_grace_ms)?;

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

    runtime.block_on(run(args, host, http_policy))
}

async fn run(
    args: Args,
    host: server::LoopbackHost,
    http_policy: http_transport::HttpShutdownPolicy,
) -> Result<()> {
    // Install Unix handlers before readiness is published. Electron and service
    // managers use SIGTERM, which must enter the same owned drain as SIGINT.
    #[cfg(unix)]
    let mut terminate = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    #[cfg(unix)]
    let mut interrupt = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::interrupt())?;

    #[cfg(windows)]
    let mut interrupt = tokio::signal::windows::ctrl_c()?;
    #[cfg(windows)]
    let mut ctrl_break = tokio::signal::windows::ctrl_break()?;

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

    let launcher_root = pumas_library::platform::paths::absolute_launcher_root(&launcher_root)?;

    info!("Launcher root configured");

    // Create the core API instance (model library, system utilities)
    // Use builder with auto_create_dirs so first-run (e.g. portable AppImage)
    // creates the directory structure automatically.
    let api = pumas_library::PumasApi::builder(&launcher_root)
        .auto_create_dirs(true)
        .build()
        .await?;

    #[cfg(feature = "inference-plugins")]
    let version_managers = initialize_version_managers(&launcher_root, &api).await;
    #[cfg(feature = "inference-plugins")]
    info!("Initialized {} version manager(s)", version_managers.len());

    #[cfg(feature = "inference-plugins")]
    let cache_dir = launcher_root.join("launcher-data").join("cache");
    #[cfg(feature = "inference-plugins")]
    let size_calculator = SizeCalculator::new_with_cache(cache_dir).await;
    #[cfg(feature = "inference-plugins")]
    info!("Size calculator initialized");

    #[cfg(feature = "inference-plugins")]
    let plugins_dir = launcher_root.join("launcher-data").join("plugins");
    #[cfg(feature = "inference-plugins")]
    let plugin_loader = match PluginLoader::new_async(plugins_dir.clone()).await {
        Ok(loader) => {
            info!("Plugin loader initialized ({} plugins)", loader.count());
            loader
        }
        Err(_) => {
            warn!("Plugin loader initialization failed; using empty loader");
            PluginLoader::new_async(std::env::temp_dir().join("pumas-plugins-fallback"))
                .await
                .unwrap()
        }
    };

    // Start the server
    let server = server::start_server(
        api,
        #[cfg(feature = "inference-plugins")]
        version_managers,
        #[cfg(feature = "inference-plugins")]
        size_calculator,
        #[cfg(feature = "inference-plugins")]
        plugin_loader,
        host,
        args.port,
        http_policy,
    )
    .await?;
    let addr = server.addr();

    // Print port for Electron to read (intentional stdout for IPC)
    // This format must match what python-bridge.ts expects
    println!("RPC_PORT={}", addr.port());

    info!("RPC server running on {}", addr);

    // Both OS signals and RPC admission converge on the same owned receipt.
    let signal = async {
        #[cfg(windows)]
        {
            tokio::select! {
                Some(()) = interrupt.recv() => {},
                Some(()) = ctrl_break.recv() => {}
            }
        }
        #[cfg(unix)]
        tokio::select! {
            _ = interrupt.recv() => {},
            _ = terminate.recv() => {},
        }
        #[cfg(not(any(unix, windows)))]
        tokio::signal::ctrl_c().await?;
        Ok::<(), anyhow::Error>(())
    };
    tokio::select! {
        result = signal => {
            info!("Shutdown signal observed, draining owned work");
            let drained = server.shutdown().await;
            match (result, drained) {
                (Ok(()), Ok(())) => {},
                (Err(error), Ok(())) | (Ok(()), Err(error)) => return Err(error),
                (Err(signal), Err(drain)) => return Err(anyhow::anyhow!(
                    "Signal observation failed: {signal}; shutdown failed: {drain}"
                )),
            }
        }
        result = server.wait() => result?,
    }
    info!("RPC shutdown completed");

    Ok(())
}

#[cfg(feature = "inference-plugins")]
async fn initialize_version_managers(
    launcher_root: &Path,
    api: &pumas_library::PumasApi,
) -> HashMap<String, VersionManager> {
    let mut version_managers = HashMap::new();

    for app_id in VERSION_MANAGED_APPS {
        let initialized = if matches!(*app_id, AppId::LlamaCpp | AppId::Torch) {
            VersionManager::new_with_acquisition(launcher_root, *app_id, api.acquisition().clone())
                .await
        } else {
            VersionManager::new(launcher_root, *app_id).await
        };
        match initialized {
            Ok(manager) => {
                info!("{app_id} version manager initialized successfully");
                version_managers.insert(app_id.as_str().to_string(), manager);
            }
            Err(_) => {
                warn!("Failed to initialize {app_id} version manager");
            }
        }
    }

    version_managers
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "inference-plugins")]
    use super::VERSION_MANAGED_APPS;
    #[cfg(feature = "inference-plugins")]
    use pumas_library::AppId;

    #[cfg(feature = "inference-plugins")]
    #[test]
    fn version_managed_apps_are_the_installable_inference_runtimes() {
        assert_eq!(
            VERSION_MANAGED_APPS,
            &[AppId::Ollama, AppId::Torch, AppId::LlamaCpp]
        );
        assert!(!VERSION_MANAGED_APPS.contains(&AppId::OnnxRuntime));
        assert!(VERSION_MANAGED_APPS.iter().all(AppId::has_version_manager));
    }
}
