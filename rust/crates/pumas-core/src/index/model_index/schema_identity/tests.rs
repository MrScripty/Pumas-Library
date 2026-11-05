use super::*;
use tempfile::TempDir;

/// The schema a 0.8.0 release left in `models.db`.
const INDEX_0_8_0: &str = include_str!("index_0_8_0.sql");

fn user_version(path: &Path) -> i64 {
    Connection::open(path)
        .unwrap()
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .unwrap()
}

fn table_exists(path: &Path, name: &str) -> bool {
    ModelIndex::table_exists(&Connection::open(path).unwrap(), name).unwrap()
}

/// Write a library the way 0.8.0 left it: its schema, a model, a person's
/// rule and an active metadata overlay, with no schema identity.
fn library_0_8_0(temp: &TempDir, schema: &str) -> PathBuf {
    let path = temp.path().join("models.db");
    let conn = Connection::open(&path).unwrap();
    conn.execute_batch("PRAGMA journal_mode=WAL;").unwrap();
    conn.execute_batch(schema).unwrap();
    conn.execute_batch(
        r#"
        INSERT INTO models VALUES (
          'llm/llama/upgrade-model', 'llm/llama/upgrade-model', 'upgrade_model',
          'Upgrade Model', 'llm', '["chat"]', '{"sha256":"abc"}',
          '{"family":"llama","description":"kept across upgrade"}',
          '2026-10-01T00:00:00Z'
        );
        INSERT INTO model_type_arch_rules (pattern, match_style, model_type, source)
          VALUES ('UserRuleForASR', 'suffix', 'audio', 'user');
        INSERT INTO model_metadata_baselines (model_id, schema_version, baseline_json, created_by)
          VALUES ('llm/llama/upgrade-model', 1, '{"family":"llama"}', 'pumas-library');
        INSERT INTO model_metadata_overlays (overlay_id, model_id, overlay_json, created_by)
          VALUES ('overlay-1', 'llm/llama/upgrade-model', '{"review_status":"reviewed"}', 'person');
        "#,
    )
    .unwrap();
    assert_eq!(user_version(&path), 0);
    path
}

fn assert_library_content_kept(index: &ModelIndex) {
    let model = index.get("llm/llama/upgrade-model").unwrap().unwrap();
    assert_eq!(model.official_name, "Upgrade Model");
    assert_eq!(model.metadata["description"], "kept across upgrade");
    assert_eq!(
        index.search("upgrade", None, None, 10, 0).unwrap().models[0].id,
        "llm/llama/upgrade-model"
    );
    assert!(index
        .list_active_model_type_arch_rules()
        .unwrap()
        .iter()
        .any(|rule| rule.pattern == "UserRuleForASR"));
    let effective: serde_json::Value = serde_json::from_str(
        &index
            .get_effective_metadata_json("llm/llama/upgrade-model")
            .unwrap()
            .unwrap(),
    )
    .unwrap();
    assert_eq!(effective["review_status"], "reviewed");
}

#[test]
fn a_new_index_is_stamped_with_the_current_schema() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("models.db");

    drop(ModelIndex::new(&path).unwrap());

    assert_eq!(user_version(&path), INDEX_SCHEMA_VERSION);
}

#[test]
fn a_0_8_0_library_opens_with_its_content_and_is_stamped_once() {
    let temp = TempDir::new().unwrap();
    let path = library_0_8_0(&temp, INDEX_0_8_0);

    let index = ModelIndex::new(&path).unwrap();
    assert_library_content_kept(&index);
    drop(index);
    assert_eq!(user_version(&path), INDEX_SCHEMA_VERSION);

    // Reopening a stamped library is the ordinary path and changes nothing.
    let index = ModelIndex::new(&path).unwrap();
    assert_library_content_kept(&index);
    drop(index);
    assert_eq!(user_version(&path), INDEX_SCHEMA_VERSION);
    let read_only = ModelIndex::open_read_only(&path).unwrap();
    assert!(read_only.get("llm/llama/upgrade-model").unwrap().is_some());
}

#[test]
fn an_unstamped_library_with_older_rule_constraints_is_migrated_and_stamped() {
    let temp = TempDir::new().unwrap();
    // A library from before the `decision` and `video` categories.
    let schema = INDEX_0_8_0
        .replace("'reranker', 'decision',", "'reranker',")
        .replace("'diffusion', 'video',", "'diffusion',");
    assert_ne!(schema, INDEX_0_8_0);
    let path = library_0_8_0(&temp, &schema);

    let index = ModelIndex::new(&path).unwrap();

    assert_library_content_kept(&index);
    let arch_sql: String = index
        .conn
        .lock()
        .unwrap()
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'model_type_arch_rules'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(arch_sql.contains("'decision'") && arch_sql.contains("'video'"));
    drop(index);
    assert_eq!(user_version(&path), INDEX_SCHEMA_VERSION);
}

#[test]
fn a_failed_upgrade_leaves_the_library_unstamped_and_unchanged() {
    let temp = TempDir::new().unwrap();
    let schema = INDEX_0_8_0.replace("'reranker', 'decision',", "'reranker',");
    let path = library_0_8_0(&temp, &schema);
    {
        // Stale search triggers make the upgrade repopulate the search
        // table, and a row whose tags cannot be projected fails that step
        // after the rule tables were already rebuilt.
        let conn = Connection::open(&path).unwrap();
        conn.execute_batch(
            r#"DROP TRIGGER model_search_ai;
               INSERT INTO models VALUES ('broken', 'broken', 'broken', 'broken', 'llm',
                 '{not json', '{}', '{}', '2026-10-01T00:00:00Z');"#,
        )
        .unwrap();
    }

    assert!(ModelIndex::new(&path).is_err());

    assert_eq!(user_version(&path), 0);
    let arch_sql: String = Connection::open(&path)
        .unwrap()
        .query_row(
            "SELECT sql FROM sqlite_master WHERE name = 'model_type_arch_rules'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert!(!arch_sql.contains("'decision'"));
    assert!(!table_exists(&path, "model_type_arch_rules_legacy"));
}

#[test]
fn a_library_from_a_newer_release_is_refused_without_changes() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("models.db");
    Connection::open(&path)
        .unwrap()
        .execute_batch(&format!(
            "PRAGMA user_version = {}; CREATE TABLE future_table (id INTEGER);",
            INDEX_SCHEMA_VERSION + 1
        ))
        .unwrap();

    for result in [
        ModelIndex::new(&path).map(drop),
        ModelIndex::open_read_only(&path).map(drop),
    ] {
        match result {
            Err(PumasError::Database { message, source }) => {
                assert!(source.is_none());
                assert!(message.contains("newer Pumas Library"), "{message}");
                assert!(
                    message.contains(&(INDEX_SCHEMA_VERSION + 1).to_string()),
                    "{message}"
                );
            }
            other => panic!("expected a refusal, got {other:?}"),
        }
    }

    assert_eq!(user_version(&path), INDEX_SCHEMA_VERSION + 1);
    assert!(!table_exists(&path, "models"));
}

#[test]
fn an_unknown_schema_identity_is_refused() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("models.db");
    Connection::open(&path)
        .unwrap()
        .execute_batch("PRAGMA user_version = -3;")
        .unwrap();

    let message = ModelIndex::new(&path).err().unwrap().to_string();
    assert!(message.contains("-3"), "{message}");
    assert!(!table_exists(&path, "models"));
}

#[test]
fn an_unstamped_database_with_a_foreign_models_table_is_refused() {
    let temp = TempDir::new().unwrap();
    let path = temp.path().join("models.db");
    Connection::open(&path)
        .unwrap()
        .execute_batch("CREATE TABLE models (id TEXT PRIMARY KEY, name TEXT);")
        .unwrap();

    let message = ModelIndex::new(&path).err().unwrap().to_string();
    assert!(message.contains("not one Pumas"), "{message}");
    assert_eq!(user_version(&path), 0);
    assert!(!table_exists(&path, "model_library_update_events"));
}

#[test]
fn a_stamped_index_missing_its_search_table_has_it_rebuilt() {
    let temp = TempDir::new().unwrap();
    let path = library_0_8_0(&temp, INDEX_0_8_0);
    drop(ModelIndex::new(&path).unwrap());
    Connection::open(&path)
        .unwrap()
        .execute_batch(
            "DROP TRIGGER model_search_ai; DROP TRIGGER model_search_au;
             DROP TRIGGER model_search_ad; DROP TABLE model_search;",
        )
        .unwrap();

    let index = ModelIndex::new(&path).unwrap();

    assert_library_content_kept(&index);
}
