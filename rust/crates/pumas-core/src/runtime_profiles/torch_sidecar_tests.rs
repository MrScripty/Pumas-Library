use super::*;

fn system_python() -> PathBuf {
    PathBuf::from(if cfg!(windows) { "python" } else { "python3" })
}

fn files_under(root: &Path) -> Vec<String> {
    let mut files = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(directory) = pending.pop() {
        for entry in std::fs::read_dir(directory).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path.strip_prefix(root).unwrap();
                files.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    files.sort();
    files
}

#[test]
fn sidecar_materializes_every_embedded_file_once_per_identity() {
    let launcher = tempfile::tempdir().unwrap();
    let sidecar = materialize_torch_sidecar(launcher.path()).unwrap();
    assert_eq!(sidecar, torch_sidecar_dir(launcher.path()));
    assert!(sidecar.starts_with(launcher.path().join("launcher-data/torch-sidecar")));
    assert!(torch_sidecar_id().starts_with(concat!(env!("CARGO_PKG_VERSION"), "-")));
    assert_eq!(
        torch_sidecar_entry(launcher.path()),
        sidecar.join("serve.py")
    );

    let mut expected = SIDECAR_FILES
        .iter()
        .map(|(name, _)| name.to_string())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(files_under(&sidecar), expected);
    for (name, contents) in SIDECAR_FILES {
        assert!(
            !contents.is_empty(),
            "embedded sidecar file is empty: {name}"
        );
        assert_eq!(
            std::fs::read(sidecar.join(name)).unwrap(),
            contents.as_bytes()
        );
    }
    // Packages are resolved per installation; the sidecar carries no lock,
    // preset validator or preset recipe.
    for absent in ["requirements.txt", "validate_runtime.py", "runtime.json"] {
        assert!(!sidecar.join(absent).exists(), "{absent}");
    }

    // A second preparation reuses the published directory unchanged.
    let marker = std::fs::metadata(sidecar.join("serve.py"))
        .unwrap()
        .modified()
        .unwrap();
    assert_eq!(materialize_torch_sidecar(launcher.path()).unwrap(), sidecar);
    assert_eq!(
        std::fs::metadata(sidecar.join("serve.py"))
            .unwrap()
            .modified()
            .unwrap(),
        marker
    );
}

#[test]
fn a_changed_missing_or_extra_file_replaces_the_sidecar() {
    let launcher = tempfile::tempdir().unwrap();
    let sidecar = materialize_torch_sidecar(launcher.path()).unwrap();
    let damage: [&dyn Fn(); 4] = [
        &|| std::fs::write(sidecar.join("validation.py"), "# changed\n").unwrap(),
        &|| std::fs::remove_file(sidecar.join("loaders/dllm_loader.py")).unwrap(),
        // An extra module would be importable ahead of the standard library.
        &|| std::fs::write(sidecar.join("json.py"), "raise SystemExit\n").unwrap(),
        &|| std::fs::create_dir(sidecar.join("__pycache__")).unwrap(),
    ];
    for damage in damage {
        damage();
        assert!(!matches_embedded(&sidecar).unwrap());
        assert_eq!(materialize_torch_sidecar(launcher.path()).unwrap(), sidecar);
        assert!(matches_embedded(&sidecar).unwrap());
    }
    // Replaced copies are pruned, leaving only the current identity.
    let parent = launcher.path().join("launcher-data/torch-sidecar");
    let names = std::fs::read_dir(&parent)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect::<Vec<_>>();
    assert_eq!(names, vec![torch_sidecar_id().to_owned()]);
}

#[test]
fn other_identities_are_pruned_but_a_live_publication_is_kept() {
    let launcher = tempfile::tempdir().unwrap();
    let parent = launcher.path().join("launcher-data/torch-sidecar");
    let earlier = parent.join("0.7.0-0123456789abcdef");
    let staging = parent.join(".staging-live");
    for directory in [&earlier, &staging] {
        std::fs::create_dir_all(directory.join("loaders")).unwrap();
        std::fs::write(directory.join("serve.py"), "# other build\n").unwrap();
    }

    materialize_torch_sidecar(launcher.path()).unwrap();

    assert!(!earlier.exists());
    assert!(staging.exists(), "a recent staging directory may be in use");
    assert!(torch_sidecar_dir(launcher.path()).is_dir());
}

fn stub_check(report: &str) -> tempfile::TempDir {
    let sidecar = tempfile::tempdir().unwrap();
    std::fs::write(sidecar.path().join(COMPATIBILITY_SCRIPT), report).unwrap();
    sidecar
}

#[test]
fn incompatible_runtime_is_refused_with_every_reason() {
    let sidecar = stub_check(
        "import json\nprint(json.dumps({'compatible': False, 'problems': \
         ['uvicorn is not installed', 'fastapi 0.100.0 is older than 0.109.1']}))\n",
    );
    let error = check_torch_runtime_compatibility(&system_python(), sidecar.path())
        .expect_err("an incompatible runtime must be refused")
        .to_string();
    assert!(
        error.contains("cannot run the sidecar shipped with Pumas"),
        "{error}"
    );
    assert!(error.contains("uvicorn is not installed"), "{error}");
    assert!(
        error.contains("fastapi 0.100.0 is older than 0.109.1"),
        "{error}"
    );
    assert!(error.contains("reinstall"), "{error}");

    let compatible =
        stub_check("import json\nprint(json.dumps({'compatible': True, 'problems': []}))\n");
    check_torch_runtime_compatibility(&system_python(), compatible.path()).unwrap();
}

#[test]
fn a_check_that_cannot_run_is_a_refusal_with_its_reason() {
    let crashing = stub_check("import sys\nsys.exit('broken interpreter fixture')\n");
    let error = check_torch_runtime_compatibility(&system_python(), crashing.path())
        .unwrap_err()
        .to_string();
    assert!(error.contains("compatibility check failed"), "{error}");
    assert!(error.contains("broken interpreter fixture"), "{error}");

    let missing = tempfile::tempdir().unwrap();
    let error = check_torch_runtime_compatibility(&missing.path().join("python"), crashing.path())
        .unwrap_err()
        .to_string();
    assert!(error.contains("cannot start"), "{error}");
}

#[test]
fn shipped_check_reports_against_a_real_interpreter_without_writing() {
    let launcher = tempfile::tempdir().unwrap();
    let sidecar = materialize_torch_sidecar(launcher.path()).unwrap();
    // The host interpreter may or may not carry the sidecar's packages; either
    // way the shipped check must run and report rather than fail.
    if let Err(error) = check_torch_runtime_compatibility(&system_python(), &sidecar) {
        let error = error.to_string();
        assert!(
            error.contains("cannot run the sidecar shipped with Pumas"),
            "{error}"
        );
    }
    assert!(
        matches_embedded(&sidecar).unwrap(),
        "the check wrote next to the sidecar"
    );
}
