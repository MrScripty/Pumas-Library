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

fn base_fixture(root: &Path, source: &str) -> PathBuf {
    let python = scripts::venv_python(root);
    std::fs::create_dir_all(python.parent().unwrap()).unwrap();
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
    python
}

async fn await_file(path: &Path) {
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while !path.exists() {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("controlled interpreter starts while runtime remains responsive");
}

#[tokio::test(flavor = "current_thread")]
async fn base_reads_share_dropped_waiter_work_then_observe_fresh_imports() {
    let root = tempfile::tempdir().unwrap();
    let python = base_fixture(root.path(), "#!/bin/sh\ntest \"$1\" = '-I' && test \"$2\" = '-B' && test \"$3\" = '-c' || exit 9\nprintf '%s\\n' \"$4\" >> \"$0.imports\"\necho $$ > \"$0.pid\"\nn=0\nwhile ! test -f \"$0.release\"; do n=$((n+1)); test \"$n\" -lt 200 || exit 8; sleep 0.02; done\ntest -f \"$0.ready\"\n");
    let manager = manager(root.path()).await;
    let mut first = Box::pin(manager.is_environment_ready_async());
    assert!(futures::poll!(first.as_mut()).is_pending());
    await_file(&python.with_file_name("python.pid")).await;
    let mut second = Box::pin(manager.is_environment_ready_async());
    assert!(futures::poll!(second.as_mut()).is_pending());
    drop(first);
    std::fs::write(python.with_file_name("python.release"), "").unwrap();
    assert!(!second.await.unwrap());
    let imports = python.with_file_name("python.imports");
    assert_eq!(
        std::fs::read_to_string(&imports).unwrap().lines().count(),
        1
    );
    std::fs::write(python.with_file_name("python.ready"), "").unwrap();
    assert!(manager.is_environment_ready_async().await.unwrap());
    let calls = std::fs::read_to_string(imports).unwrap();
    assert_eq!(calls.lines().count(), 2);
    assert!(calls
        .lines()
        .all(|line| line == super::super::readiness::BASE_CONVERSION_IMPORTS));
    assert!(
        manager.get_conversion_setup().is_none(),
        "reads do not admit setup"
    );
    manager.shutdown_setup().await.unwrap();
    assert!(matches!(
        manager.is_environment_ready_async().await,
        Err(PumasError::ConversionCancelled)
    ));
    assert!(!manager.is_environment_ready());
}

#[test]
fn base_probe_and_setup_imports_drain_with_one_blocking_thread() {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .max_blocking_threads(1)
        .build()
        .unwrap();
    runtime.block_on(async {
        for setup in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let python = base_fixture(root.path(), "#!/bin/sh\ntest \"$1\" = '-I' && test \"$2\" = '-B' && test \"$3\" = '-c' || { touch \"$0.unexpected\"; exit 9; }\nsleep 10 &\necho $! > \"$0.descendant\"\necho $$ > \"$0.pid\"\nexec sleep 10\n");
            let manager = manager(root.path()).await;
            let mut request = Box::pin(async {
                if setup {
                    manager.ensure_environment().await.map(|()| false)
                } else {
                    manager.is_environment_ready_async().await
                }
            });
            assert!(futures::poll!(request.as_mut()).is_pending());
            await_file(&python.with_file_name("python.pid")).await;
            drop(request);
            let mut shutdown = Box::pin(manager.shutdown_setup());
            let _ = futures::poll!(shutdown.as_mut());
            drop(shutdown);
            // Closure applies to every owner before the potentially held base
            // setup drain, even when the first shutdown waiter disappears.
            assert!(matches!(manager.is_environment_ready_async().await, Err(PumasError::ConversionCancelled)));
            for backend in &manager.backends {
                assert!(matches!(backend.is_ready_async().await, Err(PumasError::ConversionCancelled)));
                assert!(matches!(backend.ensure_environment().await, Err(PumasError::InstallationCancelled)));
            }
            manager.shutdown_setup().await.unwrap();
            manager.shutdown_setup().await.unwrap();
            let pid = std::fs::read_to_string(python.with_file_name("python.pid")).unwrap();
            let pid: i32 = pid.trim().parse().unwrap();
            assert!(!Path::new(&format!("/proc/{pid}")).exists());
            assert!(!super::super::linux_group::group_has_live_members(pid).unwrap());
            assert!(!python.with_file_name("python.unexpected").exists(), "setup cancellation must not continue to pip");
            assert!(matches!(manager.ensure_environment().await, Err(PumasError::InstallationCancelled)));
        }
    });
}

#[tokio::test(flavor = "current_thread")]
async fn base_signal_failure_is_not_missing_dependencies_and_shutdown_retains_it() {
    let root = tempfile::tempdir().unwrap();
    base_fixture(root.path(), "#!/bin/sh\nkill -TERM $$\n");
    let manager = manager(root.path()).await;
    let error = manager.is_environment_ready_async().await.unwrap_err();
    assert!(matches!(error, PumasError::ConversionFailed { .. }));
    let first = manager.shutdown_setup().await.unwrap_err().to_string();
    assert!(first.contains(&error.to_string()));
    assert_eq!(
        manager.shutdown_setup().await.unwrap_err().to_string(),
        first
    );
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
