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
mkdir -p "$3/bin"
cp "$0.venv" "$3/bin/python"
"#,
    );
    executable(
        &python.with_file_name("python3.venv"),
        r#"#!/bin/sh
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
 touch "$destination/convert_hf_to_gguf.py"
fi
"#,
    );
    let cmake = tools.join("cmake");
    executable(
        &cmake,
        r#"#!/bin/sh
if test "$1" = '--build'; then
 mkdir -p "$2/bin"
 touch "$2/bin/llama-quantize" "$2/bin/llama-imatrix"
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
) -> (Arc<dyn QuantizationBackend>, Arc<SetupOwner>) {
    match id {
        QuantBackend::LlamaCpp => {
            let mut backend = LlamaCppBackend::new(root);
            Arc::get_mut(&mut backend.setup)
                .unwrap()
                .set_programs(programs);
            let owner = backend.setup.clone();
            (Arc::new(backend), owner)
        }
        QuantBackend::Nvfp4 => {
            let mut backend = Nvfp4Backend::new(root);
            Arc::get_mut(&mut backend.setup)
                .unwrap()
                .set_programs(programs);
            let owner = backend.setup.clone();
            (Arc::new(backend), owner)
        }
        QuantBackend::Sherry => {
            let mut backend = SherryBackend::new(root);
            Arc::get_mut(&mut backend.setup)
                .unwrap()
                .set_programs(programs);
            let owner = backend.setup.clone();
            (Arc::new(backend), owner)
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
            let (backend, owner) = backend(root.path(), id, programs(root.path()));
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
    manager.backends = pairs.iter().map(|(backend, _)| backend.clone()).collect();
    manager.backend_setups = pairs.into_iter().map(|(_, owner)| owner).collect();
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
