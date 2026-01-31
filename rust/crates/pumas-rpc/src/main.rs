//! Pumas RPC Server - JSON-RPC backend for Electron IPC.
//!
//! This binary provides a JSON-RPC 2.0 server that wraps the pumas-core library
//! for communication with the Electron main process.

mod handler;
mod server;
mod shortcut;
mod wrapper;

use anyhow::Result;
use clap::Parser;
use pumas_app_manager::{CustomNodesManager, SizeCalculator, VersionManager};
use pumas_library::AppId;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{info, warn, Level};
use tracing_subscriber::FmtSubscriber;

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

    /// Enable debug logging
    #[arg(short, long)]
    debug: bool,

    /// Launcher root directory (defaults to current directory's parent)
    #[arg(long)]
    launcher_root: Option<PathBuf>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Set up logging
    let log_level = if args.debug { Level::DEBUG } else { Level::INFO };
    FmtSubscriber::builder()
        .with_max_level(log_level)
        .with_target(false)
        .with_thread_ids(false)
        .compact()
        .init();

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
    let api = pumas_library::PumasApi::new(&launcher_root).await?;

    // Initialize version managers for all supported apps
    let mut version_managers: HashMap<String, VersionManager> = HashMap::new();

    // ComfyUI version manager
    match VersionManager::new(&launcher_root, AppId::ComfyUI).await {
        Ok(mgr) => {
            info!("ComfyUI version manager initialized successfully");
            version_managers.insert("comfyui".to_string(), mgr);
        }
        Err(e) => {
            warn!("Failed to initialize ComfyUI version manager: {}", e);
        }
    }

    // Ollama version manager
    match VersionManager::new(&launcher_root, AppId::Ollama).await {
        Ok(mgr) => {
            info!("Ollama version manager initialized successfully");
            version_managers.insert("ollama".to_string(), mgr);
        }
        Err(e) => {
            warn!("Failed to initialize Ollama version manager: {}", e);
        }
    }

    info!("Initialized {} version manager(s)", version_managers.len());

    // Initialize custom nodes manager
    let versions_dir = launcher_root.join(AppId::ComfyUI.versions_dir_name());
    let custom_nodes_manager = CustomNodesManager::new(versions_dir);
    info!("Custom nodes manager initialized");

    // Initialize size calculator
    let cache_dir = launcher_root
        .join("launcher-data")
        .join("cache");
    let size_calculator = SizeCalculator::new(cache_dir);
    info!("Size calculator initialized");

    // Start the server
    let addr = server::start_server(
        api,
        version_managers,
        custom_nodes_manager,
        size_calculator,
        launcher_root,
        &args.host,
        args.port,
    )
    .await?;

    // Print port for Electron to read (intentional stdout for IPC)
    // This format must match what python-bridge.ts expects
    println!("RPC_PORT={}", addr.port());

    info!("RPC server running on {}", addr);

    // Wait for shutdown signal
    tokio::signal::ctrl_c().await?;
    info!("Shutdown signal received, exiting");

    Ok(())
}
