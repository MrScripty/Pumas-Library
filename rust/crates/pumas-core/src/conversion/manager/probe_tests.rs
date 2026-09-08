//! Public manager readiness and shutdown, with isolated executable fixtures.

use super::*;

fn fixture(root: &Path, directory: &str, script: &str, held: bool) -> PathBuf {
    let base = root.join("launcher-data").join(directory);
    let python = base.join("venv/bin/python");
    std::fs::create_dir_all(python.parent().unwrap()).unwrap();
    let source = if held {
        "#!/bin/sh\necho $$ > \"$0.pid\"\nexec sleep 10\n"
    } else {
        "#!/bin/sh\ntest \"$1\" = '-I' && test \"$2\" = '-B' && test \"$3\" = '-c' || exit 9\nprintf '%s\\n' \"$4\" >> \"$0.imports\"\ntest -f \"$0.ready\"\n"
    };
    let status = std::process::Command::new("/bin/sh")
        .args([
            "-c",
            "printf '%s' \"$2\" > \"$1\" && chmod 700 \"$1\"",
            "fixture",
        ])
        .arg(&python)
        .arg(source)
        .status()
        .unwrap();
    assert!(status.success());
    let script_path = base.join(script);
    std::fs::create_dir_all(script_path.parent().unwrap()).unwrap();
    std::fs::write(script_path, "fixture script, not executed").unwrap();
    if directory == "llama-cpp" {
        use std::os::unix::fs::PermissionsExt;
        let quantizer = base.join("build/bin/llama-quantize");
        std::fs::create_dir_all(quantizer.parent().unwrap()).unwrap();
        std::fs::write(&quantizer, "fixture binary, not executed").unwrap();
        std::fs::set_permissions(quantizer, std::fs::Permissions::from_mode(0o700)).unwrap();
    }
    python
}

fn fixtures(root: &Path, held: bool) -> Vec<PathBuf> {
    [
        ("llama-cpp", "source/convert_hf_to_gguf.py"),
        ("nvfp4", "quantize_nvfp4.py"),
        ("sherry", "sherry_qat.py"),
    ]
    .into_iter()
    .map(|(directory, script)| fixture(root, directory, script, held))
    .collect()
}

async fn manager(root: &Path) -> ConversionManager {
    let library = Arc::new(ModelLibrary::new(root.join("models")).await.unwrap());
    ConversionManager::new(
        root.to_path_buf(),
        library.clone(),
        Arc::new(ModelImporter::new(library)),
    )
}

#[tokio::test(flavor = "current_thread")]
async fn public_status_and_catalog_follow_fresh_import_results_without_setup() {
    let root = tempfile::tempdir().unwrap();
    let pythons = fixtures(root.path(), false);
    let manager = manager(root.path()).await;
    assert!(manager
        .backend_status_async()
        .await
        .unwrap()
        .iter()
        .all(|status| !status.ready));
    assert_eq!(
        manager.supported_quant_types_async().await.unwrap().len(),
        1
    );
    for python in &pythons {
        std::fs::write(python.with_file_name("python.ready"), "").unwrap();
    }
    assert!(manager
        .backend_status_async()
        .await
        .unwrap()
        .iter()
        .all(|status| status.ready));
    let types = manager.supported_quant_types_async().await.unwrap();
    for expected in ["Q4_K_M", "NVFP4", "Sherry-1.25bit"] {
        assert!(types.iter().any(|kind| kind.name == expected), "{expected}");
    }
    for (python, expected) in pythons.iter().zip([
        "google.protobuf",
        "export_tensorrt_llm_checkpoint",
        "TernaryQuantizer",
    ]) {
        assert!(
            std::fs::read_to_string(python.with_file_name("python.imports"))
                .unwrap()
                .contains(expected)
        );
    }
    std::fs::remove_file(pythons[1].with_file_name("python.ready")).unwrap();
    let statuses = manager.backend_status_async().await.unwrap();
    assert!(
        !statuses
            .iter()
            .find(|status| status.backend == QuantBackend::Nvfp4)
            .unwrap()
            .ready
    );
    assert!(manager.get_conversion_setup().is_none());
    assert!(manager
        .backend_setups
        .iter()
        .all(|setup| setup.snapshot().is_none()));
    manager.shutdown_setup().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn manager_shutdown_closes_all_probe_and_setup_owners_before_drain() {
    let root = tempfile::tempdir().unwrap();
    let pythons = fixtures(root.path(), true);
    let manager = manager(root.path()).await;
    let mut reads: Vec<_> = manager
        .backends
        .iter()
        .map(|backend| backend.is_ready_async())
        .collect();
    for read in &mut reads {
        assert!(futures::poll!(read.as_mut()).is_pending());
    }
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while pythons
            .iter()
            .any(|python| !python.with_file_name("python.pid").exists())
        {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("all probes start while runtime stays responsive");
    let mut status_read = Box::pin(manager.backend_status_async());
    assert!(futures::poll!(status_read.as_mut()).is_pending());
    drop(status_read);
    drop(reads);
    let mut shutdown = Box::pin(manager.shutdown_setup());
    let _ = futures::poll!(shutdown.as_mut());
    drop(shutdown);
    for backend in &manager.backends {
        assert!(matches!(
            backend.is_ready_async().await,
            Err(PumasError::ConversionCancelled)
        ));
        assert!(matches!(
            backend.ensure_environment().await,
            Err(PumasError::InstallationCancelled)
        ));
    }
    manager.shutdown_setup().await.unwrap();
    manager.shutdown_setup().await.unwrap();
    for python in pythons {
        let pid = std::fs::read_to_string(python.with_file_name("python.pid")).unwrap();
        assert!(!Path::new(&format!("/proc/{}", pid.trim())).exists());
    }
}
