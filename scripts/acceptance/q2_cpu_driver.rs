//! Disposable CI driver for repaired 1e83cfc9; no production changes or test seams.
use pumas_app_manager::version_manager::{TorchPreviewOutcome, VersionManager};
use pumas_library::acquisition::{AcquisitionPhase, AcquisitionService, AcquisitionStore};
use pumas_library::config::AppId;
use std::{path::PathBuf, sync::Arc};

#[tokio::main(flavor = "current_thread")]
async fn main() -> pumas_library::Result<()> {
    let root = PathBuf::from(
        std::env::args_os()
            .nth(1)
            .expect("empty qualification root"),
    );
    if root.exists() && std::fs::read_dir(&root)?.next().is_some() {
        return Err(pumas_library::PumasError::Other(
            "Root must be empty".into(),
        ));
    }
    std::fs::create_dir_all(&root)?;
    let root = root.canonicalize()?;
    let store = Arc::new(AcquisitionStore::new(&root.join("launcher-data/cache")));
    let acquisition = Arc::new(AcquisitionService::new(store.clone()));
    let manager =
        VersionManager::new_with_acquisition(&root, AppId::Torch, acquisition.clone()).await?;
    let install = async {
        let preview = match manager
            .preview_torch_runtime("v2.14.0", "cpu", "python3.14", "none")
            .await?
        {
            TorchPreviewOutcome::Ready { preview } => preview,
            _ => {
                return Err(pumas_library::PumasError::Other(
                    "Expected supported CPU selection".into(),
                ))
            }
        };
        std::fs::write(
            root.join("selection.json"),
            serde_json::to_vec_pretty(&preview)?,
        )?;
        let mut updates = manager
            .install_version_with_preview("v2.14.0", Some(&preview.preview_id))
            .await?;
        while let Some(update) = updates.recv().await {
            println!("{update:?}");
        }
        if !manager
            .get_installed_versions()
            .await?
            .iter()
            .any(|tag| tag == "v2.14.0")
        {
            return Err(pumas_library::PumasError::Other(
                "Installation did not publish the CPU runtime".into(),
            ));
        }
        Ok(())
    }
    .await;
    // Observe both owners even when installation fails; retain the root on error.
    let manager_cleanup = manager.shutdown_installations().await;
    let shared_cleanup = acquisition.shutdown().await;
    let records = store.acquisitions()?;
    let receipts = records
        .values()
        .map(|record| {
            Ok((
                record.id.to_string(),
                acquisition.consumer_receipt(record.id)?,
            ))
        })
        .collect::<pumas_library::Result<std::collections::BTreeMap<_, _>>>()?;
    let record_snapshots = records
        .iter()
        .map(|(id, record)| (id.to_string(), record))
        .collect::<std::collections::BTreeMap<_, _>>();
    std::fs::write(
        root.join("custody.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "records": record_snapshots, "receipts": receipts,
            "installation_error": install.as_ref().err().map(ToString::to_string),
            "manager_shutdown_error": manager_cleanup.as_ref().err().map(ToString::to_string),
            "shared_shutdown_error": shared_cleanup.as_ref().err().map(ToString::to_string),
        }))?,
    )?;
    install?;
    manager_cleanup?;
    shared_cleanup?;
    if records.len() != 1
        || records.values().any(|record| {
            record.demand.consumer != "runtime.torch"
                || !matches!(record.phase, AcquisitionPhase::Adopted { .. })
                || receipts
                    .get(&record.id.to_string())
                    .is_none_or(Option::is_none)
        })
    {
        return Err(pumas_library::PumasError::Other(
            "Exact shared Torch adoption receipt is absent".into(),
        ));
    }
    println!("Shared retained-wheel CPU installation and both shutdowns completed");
    Ok(())
}
