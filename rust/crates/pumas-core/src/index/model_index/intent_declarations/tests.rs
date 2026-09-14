use super::*;
use crate::index::{ModelPackageFactsCacheRecord, ModelPackageFactsCacheScope, ModelRecord};
use crate::models::PUMAS_MODEL_REF_CONTRACT_VERSION;
use std::collections::HashMap;
#[cfg(unix)]
use std::io::{BufRead, BufReader};
use std::path::Path;
#[cfg(unix)]
use std::process::{Command, Stdio};
use std::sync::{Arc, Barrier};
use std::thread;
use tempfile::TempDir;

const COMMIT: &str = "0123456789abcdef0123456789abcdef01234567";
const OTHER_COMMIT: &str = "89abcdef0123456789abcdef0123456789abcdef";

fn local_requirement(model_id: &str) -> ModelRequirement {
    ModelRequirement {
        selector: ModelSelector::LocalModel {
            model_ref: PumasModelRef {
                model_ref_contract_version: PUMAS_MODEL_REF_CONTRACT_VERSION,
                model_id: model_id.to_string(),
                revision: None,
                selected_artifact_id: Some("artifact".to_string()),
                selected_artifact_path: None,
                migration_diagnostics: Vec::new(),
            },
        },
        artifact: Default::default(),
        acquisition_policy: AcquisitionPolicy::LocalOnly,
    }
}

fn upstream_requirement() -> ModelRequirement {
    ModelRequirement {
        selector: ModelSelector::UpstreamRepository {
            repository_id: "acme/model".to_string(),
            revision: Some(COMMIT.to_string()),
        },
        artifact: crate::intent::ArtifactRequirement {
            format: Some(PackageArtifactKind::Gguf),
            quantization: Some("q4-k-m".to_string()),
            selected_artifact_id: None,
        },
        acquisition_policy: AcquisitionPolicy::AllowUpstream,
    }
}

fn upstream_target(
    repository_id: &str,
    commit: &str,
    filename: &str,
    format: PackageArtifactKind,
) -> BoundTarget {
    let (family, official_name) = repository_id.split_once('/').unwrap();
    let request = DownloadRequest {
        repo_id: repository_id.to_string(),
        family: family.to_string(),
        official_name: official_name.to_string(),
        model_type: (format == PackageArtifactKind::Gguf).then(|| "llm".to_string()),
        quant: None,
        filename: Some(filename.to_string()),
        filenames: None,
        pipeline_tag: None,
        bundle_format: None,
        pipeline_class: None,
        release_date: None,
        download_url: None,
        model_card_json: None,
        license_status: None,
    };
    let revision = DownloadRevision::from_commit(commit).unwrap();
    let identity = SelectedArtifactIdentity::from_download_request_at_revision(
        &request,
        Some(vec![filename.to_string()]),
        &revision,
    );
    BoundTarget::Upstream {
        model_ref: PumasModelRef {
            model_ref_contract_version: PUMAS_MODEL_REF_CONTRACT_VERSION,
            model_id: repository_id.to_string(),
            revision: Some(commit.to_string()),
            selected_artifact_id: Some(identity.artifact_id),
            selected_artifact_path: None,
            migration_diagnostics: Vec::new(),
        },
        repository_id: repository_id.to_string(),
        commit: commit.to_string(),
        filename: filename.to_string(),
        format,
    }
}

fn record(model_id: &str) -> ModelRecord {
    ModelRecord {
        id: model_id.to_string(),
        path: format!("models/{model_id}"),
        cleaned_name: "model".to_string(),
        official_name: "Model".to_string(),
        model_type: "llm".to_string(),
        tags: vec!["test".to_string()],
        hashes: HashMap::new(),
        metadata: serde_json::json!({"schema_version": 2}),
        updated_at: "2026-09-12T00:00:00Z".to_string(),
    }
}

fn drop_intent_schema(path: &Path) {
    let conn = Connection::open(path).unwrap();
    conn.execute_batch(
        "DROP INDEX idx_intent_declarations_model;
         DROP INDEX idx_intent_declarations_consumer;
         DROP TABLE intent_deletion_claims;
         DROP TABLE intent_declarations;
         DROP TABLE intent_schema_meta;",
    )
    .unwrap();
}

fn intent_object_count(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM sqlite_master WHERE name LIKE 'intent_%'",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn fresh_and_populated_legacy_databases_migrate_without_losing_existing_rows() {
    let fresh = TempDir::new().unwrap();
    let fresh_path = fresh.path().join("fresh.db");
    let fresh_index = ModelIndex::new(&fresh_path).unwrap();
    assert!(fresh_index.list_intent_declarations().unwrap().is_empty());
    let sync: i64 = fresh_index
        .intent_connection()
        .unwrap()
        .query_row("PRAGMA synchronous", [], |row| row.get(0))
        .unwrap();
    assert_eq!(sync, 2);

    let legacy = TempDir::new().unwrap();
    let path = legacy.path().join("legacy.db");
    let index = ModelIndex::new(&path).unwrap();
    index.upsert(&record("acme/model")).unwrap();
    index
        .upsert_model_package_facts_cache(&ModelPackageFactsCacheRecord {
            model_id: "acme/model".to_string(),
            selected_artifact_id: "artifact".to_string(),
            cache_scope: ModelPackageFactsCacheScope::Detail,
            package_facts_contract_version: 3,
            producer_revision: Some("producer".to_string()),
            source_fingerprint: "fingerprint".to_string(),
            facts_json: "{}".to_string(),
            cached_at: "2026-09-12T00:00:00Z".to_string(),
            updated_at: "2026-09-12T00:00:00Z".to_string(),
        })
        .unwrap();
    drop(index);
    let governance_count: i64 = Connection::open(&path)
        .unwrap()
        .query_row("SELECT COUNT(*) FROM task_signature_mappings", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(governance_count > 0);
    drop_intent_schema(&path);

    let migrated = ModelIndex::new(&path).unwrap();
    assert!(migrated.get("acme/model").unwrap().is_some());
    assert_eq!(
        migrated
            .count_model_package_facts_cache_rows("acme/model")
            .unwrap(),
        1
    );
    assert_eq!(
        Connection::open(&path)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM task_signature_mappings", [], |row| {
                row.get::<_, i64>(0)
            })
            .unwrap(),
        governance_count
    );
}

#[test]
fn failed_schema_install_rolls_back_every_intent_object_and_preserves_legacy_data() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("failure.db");
    let mut conn = Connection::open(&path).unwrap();
    conn.execute_batch(
        "CREATE TABLE legacy_guard(value TEXT NOT NULL); INSERT INTO legacy_guard VALUES ('held');",
    )
    .unwrap();
    let error = ModelIndex::ensure_intent_schema_with_hook(&mut conn, |tx| {
        tx.execute("CREATE TABLE intent_test_partial(value TEXT)", [])?;
        Err(PumasError::Other("injected migration failure".to_string()))
    })
    .unwrap_err();
    assert!(error.to_string().contains("injected migration failure"));
    assert_eq!(intent_object_count(&path), 0);
    assert_eq!(
        conn.query_row("SELECT value FROM legacy_guard", [], |row| row
            .get::<_, String>(0))
            .unwrap(),
        "held"
    );
}

#[test]
fn read_only_legacy_is_empty_and_future_partial_or_malformed_state_is_rejected_unchanged() {
    let legacy = TempDir::new().unwrap();
    let legacy_path = legacy.path().join("legacy-read.db");
    Connection::open(&legacy_path)
        .unwrap()
        .execute("CREATE TABLE legacy_guard(value TEXT)", [])
        .unwrap();
    let reader = ModelIndex::open_read_only(&legacy_path).unwrap();
    assert!(reader.list_intent_declarations().unwrap().is_empty());
    drop(reader);
    assert_eq!(intent_object_count(&legacy_path), 0);

    for corruption in ["future", "partial", "malformed", "local_null", "overlap"] {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(format!("{corruption}.db"));
        let index = ModelIndex::new(&path).unwrap();
        let declaration = index
            .commit_intent_declaration("consumer", &local_requirement("acme/model"))
            .unwrap();
        drop(index);
        let conn = Connection::open(&path).unwrap();
        match corruption {
            "future" => {
                conn.execute("UPDATE intent_schema_meta SET schema_version = 2", [])
                    .unwrap();
            }
            "partial" => {
                conn.execute("DROP INDEX idx_intent_declarations_consumer", [])
                    .unwrap();
            }
            "malformed" => {
                conn.execute(
                    "UPDATE intent_declarations SET original_requirement_json = ?1
                     WHERE declaration_id = ?2",
                    params![
                        serde_json::to_string(&upstream_requirement()).unwrap(),
                        declaration.declaration_id
                    ],
                )
                .unwrap();
            }
            "local_null" => {
                conn.execute(
                    "UPDATE intent_declarations
                     SET model_id = NULL, bound_target_json = NULL
                     WHERE declaration_id = ?1",
                    [&declaration.declaration_id],
                )
                .unwrap();
            }
            "overlap" => {
                conn.execute(
                    "INSERT INTO intent_deletion_claims(model_id, claim_token, created_at)
                     VALUES ('acme/model', ?1, '2026-09-12T00:00:00Z')",
                    [Uuid::new_v4().to_string()],
                )
                .unwrap();
            }
            _ => unreachable!(),
        }
        drop(conn);
        assert!(ModelIndex::open_read_only(&path).is_err());
        assert!(ModelIndex::new(&path).is_err());
        let conn = Connection::open(&path).unwrap();
        assert_eq!(
            conn.query_row("SELECT COUNT(*) FROM intent_declarations", [], |row| row
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        if corruption == "future" {
            assert_eq!(
                conn.query_row("SELECT schema_version FROM intent_schema_meta", [], |row| {
                    row.get::<_, i64>(0)
                })
                .unwrap(),
                2
            );
        }
    }
}

#[test]
fn duplicate_consumers_binding_and_release_obey_generation_and_identity_contracts() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("declarations.db");
    let index = ModelIndex::new(&path).unwrap();
    let requirement = upstream_requirement();
    let first = index
        .commit_intent_declaration("consumer-a", &requirement)
        .unwrap();
    assert_eq!(
        first.original_requirement.artifact.quantization.as_deref(),
        Some("Q4-K-M")
    );
    let mut equivalent = requirement.clone();
    equivalent.artifact.quantization = Some("Q4-K-M".to_string());
    let duplicate = index
        .commit_intent_declaration("consumer-a", &equivalent)
        .unwrap();
    assert_eq!(duplicate.declaration_id, first.declaration_id);
    assert_eq!(duplicate.generation, first.generation);
    equivalent.artifact.quantization = Some(" mostly_q4-k-m ".to_string());
    let spaced_duplicate = index
        .commit_intent_declaration("consumer-a", &equivalent)
        .unwrap();
    assert_eq!(spaced_duplicate.declaration_id, first.declaration_id);
    assert_eq!(spaced_duplicate.generation, first.generation);
    let other_consumer = index
        .commit_intent_declaration("consumer-b", &requirement)
        .unwrap();
    assert_ne!(other_consumer.declaration_id, first.declaration_id);

    let target = upstream_target(
        "acme/model",
        COMMIT,
        "model-Q4_K_M.gguf",
        PackageArtifactKind::Gguf,
    );
    let bound = index
        .bind_intent_declaration(
            &first.declaration_id,
            "consumer-a",
            first.generation,
            &target,
        )
        .unwrap();
    assert_eq!(bound.generation, first.generation);
    assert_eq!(
        index
            .bind_intent_declaration(
                &first.declaration_id,
                "consumer-a",
                first.generation,
                &target,
            )
            .unwrap()
            .generation,
        first.generation
    );
    assert!(index
        .bind_intent_declaration(
            &first.declaration_id,
            "consumer-a",
            first.generation,
            &upstream_target(
                "acme/model",
                COMMIT,
                "other-Q4_K_M.gguf",
                PackageArtifactKind::Gguf
            ),
        )
        .is_err());

    assert_eq!(
        index
            .release_intent_declaration(&first.declaration_id, "consumer-a", first.generation)
            .unwrap(),
        IntentDeclarationRelease::Released
    );
    assert!(index
        .get_intent_declaration(&other_consumer.declaration_id)
        .unwrap()
        .is_some());
    let recreated = index
        .commit_intent_declaration("consumer-a", &requirement)
        .unwrap();
    assert_eq!(recreated.declaration_id, first.declaration_id);
    assert_ne!(recreated.generation, first.generation);
    assert!(index
        .bind_intent_declaration(
            &recreated.declaration_id,
            "consumer-a",
            first.generation,
            &target,
        )
        .is_err());
    assert!(index
        .release_intent_declaration(&recreated.declaration_id, "consumer-a", first.generation)
        .is_err());
}

#[test]
fn local_declaration_preserves_observed_path_constraint_exactly() {
    let temp = TempDir::new().unwrap();
    let index = ModelIndex::new(temp.path().join("local-path.db")).unwrap();
    let mut requirement = local_requirement("acme/model");
    let ModelSelector::LocalModel { model_ref } = &mut requirement.selector else {
        unreachable!()
    };
    model_ref.selected_artifact_path = Some("/managed/root/acme/model/model.gguf".to_string());
    let declaration = index
        .commit_intent_declaration("consumer", &requirement)
        .unwrap();
    assert_eq!(declaration.original_requirement, requirement);
}

#[test]
fn binding_rejects_wrong_repository_revision_and_artifact_without_mutating_row() {
    let temp = TempDir::new().unwrap();
    let index = ModelIndex::new(temp.path().join("binding.db")).unwrap();
    let declaration = index
        .commit_intent_declaration("consumer", &upstream_requirement())
        .unwrap();
    let mut wrong_artifact = upstream_target(
        "acme/model",
        COMMIT,
        "model-Q4_K_M.gguf",
        PackageArtifactKind::Gguf,
    );
    if let BoundTarget::Upstream { model_ref, .. } = &mut wrong_artifact {
        model_ref.selected_artifact_id = Some("wrong".to_string());
    }
    for target in [
        upstream_target(
            "other/model",
            COMMIT,
            "model-Q4_K_M.gguf",
            PackageArtifactKind::Gguf,
        ),
        upstream_target(
            "acme/model",
            OTHER_COMMIT,
            "model-Q4_K_M.gguf",
            PackageArtifactKind::Gguf,
        ),
        wrong_artifact,
        upstream_target("acme/model", COMMIT, "CON.gguf", PackageArtifactKind::Gguf),
        upstream_target("acme/model", COMMIT, "bad*.gguf", PackageArtifactKind::Gguf),
        upstream_target(
            "acme/model",
            COMMIT,
            "trailing.gguf.",
            PackageArtifactKind::Gguf,
        ),
    ] {
        assert!(index
            .bind_intent_declaration(
                &declaration.declaration_id,
                "consumer",
                declaration.generation,
                &target,
            )
            .is_err());
        assert!(index
            .get_intent_declaration(&declaration.declaration_id)
            .unwrap()
            .unwrap()
            .bound_target
            .is_none());
    }
}

#[test]
fn unsupported_policy_selector_combinations_and_invalid_keys_write_nothing() {
    let temp = TempDir::new().unwrap();
    let index = ModelIndex::new(temp.path().join("invalid-input.db")).unwrap();
    let mut unsupported = upstream_requirement();
    unsupported.acquisition_policy = AcquisitionPolicy::LocalOnly;
    assert!(index
        .commit_intent_declaration("consumer", &unsupported)
        .is_err());
    assert!(index
        .commit_intent_declaration(" consumer", &local_requirement("acme/model"))
        .is_err());
    let mut unknown = local_requirement("acme/model");
    unknown.artifact.format = Some(PackageArtifactKind::Unknown);
    assert!(index
        .commit_intent_declaration("consumer", &unknown)
        .is_err());
    for revision in [" main", "main ", " main "] {
        let mut requirement = upstream_requirement();
        let ModelSelector::UpstreamRepository {
            revision: selector, ..
        } = &mut requirement.selector
        else {
            unreachable!()
        };
        *selector = Some(revision.to_string());
        assert!(index
            .commit_intent_declaration("consumer", &requirement)
            .is_err());
    }
    assert!(index.list_intent_declarations().unwrap().is_empty());
}

#[test]
fn declaration_commit_and_deletion_claim_are_atomic_across_independent_connections() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("race.db");
    let commit_index = ModelIndex::new(&path).unwrap();
    let claim_index = ModelIndex::new(&path).unwrap();
    let barrier = Arc::new(Barrier::new(3));
    let commit_barrier = Arc::clone(&barrier);
    let commit = thread::spawn(move || {
        commit_barrier.wait();
        commit_index.commit_intent_declaration("consumer", &local_requirement("acme/model"))
    });
    let claim_barrier = Arc::clone(&barrier);
    let token = Uuid::new_v4();
    let claim = thread::spawn(move || {
        claim_barrier.wait();
        claim_index.claim_intent_model_deletion("acme/model", token)
    });
    barrier.wait();
    let commit = commit.join().unwrap();
    let claim = claim.join().unwrap().unwrap();
    assert!(matches!(
        (&commit, claim),
        (Ok(_), IntentDeletionClaimResult::Retained) | (Err(_), IntentDeletionClaimResult::Claimed)
    ));

    let reopened = ModelIndex::new(&path).unwrap();
    if commit.is_ok() {
        assert_eq!(reopened.list_intent_declarations().unwrap().len(), 1);
        assert!(reopened
            .list_intent_model_deletion_claims()
            .unwrap()
            .is_empty());
    } else {
        assert_eq!(
            reopened.list_intent_model_deletion_claims().unwrap().len(),
            1
        );
        assert!(!reopened
            .release_intent_model_deletion("acme/model", Uuid::new_v4())
            .unwrap());
        assert!(reopened
            .commit_intent_declaration("consumer", &local_requirement("acme/model"))
            .is_err());
        assert!(reopened
            .release_intent_model_deletion("acme/model", token)
            .unwrap());
    }
}

#[test]
fn consumer_retention_and_claim_tokens_are_scoped_and_durable() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("claims.db");
    let index = ModelIndex::new(&path).unwrap();
    let requirement = local_requirement("acme/model");
    let first = index
        .commit_intent_declaration("consumer-a", &requirement)
        .unwrap();
    let second = index
        .commit_intent_declaration("consumer-b", &requirement)
        .unwrap();
    assert_eq!(
        index
            .list_intent_declarations_for_model("acme/model")
            .unwrap()
            .len(),
        2
    );
    let token = Uuid::new_v4();
    assert_eq!(
        index
            .claim_intent_model_deletion("acme/model", token)
            .unwrap(),
        IntentDeletionClaimResult::Retained
    );
    index
        .release_intent_declaration(&first.declaration_id, "consumer-a", first.generation)
        .unwrap();
    assert_eq!(
        index
            .claim_intent_model_deletion("acme/model", token)
            .unwrap(),
        IntentDeletionClaimResult::Retained
    );
    index
        .release_intent_declaration(&second.declaration_id, "consumer-b", second.generation)
        .unwrap();
    assert_eq!(
        index
            .claim_intent_model_deletion("acme/model", token)
            .unwrap(),
        IntentDeletionClaimResult::Claimed
    );
    assert_eq!(
        index
            .claim_intent_model_deletion("acme/model", token)
            .unwrap(),
        IntentDeletionClaimResult::AlreadyClaimed
    );
    assert_eq!(
        index
            .claim_intent_model_deletion("acme/model", Uuid::new_v4())
            .unwrap(),
        IntentDeletionClaimResult::Conflict
    );
    assert!(!index
        .release_intent_model_deletion("acme/model", Uuid::new_v4())
        .unwrap());
    drop(index);
    let reopened = ModelIndex::new(&path).unwrap();
    assert_eq!(
        reopened.list_intent_model_deletion_claims().unwrap().len(),
        1
    );
    assert!(reopened
        .release_intent_model_deletion("acme/model", token)
        .unwrap());
    assert!(reopened
        .list_intent_model_deletion_claims()
        .unwrap()
        .is_empty());
}

#[test]
fn catalog_delete_clear_and_fts_rebuild_preserve_intent_authority() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("catalog.db");
    let index = ModelIndex::new(&path).unwrap();
    index.upsert(&record("acme/model")).unwrap();
    let declaration = index
        .commit_intent_declaration("consumer", &local_requirement("acme/model"))
        .unwrap();
    let claim_token = Uuid::new_v4();
    assert_eq!(
        index
            .claim_intent_model_deletion("absent/model", claim_token)
            .unwrap(),
        IntentDeletionClaimResult::Claimed
    );
    assert!(index.delete("acme/model").unwrap());
    index.clear().unwrap();
    index.rebuild_fts5().unwrap();
    assert!(index
        .get_intent_declaration(&declaration.declaration_id)
        .unwrap()
        .is_some());
    assert_eq!(index.list_intent_model_deletion_claims().unwrap().len(), 1);
    drop(index);
    let reopened = ModelIndex::new(&path).unwrap();
    assert!(reopened
        .get_intent_declaration(&declaration.declaration_id)
        .unwrap()
        .is_some());
    assert_eq!(
        reopened.list_intent_model_deletion_claims().unwrap().len(),
        1
    );
}

#[cfg(unix)]
#[test]
#[ignore = "subprocess helper invoked by forced_exit_preserves_only_committed_declarations"]
fn intent_declaration_exit_child() {
    let Some(path) = std::env::var_os("PUMAS_INTENT_CHILD_DB") else {
        return;
    };
    let phase = std::env::var("PUMAS_INTENT_CHILD_PHASE").unwrap();
    if phase == "before" {
        let index = ModelIndex::new(&path).unwrap();
        index
            .commit_intent_declaration_with_hook(
                "consumer",
                &local_requirement("acme/model"),
                |_| {
                    println!("PUMAS_INTENT_BEFORE_COMMIT");
                    use std::io::Write;
                    std::io::stdout().flush().unwrap();
                    let mut release = String::new();
                    std::io::stdin().read_line(&mut release).unwrap();
                    Ok(())
                },
            )
            .unwrap();
    } else {
        let index = ModelIndex::new(&path).unwrap();
        index
            .commit_intent_declaration("consumer", &local_requirement("acme/model"))
            .unwrap();
        println!("PUMAS_INTENT_AFTER_COMMIT");
        use std::io::Write;
        std::io::stdout().flush().unwrap();
        let mut release = String::new();
        std::io::stdin().read_line(&mut release).unwrap();
    }
}

#[cfg(unix)]
#[test]
fn forced_exit_preserves_only_committed_declarations() {
    for (phase, marker, expected) in [
        ("before", "PUMAS_INTENT_BEFORE_COMMIT", 0),
        ("after", "PUMAS_INTENT_AFTER_COMMIT", 1),
    ] {
        let temp = TempDir::new().unwrap();
        let path = temp.path().join(format!("{phase}.db"));
        drop(ModelIndex::new(&path).unwrap());
        let mut child = Command::new(std::env::current_exe().unwrap())
            .arg("--ignored")
            .arg("--exact")
            .arg("index::model_index::intent_declarations::tests::intent_declaration_exit_child")
            .arg("--nocapture")
            .env("PUMAS_INTENT_CHILD_DB", &path)
            .env("PUMAS_INTENT_CHILD_PHASE", phase)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        let mut line = String::new();
        loop {
            line.clear();
            assert_ne!(output.read_line(&mut line).unwrap(), 0);
            if line.contains(marker) {
                break;
            }
        }
        child.kill().unwrap();
        child.wait().unwrap();
        let reopened = ModelIndex::new(&path).unwrap();
        assert_eq!(reopened.list_intent_declarations().unwrap().len(), expected);
    }
}
