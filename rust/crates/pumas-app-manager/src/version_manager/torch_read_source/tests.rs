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
