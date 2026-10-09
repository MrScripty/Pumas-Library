//! Actual CLI processes; disposable stores only, no model/runtime execution.
#![cfg(target_os = "linux")]
use pumas_library::registry::LibraryRegistry;
use rusqlite::Connection;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::{Duration, Instant, SystemTime};

fn run(registry: &Path, arguments: &[&str]) -> Output {
    let temporary = tempfile::tempdir().unwrap();
    let out = temporary.path().join("stdout");
    let err = temporary.path().join("stderr");
    let mut child = Command::new(env!("CARGO_BIN_EXE_pumas-rpc"))
        .args(arguments)
        .env("PUMAS_REGISTRY_DB_PATH", registry)
        .env("ORT_SKIP_DOWNLOAD", "1")
        .env("TMPDIR", registry.parent().unwrap())
        .stdin(Stdio::null())
        .stdout(std::fs::File::create(&out).unwrap())
        .stderr(std::fs::File::create(&err).unwrap())
        .spawn()
        .unwrap();
    let owned_pid = child.id();
    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break status;
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            panic!("CLI exceeded owned-process deadline");
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let prefix = format!("pumas-registry-observation-{owned_pid}-");
    assert!(
        !std::fs::read_dir("/tmp").unwrap().any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(&prefix)),
        "owned CLI private scratch not cleaned"
    );
    Output {
        status,
        stdout: std::fs::read(out).unwrap(),
        stderr: std::fs::read(err).unwrap(),
    }
}

#[derive(Debug, PartialEq, Eq)]
struct FileState {
    bytes: Vec<u8>,
    directory: bool,
    modified: SystemTime,
    #[cfg(unix)]
    change: (i64, i64),
}

fn tree(root: &Path) -> BTreeMap<PathBuf, FileState> {
    let mut result = BTreeMap::new();
    if root.exists() {
        let metadata = std::fs::metadata(root).unwrap();
        result.insert(
            root.to_owned(),
            FileState {
                bytes: Vec::new(),
                directory: true,
                modified: metadata.modified().unwrap(),
                #[cfg(unix)]
                change: {
                    use std::os::unix::fs::MetadataExt;
                    (metadata.ctime(), metadata.ctime_nsec())
                },
            },
        );
        for entry in std::fs::read_dir(root).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                result.extend(tree(&path));
            } else {
                let metadata = std::fs::metadata(&path).unwrap();
                result.insert(
                    path.clone(),
                    FileState {
                        bytes: std::fs::read(&path).unwrap(),
                        directory: false,
                        modified: metadata.modified().unwrap(),
                        #[cfg(unix)]
                        change: {
                            use std::os::unix::fs::MetadataExt;
                            (metadata.ctime(), metadata.ctime_nsec())
                        },
                    },
                );
            }
        }
    }
    result
}

fn document(output: &Output) -> Value {
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["cli_schema_version"], 1);
    assert_eq!(value["discovery_schema_version"], 1);
    assert_eq!(value["observation"], "unverified_registry_copy");
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    value
}

#[test]
fn actual_missing_corrupt_and_conflicting_modes_do_not_create_or_repair() {
    let temp = tempfile::tempdir().unwrap();
    let missing = temp.path().join("absent-parent/registry.db");
    let before = tree(temp.path());
    let output = run(&missing, &["--discover-local"]);
    assert!(output.status.success());
    let value = document(&output);
    assert_eq!(value["registry_state"], "missing");
    assert_eq!(value["libraries"], serde_json::json!([]));
    assert!(
        tree(temp.path()) == before,
        "source contents, directory entries or modification/change metadata changed"
    );
    for mode in [
        "--build-info",
        "--describe-local-http",
        "--attach-or-start-local-http",
        "--retain-local-http-owner",
    ] {
        assert!(!run(&missing, &["--discover-local", mode]).status.success());
    }
    assert!(
        tree(temp.path()) == before,
        "source contents, directory entries or modification/change metadata changed"
    );
    let corrupt = temp.path().join("registry.db");
    std::fs::write(&corrupt, b"NOT-A-REGISTRY-PRIVATE-CONTENT").unwrap();
    let before = tree(temp.path());
    let output = run(&corrupt, &["--discover-local"]);
    assert!(!output.status.success());
    assert_eq!(document(&output)["registry_state"], "unavailable");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("PRIVATE-CONTENT"));
    assert!(
        tree(temp.path()) == before,
        "source contents, directory entries or modification/change metadata changed"
    );
}

#[test]
fn actual_closed_wal_enumeration_is_stable_redacted_and_source_unchanged() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("registered-library");
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("model-sentinel"), b"OWNED-DISPOSABLE-MODEL").unwrap();
    let database = temp.path().join("registry.db");
    let registry = LibraryRegistry::open_at(&database).unwrap();
    let library = registry.register(&root, "PRIVATE-LIBRARY-NAME").unwrap();
    registry.register_instance(&root, u32::MAX, 1).unwrap();
    drop(registry);
    let sql = Connection::open(&database).unwrap();
    sql.execute(
        "UPDATE libraries SET metadata_json=?1",
        ["{\"credential\":\"PRIVATE-METADATA-CREDENTIAL\"}"],
    )
    .unwrap();
    sql.execute(
        "UPDATE instances SET connection_token=?1, endpoint=?2",
        [
            "PRIVATE-CONNECTION-TOKEN",
            "/unrelated-private-transport-path",
        ],
    )
    .unwrap();
    drop(sql);
    assert!(!temp.path().join("registry.db-wal").exists());
    let before = tree(temp.path());
    let first = run(&database, &["--discover-local"]);
    let second = run(&database, &["--discover-local"]);
    assert!(first.status.success() && second.status.success());
    assert_eq!(first.stdout, second.stdout);
    let value = document(&first);
    assert_eq!(value["registry_state"], "observed");
    assert_eq!(value["libraries"].as_array().unwrap().len(), 1);
    assert_eq!(value["libraries"][0]["registry_library_id"], library.id);
    assert_eq!(
        value["libraries"][0]["library_root"],
        root.to_str().unwrap()
    );
    assert_eq!(value["libraries"][0]["owner"]["status"], "ready");
    assert!(value["libraries"][0].get("metadata_json").is_none());
    let text = String::from_utf8(first.stdout).unwrap();
    for secret in [
        "PRIVATE-LIBRARY-NAME",
        "PRIVATE-METADATA-CREDENTIAL",
        "PRIVATE-CONNECTION-TOKEN",
        "unrelated-private-transport-path",
        "pid",
        "endpoint",
    ] {
        assert!(!text.contains(secret));
    }
    // Dead/invalid PID is only a historical hint; no probing, cleanup or takeover.
    assert!(
        tree(temp.path()) == before,
        "source contents, directory entries or modification/change metadata changed"
    );
}

#[test]
fn actual_wal_only_committed_rows_are_observed_without_touching_source_shm() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("wal-only-library");
    std::fs::create_dir(&root).unwrap();
    let database = temp.path().join("registry.db");
    let registry = LibraryRegistry::open_at(&database).unwrap();
    let library = registry.register(&root, "WAL fixture").unwrap();
    assert!(!std::fs::read(&database)
        .unwrap()
        .windows(library.id.len())
        .any(|w| w == library.id.as_bytes()));
    assert!(std::fs::read(temp.path().join("registry.db-wal"))
        .unwrap()
        .windows(library.id.len())
        .any(|w| w == library.id.as_bytes()));
    let before = tree(temp.path());
    let output = run(&database, &["--discover-local"]);
    assert!(output.status.success());
    let value = document(&output);
    assert_eq!(value["libraries"][0]["registry_library_id"], library.id);
    assert!(value["libraries"][0]["owner"].is_null());
    assert!(
        tree(temp.path()) == before,
        "source contents, directory entries or modification/change metadata changed"
    );
    drop(registry);
}

#[test]
fn actual_invalid_schema_journal_alias_and_bounds_refuse_without_mutation() {
    let temp = tempfile::tempdir().unwrap();
    let database = temp.path().join("registry.db");
    let sql = Connection::open(&database).unwrap();
    sql.execute_batch("CREATE TABLE libraries(id TEXT,path TEXT); CREATE TABLE instances(library_path TEXT,started_at TEXT,status TEXT,transport_kind TEXT);").unwrap();
    sql.execute(
        "INSERT INTO libraries VALUES(?1,?2)",
        rusqlite::params!["x".repeat(4097), temp.path().to_str().unwrap()],
    )
    .unwrap();
    drop(sql);
    let before = tree(temp.path());
    let output = run(&database, &["--discover-local"]);
    assert!(!output.status.success());
    assert_eq!(document(&output)["registry_state"], "unavailable");
    assert!(
        tree(temp.path()) == before,
        "source contents, directory entries or modification/change metadata changed"
    );
    std::fs::write(temp.path().join("registry.db-journal"), b"fixture journal").unwrap();
    let before = tree(temp.path());
    assert!(!run(&database, &["--discover-local"]).status.success());
    assert!(
        tree(temp.path()) == before,
        "source contents, directory entries or modification/change metadata changed"
    );
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&database, temp.path().join("alias.db")).unwrap();
        let before = tree(temp.path());
        assert!(!run(&temp.path().join("alias.db"), &["--discover-local"])
            .status
            .success());
        assert!(
            tree(temp.path()) == before,
            "source contents, directory entries or modification/change metadata changed"
        );
    }
}

#[test]
fn actual_views_missing_schema_and_valid_schema_bounds_refuse_redacted() {
    for case in ["view", "missing-table", "field-bound", "row-bound", "empty"] {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        std::fs::create_dir(&root).unwrap();
        let database = temp.path().join("registry.db");
        let registry = LibraryRegistry::open_at(&database).unwrap();
        if case != "empty" {
            registry.register(&root, "fixture").unwrap();
        }
        drop(registry);
        let sql = Connection::open(&database).unwrap();
        match case {
            "view" => sql.execute_batch("ALTER TABLE libraries RENAME TO private_libraries; CREATE VIEW libraries AS SELECT metadata_json AS id,path FROM private_libraries;").unwrap(),
            "missing-table" => sql.execute_batch("DROP TABLE instances;").unwrap(),
            "field-bound" => { sql.execute("UPDATE libraries SET id=?1", ["x".repeat(4097)]).unwrap(); },
            "row-bound" => { sql.execute("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<4097) INSERT INTO libraries(id,name,path,created_at,last_accessed) SELECT 'fixture-'||x,'fixture',?1||'/'||x,'fixture','fixture' FROM n", [root.to_str().unwrap()]).unwrap(); },
            "empty" => {},
            _ => unreachable!(),
        }
        drop(sql);
        let before = tree(temp.path());
        let output = run(&database, &["--discover-local"]);
        let value = document(&output);
        assert_eq!(output.status.success(), case == "empty");
        assert_eq!(
            value["registry_state"],
            if case == "empty" {
                "observed"
            } else {
                "unavailable"
            }
        );
        assert_eq!(value["libraries"], serde_json::json!([]));
        assert!(
            tree(temp.path()) == before,
            "source changed for refusal/empty fixture"
        );
    }
}
