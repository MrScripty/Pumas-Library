//! Explicit bounded non-model qualification. Never enables shipping audio.
use pumas_app_manager::version_manager::{ProgressUpdate, TorchPreviewOutcome};
use pumas_app_manager::{AppId, VersionManager};

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    pumas_library::runtime_read_source::audio_dependency_qualification_host_preflight()?;
    let root = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("explicit isolated root required")?,
    );
    if !root.is_absolute() {
        return Err("qualification root must be absolute".into());
    }
    if root.exists() && std::fs::read_dir(&root)?.next().is_some() {
        return Err("qualification root must be empty".into());
    }
    std::fs::create_dir_all(&root)?;
    // The empty root guarantees a cold private cache. Removing only the first
    // managed runtime keeps that same cache for a fresh warm installation.
    let cold = install_and_qualify(&root, "cold").await?;
    let warm = install_and_qualify(&root, "warm").await?;
    if cold != warm {
        return Err("cold and warm installs selected different dependency identities".into());
    }
    std::fs::copy(
        root.join("audio-warm-confined-dependency-probe.json"),
        root.join("audio-confined-dependency-probe.json"),
    )?;
    Ok(())
}

async fn install_and_qualify(
    root: &std::path::Path,
    phase: &str,
) -> Result<serde_json::Value, Box<dyn std::error::Error>> {
    let manager = VersionManager::new(root, AppId::Torch).await?;
    if phase == "warm" && !manager.remove_version("v2.10.0").await? {
        return Err("warm-cache qualification did not remove the cold installation".into());
    }
    let preview = match manager
        .preview_torch_runtime("v2.10.0", "cpu", "python3.12", "cohere-asr")
        .await?
    {
        TorchPreviewOutcome::Ready { preview } => preview,
        other => return Err(format!("selection refused: {other:?}").into()),
    };
    let mut progress = manager
        .install_version_with_preview("v2.10.0", Some(&preview.preview_id))
        .await?;
    let mut success = false;
    while let Some(update) = progress.recv().await {
        if let ProgressUpdate::Completed { success: completed } = &update {
            success = *completed;
        }
        if !matches!(&update, ProgressUpdate::Download { .. }) {
            println!("{update:?}");
        }
    }
    manager.shutdown_installations().await?;
    manager.shutdown_torch_cleanup().await?;
    if !success {
        return Err("managed runtime acquisition failed; see progress".into());
    }
    // Installation shutdown is terminal admission closure. Use a fresh owner
    // for byte preparation, with no second installation, then join it even when
    // capture refuses. Retained source custody outlives this manager phase.
    let preparation = VersionManager::new(root, AppId::Torch).await?;
    let prepared = preparation
        .prepare_torch_audio_runtime_bytes("v2.10.0")
        .await;
    let settled = preparation.shutdown_installations().await;
    let source = prepared?;
    settled?;
    let metadata: serde_json::Value = serde_json::from_slice(&std::fs::read(
        manager.version_path("v2.10.0").join("runtime.json"),
    )?)?;
    std::fs::write(
        root.join(format!("audio-{phase}-runtime.json")),
        serde_json::to_vec_pretty(&metadata)?,
    )?;
    let report = pumas_library::runtime_read_source::qualify_audio_dependency_reads(source).await?;
    std::fs::write(
        root.join(format!("audio-{phase}-confined-dependency-probe.json")),
        serde_json::to_vec_pretty(&report)?,
    )?;
    println!("{}", serde_json::to_string(&report)?);
    let mut artifacts = metadata["artifacts"]
        .as_array()
        .ok_or("missing artifact identities")?
        .clone();
    artifacts.sort_by(|a, b| a["name"].as_str().cmp(&b["name"].as_str()));
    Ok(serde_json::json!({
        "artifacts": artifacts,
        "distribution": metadata["managed_python"]["distribution"],
        "provider": metadata["managed_python"]["provider"],
    }))
}
