//! Read-only JSON observations for a headless application's local-first bootstrap.
use pumas_library::discovery::LocalDiscovery;
use pumas_library::models::ModelLibrarySelectorSnapshotRequest;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let discovery = match std::env::args_os().nth(1) {
        Some(path) => LocalDiscovery::open_at(std::path::Path::new(&path))?,
        None => LocalDiscovery::open()?,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&serde_json::json!({
            "discovery": discovery.snapshot()?,
            "local_models": discovery.local_model_snapshots(ModelLibrarySelectorSnapshotRequest::default())?,
        }))?
    );
    Ok(())
}
