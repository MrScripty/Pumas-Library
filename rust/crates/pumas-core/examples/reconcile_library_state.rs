//! Run full model-library reconciliation through the public API path.
//!
//! This executes the same scoped reconciliation used by GUI refresh/read paths.
//!
//! Usage:
//!   cd rust
//!   cargo run --package pumas-library --example reconcile_library_state -- /path/to/launcher-root

use pumas_library::PumasApi;
use std::env;
use std::path::PathBuf;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <launcher_root>", args[0]);
        std::process::exit(1);
    }

    let launcher_root = PathBuf::from(&args[1]);
    if !launcher_root.exists() {
        eprintln!(
            "Error: launcher root does not exist: {}",
            launcher_root.display()
        );
        std::process::exit(1);
    }

    println!(
        "Running API reconciliation from launcher root: {}",
        launcher_root.display()
    );

    let api = PumasApi::builder(&launcher_root)
        .with_process_manager(false)
        .build()
        .await?;

    let model_count = api.rebuild_model_index().await?;
    println!("Reconciled model count: {}", model_count);
    Ok(())
}
