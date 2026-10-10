use super::*;

struct Installed {
    launcher: tempfile::TempDir,
    runtime: PathBuf,
    depot: PathBuf,
    packages: PathBuf,
}
impl Installed {
    fn new() -> Self {
        let launcher = tempfile::tempdir().unwrap();
        let versions = launcher.path().join(AppId::Torch.versions_dir_name());
        let runtime = versions.join("synthetic");
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
        capture_installed(self.launcher.path(), &self.runtime, lock)
    }
}

#[test]
fn closed_selected_bytes_hold_both_existing_mutation_leases() {
    let installed = Installed::new();
    let owner = installed.capture().unwrap();
    assert!(owner
        .manifest()
        .any(|(role, member)| role == RuntimeReadRole::Interpreter
            && member.path() == "lib/python3.12/os.py"));
    assert!(owner.manifest().any(
        |(role, member)| role == RuntimeReadRole::Sidecar && member.path() == "owned_worker.py"
    ));
    assert!(TorchVersionsLock::try_acquire(installed.runtime.parent().unwrap()).is_err());
    assert!(ManagedDepotLease::mutation(&installed.depot).is_err());
    let child_owner = owner.clone();
    drop(owner);
    assert!(ManagedDepotLease::mutation(&installed.depot).is_err());
    drop(child_owner);
    TorchVersionsLock::try_acquire(installed.runtime.parent().unwrap()).unwrap();
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

#[test]
fn inert_bytecode_and_site_hook_names_are_source_fixed_and_unselected() {
    for name in [
        "a.pyc",
        "a.pyo",
        "a.pth",
        "sitecustomize.py",
        "usercustomize.py",
        "x/__pycache__/cached.pyc",
        "unselected.pth/nested.py",
    ] {
        assert!(inert_import_member(name));
    }
    for name in [
        "worker.py",
        "sitecustomize.py.extra",
        "x/compiled.so",
        "x/cache.py",
    ] {
        assert!(!inert_import_member(name));
    }
    let installed = Installed::new();
    std::fs::write(
        installed.depot.join("lib/python3.12/cached.pyc"),
        b"inert bytecode",
    )
    .unwrap();
    let owner = installed.capture().unwrap();
    assert!(!owner
        .manifest()
        .any(|(_, file)| file.path().ends_with(".pyc")));
    assert!(owner
        .clone_member(RuntimeReadRole::Interpreter, "lib/python3.12/cached.pyc")
        .is_err());
}

#[test]
fn only_exact_generated_entrypoints_and_records_are_inert_dependencies() {
    assert!(inert_dependency_member("bin/torchrun"));
    assert!(inert_dependency_member("torch-2.10.0+cpu.dist-info/RECORD"));
    for name in [
        "bin/custom.py",
        "torch/bin/FileStoreTest",
        "torch-2.10.0+cpu.dist-info/METADATA",
        "other.dist-info/RECORD",
        "nested/bin/torchrun",
    ] {
        assert!(!inert_dependency_member(name), "{name}");
    }
    let installed = Installed::new();
    for name in [
        "bin/torchrun",
        "torch-2.10.0+cpu.dist-info/RECORD",
        "torch-2.10.0+cpu.dist-info/METADATA",
    ] {
        let path = installed.packages.join(name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"fixture metadata or entrypoint").unwrap();
    }
    let (files, _) = tree_manifest(&installed.packages, &[], false).unwrap();
    let files = files.iter().map(|file| serde_json::json!({"path":file.path(), "size":file.size(), "sha256":file.sha256()})).collect::<Vec<_>>();
    std::fs::write(
        installed.runtime.join("installed-files.json"),
        serde_json::to_vec(&serde_json::json!({"files":files})).unwrap(),
    )
    .unwrap();
    let retained = installed.capture().unwrap();
    assert!(retained
        .clone_member(RuntimeReadRole::Dependencies, "bin/torchrun")
        .is_err());
    assert!(retained
        .clone_member(
            RuntimeReadRole::Dependencies,
            "torch-2.10.0+cpu.dist-info/RECORD"
        )
        .is_err());
    assert!(retained
        .clone_member(
            RuntimeReadRole::Dependencies,
            "torch-2.10.0+cpu.dist-info/METADATA"
        )
        .is_ok());
    let script = installed.packages.join("bin/torchrun");
    std::fs::rename(&script, script.with_extension("old")).unwrap();
    std::fs::write(&script, b"fixture metadata or entrypoint").unwrap();
    assert!(retained.validate().is_err());
}

#[test]
#[ignore = "requires two independently installed exact official managed runtimes; no execution"]
fn independently_installed_roots_match_fixed_candidate_selections() {
    let roots: Vec<PathBuf> = serde_json::from_str(
        &std::env::var("PUMAS_AUDIO_RECIPE_ROOTS")
            .expect("two explicit installation roots required"),
    )
    .unwrap();
    assert_eq!(roots.len(), 2);
    for launcher in roots {
        let runtime = launcher.join("torch-versions/v2.10.0");
        let recipe: serde_json::Value =
            serde_json::from_slice(&std::fs::read(runtime.join("runtime.json")).unwrap()).unwrap();
        let executable = PathBuf::from(
            recipe["managed_python"]["executable"]["path"]
                .as_str()
                .unwrap(),
        );
        let depot = executable
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .parent()
            .unwrap();
        assert!(depot.starts_with(&launcher));
        let lease = Arc::new(ManagedDepotLease::read(depot).unwrap());
        let (files, omissions) = tree_manifest(depot, &[], true).unwrap();
        let interpreter = RuntimeReadRoot::new(
            RuntimeReadRole::Interpreter,
            strict_directory(depot).unwrap(),
            files,
            omissions,
            lease,
        )
        .unwrap();
        let packages = runtime.join("venv/lib/python3.12/site-packages");
        let installed: StagedFilesManifest =
            serde_json::from_slice(&std::fs::read(runtime.join("installed-files.json")).unwrap())
                .unwrap();
        installer::validate_staged_files(&packages, &installed).unwrap();
        let omissions = installed
            .files
            .iter()
            .filter(|file| inert_dependency_member(&file.path))
            .map(|file| file.path.clone())
            .collect();
        let files = installed
            .files
            .into_iter()
            .filter(|file| !inert_dependency_member(&file.path))
            .map(|file| RuntimeReadFile::new(file.path, file.size, file.sha256).unwrap())
            .collect();
        let lock =
            Arc::new(TorchVersionsLock::try_acquire_read(runtime.parent().unwrap()).unwrap());
        let dependencies = RuntimeReadRoot::new(
            RuntimeReadRole::Dependencies,
            strict_directory(&packages).unwrap(),
            files,
            omissions,
            lock,
        )
        .unwrap();
        let retained = RetainedRuntimeReadSource::capture(vec![interpreter, dependencies]).unwrap();
        super::super::audio_runtime_recipe::validate(&recipe, &retained).unwrap();
        // Parse each actual relocated field as a double-quoted JSON string,
        // without executing sysconfig source. All 27 substitutions must occur
        // inside those fixed string values, never Python expression positions.
        let pin: serde_json::Value = serde_json::from_str(
            pumas_library::runtime_read_source::AUDIO_RUNTIME_CANDIDATE_RECIPE,
        )
        .unwrap();
        let relocation = &pin["interpreter_relocation"];
        let prefix = depot
            .join(relocation["distribution"].as_str().unwrap())
            .to_str()
            .unwrap()
            .to_owned();
        let mut text = String::new();
        retained
            .clone_member(
                RuntimeReadRole::Interpreter,
                relocation["member"].as_str().unwrap(),
            )
            .unwrap()
            .read_to_string(&mut text)
            .unwrap();
        let mut occurrences = 0;
        for line in text.lines().filter(|line| line.contains(&prefix)) {
            let value = line.split_once(": ").unwrap().1.trim_end_matches(',');
            let string: String = serde_json::from_str(value).unwrap();
            occurrences += string.matches(&prefix).count();
        }
        assert_eq!(occurrences, 27);
        assert_eq!(occurrences, text.matches(&prefix).count());
        retained.validate().unwrap();
    }
}

#[test]
fn only_fixed_uv_directory_alias_is_identity_retained_without_traversal() {
    let root = tempfile::tempdir().unwrap();
    let target = root.path().join("cpython-3.12.14-linux-x86_64-gnu");
    std::fs::create_dir(&target).unwrap();
    std::fs::write(target.join("selected.py"), b"fixed fixture").unwrap();
    let alias = root.path().join("cpython-3.12-linux-x86_64-gnu");
    std::os::unix::fs::symlink(&target, &alias).unwrap();
    let (members, omitted) = tree_manifest(root.path(), &[], true).unwrap();
    assert_eq!(members.len(), 1);
    assert_eq!(omitted, ["cpython-3.12-linux-x86_64-gnu"]);
    assert!(tree_manifest(root.path(), &[], false).is_err());
    std::os::unix::fs::symlink(&target, root.path().join("caller-alias")).unwrap();
    assert!(tree_manifest(root.path(), &[], true).is_err());
}

#[tokio::test]
async fn cancelled_preparation_waiter_keeps_lifecycle_and_shutdown_joins_nested_work() {
    let root = tempfile::tempdir().unwrap();
    let manager = VersionManager::new(root.path(), AppId::Torch)
        .await
        .unwrap();
    let (entered_send, entered) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let preparing = manager.clone();
    let waiter = tokio::spawn(async move {
        preparing
            .owned_runtime_preparation(move |_owner| async move {
                tokio::task::spawn_blocking(move || {
                    entered_send.send(()).unwrap();
                    released.recv().unwrap();
                })
                .await
                .map_err(|error| refused(error.to_string()))?;
                Ok(())
            })
            .await
    });
    entered.await.unwrap();
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    assert!(manager.lifecycle_lock.try_lock().is_err());
    let shutting = manager.clone();
    let shutdown_waiter = tokio::spawn(async move { shutting.shutdown_installations().await });
    while !manager.torch_shutting_down.load(Ordering::SeqCst) {
        tokio::task::yield_now().await;
    }
    shutdown_waiter.abort();
    assert!(shutdown_waiter.await.unwrap_err().is_cancelled());
    assert!(manager
        .owned_runtime_preparation(|_| async { Ok(()) })
        .await
        .is_err());
    let joined = manager.shutdown_installations();
    tokio::pin!(joined);
    assert!(tokio::time::timeout(Duration::from_millis(20), &mut joined)
        .await
        .is_err());
    assert!(manager.lifecycle_lock.try_lock().is_err());
    release.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), &mut joined)
        .await
        .unwrap()
        .unwrap();
    assert!(manager.lifecycle_lock.try_lock().is_ok());
    manager.shutdown_installations().await.unwrap();
}

#[tokio::test]
async fn ordinary_preparation_refusal_is_not_shutdown_cleanup_failure() {
    let root = tempfile::tempdir().unwrap();
    let manager = VersionManager::new(root.path(), AppId::Torch)
        .await
        .unwrap();
    let result: Result<()> = manager
        .owned_runtime_preparation(|_| async { Err(refused("controlled source refusal")) })
        .await;
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("controlled source refusal"));
    manager.shutdown_installations().await.unwrap();
}

#[tokio::test]
async fn active_preparation_refuses_missing_or_changed_selection_before_capture() {
    let root = tempfile::tempdir().unwrap();
    let manager = VersionManager::new(root.path(), AppId::Torch)
        .await
        .unwrap();
    let error = manager
        .prepare_active_torch_audio_runtime_bytes("unselected-tag")
        .await
        .expect_err("stale active selection must refuse")
        .to_string();
    assert!(error.contains("Active Torch runtime selection changed"));
    manager.shutdown_installations().await.unwrap();
}
