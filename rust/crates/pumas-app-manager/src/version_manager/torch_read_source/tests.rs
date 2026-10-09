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
