//! Generate a migration dry-run report for a model library root.
//!
//! Usage:
//!   cargo run --package pumas-library --example generate_migration_dry_run -- /path/to/shared-resources/models

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

    let library = ModelLibrary::new(&library_root).await?;
    let report = library.generate_migration_dry_run_report_with_artifacts()?;

    println!(
        "Dry run complete: total={} keep={} move={} collisions={} blocked_partial={} errors={}",
        report.total_models,
        report.keep_candidates,
        report.move_candidates,
        report.collision_count,
        report.blocked_partial_count,
        report.error_count
    );
    if let Some(path) = report.machine_readable_report_path.as_deref() {
        println!("JSON report: {}", path);
    }
    if let Some(path) = report.human_readable_report_path.as_deref() {
        println!("Markdown report: {}", path);
    }

    Ok(())
}
