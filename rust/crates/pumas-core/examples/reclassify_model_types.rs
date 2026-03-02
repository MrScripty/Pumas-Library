//! One-time migration script to reclassify model types in the library.
//!
//! This updates model metadata and moves model directories into canonical
//! `{model_type}/{family}/{cleaned_name}` paths.
//!
//! Usage:
//!   cd rust
//!   cargo run --package pumas-library --example reclassify_model_types -- /path/to/shared-resources/models

use pumas_library::ModelLibrary;
use std::env;
use std::path::PathBuf;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
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

    println!("Reclassifying model types in: {}", library_root.display());
    let library = ModelLibrary::new(&library_root).await?;

    let result = library.reclassify_all_models().await?;
    println!(
        "Reclassification complete: moved={} total={} errors={}",
        result.reclassified,
        result.total,
        result.errors.len()
    );

    if !result.errors.is_empty() {
        println!("Errors:");
        for (model_id, err) in &result.errors {
            println!("  - {}: {}", model_id, err);
        }
    }

    if !result.changes.is_empty() {
        println!("Changes:");
        for (from, to) in &result.changes {
            println!("  - {} -> {}", from, to);
        }
    }

    Ok(())
}
