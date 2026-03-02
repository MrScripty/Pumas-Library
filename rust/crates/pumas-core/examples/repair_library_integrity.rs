//! One-shot maintenance script for model library integrity cleanup.
//!
//! Performs:
//! 1. duplicate repo_id cleanup (stub/identical dedupe)
//! 2. reclassify all models
//! 3. duplicate cleanup again (post-move collisions)
//! 4. index rebuild
//!
//! Usage:
//!   cd rust
//!   cargo run --package pumas-library --example repair_library_integrity -- /path/to/shared-resources/models

use pumas_library::ModelLibrary;
use std::env;
use std::path::PathBuf;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <library_root>", args[0]);
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

    println!(
        "Repairing model library integrity at: {}",
        library_root.display()
    );
    let library = ModelLibrary::new(&library_root).await?;

    let pre = library.cleanup_duplicate_repo_entries()?;
    println!(
        "Pre-cleanup: groups={}, removed={}, unresolved_groups={}, unresolved_dirs={}, normalized_ids={}",
        pre.duplicate_repo_groups,
        pre.removed_duplicate_dirs,
        pre.unresolved_duplicate_groups,
        pre.unresolved_duplicate_dirs,
        pre.normalized_metadata_ids
    );

    let reclassify = library.reclassify_all_models().await?;
    println!(
        "Reclassify: moved={}, total={}, errors={}",
        reclassify.reclassified,
        reclassify.total,
        reclassify.errors.len()
    );

    let post = library.cleanup_duplicate_repo_entries()?;
    println!(
        "Post-cleanup: groups={}, removed={}, unresolved_groups={}, unresolved_dirs={}, normalized_ids={}",
        post.duplicate_repo_groups,
        post.removed_duplicate_dirs,
        post.unresolved_duplicate_groups,
        post.unresolved_duplicate_dirs,
        post.normalized_metadata_ids
    );

    let indexed = library.rebuild_index().await?;
    println!("Rebuild index: indexed={}", indexed);
    Ok(())
}
