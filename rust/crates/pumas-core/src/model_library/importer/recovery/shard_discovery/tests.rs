use super::*;
use crate::model_library::{MissingShardRange, ShardSetDiscoveryStatus as Status};
use std::collections::BTreeMap;
use std::fs;

fn write(root: &Path, relative: &str, bytes: &[u8]) {
    let path = root.join(relative);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, bytes).unwrap();
}

fn has(report: &ShardRecoveryDiscovery, kind: Kind) -> bool {
    report
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.kind == kind)
}

fn snapshot(root: &Path) -> BTreeSet<PathBuf> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(|entry| entry.unwrap().path().strip_prefix(root).unwrap().to_owned())
        .collect()
}

#[test]
fn canonical_models_keep_all_nested_sets_separate_without_writes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    for file in [
        "llm/family/one/a/weights-1-of-2.gguf",
        "llm/family/one/a/weights-2-of-2.gguf",
        "llm/family/one/b/weights-1-of-2.gguf",
        "llm/family/one/b/weights-1-of-3.bin",
        "llm/family/one/standalone.gguf",
        "llm/family/two/weights-2-of-3.gguf",
        "llm/family/three__variant--q4/weights-1-of-1.gguf",
    ] {
        write(root, file, b"weight");
    }
    let before = snapshot(root);
    let report = discover(root);
    assert!(report.enumeration_complete, "{:?}", report.diagnostics);
    assert_eq!(report.model_roots.len(), 3);
    let one = report
        .model_roots
        .iter()
        .find(|model| model.library_relative_root == Path::new("llm/family/one"))
        .unwrap();
    assert_eq!(one.shard_sets.len(), 3);
    assert!(one
        .shard_sets
        .iter()
        .any(|set| set.relative_directory == Path::new("a")
            && set.status == Status::CountedOrdinalsPresent));
    let nested = one
        .shard_sets
        .iter()
        .find(|set| set.relative_directory == Path::new("b") && set.base_name == "weights.gguf")
        .unwrap();
    assert_eq!(nested.status, Status::MissingOrdinals);
    assert_eq!(
        nested.missing_ordinals,
        vec![MissingShardRange { first: 2, last: 2 }]
    );
    assert_eq!(nested.files, vec![PathBuf::from("b/weights-1-of-2.gguf")]);
    assert_eq!(snapshot(root), before);
    assert!(!root.join(".pumas-library-id.json").exists());
    let legacy = legacy_projection(report);
    assert_eq!(legacy.len(), 2);
    assert!(legacy
        .iter()
        .any(|item| item.existing_files.contains(&"standalone.gguf".into())));
    assert!(legacy.iter().any(|item| item
        .existing_files
        .contains(&"b/weights-1-of-2.gguf".into())));
}

#[test]
fn metadata_does_not_hide_nested_observations() {
    let temp = tempfile::tempdir().unwrap();
    write(temp.path(), "llm/family/model/metadata.json", b"{}");
    write(
        temp.path(),
        "llm/family/model/nested/model-1-of-2.gguf",
        b"one",
    );
    let report = discover(temp.path());
    assert!(report.enumeration_complete);
    assert!(report.model_roots[0].has_metadata);
    assert_eq!(
        report.model_roots[0].shard_sets[0].status,
        Status::MissingOrdinals
    );
    assert!(legacy_projection(report).is_empty());
}

#[test]
fn legacy_ancestors_and_noncanonical_names_are_not_repository_roots() {
    let temp = tempfile::tempdir().unwrap();
    write(temp.path(), "legacy/weights-1-of-2.gguf", b"one");
    write(
        temp.path(),
        "legacy/nested/not-a-root/weights-1-of-2.gguf",
        b"one",
    );
    write(temp.path(), "llm/family/weights-1-of-2.gguf", b"one");
    write(temp.path(), "llm/family/nested/weights-1-of-2.gguf", b"one");
    write(
        temp.path(),
        "llm/Other Family/model/weights-1-of-2.gguf",
        b"one",
    );
    let report = discover(temp.path());
    assert!(!report.enumeration_complete);
    assert!(has(&report, Kind::NonCanonicalLayout));
    assert!(report.model_roots.is_empty());
}

#[test]
fn nested_package_boundary_is_ambiguous_not_a_second_root() {
    let temp = tempfile::tempdir().unwrap();
    write(temp.path(), "llm/family/model/nested/metadata.json", b"{}");
    write(
        temp.path(),
        "llm/family/model/nested/model-1-of-2.gguf",
        b"one",
    );
    let report = discover(temp.path());
    assert_eq!(report.model_roots.len(), 1);
    assert!(!report.enumeration_complete);
    assert!(!report.model_roots[0].enumeration_complete);
    assert!(has(&report, Kind::NonCanonicalLayout));
    assert!(legacy_projection(report).is_empty());
}

#[test]
fn failed_enumeration_and_failed_entry_never_mean_empty_or_complete() {
    for phase in [
        Phase::BeforeEnumeration,
        Phase::EnumerationEntry,
        Phase::AfterEnumeration,
    ] {
        let temp = tempfile::tempdir().unwrap();
        write(
            temp.path(),
            "llm/family/model/nested/model-1-of-2.gguf",
            b"one",
        );
        let mut hook = |path: &Path, current| {
            if path == Path::new("llm/family/model/nested") && current == phase {
                Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "injected unreadable directory",
                ))
            } else {
                Ok(())
            }
        };
        let report = scan(temp.path(), &mut hook);
        assert!(!report.enumeration_complete);
        assert!(!report.model_roots[0].enumeration_complete);
        assert!(has(&report, Kind::EnumerationFailed));
        assert!(legacy_projection(report).is_empty());
    }
}

#[test]
fn failed_worker_is_not_an_empty_success() {
    let report = worker_failed(Path::new("library"), "worker cancelled");
    assert!(!report.enumeration_complete);
    assert!(has(&report, Kind::WorkerFailed));
    assert!(report.model_roots.is_empty());
}

#[test]
fn malformed_ordinals_are_diagnostic_and_never_counted_as_coverage() {
    let temp = tempfile::tempdir().unwrap();
    for file in [
        "zero-0-of-2.gguf",
        "signed--1-of-2.gguf",
        "signedtotal-1-of--2.gguf",
        "total-1-of-0.gguf",
        "range-3-of-2.gguf",
        "duplicate-01-of-2.gguf",
        "duplicate-1-of-2.gguf",
        "inconsistent-1-of-2.gguf",
        "inconsistent-2-of-3.gguf",
        "overflow-1-of-99999999999999999999999999999999999999.gguf",
        "overflow-99999999999999999999999999999999999999-of-2.gguf",
        "malformed-one-of-2.gguf",
        "multiple-1-of-2-1-of-2.gguf",
        "unknown_00001.gguf",
        "parts.gguf.part1",
        "mixed.gguf",
        "mixed-1-of-2.gguf",
    ] {
        write(temp.path(), &format!("llm/family/model/{file}"), b"one");
    }
    let report = discover(temp.path());
    for kind in [
        Kind::InvalidShardOrdinal,
        Kind::DuplicateShardOrdinal,
        Kind::InconsistentShardTotals,
        Kind::InvalidShardName,
        Kind::AmbiguousShardNames,
    ] {
        assert!(has(&report, kind), "{kind:?}");
    }
    assert!(report.model_roots[0]
        .shard_sets
        .iter()
        .all(|set| set.status == Status::Ambiguous));
    assert!(legacy_projection(report).is_empty());
}

#[test]
fn huge_declared_total_has_compact_missing_ranges() {
    let temp = tempfile::tempdir().unwrap();
    write(
        temp.path(),
        &format!("llm/family/model/weight-1-of-{}.gguf", usize::MAX),
        b"one",
    );
    let report = discover(temp.path());
    assert_eq!(
        report.model_roots[0].shard_sets[0].missing_ordinals,
        vec![MissingShardRange {
            first: 2,
            last: usize::MAX
        }]
    );
}

#[test]
fn counted_ordinals_do_not_hide_missing_or_invalid_index_evidence() {
    let temp = tempfile::tempdir().unwrap();
    write(
        temp.path(),
        "llm/family/missing/model-1-of-1.safetensors",
        b"one",
    );
    write(
        temp.path(),
        "llm/family/unobserved/model-1-of-2.bin",
        b"one",
    );
    write(
        temp.path(),
        "llm/family/unobserved/model.bin.index.json",
        br#"{"weight_map":{"one":"model-1-of-2.bin","two":"model-2-of-2.bin"}}"#,
    );
    write(temp.path(), "llm/family/omitted/model-1-of-1.bin", b"one");
    write(
        temp.path(),
        "llm/family/omitted/model.bin.index.json",
        br#"{"weight_map":{"one":"other.bin"}}"#,
    );
    let report = discover(temp.path());
    assert!(has(&report, Kind::MissingShardIndex));
    assert!(has(&report, Kind::MissingIndexedFile));
    assert!(has(&report, Kind::InvalidShardIndex));
    let missing = report
        .model_roots
        .iter()
        .find(|model| model.library_relative_root.ends_with("missing"))
        .unwrap();
    assert_eq!(missing.shard_sets[0].status, Status::CountedOrdinalsPresent);
    let unobserved = report
        .model_roots
        .iter()
        .find(|model| model.library_relative_root.ends_with("unobserved"))
        .unwrap();
    assert_eq!(
        unobserved.indexes[0].missing_files,
        vec![PathBuf::from("model-2-of-2.bin")]
    );
}

#[test]
fn index_decode_rejects_malformed_duplicate_and_escaping_references() {
    for bytes in [
        b"not json".as_slice(),
        br#"{}"#,
        br#"{"weight_map":{}}"#,
        br#"{"weight_map":{"a":1}}"#,
        br#"{"weight_map":{"a":"one.bin","a":"two.bin"}}"#,
        br#"{"weight_map":{"a":"one.bin"},"weight_map":{"b":"two.bin"}}"#,
        br#"{"weight_map":{"a":"../sentinel.bin"}}"#,
        br#"{"weight_map":{"a":"/sentinel.bin"}}"#,
        br#"{"weight_map":{"a":"C:\\sentinel.bin"}}"#,
        br#"{"weight_map":{"a":"a//b.bin"}}"#,
    ] {
        let temp = tempfile::tempdir().unwrap();
        write(temp.path(), "llm/family/model/model.bin.index.json", bytes);
        let report = discover(temp.path());
        assert!(!report.enumeration_complete, "{bytes:?}");
        assert!(has(&report, Kind::InvalidShardIndex));
        assert!(!report.model_roots[0].indexes[0].valid);
    }
}

#[test]
fn index_references_remain_relative_to_their_held_directory() {
    let temp = tempfile::tempdir().unwrap();
    write(
        temp.path(),
        "llm/family/model/encoder/model-1-of-1.bin",
        b"one",
    );
    write(
        temp.path(),
        "llm/family/model/encoder/model.bin.index.json",
        br#"{"weight_map":{"a":"model-1-of-1.bin"}}"#,
    );
    let report = discover(temp.path());
    assert!(report.enumeration_complete);
    assert!(report.diagnostics.is_empty());
    assert_eq!(
        report.model_roots[0].indexes[0].referenced_files,
        vec![PathBuf::from("encoder/model-1-of-1.bin")]
    );
    assert!(report.model_roots[0].indexes[0].missing_files.is_empty());
}

fn byte_snapshot(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    walkdir::WalkDir::new(root)
        .into_iter()
        .map(|entry| {
            let entry = entry.unwrap();
            let relative = entry.path().strip_prefix(root).unwrap().to_owned();
            let contents = entry
                .file_type()
                .is_file()
                .then(|| fs::read(entry.path()).unwrap());
            (relative, contents)
        })
        .collect()
}

fn rename_control(target: &Path, moved: &Path) {
    assert!(!moved.exists());
    fs::rename(target, moved).expect("unbound rename positive control must succeed");
    assert!(!target.exists());
    fs::rename(moved, target).expect("unbound rename control must restore the source");
}

/// False is only a Windows refusal candidate. Each caller must also demonstrate
/// unchanged bytes/names and a successful same-path rename after handles drop.
fn attempt_held_rename(target: &Path, moved: &Path) -> bool {
    match fs::rename(target, moved) {
        Ok(()) => true,
        #[cfg(windows)]
        Err(error) => {
            use windows_sys::Win32::Foundation::{ERROR_ACCESS_DENIED, ERROR_SHARING_VIOLATION};
            let code = error.raw_os_error();
            assert!(
                code == Some(ERROR_ACCESS_DENIED as i32)
                    || code == Some(ERROR_SHARING_VIOLATION as i32),
                "unexpected rename error, not evidence of held-directory pinning: {error}"
            );
            false
        }
        #[cfg(not(windows))]
        Err(error) => {
            panic!("held-directory replacement must reach rebinding on this target: {error}")
        }
    }
}

#[test]
fn root_or_configured_ancestor_replacement_discards_evidence_or_is_natively_pinned() {
    for replace_ancestor in [false, true] {
        let temp = tempfile::tempdir().unwrap();
        let outer = temp.path().join("outer");
        let root = outer.join("library");
        let original = "llm/family/model/model-1-of-2.gguf";
        write(&root, original, b"original");
        write(temp.path(), "outside/sentinel", b"preserve");
        let target = if replace_ancestor {
            outer.clone()
        } else {
            root.clone()
        };
        let original_below_target = root
            .join(original)
            .strip_prefix(&target)
            .unwrap()
            .to_owned();
        let moved = temp.path().join("held-original");
        rename_control(&target, &moved);
        let before = byte_snapshot(temp.path());
        let mut outcome = None;
        let mut hook = |_path: &Path, phase| {
            if phase == Phase::BeforeFinish && outcome.is_none() {
                let renamed = attempt_held_rename(&target, &moved);
                if renamed {
                    fs::create_dir_all(&root).unwrap();
                    write(
                        &root,
                        "llm/family/sentinel/model-1-of-1.gguf",
                        b"replacement",
                    );
                }
                outcome = Some(renamed);
            }
            Ok(())
        };
        let report = scan(&root, &mut hook);
        assert_eq!(
            fs::read(temp.path().join("outside/sentinel")).unwrap(),
            b"preserve"
        );
        if outcome.expect("replacement checkpoint must run") {
            assert!(!report.enumeration_complete);
            assert!(has(&report, Kind::BindingChanged));
            assert!(report.model_roots.is_empty());
            assert_eq!(
                fs::read(root.join("llm/family/sentinel/model-1-of-1.gguf")).unwrap(),
                b"replacement"
            );
            assert_eq!(
                fs::read(moved.join(original_below_target)).unwrap(),
                b"original"
            );
        } else {
            assert!(cfg!(windows), "only the native Windows refusal is accepted");
            assert!(report.enumeration_complete, "{:?}", report.diagnostics);
            assert!(!has(&report, Kind::BindingChanged));
            assert_eq!(byte_snapshot(temp.path()), before);
            // Discovery has returned and dropped all held directories. The same
            // operation must now work, ruling out a persistent permission/path error.
            rename_control(&target, &moved);
            assert_eq!(byte_snapshot(temp.path()), before);
            assert_eq!(
                report.model_roots[0].shard_sets[0].status,
                Status::MissingOrdinals
            );
        }
    }
}

#[test]
fn family_model_and_nested_child_replacement_is_refused_or_natively_pinned() {
    for target_relative in ["llm/family", "llm/family/model", "llm/family/model/nested"] {
        for phase in [Phase::DirectoryBound, Phase::BeforeFinish] {
            let temp = tempfile::tempdir().unwrap();
            let root = temp.path().join("library");
            let original = "llm/family/model/nested/model-1-of-2.gguf";
            write(&root, original, b"original");
            write(temp.path(), "outside/sentinel", b"preserve");
            let target = root.join(target_relative);
            let original_below_target = root
                .join(original)
                .strip_prefix(&target)
                .unwrap()
                .to_owned();
            let moved = temp.path().join("held-original");
            rename_control(&target, &moved);
            let before = byte_snapshot(temp.path());
            let mut outcome = None;
            let mut hook = |path: &Path, current| {
                if outcome.is_none()
                    && current == phase
                    && (phase == Phase::BeforeFinish || path == Path::new(target_relative))
                {
                    let renamed = attempt_held_rename(&target, &moved);
                    if renamed {
                        fs::create_dir_all(&target).unwrap();
                        write(&target, "sentinel-1-of-1.gguf", b"replacement");
                    }
                    outcome = Some(renamed);
                }
                Ok(())
            };
            let report = scan(&root, &mut hook);
            assert_eq!(
                fs::read(temp.path().join("outside/sentinel")).unwrap(),
                b"preserve"
            );
            if outcome.expect("replacement checkpoint must run") {
                assert!(!report.enumeration_complete, "{target_relative}");
                assert!(has(&report, Kind::BindingChanged));
                assert!(report
                    .model_roots
                    .iter()
                    .all(|model| !model.enumeration_complete || model.shard_sets.is_empty()));
                assert!(legacy_projection(report).is_empty());
                assert_eq!(
                    fs::read(target.join("sentinel-1-of-1.gguf")).unwrap(),
                    b"replacement"
                );
                assert_eq!(
                    fs::read(moved.join(original_below_target)).unwrap(),
                    b"original"
                );
            } else {
                assert!(cfg!(windows), "only the native Windows refusal is accepted");
                assert!(report.enumeration_complete, "{:?}", report.diagnostics);
                assert!(!has(&report, Kind::BindingChanged));
                assert_eq!(byte_snapshot(temp.path()), before);
                rename_control(&target, &moved);
                assert_eq!(byte_snapshot(temp.path()), before);
                assert_eq!(
                    report.model_roots[0].shard_sets[0].status,
                    Status::MissingOrdinals
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn configured_root_alias_is_supported_but_descendant_symlinks_are_not_read() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("library");
    write(&root, "llm/family/model/model-1-of-2.gguf", b"one");
    let outside = temp.path().join("outside");
    write(&outside, "sentinel-1-of-1.gguf", b"preserve");
    symlink(&root, temp.path().join("configured-alias")).unwrap();
    let initial = discover(&temp.path().join("configured-alias"));
    assert!(initial.enumeration_complete);
    assert_eq!(
        initial.model_roots[0].model_dir,
        root.canonicalize().unwrap().join("llm/family/model")
    );
    symlink(&outside, root.join("llm/family/model/escape")).unwrap();
    symlink(
        outside.join("sentinel-1-of-1.gguf"),
        root.join("llm/family/model/model-2-of-2.gguf"),
    )
    .unwrap();
    let report = discover(&temp.path().join("configured-alias"));
    assert!(!report.enumeration_complete);
    assert!(has(&report, Kind::SymlinkRefused));
    assert_eq!(report.model_roots[0].shard_sets.len(), 1);
    assert_eq!(
        report.model_roots[0].shard_sets[0].status,
        Status::MissingOrdinals
    );
    assert_eq!(
        fs::read(outside.join("sentinel-1-of-1.gguf")).unwrap(),
        b"preserve"
    );
}

#[cfg(unix)]
#[test]
fn bound_child_replaced_by_symlink_never_reads_the_sentinel_tree() {
    use std::os::unix::fs::symlink;
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("library");
    write(&root, "llm/family/model/nested/model-1-of-2.gguf", b"one");
    let outside = temp.path().join("outside");
    write(&outside, "sentinel-1-of-1.gguf", b"preserve");
    let mut changed = false;
    let mut hook = |path: &Path, phase| {
        if !changed
            && phase == Phase::DirectoryBound
            && path == Path::new("llm/family/model/nested")
        {
            fs::rename(root.join(path), temp.path().join("held-original")).unwrap();
            symlink(&outside, root.join(path)).unwrap();
            changed = true;
        }
        Ok(())
    };
    let report = scan(&root, &mut hook);
    assert!(!report.enumeration_complete);
    assert!(has(&report, Kind::BindingChanged));
    assert!(report
        .model_roots
        .iter()
        .all(|model| model.shard_sets.is_empty()));
    assert_eq!(
        fs::read(outside.join("sentinel-1-of-1.gguf")).unwrap(),
        b"preserve"
    );
}

#[test]
fn entry_and_depth_budgets_are_explicit_incomplete_observations() {
    let temp = tempfile::tempdir().unwrap();
    write(temp.path(), "llm/family/model/model-1-of-2.gguf", b"one");
    let binding = RootBinding::open(temp.path()).unwrap();
    let mut scanner = Scanner {
        report: empty_report(&binding.canonical),
        root: binding,
        remaining_entries: 0,
        bindings: Vec::new(),
        checkpoint: |_: &Path, _: Phase| Ok(()),
    };
    scanner.layout(scanner.root.held.clone(), Path::new(""), 0);
    assert!(!scanner.report.enumeration_complete);
    assert!(has(&scanner.report, Kind::ResourceLimit));

    let mut nested = PathBuf::from("llm/family/deep");
    for _ in 0..=MAX_MODEL_DEPTH {
        nested.push("d");
    }
    fs::create_dir_all(temp.path().join(&nested)).unwrap();
    fs::write(temp.path().join(nested).join("model-1-of-2.gguf"), b"one").unwrap();
    let report = discover(temp.path());
    assert!(!report.enumeration_complete);
    assert!(has(&report, Kind::ResourceLimit));
    assert!(report
        .model_roots
        .iter()
        .find(|model| model.library_relative_root.ends_with("deep"))
        .is_some_and(|model| !model.enumeration_complete));
}

#[cfg(windows)]
#[test]
fn junction_child_is_refused_without_reading_its_sentinel_tree() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("library");
    write(&root, "llm/family/model/model-1-of-2.gguf", b"one");
    let outside = temp.path().join("outside");
    write(&outside, "sentinel-1-of-1.gguf", b"preserve");
    let junction = root.join("llm/family/model/escape");
    let output = std::process::Command::new("cmd.exe")
        .args(["/d", "/c", "mklink", "/j"])
        .arg(&junction)
        .arg(&outside)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let report = discover(&root);
    assert!(!report.enumeration_complete);
    assert!(report
        .model_roots
        .iter()
        .flat_map(|model| &model.shard_sets)
        .all(|set| set.base_name != "sentinel.gguf"));
    assert_eq!(
        fs::read(outside.join("sentinel-1-of-1.gguf")).unwrap(),
        b"preserve"
    );
    fs::remove_dir(junction).unwrap();
}
