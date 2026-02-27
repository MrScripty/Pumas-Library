//! One-time migration script to re-detect model types in the library.
//!
//! Run this script after updating the type detection logic to fix existing
//! models that may have been incorrectly classified.
//!
//! Usage:
//!   cd rust
//!   cargo run --package pumas-library --example migrate_model_types -- /path/to/shared-resources/models

use pumas_library::ModelLibrary;
use std::env;
use std::path::PathBuf;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get library path from args
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <library_root>", args[0]);
        eprintln!("Example: {} ./shared-resources/models", args[0]);
        std::process::exit(1);
    }

    let library_root = PathBuf::from(&args[1]);
    if !library_root.exists() {
        eprintln!(
            "Error: Library path does not exist: {}",
            library_root.display()
        );
        std::process::exit(1);
    }

    println!("Migrating model types in: {}", library_root.display());

    // Create model library instance
    let library = ModelLibrary::new(&library_root).await?;

    // Run re-detection
    println!("Re-detecting model types...");
    let updated = library.redetect_all_model_types().await?;

    println!("Migration complete: {} models updated", updated);
    Ok(())
}
