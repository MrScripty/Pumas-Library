//! Fault-injection tests: a failure between a mutation and its update-feed
//! event must leave neither behind.

use super::*;
use tempfile::TempDir;

fn index() -> (ModelIndex, TempDir) {
    let temp = TempDir::new().unwrap();
    let index = ModelIndex::new(temp.path().join("models.db")).unwrap();
    (index, temp)
}

fn record(id: &str) -> ModelRecord {
    ModelRecord {
        id: id.to_string(),
        path: format!("models/{id}"),
        cleaned_name: id.to_string(),
        official_name: id.to_string(),
        model_type: "llm".to_string(),
        tags: vec!["test".to_string()],
        hashes: HashMap::from([("sha256".to_string(), "abc".to_string())]),
        metadata: serde_json::json!({"family": "test"}),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

/// Make every later update-feed append fail, as a crash between the mutation
/// statement and the event statement would.
fn fail_event_appends(index: &ModelIndex) {
    index
        .conn
        .lock()
        .unwrap()
        .execute_batch(
            "CREATE TEMP TRIGGER injected_event_failure
             BEFORE INSERT ON main.model_library_update_events
             BEGIN SELECT RAISE(ABORT, 'injected event failure'); END;",
        )
        .unwrap();
}

fn restore_event_appends(index: &ModelIndex) {
    index
        .conn
        .lock()
        .unwrap()
        .execute_batch("DROP TRIGGER temp.injected_event_failure;")
        .unwrap();
}

fn event_count(index: &ModelIndex) -> i64 {
    index
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT COUNT(*) FROM model_library_update_events",
            [],
            |row| row.get(0),
        )
        .unwrap()
}

#[test]
fn delete_keeps_the_row_when_its_event_cannot_be_recorded() {
    let (index, _temp) = index();
    index.upsert(&record("kept")).unwrap();
    let events = event_count(&index);

    fail_event_appends(&index);
    assert!(index.delete("kept").is_err());
    restore_event_appends(&index);

    assert!(index.get("kept").unwrap().is_some());
    assert_eq!(event_count(&index), events);

    // Retrying after the fault records both the removal and its event.
    assert!(index.delete("kept").unwrap());
    assert!(index.get("kept").unwrap().is_none());
    assert_eq!(event_count(&index), events + 1);
}

#[test]
fn clear_keeps_every_row_when_an_event_cannot_be_recorded() {
    let (index, _temp) = index();
    index.upsert(&record("a")).unwrap();
    index.upsert(&record("b")).unwrap();
    let events = event_count(&index);

    fail_event_appends(&index);
    assert!(index.clear().is_err());
    restore_event_appends(&index);

    assert_eq!(index.count().unwrap(), 2);
    assert_eq!(event_count(&index), events);
    assert_eq!(
        index.search("a", None, None, 10, 0).unwrap().models.len(),
        1
    );
}

fn facts(
    model_id: &str,
    selected_artifact_id: &str,
    fingerprint: &str,
) -> ModelPackageFactsCacheRecord {
    ModelPackageFactsCacheRecord {
        model_id: model_id.to_string(),
        selected_artifact_id: selected_artifact_id.to_string(),
        cache_scope: ModelPackageFactsCacheScope::Detail,
        package_facts_contract_version: i64::from(crate::models::PACKAGE_FACTS_CONTRACT_VERSION),
        producer_revision: Some("test".to_string()),
        source_fingerprint: fingerprint.to_string(),
        facts_json: serde_json::json!({"detail": fingerprint}).to_string(),
        cached_at: "2026-01-01T00:00:00Z".to_string(),
        updated_at: "2026-01-01T00:00:00Z".to_string(),
    }
}

fn cached_fingerprint(index: &ModelIndex, model_id: &str) -> Option<String> {
    index
        .get_model_package_facts_cache(model_id, None, ModelPackageFactsCacheScope::Detail)
        .unwrap()
        .map(|row| row.source_fingerprint)
}

#[test]
fn package_facts_upsert_is_undone_when_its_event_cannot_be_recorded() {
    let (index, _temp) = index();
    index.upsert(&record("m")).unwrap();
    index
        .upsert_model_package_facts_cache(&facts("m", "", "v1"))
        .unwrap();
    let events = event_count(&index);

    fail_event_appends(&index);
    assert!(index
        .upsert_model_package_facts_cache(&facts("m", "", "v2"))
        .is_err());
    restore_event_appends(&index);

    assert_eq!(cached_fingerprint(&index, "m").as_deref(), Some("v1"));
    assert_eq!(event_count(&index), events);
}

#[test]
fn package_facts_deletes_are_undone_when_their_event_cannot_be_recorded() {
    let (index, _temp) = index();
    index.upsert(&record("m")).unwrap();
    index
        .upsert_model_package_facts_cache(&facts("m", "", "v1"))
        .unwrap();
    let events = event_count(&index);

    fail_event_appends(&index);
    assert!(index.delete_model_package_facts_cache("m").is_err());
    assert!(index
        .delete_model_package_facts_cache_without_selected_artifact("m")
        .is_err());
    restore_event_appends(&index);

    assert_eq!(index.count_model_package_facts_cache_rows("m").unwrap(), 1);
    assert_eq!(event_count(&index), events);
}

#[test]
fn package_facts_delete_records_an_invalidation_event() {
    let (index, _temp) = index();
    index.upsert(&record("m")).unwrap();
    index
        .upsert_model_package_facts_cache(&facts("m", "", "v1"))
        .unwrap();
    index
        .upsert_model_package_facts_cache(&facts("m", "q5", "v1"))
        .unwrap();
    let cursor = index.current_model_library_update_cursor().unwrap();
    let mut updates = index.subscribe_model_library_update_events();

    assert_eq!(index.delete_model_package_facts_cache("m").unwrap(), 2);
    // Deleting nothing records nothing.
    assert_eq!(index.delete_model_package_facts_cache("m").unwrap(), 0);

    let feed = index
        .list_model_library_updates_since(Some(&cursor), 100)
        .unwrap();
    assert_eq!(feed.events.len(), 1);
    let event = &feed.events[0];
    assert_eq!(event.model_id, "m");
    assert_eq!(
        event.change_kind,
        ModelLibraryChangeKind::PackageFactsModified
    );
    assert_eq!(event.fact_family, ModelFactFamily::PackageFacts);
    assert_eq!(
        event.refresh_scope,
        ModelLibraryRefreshScope::SummaryAndDetail
    );
    assert_eq!(updates.try_recv().unwrap().model_id, "m");
    assert!(updates.try_recv().is_err());
}
