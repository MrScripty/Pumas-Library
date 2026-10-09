//! Fingerprint identity regressions; these exercise the production hash protocol.
use super::*;
use crate::models::{AssetValidationState, StorageKind};
use std::collections::{HashMap, HashSet};

fn descriptor(root: &Path) -> ModelExecutionDescriptor {
    ModelExecutionDescriptor {
        execution_contract_version: 1,
        model_id: "llm/fixture/model".into(),
        entry_path: root.join("model.gguf").display().to_string(),
        model_type: "llm".into(),
        task_type_primary: "text-generation".into(),
        recommended_backend: None,
        runtime_engine_hints: vec![],
        storage_kind: StorageKind::LibraryOwned,
        validation_state: AssetValidationState::Valid,
        dependency_resolution: None,
    }
}
fn metadata(rotation: usize) -> ModelMetadata {
    let entries = [
        ("license", serde_json::json!("apache-2.0")),
        ("language", serde_json::json!(["en", "zh"])),
        ("pipeline_tag", serde_json::json!("text-generation")),
        ("tags", serde_json::json!(["chat", "gguf"])),
        (
            "nested",
            serde_json::json!({"z": {"b": 2, "a": 1}, "a": [3, 1, 2]}),
        ),
        ("base_model", serde_json::json!("publisher/model")),
    ];
    let mut card = HashMap::new();
    for i in 0..entries.len() {
        let (key, value) = &entries[(i + rotation) % entries.len()];
        card.insert((*key).into(), value.clone());
    }
    let nested = if rotation.is_multiple_of(2) {
        r#"{"z":{"b":2,"a":1},"a":[3,1,2]}"#
    } else {
        r#"{"a":[3,1,2],"z":{"a":1,"b":2}}"#
    };
    card.insert("nested".into(), serde_json::from_str(nested).unwrap());
    ModelMetadata {
        model_card: Some(card),
        ..Default::default()
    }
}
async fn fingerprint(root: &Path, metadata: &ModelMetadata) -> String {
    PackageInspectionManifest::build(root, metadata)
        .await
        .unwrap()
        .source_fingerprint(root, &descriptor(root), metadata, &[])
        .await
        .unwrap()
}
#[tokio::test]
async fn metadata_read_stability_insertion_order_nested_maps_and_seeds() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::write(
        root.join("model.gguf"),
        b"fingerprint fixture, parser tested separately",
    )
    .unwrap();
    let baseline = fingerprint(root, &metadata(0)).await;
    for rotation in 0..6 {
        let bytes = serde_json::to_vec(&metadata(rotation)).unwrap();
        for _ in 0..16 {
            let reread: ModelMetadata = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(
                baseline,
                fingerprint(root, &reread).await,
                "semantic identity must ignore HashMap insertion order and fresh RandomState seeds"
            );
        }
    }
}
#[tokio::test]
async fn metadata_and_ordered_array_changes_remain_invalidating() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    std::fs::write(root.join("model.gguf"), b"fingerprint fixture").unwrap();
    let original = metadata(0);
    let baseline = fingerprint(root, &original).await;
    for field in ["license", "nested", "language"] {
        let mut changed = original.clone();
        let card = changed.model_card.as_mut().unwrap();
        let value = match field {
            "license" => serde_json::json!("different-license"),
            "nested" => serde_json::json!({"z":{"b":2,"a":99},"a":[3,1,2]}),
            _ => serde_json::json!(["zh", "en"]),
        };
        card.insert(field.into(), value);
        assert_ne!(baseline, fingerprint(root, &changed).await, "{field}");
    }
    std::fs::write(
        root.join("model.gguf"),
        b"changed fingerprint fixture bytes",
    )
    .unwrap();
    assert_ne!(baseline, fingerprint(root, &original).await);
}
#[tokio::test]
async fn file_timestamp_changes_remain_invalidating() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let path = root.join("model.gguf");
    std::fs::write(&path, b"unchanged payload").unwrap();
    let m = metadata(0);
    let before = fingerprint(root, &m).await;
    let file = std::fs::OpenOptions::new().write(true).open(&path).unwrap();
    let original = file.metadata().unwrap().modified().unwrap();
    file.set_times(
        std::fs::FileTimes::new().set_modified(original + std::time::Duration::from_secs(1)),
    )
    .unwrap();
    assert_ne!(before, fingerprint(root, &m).await);
    file.set_times(std::fs::FileTimes::new().set_modified(original))
        .unwrap();
    assert_eq!(before, fingerprint(root, &m).await);
}
#[tokio::test]
async fn metadata_process_probe() {
    let Ok(root) = std::env::var("PUMAS_FINGERPRINT_PROBE_ROOT") else {
        return;
    };
    let rotation = std::env::var("PUMAS_FINGERPRINT_PROBE_ROTATION")
        .unwrap()
        .parse()
        .unwrap();
    println!(
        "PACKAGE_FP={}",
        fingerprint(Path::new(&root), &metadata(rotation)).await
    );
}
#[test]
fn fingerprints_are_stable_across_process_hash_seeds() {
    let temp = tempfile::tempdir().unwrap();
    std::fs::write(temp.path().join("model.gguf"), b"process seed fixture").unwrap();
    let name = format!(
        "{}::metadata_process_probe",
        module_path!().split_once("::").unwrap().1
    );
    let mut fingerprints = HashSet::new();
    for rotation in 0..6 {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([name.as_str(), "--exact", "--nocapture"])
            .env("PUMAS_FINGERPRINT_PROBE_ROOT", temp.path())
            .env("PUMAS_FINGERPRINT_PROBE_ROTATION", rotation.to_string())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let stdout = String::from_utf8(output.stdout).unwrap();
        let fp = stdout
            .lines()
            .find_map(|line| line.strip_prefix("PACKAGE_FP="))
            .expect("child probe ran");
        fingerprints.insert(fp.to_owned());
    }
    assert_eq!(
        fingerprints.len(),
        1,
        "independent RandomState process seeds must not change identity"
    );
}

#[tokio::test]
async fn legacy_raw_metadata_fingerprints_require_reobservation() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let path = root.join("model.gguf");
    std::fs::write(&path, b"legacy protocol fixture").unwrap();
    let metadata = metadata(0);
    // Reproduce the former protocol for the same descriptor, metadata and
    // filesystem observation, rather than accepting an arbitrary stale token.
    let mut hasher = Sha256::new();
    for (key, value) in [
        (
            "contract_version",
            PACKAGE_FACTS_CONTRACT_VERSION.to_string(),
        ),
        (
            "gguf_inspector_revision",
            super::super::gguf::GGUF_FILE_TYPE_INSPECTOR_REVISION.to_string(),
        ),
        (
            "descriptor",
            serde_json::to_string(&descriptor(root)).unwrap(),
        ),
        ("metadata", serde_json::to_string(&metadata).unwrap()),
        ("dependency_bindings", "[]".into()),
        ("file", "model.gguf".into()),
        ("file_state", "present".into()),
    ] {
        update_package_facts_hash_part(&mut hasher, key, &value);
    }
    let stat = std::fs::metadata(&path).unwrap();
    let modified = stat.modified().unwrap().duration_since(UNIX_EPOCH).unwrap();
    for (key, value) in [
        ("file_len", stat.len().to_string()),
        ("file_mtime_secs", modified.as_secs().to_string()),
        ("file_mtime_nanos", modified.subsec_nanos().to_string()),
    ] {
        update_package_facts_hash_part(&mut hasher, key, &value);
    }
    let old = hex::encode(hasher.finalize());
    assert_ne!(old, fingerprint(root, &metadata).await);
}
