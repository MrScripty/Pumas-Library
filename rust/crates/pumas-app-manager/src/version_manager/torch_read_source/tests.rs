use super::*;

struct Installed {
    launcher: tempfile::TempDir,
    runtime: PathBuf,
    depot: PathBuf,
    packages: PathBuf,
}
impl Installed {
    fn new() -> Self {
        Self::new_at_tag("synthetic")
    }
    fn new_at_tag(tag: &str) -> Self {
        let launcher = tempfile::tempdir().unwrap();
        let versions = launcher.path().join(AppId::Torch.versions_dir_name());
        let runtime = versions.join(tag);
        std::fs::create_dir_all(&runtime).unwrap();
        installer::write_embedded_torch_runtime(&runtime).unwrap();
        let depot = launcher
            .path()
            .join("launcher-data/managed-python/python")
            .join("a".repeat(64));
        std::fs::create_dir_all(depot.join("bin")).unwrap();
        std::fs::create_dir_all(depot.join("lib/python3.12")).unwrap();
        let executable = depot.join("bin/python3.12");
        std::fs::write(
            &executable,
            b"inert synthetic interpreter bytes; never executed",
        )
        .unwrap();
        std::fs::write(
            depot.join("lib/python3.12/os.py"),
            b"synthetic stdlib bytes",
        )
        .unwrap();
        std::os::unix::fs::symlink("python3.12", depot.join("bin/python3")).unwrap();
        let packages = runtime.join("venv/lib/python3.12/site-packages");
        std::fs::create_dir_all(&packages).unwrap();
        std::fs::create_dir_all(runtime.join("venv/bin")).unwrap();
        std::os::unix::fs::symlink(&executable, runtime.join("venv/bin/python")).unwrap();
        std::fs::write(packages.join("dependency.py"), b"selected dependency").unwrap();
        let dependency =
            file_manifest(&packages.join("dependency.py"), "dependency.py".into()).unwrap();
        std::fs::write(runtime.join("installed-files.json"), serde_json::to_vec(&serde_json::json!({"files":[{"path":dependency.path(), "size":dependency.size(), "sha256":dependency.sha256()}]})).unwrap()).unwrap();
        let sha = file_manifest(&executable, "executable".into()).unwrap();
        std::fs::write(runtime.join("runtime.json"), serde_json::to_vec(&serde_json::json!({"python":"python3.12","managed_python":{"executable":{"path":executable,"sha256":sha.sha256()}}})).unwrap()).unwrap();
        Self {
            launcher,
            runtime,
            depot,
            packages,
        }
    }
    fn capture(&self) -> Result<Arc<RetainedRuntimeReadSource>> {
        let lock = TorchVersionsLock::try_acquire_read(self.runtime.parent().unwrap()).unwrap();
        let tag = self.runtime.file_name().unwrap().to_str().unwrap();
        let revision =
            TorchRevisionLease::read(self.runtime.parent().unwrap(), tag, &lock).unwrap();
        drop(lock);
        capture_installed(self.launcher.path(), &self.runtime, revision)
    }
}

fn synthetic_installer(installed: &Installed) -> installer::VersionInstaller {
    let metadata = Arc::new(MetadataManager::new(installed.launcher.path()));
    metadata.ensure_directories().unwrap();
    let progress = Arc::new(RwLock::new(super::super::InstallationProgressTracker::new(
        installed.launcher.path().join("launcher-data/cache"),
    )));
    installer::VersionInstaller::new(
        installed.launcher.path().to_owned(),
        AppId::Torch,
        metadata,
        progress,
        Arc::new(std::sync::atomic::AtomicBool::new(false)),
    )
    .with_torch_stage_override(Arc::new(|stage| {
        let runtime = stage.join("runtime");
        std::fs::create_dir(&runtime).map_err(PumasError::from)?;
        // Inert stage bytes exercise publication, not a Torch/wheel install.
        std::fs::write(
            runtime.join("runtime.json"),
            br#"{"recipe_id":"torch-upstream-2.9.1-r1"}"#,
        )
        .map_err(PumasError::from)?;
        Ok(runtime)
    }))
}

fn synthetic_release() -> pumas_library::network::GitHubRelease {
    pumas_library::network::GitHubRelease {
        tag_name: "v2.9.1".into(),
        name: "inert publication fixture".into(),
        published_at: "2026-10-09T00:00:00Z".into(),
        body: None,
        tarball_url: None,
        zipball_url: None,
        prerelease: false,
        assets: vec![],
        html_url: "https://github.com/pytorch/pytorch/releases/tag/v2.9.1".into(),
        total_size: None,
        archive_size: None,
        dependencies_size: None,
    }
}

async fn registered_manager(installed: &Installed) -> VersionManager {
    let tag = installed.runtime.file_name().unwrap().to_str().unwrap();
    let metadata = MetadataManager::new(installed.launcher.path());
    metadata.ensure_directories().unwrap();
    std::fs::write(
        installed.runtime.join("resolution.json"),
        br#"{"torch":"inert-unqualified-fixture"}"#,
    )
    .unwrap();
    metadata
        .update_installed_version(
            tag,
            pumas_library::metadata::InstalledVersionMetadata {
                path: tag.into(),
                release_tag: tag.into(),
                ..Default::default()
            },
            Some(AppId::Torch),
        )
        .unwrap();
    let client = GitHubClient::with_loopback_api(
        installed.launcher.path().join("launcher-data/cache"),
        Duration::from_secs(1),
        "http://127.0.0.1:9".into(),
    )
    .unwrap();
    VersionManager::new_with_github_client(
        installed.launcher.path().to_owned(),
        AppId::Torch,
        Some(Arc::new(client)),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn registered_retained_revision_allows_unrelated_atomic_publication_then_blocks_its_removal()
{
    let installed = Installed::new();
    let manager = registered_manager(&installed).await;
    let owner = manager
        .retain_torch_runtime_bytes("synthetic")
        .await
        .unwrap();
    let retained = owner.clone();
    let installer = synthetic_installer(&installed);
    let (tx, _rx) = mpsc::channel(32);
    installer
        .install_version("v2.9.1", &synthetic_release(), tx)
        .await
        .unwrap();
    assert!(installed
        .runtime
        .parent()
        .unwrap()
        .join("v2.9.1/runtime.json")
        .is_file());
    assert!(manager
        .metadata_manager
        .get_installed_version("v2.9.1", Some(AppId::Torch))
        .unwrap()
        .is_some());
    owner.validate().unwrap();
    // Change only owner selection metadata, without invoking an interpreter.
    let global = TorchVersionsLock::try_acquire(installed.runtime.parent().unwrap()).unwrap();
    {
        let mut state = manager.state.write().await;
        state.refresh_with_lock(&global).await.unwrap();
        state
            .set_active_version_with_lock("v2.9.1", &global)
            .await
            .unwrap();
    }
    drop(global);
    drop(owner);
    let refused = manager.remove_version("synthetic").await;
    assert!(
        matches!(refused, Err(PumasError::Io { source: Some(ref error), .. }) if error.kind() == io::ErrorKind::WouldBlock)
    );
    assert!(installed.runtime.join("runtime.json").is_file());
    assert!(manager
        .metadata_manager
        .get_installed_version("synthetic", Some(AppId::Torch))
        .unwrap()
        .is_some());
    retained.validate().unwrap();
    drop(retained);
    assert!(manager.remove_version("synthetic").await.unwrap());
    assert!(!installed.runtime.exists());
    ManagedDepotLease::mutation(&installed.depot).unwrap();
    installer.shutdown_torch_cleanup().await.unwrap();
    manager.shutdown_installations().await.unwrap();
}

#[tokio::test]
async fn same_tag_installer_refuses_retained_bytes_before_orphan_recovery_or_staging() {
    let installed = Installed::new_at_tag("v2.9.1");
    std::fs::write(
        installed.runtime.join(".pumas-publishing"),
        b"metadata pending",
    )
    .unwrap();
    std::fs::write(
        installed
            .runtime
            .parent()
            .unwrap()
            .join(".torch-pending-publish-v2.9.1"),
        b"metadata pending",
    )
    .unwrap();
    let owner = installed.capture().unwrap();
    let attempted = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let stage_attempt = attempted.clone();
    let installer =
        synthetic_installer(&installed).with_torch_stage_override(Arc::new(move |_| {
            stage_attempt.store(true, Ordering::SeqCst);
            panic!("retained registration must refuse before staging")
        }));
    let (tx, _rx) = mpsc::channel(32);
    assert!(installer
        .install_version("v2.9.1", &synthetic_release(), tx)
        .await
        .is_err());
    assert!(!attempted.load(Ordering::SeqCst));
    assert!(installed.runtime.join(".pumas-publishing").is_file());
    assert!(!std::fs::read_dir(installed.runtime.parent().unwrap())
        .unwrap()
        .any(|e| e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".torch-orphan-")));
    owner.validate().unwrap();
    installer.shutdown_torch_cleanup().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn registered_child_cleanup_retains_revision_and_depot_until_process_tree_drain() {
    let installed = Installed::new();
    let owner = installed.capture().unwrap();
    let cleanup = Arc::new(installer::TorchCleanupTasks::default());
    let slot = cleanup.new_child_slot().unwrap();
    let mut command = std::process::Command::new("/bin/sh");
    command
        .args(["-c", "sleep 30"])
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null());
    // Generic inert OS process test, not managed CPython/import proof.
    let mut child =
        pumas_library::platform::managed_child::ManagedChild::spawn(&mut command, slot.clone())
            .unwrap();
    child.attach_cleanup_lease(owner.clone());
    let pid = child.id();
    drop(owner);
    drop(child);
    assert!(slot.has_parked_child());
    let global = TorchVersionsLock::try_acquire(installed.runtime.parent().unwrap()).unwrap();
    assert!(TorchRevisionLease::mutation(
        installed.runtime.parent().unwrap(),
        "synthetic",
        &global
    )
    .is_err());
    TorchRevisionLease::mutation(installed.runtime.parent().unwrap(), "unrelated", &global)
        .unwrap();
    assert!(ManagedDepotLease::mutation(&installed.depot).is_err());
    drop(global);
    tokio::time::timeout(Duration::from_secs(5), cleanup.drain_child_slot(&slot))
        .await
        .unwrap()
        .unwrap();
    assert!(!slot.is_active());
    assert!(!pumas_library::platform::is_process_alive(pid));
    let global = TorchVersionsLock::try_acquire(installed.runtime.parent().unwrap()).unwrap();
    TorchRevisionLease::mutation(installed.runtime.parent().unwrap(), "synthetic", &global)
        .unwrap();
    ManagedDepotLease::mutation(&installed.depot).unwrap();
    cleanup.close();
    cleanup.drain().await.unwrap();
}

#[test]
fn closed_selected_bytes_hold_revision_and_depot_but_release_global_lock() {
    let installed = Installed::new();
    let owner = installed.capture().unwrap();
    assert!(owner
        .manifest()
        .any(|(role, member)| role == RuntimeReadRole::Interpreter
            && member.path() == "lib/python3.12/os.py"));
    assert!(owner.manifest().any(
        |(role, member)| role == RuntimeReadRole::Sidecar && member.path() == "owned_worker.py"
    ));
    let global = TorchVersionsLock::try_acquire(installed.runtime.parent().unwrap()).unwrap();
    assert!(TorchRevisionLease::mutation(
        installed.runtime.parent().unwrap(),
        "synthetic",
        &global
    )
    .is_err());
    TorchRevisionLease::mutation(installed.runtime.parent().unwrap(), "unrelated", &global)
        .unwrap();
    assert!(ManagedDepotLease::mutation(&installed.depot).is_err());
    let child_owner = owner.clone();
    drop(owner);
    assert!(ManagedDepotLease::mutation(&installed.depot).is_err());
    assert!(TorchRevisionLease::mutation(
        installed.runtime.parent().unwrap(),
        "synthetic",
        &global
    )
    .is_err());
    drop(child_owner);
    TorchRevisionLease::mutation(installed.runtime.parent().unwrap(), "synthetic", &global)
        .unwrap();
    ManagedDepotLease::mutation(&installed.depot).unwrap();
}

#[test]
fn missing_legacy_manifest_and_unreported_dependency_are_refused() {
    for case in 0..3 {
        let installed = Installed::new();
        match case {
            0 => std::fs::remove_file(installed.runtime.join("installed-files.json")).unwrap(),
            1 => std::fs::write(installed.runtime.join("installed-files.json"), b"{}").unwrap(),
            _ => std::fs::write(installed.packages.join("extra.py"), b"unreported").unwrap(),
        }
        assert!(installed.capture().is_err());
        ManagedDepotLease::mutation(&installed.depot).unwrap();
    }
}

#[test]
fn external_interpreter_sidecar_change_and_external_alias_refuse() {
    for case in 0..3 {
        let installed = Installed::new();
        match case {
            0 => {
                let mut recipe: serde_json::Value = serde_json::from_slice(
                    &std::fs::read(installed.runtime.join("runtime.json")).unwrap(),
                )
                .unwrap();
                recipe["managed_python"]["executable"]["path"] =
                    serde_json::json!("/usr/bin/python3");
                std::fs::write(
                    installed.runtime.join("runtime.json"),
                    serde_json::to_vec(&recipe).unwrap(),
                )
                .unwrap();
            }
            1 => {
                std::fs::write(installed.runtime.join("owned_worker.py"), b"replaced code").unwrap()
            }
            _ => std::os::unix::fs::symlink("/usr/bin/python3", installed.depot.join("external"))
                .unwrap(),
        }
        assert!(installed.capture().is_err());
    }
}

#[test]
fn interpreter_and_dependency_replacement_refuse_retained_validation() {
    for role in [RuntimeReadRole::Interpreter, RuntimeReadRole::Dependencies] {
        let installed = Installed::new();
        let owner = installed.capture().unwrap();
        let path = if role == RuntimeReadRole::Interpreter {
            installed.depot.join("lib/python3.12/os.py")
        } else {
            installed.packages.join("dependency.py")
        };
        let replacement = path.with_extension("replacement");
        std::fs::write(&replacement, std::fs::read(&path).unwrap()).unwrap();
        std::fs::rename(replacement, path).unwrap();
        assert!(owner.validate().is_err());
    }
}

#[test]
fn installer_removed_validation_script_is_optional_but_remaining_code_is_required() {
    let installed = Installed::new();
    std::fs::remove_file(installed.runtime.join("validate_runtime.py")).unwrap();
    let owner = installed
        .capture()
        .expect("direct installer removes its validation script");
    assert!(!owner.manifest().any(
        |(role, file)| role == RuntimeReadRole::Sidecar && file.path() == "validate_runtime.py"
    ));
    drop(owner);
    std::fs::remove_file(installed.runtime.join("owned_audio.py")).unwrap();
    assert!(installed.capture().is_err());
}

#[test]
fn retained_validation_script_must_still_match_the_embedded_bytes() {
    let installed = Installed::new();
    let owner = installed.capture().unwrap();
    assert!(owner.manifest().any(
        |(role, file)| role == RuntimeReadRole::Sidecar && file.path() == "validate_runtime.py"
    ));
    drop(owner);
    std::fs::write(
        installed.runtime.join("validate_runtime.py"),
        b"unreported validation code",
    )
    .unwrap();
    assert!(installed.capture().is_err());
}

#[test]
fn real_venv_bootstrap_is_removed_before_resolved_overlap_and_byte_capture() {
    real_venv_round_trip("pip");
}

#[test]
fn real_venv_bootstrap_is_removed_before_nonoverlap_byte_capture() {
    real_venv_round_trip("selected_fixture");
}

fn real_venv_round_trip(package: &str) {
    let mut installed = Installed::new();
    // Only this fixture's managed-depot bytes are synthetic. Bootstrap, pip,
    // the local wheel, its RECORD and the resulting venv tree are real. This
    // does not qualify the executing host interpreter or native dependencies.
    std::fs::remove_dir_all(installed.runtime.join("venv")).unwrap();
    let result = std::process::Command::new("python3")
        .args(["-I", "-B", "-m", "venv"])
        .arg(installed.runtime.join("venv"))
        .output()
        .expect("runtime custody tests require Python with venv");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let python = installed.runtime.join("venv/bin/python");
    let minor = std::process::Command::new(&python)
        .args([
            "-I",
            "-B",
            "-c",
            "import sys;print(f'{sys.version_info.major}.{sys.version_info.minor}')",
        ])
        .output()
        .unwrap();
    assert!(minor.status.success());
    let minor = String::from_utf8(minor.stdout).unwrap().trim().to_owned();
    installed.packages = installed
        .runtime
        .join(format!("venv/lib/python{minor}/site-packages"));
    assert!(installed.packages.join("pip/__init__.py").is_file());
    let bootstrap = installer::BootstrapPackages::capture(&installed.runtime, &minor).unwrap();
    let workspace = tempfile::tempdir().unwrap();
    let target = workspace.path().join("selected");
    let resolver = installed.runtime.join("resolve_runtime.py");
    let output = std::process::Command::new(&python).args(["-I", "-B", "-c", r#"
import base64, hashlib, json, pathlib, runpy, subprocess, sys, zipfile
root, resolver = map(pathlib.Path, sys.argv[1:3])
name = sys.argv[3]
target = root / 'selected'
wheel = root / f'{name}-99.0-py3-none-any.whl'
files = {
    f'{name}/__init__.py': b'VALUE = "resolved overlap, not bootstrap"\n',
    f'{name}-99.0.dist-info/METADATA': f'Metadata-Version: 2.1\nName: {name}\nVersion: 99.0\n'.encode(),
    f'{name}-99.0.dist-info/WHEEL': b'Wheel-Version: 1.0\nGenerator: pumas-fixture\nRoot-Is-Purelib: true\nTag: py3-none-any\n',
}
record = ''.join(f'{name},sha256={base64.urlsafe_b64encode(hashlib.sha256(data).digest()).rstrip(b"=").decode()},{len(data)}\n' for name, data in files.items())
files[f'{name}-99.0.dist-info/RECORD'] = (record + f'{name}-99.0.dist-info/RECORD,,\n').encode()
with zipfile.ZipFile(wheel, 'w') as archive:
    for member, data in files.items(): archive.writestr(member, data)
# Exercise the actual resolver imports and actual offline pip worker.
subprocess.run([sys.executable, '-I', '-B', str(resolver), '--help'], check=True, capture_output=True)
subprocess.run([sys.executable, '-I', '-B', str(resolver), '--_pumas-pip-progress-worker', str(root / 'progress.json'), '--isolated', 'install', '--no-index', '--no-deps', '--ignore-installed', '--no-compile', '--target', str(target), str(wheel)], check=True, capture_output=True)
namespace = runpy.run_path(str(resolver))
manifest = namespace['installed_file_manifest'](target, [{'name': name, 'version': '99.0'}])
(root / 'installed-files.json').write_text(json.dumps(manifest))
"#]).arg(workspace.path()).arg(&resolver).arg(package).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let manifest_bytes = std::fs::read(workspace.path().join("installed-files.json")).unwrap();
    let manifest: StagedFilesManifest = serde_json::from_slice(&manifest_bytes).unwrap();
    installer::validate_staged_files(&target, &manifest).unwrap();
    assert!(
        installer::validate_staged_files(&installed.packages, &manifest).is_err(),
        "unreported pip bootstrap must be refused"
    );
    bootstrap.remove().unwrap();
    installer::move_verified_packages(&target, &installed.runtime, &minor).unwrap();
    installer::validate_staged_files(&installed.packages, &manifest).unwrap();
    assert_eq!(
        std::fs::read_to_string(installed.packages.join(format!("{package}/__init__.py"))).unwrap(),
        "VALUE = \"resolved overlap, not bootstrap\"\n"
    );
    assert!(!installed.packages.join("pip/_internal").exists());
    let probe = std::process::Command::new(&python)
        .args([
            "-c",
            &format!(
                "import {package};assert {package}.VALUE == 'resolved overlap, not bootstrap'"
            ),
        ])
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(
        probe.status.success(),
        "{}",
        String::from_utf8_lossy(&probe.stderr)
    );
    installer::validate_staged_files(&installed.packages, &manifest).unwrap();
    std::fs::write(
        installed.runtime.join("installed-files.json"),
        manifest_bytes,
    )
    .unwrap();
    std::fs::remove_file(installed.runtime.join("validate_runtime.py")).unwrap();
    // Bind static capture back to the fixture-owned depot only after all real
    // venv processes have exited. No command runs the synthetic interpreter.
    std::fs::remove_file(&python).unwrap();
    std::os::unix::fs::symlink(installed.depot.join("bin/python3.12"), &python).unwrap();
    let mut recipe: serde_json::Value =
        serde_json::from_slice(&std::fs::read(installed.runtime.join("runtime.json")).unwrap())
            .unwrap();
    recipe["python"] = serde_json::json!(format!("python{minor}"));
    std::fs::write(
        installed.runtime.join("runtime.json"),
        serde_json::to_vec(&recipe).unwrap(),
    )
    .unwrap();
    let owner = installed
        .capture()
        .expect("published direct runtime tree must be retainable");
    assert!(owner
        .manifest()
        .any(|(role, file)| role == RuntimeReadRole::Dependencies
            && file.path() == format!("{package}/__init__.py")));
    owner.validate().unwrap();
    drop(owner);
    std::fs::write(installed.packages.join("unreported.py"), b"unexpected").unwrap();
    assert!(installed.capture().is_err());
}
