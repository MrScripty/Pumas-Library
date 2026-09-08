//! Controlled installer programs, not package installs or real model tools.
use super::super::{backend_setup::Programs, setup::SetupOwner};
use super::*;
use std::os::unix::fs::PermissionsExt;
use std::time::Duration;

fn executable(path: &Path, content: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
}

fn programs(root: &Path) -> Programs {
    let tools = root.join("fixture-tools");
    let python = tools.join("python3");
    executable(
        &python,
        r#"#!/bin/sh
printf 'create\n' >> "$0.creations"
mkdir -p "$3/bin"
cp "$0.venv" "$3/bin/python"
"#,
    );
    executable(
        &python.with_file_name("python3.venv"),
        r#"#!/bin/sh
if test "$1" = '-I'; then
 test "$2" = '-B' && test "$3" = '-c' || exit 8
 printf '%s\n' "$4" >> "$0.probes"
 test -f "$0.installed" && ! test -f "$0.broken"
 exit $?
fi
if test "$4" = '--upgrade'; then exit 0; fi
printf 'start\n' >> "$0.starts"
echo $$ > "$0.started"
attempt=0
while ! test -f "$0.release"; do
 attempt=$((attempt + 1))
 if test "$attempt" -ge 500; then exit 9; fi
 sleep 0.02
done
if test -f "$0.fail"; then exit 7; fi
touch "$0.installed"
"#,
    );
    let git = tools.join("git");
    executable(
        &git,
        r#"#!/bin/sh
if test "$1" = 'clone'; then
 for destination do :; done
 mkdir -p "$destination/.git"
 printf 'fixture converter\n' > "$destination/convert_hf_to_gguf.py"
fi
"#,
    );
    let cmake = tools.join("cmake");
    executable(
        &cmake,
        r#"#!/bin/sh
printf '%s\n' "$@" >> "$0.args"
if test "$1" = '--build'; then
 if test -f "$0.omit"; then exit 0; fi
 mkdir -p "$2/bin"
 printf '#!/bin/sh\nexit 0\n' > "$2/bin/llama-quantize"
 printf '#!/bin/sh\nexit 0\n' > "$2/bin/llama-imatrix"
 chmod 700 "$2/bin/llama-quantize" "$2/bin/llama-imatrix"
fi
"#,
    );
    Programs {
        python,
        git,
        cmake,
        nvcc: tools.join("absent-nvcc"),
    }
}

fn backend(
    root: &Path,
    id: QuantBackend,
    programs: Programs,
) -> (
    Arc<dyn QuantizationBackend>,
    Arc<SetupOwner>,
    Arc<super::super::readiness::ProbeOwner>,
) {
    match id {
        QuantBackend::LlamaCpp => {
            let mut backend = LlamaCppBackend::new(root);
            Arc::get_mut(&mut backend.setup)
                .unwrap()
                .set_programs(programs);
            let owner = backend.setup.clone();
            let probes = backend.readiness.clone();
            (Arc::new(backend), owner, probes)
        }
        QuantBackend::Nvfp4 => {
            let mut backend = Nvfp4Backend::new(root);
            Arc::get_mut(&mut backend.setup)
                .unwrap()
                .set_programs(programs);
            let owner = backend.setup.clone();
            let probes = backend.readiness.clone();
            (Arc::new(backend), owner, probes)
        }
        QuantBackend::Sherry => {
            let mut backend = SherryBackend::new(root);
            Arc::get_mut(&mut backend.setup)
                .unwrap()
                .set_programs(programs);
            let owner = backend.setup.clone();
            let probes = backend.readiness.clone();
            (Arc::new(backend), owner, probes)
        }
        QuantBackend::PythonConversion => unreachable!(),
    }
}

fn marker(root: &Path, directory: &str, suffix: &str) -> PathBuf {
    root.join("launcher-data")
        .join(directory)
        .join("venv/bin")
        .join(format!("python.{suffix}"))
}

async fn started(root: &Path, directory: &str) -> u32 {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(contents) = std::fs::read_to_string(marker(root, directory, "started")) {
                if let Ok(pid) = contents.trim().parse() {
                    return pid;
                }
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("controlled dependency installer reached")
}

#[tokio::test]
async fn every_backend_retains_dropped_installers_and_observes_outcomes() {
    for (id, directory) in [
        (QuantBackend::LlamaCpp, "llama-cpp"),
        (QuantBackend::Nvfp4, "nvfp4"),
        (QuantBackend::Sherry, "sherry"),
    ] {
        for outcome in ["success", "failure", "cancel"] {
            let root = tempfile::tempdir().unwrap();
            let (backend, owner, _probes) = backend(root.path(), id, programs(root.path()));
            let mut waiter = Box::pin(backend.ensure_environment());
            let pid = tokio::select! {
                result = &mut waiter => panic!("installer returned before hold: {result:?}"),
                pid = started(root.path(), directory) => pid,
            };
            drop(waiter);
            assert!(Path::new(&format!("/proc/{pid}")).exists());
            let contender = Nvfp4Backend::new(root.path());
            let error = contender.ensure_environment().await.unwrap_err();
            assert!(error.to_string().contains("already running"));
            assert!(contender.shutdown_setup().await.is_err());
            if outcome == "cancel" {
                tokio::time::timeout(Duration::from_secs(5), owner.shutdown())
                    .await
                    .unwrap()
                    .unwrap();
                assert!(!marker(root.path(), directory, "installed").exists());
            } else {
                if outcome == "failure" {
                    std::fs::write(marker(root.path(), directory, "fail"), "").unwrap();
                }
                // Join the still-held operation, rather than retrying a terminal one.
                let mut second = Box::pin(backend.ensure_environment());
                assert!(futures::poll!(second.as_mut()).is_pending());
                assert_eq!(
                    std::fs::read_to_string(marker(root.path(), directory, "starts")).unwrap(),
                    "start\n"
                );
                std::fs::write(marker(root.path(), directory, "release"), "").unwrap();
                let result = tokio::time::timeout(Duration::from_secs(5), second)
                    .await
                    .unwrap();
                if outcome == "success" {
                    result.unwrap();
                    assert!(marker(root.path(), directory, "installed").is_file());
                    owner.shutdown().await.unwrap();
                } else {
                    assert!(result.unwrap_err().to_string().contains("dependencies"));
                    let first = owner.shutdown().await.unwrap_err().to_string();
                    assert_eq!(owner.shutdown().await.unwrap_err().to_string(), first);
                    assert!(!marker(root.path(), directory, "installed").exists());
                }
            }
            assert!(
                !Path::new(&format!("/proc/{pid}")).exists(),
                "direct child reaped"
            );
            assert!(matches!(
                backend.ensure_environment().await,
                Err(PumasError::InstallationCancelled)
            ));
        }
    }
}

#[tokio::test]
async fn aggregate_setup_shutdown_closes_all_owners_and_survives_waiter_drop() {
    let root = tempfile::tempdir().unwrap();
    let library = Arc::new(ModelLibrary::new(root.path().join("models")).await.unwrap());
    let mut manager = ConversionManager::new(
        root.path().to_path_buf(),
        library.clone(),
        Arc::new(ModelImporter::new(library)),
    );
    let fixture_programs = programs(root.path());
    let pairs: Vec<_> = [
        QuantBackend::LlamaCpp,
        QuantBackend::Nvfp4,
        QuantBackend::Sherry,
    ]
    .into_iter()
    .map(|id| backend(root.path(), id, fixture_programs.clone()))
    .collect();
    manager.backends = pairs
        .iter()
        .map(|(backend, _, _)| backend.clone())
        .collect();
    manager.backend_probes = pairs.iter().map(|(_, _, probes)| probes.clone()).collect();
    manager.backend_setups = pairs.into_iter().map(|(_, owner, _)| owner).collect();
    let mut waiter = Box::pin(manager.ensure_backend_environment(QuantBackend::Nvfp4));
    let pid = tokio::select! {
        result = &mut waiter => panic!("installer returned before hold: {result:?}"),
        pid = started(root.path(), "nvfp4") => pid,
    };
    drop(waiter);
    let mut shutdown = Box::pin(manager.shutdown_setup());
    // Every owner closes before the first drain await, even if cleanup wins.
    let _ = futures::poll!(shutdown.as_mut());
    drop(shutdown);
    for id in [
        QuantBackend::LlamaCpp,
        QuantBackend::Nvfp4,
        QuantBackend::Sherry,
    ] {
        assert!(matches!(
            manager.ensure_backend_environment(id).await,
            Err(PumasError::InstallationCancelled)
        ));
    }
    assert!(matches!(
        manager.ensure_environment().await,
        Err(PumasError::InstallationCancelled)
    ));
    tokio::time::timeout(Duration::from_secs(5), manager.shutdown_setup())
        .await
        .unwrap()
        .unwrap();
    manager.shutdown_setup().await.unwrap();
    assert!(!Path::new(&format!("/proc/{pid}")).exists());
    assert!(!root.path().join("launcher-data/sherry").exists());
    assert!(!root.path().join("launcher-data/llama-cpp").exists());
}

#[tokio::test]
async fn setup_repairs_existing_interpreters_and_skips_healthy_dependencies() {
    for (id, directory, expected_import) in [
        (QuantBackend::LlamaCpp, "llama-cpp", "google.protobuf"),
        (
            QuantBackend::Nvfp4,
            "nvfp4",
            "export_tensorrt_llm_checkpoint",
        ),
        (QuantBackend::Sherry, "sherry", "TernaryQuantizer"),
    ] {
        for initially_healthy in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let tools = programs(root.path());
            let python = marker(root.path(), directory, "unused").with_file_name("python");
            let fixture =
                std::fs::read_to_string(tools.python.with_file_name("python3.venv")).unwrap();
            executable(&python, &fixture);
            std::fs::write(
                marker(root.path(), directory, "keep"),
                "existing environment",
            )
            .unwrap();
            std::fs::write(marker(root.path(), directory, "release"), "").unwrap();
            if initially_healthy {
                std::fs::write(marker(root.path(), directory, "installed"), "").unwrap();
            }
            let creations = tools.python.with_file_name("python3.creations");
            let (backend, owner, _probes) = backend(root.path(), id, tools);
            backend.ensure_environment().await.unwrap();
            assert!(!creations.exists(), "existing venv must not be recreated");
            assert_eq!(
                marker(root.path(), directory, "starts").exists(),
                !initially_healthy,
                "pip runs only when imports are missing"
            );
            assert!(
                std::fs::read_to_string(marker(root.path(), directory, "probes"))
                    .unwrap()
                    .contains(expected_import)
            );
            // A later dependency loss must invalidate the old successful setup.
            std::fs::remove_file(marker(root.path(), directory, "installed")).unwrap();
            backend.ensure_environment().await.unwrap();
            let expected_starts = if initially_healthy {
                "start\n"
            } else {
                "start\nstart\n"
            };
            assert_eq!(
                std::fs::read_to_string(marker(root.path(), directory, "starts")).unwrap(),
                expected_starts
            );
            backend.ensure_environment().await.unwrap();
            assert_eq!(
                std::fs::read_to_string(marker(root.path(), directory, "starts")).unwrap(),
                expected_starts
            );
            assert!(!creations.exists());
            assert_eq!(
                std::fs::read_to_string(marker(root.path(), directory, "keep")).unwrap(),
                "existing environment"
            );
            owner.shutdown().await.unwrap();
        }
    }
}

#[tokio::test]
async fn successful_pip_exit_cannot_complete_setup_with_failed_imports() {
    for (id, directory) in [
        (QuantBackend::LlamaCpp, "llama-cpp"),
        (QuantBackend::Nvfp4, "nvfp4"),
        (QuantBackend::Sherry, "sherry"),
    ] {
        let root = tempfile::tempdir().unwrap();
        let tools = programs(root.path());
        let python = marker(root.path(), directory, "unused").with_file_name("python");
        executable(
            &python,
            &std::fs::read_to_string(tools.python.with_file_name("python3.venv")).unwrap(),
        );
        std::fs::write(marker(root.path(), directory, "release"), "").unwrap();
        std::fs::write(marker(root.path(), directory, "broken"), "").unwrap();
        let (backend, owner, _probes) = backend(root.path(), id, tools);
        let error = backend.ensure_environment().await.unwrap_err();
        assert!(matches!(error, PumasError::ConversionFailed { .. }));
        assert!(error.to_string().contains("imports"), "{error}");
        assert!(
            marker(root.path(), directory, "installed").exists(),
            "pip fixture exited successfully"
        );
        assert_eq!(
            owner.snapshot().unwrap().status,
            super::super::ConversionSetupStatus::Failed
        );
        // Repair the fixture's package source, not its venv; an explicit retry
        // rechecks imports and runs installation again.
        std::fs::remove_file(marker(root.path(), directory, "broken")).unwrap();
        std::fs::remove_file(marker(root.path(), directory, "installed")).unwrap();
        backend.ensure_environment().await.unwrap();
        assert_eq!(
            owner.snapshot().unwrap().status,
            super::super::ConversionSetupStatus::Completed
        );
        assert_eq!(
            std::fs::read_to_string(marker(root.path(), directory, "starts")).unwrap(),
            "start\nstart\n"
        );
        owner.shutdown().await.unwrap();
    }
}

fn native_fixture(root: &Path, tools: &Programs) -> PathBuf {
    let base = root.join("launcher-data/llama-cpp");
    std::fs::create_dir_all(base.join("source/.git")).unwrap();
    std::fs::write(base.join("source/keep"), "preserve checkout").unwrap();
    std::fs::write(
        base.join("source/convert_hf_to_gguf.py"),
        "fixture converter",
    )
    .unwrap();
    for binary in ["llama-quantize", "llama-imatrix"] {
        executable(&base.join("build/bin").join(binary), "#!/bin/sh\nexit 0\n");
    }
    executable(
        &base.join("venv/bin/python"),
        &std::fs::read_to_string(tools.python.with_file_name("python3.venv")).unwrap(),
    );
    std::fs::write(marker(root, "llama-cpp", "release"), "").unwrap();
    base
}

#[tokio::test]
async fn llama_setup_repairs_each_unusable_native_artifact_and_skips_healthy_pair() {
    for binary in ["llama-quantize", "llama-imatrix"] {
        for state in ["healthy", "missing", "empty", "not_executable"] {
            let root = tempfile::tempdir().unwrap();
            let tools = programs(root.path());
            let base = native_fixture(root.path(), &tools);
            let target = base.join("build/bin").join(binary);
            match state {
                "missing" => std::fs::remove_file(&target).unwrap(),
                "empty" => std::fs::write(&target, "").unwrap(),
                "not_executable" => {
                    std::fs::set_permissions(&target, std::fs::Permissions::from_mode(0o600))
                        .unwrap()
                }
                _ => {}
            }
            let cmake_log = tools.cmake.with_file_name("cmake.args");
            let (backend, owner, probes) = backend(root.path(), QuantBackend::LlamaCpp, tools);
            backend.ensure_environment().await.unwrap();
            assert_eq!(
                owner.snapshot().unwrap().status,
                super::super::ConversionSetupStatus::Completed
            );
            assert_eq!(cmake_log.exists(), state != "healthy");
            if state != "healthy" {
                let arguments = std::fs::read_to_string(cmake_log).unwrap();
                assert!(arguments.lines().any(|arg| arg == "--clean-first"));
                assert!(arguments.lines().any(|arg| arg == "--build"));
            }
            assert!(backend.is_ready_async().await.unwrap());
            assert_eq!(
                std::fs::read_to_string(base.join("source/keep")).unwrap(),
                "preserve checkout"
            );
            super::super::readiness::shutdown_backend(&owner, &probes)
                .await
                .unwrap();
        }
    }
}

#[tokio::test]
async fn llama_setup_rejects_zero_exit_without_usable_outputs_then_retries() {
    for state in ["missing", "empty", "not_executable"] {
        let root = tempfile::tempdir().unwrap();
        let tools = programs(root.path());
        let base = native_fixture(root.path(), &tools);
        let imatrix = base.join("build/bin/llama-imatrix");
        match state {
            "missing" => std::fs::remove_file(&imatrix).unwrap(),
            "empty" => std::fs::write(&imatrix, "").unwrap(),
            _ => {
                std::fs::set_permissions(&imatrix, std::fs::Permissions::from_mode(0o600)).unwrap()
            }
        }
        let omit = tools.cmake.with_file_name("cmake.omit");
        std::fs::write(&omit, "").unwrap();
        let (backend, owner, probes) = backend(root.path(), QuantBackend::LlamaCpp, tools);
        let error = backend.ensure_environment().await.unwrap_err();
        assert!(error.to_string().contains("llama-imatrix"), "{error}");
        assert_eq!(
            owner.snapshot().unwrap().status,
            super::super::ConversionSetupStatus::Failed
        );
        assert!(
            !marker(root.path(), "llama-cpp", "starts").exists(),
            "native verification precedes pip"
        );
        assert!(
            !marker(root.path(), "llama-cpp", "probes").exists(),
            "native verification precedes import probes"
        );
        std::fs::remove_file(omit).unwrap();
        backend.ensure_environment().await.unwrap();
        assert_eq!(
            owner.snapshot().unwrap().status,
            super::super::ConversionSetupStatus::Completed
        );
        assert!(backend.is_ready_async().await.unwrap());
        assert_eq!(
            std::fs::read_to_string(base.join("source/keep")).unwrap(),
            "preserve checkout"
        );
        super::super::readiness::shutdown_backend(&owner, &probes)
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn llama_setup_rejects_bad_converter_without_building_or_installing() {
    for state in ["missing", "empty", "directory"] {
        let root = tempfile::tempdir().unwrap();
        let tools = programs(root.path());
        let base = native_fixture(root.path(), &tools);
        let converter = base.join("source/convert_hf_to_gguf.py");
        std::fs::remove_file(&converter).unwrap();
        match state {
            "empty" => std::fs::write(&converter, "").unwrap(),
            "directory" => std::fs::create_dir(&converter).unwrap(),
            _ => {}
        }
        let cmake_log = tools.cmake.with_file_name("cmake.args");
        let (backend, owner, probes) = backend(root.path(), QuantBackend::LlamaCpp, tools);
        let error = backend.ensure_environment().await.unwrap_err();
        assert!(
            error.to_string().contains("convert_hf_to_gguf.py"),
            "{error}"
        );
        assert_eq!(
            owner.snapshot().unwrap().status,
            super::super::ConversionSetupStatus::Failed
        );
        assert!(!cmake_log.exists());
        assert!(!marker(root.path(), "llama-cpp", "starts").exists());
        assert!(!marker(root.path(), "llama-cpp", "probes").exists());
        assert_eq!(
            std::fs::read_to_string(base.join("source/keep")).unwrap(),
            "preserve checkout"
        );
        assert!(super::super::readiness::shutdown_backend(&owner, &probes)
            .await
            .is_err());
    }
}

#[tokio::test]
async fn llama_setup_preserves_unexpected_native_outputs_before_cmake_clean() {
    for binary in ["llama-quantize", "llama-imatrix"] {
        for occupied in ["directory", "symlink"] {
            let root = tempfile::tempdir().unwrap();
            let tools = programs(root.path());
            let base = native_fixture(root.path(), &tools);
            let target = base.join("build/bin").join(binary);
            std::fs::remove_file(&target).unwrap();
            let outside = root.path().join("outside-native-tool");
            if occupied == "directory" {
                std::fs::create_dir(&target).unwrap();
                std::fs::write(target.join("keep"), "preserve occupied directory").unwrap();
            } else {
                executable(&outside, "#!/bin/sh\n# preserve external tool\nexit 0\n");
                std::os::unix::fs::symlink(&outside, &target).unwrap();
                // The link itself is usable. Another missing tool forces the
                // clean-first decision to inspect every generated-output entry.
                let other = if binary == "llama-quantize" {
                    "llama-imatrix"
                } else {
                    "llama-quantize"
                };
                std::fs::remove_file(base.join("build/bin").join(other)).unwrap();
            }
            let cmake_log = tools.cmake.with_file_name("cmake.args");
            let (backend, owner, probes) = backend(root.path(), QuantBackend::LlamaCpp, tools);
            let error = backend.ensure_environment().await.unwrap_err();
            assert!(error.to_string().contains(binary), "{error}");
            assert_eq!(
                owner.snapshot().unwrap().status,
                super::super::ConversionSetupStatus::Failed
            );
            assert!(
                !cmake_log.exists(),
                "unexpected output must block configure and clean"
            );
            assert!(!marker(root.path(), "llama-cpp", "starts").exists());
            assert!(!marker(root.path(), "llama-cpp", "probes").exists());
            if occupied == "directory" {
                assert_eq!(
                    std::fs::read_to_string(target.join("keep")).unwrap(),
                    "preserve occupied directory"
                );
            } else {
                assert!(std::fs::symlink_metadata(&target)
                    .unwrap()
                    .file_type()
                    .is_symlink());
                assert_eq!(std::fs::read_link(&target).unwrap(), outside);
                assert_eq!(
                    std::fs::read_to_string(&outside).unwrap(),
                    "#!/bin/sh\n# preserve external tool\nexit 0\n"
                );
            }
            let first = super::super::readiness::shutdown_backend(&owner, &probes)
                .await
                .unwrap_err()
                .to_string();
            assert_eq!(
                super::super::readiness::shutdown_backend(&owner, &probes)
                    .await
                    .unwrap_err()
                    .to_string(),
                first
            );
        }
    }
}
