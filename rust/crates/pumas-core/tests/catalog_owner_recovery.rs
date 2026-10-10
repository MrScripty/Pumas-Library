//! Actual owned Linux processes. Raw corruption/transition fixtures are labelled.
#![cfg(target_os = "linux")]
use pumas_library::discovery::{recover_catalog_owner, CompatibilityRequirements, LocalDiscovery};
use pumas_library::registry::LibraryRegistry;
use pumas_library::{
    CatalogOwnerCheckpoint, CatalogQueryRequest, CatalogQueryResponse, InstanceProfile, ModelIndex,
    ModelRecord, PumasApi, PumasLocalClient,
};
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn record() -> ModelRecord {
    ModelRecord {
        id: "llm/fixture/acknowledged".into(),
        path: "llm/fixture/acknowledged".into(),
        cleaned_name: "acknowledged".into(),
        official_name: "Owned durable catalog sentinel".into(),
        model_type: "llm".into(),
        tags: vec!["disposable".into()],
        hashes: Default::default(),
        metadata: serde_json::json!({"notes":"ACK-CATALOG-17","revision":17}),
        updated_at: "2026-10-09T00:00:00Z".into(),
    }
}
fn fixture() -> (tempfile::TempDir, PathBuf, LibraryRegistry) {
    let t = tempfile::tempdir().unwrap();
    let root = t.path().join("root");
    std::fs::create_dir_all(root.join("shared-resources/models")).unwrap();
    std::fs::write(root.join("sentinel"), b"owned store unchanged").unwrap();
    let registry = LibraryRegistry::open_at(&t.path().join("registry.db")).unwrap();
    registry.register(&root, "disposable").unwrap();
    (t, root, registry)
}
fn seed(root: &Path) {
    let index = ModelIndex::new(root.join("shared-resources/models/models.db")).unwrap();
    index.upsert(&record()).unwrap();
}
async fn build(root: &Path, registry: LibraryRegistry) -> PumasApi {
    PumasApi::builder(root)
        .with_registry(registry)
        .with_instance_profile(InstanceProfile::CatalogQuery)
        .build()
        .await
        .unwrap()
}
fn publish(path: &Path, bytes: &[u8]) {
    std::fs::write(path.with_extension("tmp"), bytes).unwrap();
    std::fs::rename(path.with_extension("tmp"), path).unwrap();
}
struct Owned {
    child: Child,
    control: PathBuf,
}
impl Owned {
    fn spawn(temp: &Path, mode: &str, label: &str, gate: Option<&Path>) -> Self {
        let control = temp.join(label);
        std::fs::create_dir(&control).unwrap();
        let log = std::fs::File::create(control.join("process.log")).unwrap();
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args(["--exact", "catalog_process", "--ignored", "--nocapture"])
            .env("PUMAS_CATALOG_FIXTURE", temp)
            .env("PUMAS_CATALOG_MODE", mode)
            .env("PUMAS_CATALOG_CONTROL", &control)
            .stdout(log.try_clone().unwrap())
            .stderr(log);
        if let Some(g) = gate {
            cmd.env("PUMAS_CATALOG_GATE", g);
        }
        Self {
            child: cmd.spawn().unwrap(),
            control,
        }
    }
    async fn marker(&mut self, name: &str) {
        tokio::time::timeout(Duration::from_secs(20), async {
            while !self.control.join(name).exists() {
                if let Some(s) = self.child.try_wait().unwrap() {
                    panic!(
                        "owned child exited {s}: {}",
                        std::fs::read_to_string(self.control.join("process.log")).unwrap()
                    );
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }
    fn kill(&mut self) {
        self.child.kill().unwrap();
        let s = self.child.wait().unwrap();
        assert!(!s.success());
        println!("owned SIGKILL/reap pid={} status={s}", self.child.id());
    }
    async fn join_refused(&mut self) {
        assert!(self.control.join("refused").exists());
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    assert!(status.success(), "refused fixture child failed: {status}");
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("refused fixture child did not exit");
    }
    fn checkpoint(&self) -> CatalogOwnerCheckpoint {
        serde_json::from_slice(&std::fs::read(self.control.join("checkpoint")).unwrap()).unwrap()
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        if self.child.try_wait().unwrap().is_none() {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
#[tokio::test]
#[ignore = "owned process helper"]
async fn catalog_process() {
    let Some(temp) = std::env::var_os("PUMAS_CATALOG_FIXTURE") else {
        return;
    };
    let temp = PathBuf::from(temp);
    let root = temp.join("root");
    let control = PathBuf::from(std::env::var_os("PUMAS_CATALOG_CONTROL").unwrap());
    let mode = std::env::var("PUMAS_CATALOG_MODE").unwrap();
    if mode == "wal_seed" {
        let index = ModelIndex::new(root.join("shared-resources/models/models.db")).unwrap();
        index.checkpoint_wal().unwrap();
        let conn = rusqlite::Connection::open(index.db_path()).unwrap();
        conn.execute_batch("PRAGMA synchronous=FULL; PRAGMA wal_autocheckpoint=0;")
            .unwrap();
        index.upsert(&record()).unwrap();
        publish(&control.join("committed"), b"ACK-CATALOG-17");
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
    let registry = LibraryRegistry::open_at(&temp.join("registry.db")).unwrap();
    if let Some(gate) = std::env::var_os("PUMAS_CATALOG_GATE") {
        publish(&control.join("waiting"), b"waiting");
        while !Path::new(&gate).exists() {
            tokio::time::sleep(Duration::from_millis(2)).await;
        }
    }
    let result = if mode == "full" {
        PumasApi::builder(&root)
            .with_registry(registry.clone())
            .auto_create_dirs(true)
            .with_hf_client(false)
            .with_process_manager(false)
            .with_connectivity_probe(false)
            .build()
            .await
    } else if mode == "recover" {
        let expected =
            serde_json::from_slice(&std::fs::read(temp.join("expected.json")).unwrap()).unwrap();
        recover_catalog_owner(registry.clone(), &root, &expected).await
    } else {
        PumasApi::builder(&root)
            .with_registry(registry.clone())
            .with_instance_profile(InstanceProfile::CatalogQuery)
            .build()
            .await
    };
    let api = match result {
        Ok(a) => a,
        Err(e) => {
            publish(&control.join("refused"), e.to_string().as_bytes());
            return;
        }
    };
    if mode == "full" {
        publish(
            &control.join("full_ready"),
            &serde_json::to_vec(&api.instance_description().unwrap()).unwrap(),
        );
        while !control.join("stop").exists() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        api.shutdown_instance().await.unwrap();
        return;
    }
    let records = api.list_models().await.unwrap();
    assert_eq!(records.len(), 1);
    assert_eq!(records[0].metadata["notes"], "ACK-CATALOG-17");
    let checkpoint = api.catalog_checkpoint().await.unwrap();
    publish(
        &control.join("checkpoint"),
        &serde_json::to_vec(&checkpoint).unwrap(),
    );
    println!(
        "ready owned pid={} generation={} models_sha256={}",
        std::process::id(),
        checkpoint.generation,
        checkpoint.models_sha256
    );
    while !control.join("stop").exists() {
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    api.shutdown_instance().await.unwrap();
    drop(api);
}
#[tokio::test]
async fn actual_sigkill_wal_payload_cold_reopen_and_stale_generation_fence() {
    let (t, root, registry) = fixture();
    let mut seeder = Owned::spawn(t.path(), "wal_seed", "seeder", None);
    seeder.marker("committed").await;
    assert!(
        root.join("shared-resources/models/models.db-wal")
            .metadata()
            .unwrap()
            .len()
            > 0
    );
    assert!(
        !std::fs::read(root.join("shared-resources/models/models.db"))
            .unwrap()
            .windows(b"ACK-CATALOG-17".len())
            .any(|w| w == b"ACK-CATALOG-17")
    );
    seeder.kill();
    let mut owner = Owned::spawn(t.path(), "start", "owner", None);
    owner.marker("checkpoint").await;
    let expected = owner.checkpoint();
    assert!(recover_catalog_owner(registry.clone(), &root, &expected)
        .await
        .is_err());
    let client = PumasLocalClient::connect(registry.get_instance(&root).unwrap().unwrap())
        .await
        .unwrap();
    assert_eq!(client.catalog_checkpoint().await.unwrap(), expected);
    let CatalogQueryResponse::List(rows) = client
        .catalog_query(CatalogQueryRequest::List)
        .await
        .unwrap()
    else {
        panic!("list")
    };
    assert_eq!(rows[0].metadata["revision"], 17);
    drop(client);
    owner.kill();
    let api = recover_catalog_owner(registry.clone(), &root, &expected)
        .await
        .unwrap();
    let successor = api.catalog_checkpoint().await.unwrap();
    assert_ne!(successor.generation, expected.generation);
    assert_eq!(successor.models_sha256, expected.models_sha256);
    assert_eq!(
        api.get_model(&record().id).await.unwrap().unwrap().metadata["notes"],
        "ACK-CATALOG-17"
    );
    assert_eq!(
        api.search_models("durable", 10, 0)
            .await
            .unwrap()
            .total_count,
        1
    );
    assert!(recover_catalog_owner(registry.clone(), &root, &expected)
        .await
        .is_err());
    api.shutdown_instance().await.unwrap();
    drop(api);
    assert!(registry.get_instance(&root).unwrap().is_none());
    assert!(recover_catalog_owner(registry, &root, &expected)
        .await
        .is_err());
    assert_eq!(
        std::fs::read(root.join("sentinel")).unwrap(),
        b"owned store unchanged"
    );
}
#[tokio::test]
async fn actual_two_competing_recoverers_one_winner_and_killed_successor_replay() {
    let (t, root, registry) = fixture();
    seed(&root);
    let mut owner = Owned::spawn(t.path(), "start", "owner", None);
    owner.marker("checkpoint").await;
    let expected = owner.checkpoint();
    owner.kill();
    publish(
        &t.path().join("expected.json"),
        &serde_json::to_vec(&expected).unwrap(),
    );
    let gate = t.path().join("gate");
    let mut a = Owned::spawn(t.path(), "recover", "a", Some(&gate));
    let mut b = Owned::spawn(t.path(), "recover", "b", Some(&gate));
    a.marker("waiting").await;
    b.marker("waiting").await;
    publish(&gate, b"go");
    tokio::time::timeout(Duration::from_secs(20), async {
        while !(a.control.join("checkpoint").exists() || a.control.join("refused").exists())
            || !(b.control.join("checkpoint").exists() || b.control.join("refused").exists())
        {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_ne!(
        a.control.join("checkpoint").exists(),
        b.control.join("checkpoint").exists()
    );
    let (winner, loser) = if a.control.join("checkpoint").exists() {
        (&mut a, &mut b)
    } else {
        (&mut b, &mut a)
    };
    // The refusal marker precedes process teardown. Join that exact loser while
    // the acknowledged winner still owns the store before testing cold reopen.
    loser.join_refused().await;
    let next = winner.checkpoint();
    assert_eq!(next.models_sha256, expected.models_sha256);
    assert_ne!(next.generation, expected.generation);
    winner.kill();
    assert!(recover_catalog_owner(registry.clone(), &root, &expected)
        .await
        .is_err());
    let api = after_fork_release(|| recover_catalog_owner(registry.clone(), &root, &next)).await;
    api.shutdown_instance().await.unwrap();
}
// Like api::catalog::custody_tests::after_fork_release: other parallel
// process fixtures can briefly inherit an open lock until exec. This test-only
// bounded retry never changes production acquisition or recovery authority.
async fn after_fork_release<T, F, Fut>(mut acquire: F) -> T
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = pumas_library::Result<T>>,
{
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        match acquire().await {
            Ok(value) => return value,
            Err(pumas_library::PumasError::InvalidParams { message })
                if message.starts_with(
                    "Pumas library instance is already running for physical store",
                ) && tokio::time::Instant::now() < deadline =>
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
            Err(error) => panic!("catalog physical owner did not settle: {error}"),
        }
    }
}
async fn wire(port: u16, token: &str, method: &str, extra: serde_json::Value) -> serde_json::Value {
    let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port))
        .await
        .unwrap();
    let mut params = extra;
    params["connection_token"] = token.into();
    let bytes = serde_json::to_vec(
        &serde_json::json!({"jsonrpc":"2.0","id":1,"method":method,"params":params}),
    )
    .unwrap();
    stream.write_u32(bytes.len() as u32).await.unwrap();
    stream.write_all(&bytes).await.unwrap();
    let n = stream.read_u32().await.unwrap();
    let mut bytes = vec![0; n as usize];
    stream.read_exact(&mut bytes).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}
#[tokio::test]
async fn sealed_constructor_rust_ipc_http_and_honest_discovery() {
    let (t, root, registry) = fixture();
    seed(&root);
    let api = build(&root, registry.clone()).await;
    assert_eq!(api.instance_profile(), InstanceProfile::CatalogQuery);
    let description = api.instance_description().unwrap();
    assert_eq!(
        description.capabilities,
        vec![
            "catalog.indexed-query@1",
            "catalog.literal-search@1",
            "catalog.same-boot-recovery@1"
        ]
    );
    let observed = LocalDiscovery::open_at(&t.path().join("registry.db")).unwrap();
    let requirements = CompatibilityRequirements {
        required_capabilities: vec!["catalog.indexed-query@1".into()],
        ..CompatibilityRequirements::default()
    };
    assert!(matches!(
        observed.probe(&requirements).await.unwrap()[0],
        pumas_library::discovery::LiveInstanceObservation::Verified { .. }
    ));
    assert!(api.rebuild_model_index().await.is_err());
    assert!(api.is_conversion_environment_ready().await.is_err());
    assert!(api.ensure_conversion_environment().await.is_err());
    assert!(api.restart_launcher().await.is_err());
    assert!(api.prepare_local_startup_custody().is_err());
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| api.model_library())).is_err()
    );
    assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| api.acquisition())).is_err());
    let endpoint =
        pumas_library::discovery::LoopbackHttpEndpoint::parse("http://127.0.0.1:12345").unwrap();
    assert!(api
        .prepare_http_service(endpoint, pumas_library::PumasBuildInfo::library())
        .is_err());
    assert!(api.advertised_http_service().unwrap().is_none());
    let checkpoint = api.catalog_checkpoint().await.unwrap();
    let row = registry.get_instance(&root).unwrap().unwrap();
    for method in [
        "launch_runtime_profile",
        "start_conversion",
        "is_conversion_environment_ready",
        "ensure_conversion_environment",
        "intent_list_declarations",
        "subscribe_model_library_update_stream_since",
    ] {
        assert!(wire(
            row.port,
            row.connection_token.as_ref().unwrap(),
            method,
            if method == "subscribe_model_library_update_stream_since" {
                serde_json::json!({"cursor":"0"})
            } else {
                serde_json::json!({})
            }
        )
        .await["error"]
            .is_object());
    }
    assert!(wire(
        row.port,
        "bad-token",
        "catalog_query",
        serde_json::json!({"request":{"operation":"list"}})
    )
    .await["error"]
        .is_object());
    assert_eq!(api.catalog_checkpoint().await.unwrap(), checkpoint);
    assert!(!root.join("launcher-data").exists());
    api.shutdown_instance().await.unwrap();
}
#[tokio::test]
async fn cancelled_shutdown_waiter_retains_passive_guard_and_generation() {
    let (_t, root, registry) = fixture();
    seed(&root);
    let api = std::sync::Arc::new(build(&root, registry.clone()).await);
    let checkpoint = api.catalog_checkpoint().await.unwrap();
    let retention = api
        .retain_local_owner(&api.instance_description().unwrap())
        .unwrap();
    let waiter = tokio::spawn({
        let api = api.clone();
        async move { api.shutdown_instance().await }
    });
    while api.instance_description().is_ok() {
        tokio::task::yield_now().await;
    }
    waiter.abort();
    assert!(waiter.await.unwrap_err().is_cancelled());
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().started_at,
        checkpoint.generation
    );
    assert!(api.list_models().await.is_err());
    assert!(recover_catalog_owner(registry.clone(), &root, &checkpoint)
        .await
        .is_err());
    retention.release().await.unwrap();
    api.shutdown_instance().await.unwrap();
    assert!(registry.get_instance(&root).unwrap().is_none());
}
#[tokio::test]
async fn labelled_corrupt_changed_legacy_identity_and_constructor_fixtures_refuse() {
    for mode in [
        "corrupt_json",
        "changed_payload",
        "missing_receipt",
        "changed_generation",
        "bad_registry",
        "bad_boot",
        "bad_policy",
        "bad_database",
        "http",
        "symlink",
        "hardlink",
        "oversized_receipt",
        "oversized_library_id",
        "oversized_instance",
        "unsupported_schema",
    ] {
        let (t, root, registry) = fixture();
        seed(&root);
        let mut owner = Owned::spawn(t.path(), "start", "owner", None);
        owner.marker("checkpoint").await;
        let expected = owner.checkpoint();
        owner.kill();
        let conn = rusqlite::Connection::open(t.path().join("registry.db")).unwrap();
        match mode {
            "oversized_receipt" => {
                conn.execute(
                    "UPDATE pending_reservation_checkpoints SET qualification_json=?1",
                    ["x".repeat(1024 * 1024)],
                )
                .unwrap();
            }
            "oversized_library_id" => {
                conn.execute("UPDATE libraries SET id=?1", ["x".repeat(1024 * 1024)])
                    .unwrap();
            }
            "oversized_instance" => {
                let q: String = conn
                    .query_row(
                        "SELECT qualification_json FROM pending_reservation_checkpoints",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap();
                conn.execute(
                    "UPDATE instances SET endpoint=?1",
                    ["x".repeat(1024 * 1024)],
                )
                .unwrap();
                conn.execute(
                    "INSERT INTO pending_reservation_checkpoints VALUES(?1,?2)",
                    rusqlite::params![root.to_string_lossy(), q],
                )
                .unwrap();
            }
            "unsupported_schema" => {
                let db = rusqlite::Connection::open(root.join("shared-resources/models/models.db"))
                    .unwrap();
                db.execute_batch("ALTER TABLE models RENAME TO old_models; CREATE TABLE models AS SELECT * FROM old_models;").unwrap();
            }

            "corrupt_json" => {
                let index =
                    rusqlite::Connection::open(root.join("shared-resources/models/models.db"))
                        .unwrap();
                index
                    .execute("UPDATE models SET tags_json='{}'", [])
                    .unwrap();
            }
            "changed_payload" => {
                let index =
                    ModelIndex::new(root.join("shared-resources/models/models.db")).unwrap();
                let mut r = record();
                r.metadata["revision"] = 18.into();
                index.upsert(&r).unwrap();
            }
            "missing_receipt" => {
                conn.execute("DELETE FROM pending_reservation_checkpoints", [])
                    .unwrap();
            }
            "changed_generation" => {
                conn.execute("UPDATE instances SET started_at='fixture-other'", [])
                    .unwrap();
            }
            "bad_registry" => {
                conn.execute(
                    "UPDATE registry_config SET value='00000000-0000-0000-0000-000000000000'",
                    [],
                )
                .unwrap();
            }
            "bad_boot" | "bad_policy" | "bad_database" => {
                let text: String = conn
                    .query_row(
                        "SELECT qualification_json FROM pending_reservation_checkpoints",
                        [],
                        |r| r.get(0),
                    )
                    .unwrap();
                let mut q: serde_json::Value = serde_json::from_str(&text).unwrap();
                let field = match mode {
                    "bad_boot" => "boot_id",
                    "bad_policy" => "policy",
                    _ => "database_identity",
                };
                q[field] = serde_json::Value::Null;
                conn.execute(
                    "UPDATE pending_reservation_checkpoints SET qualification_json=?1",
                    [serde_json::to_string(&q).unwrap()],
                )
                .unwrap();
            }
            "http" => {
                conn.execute(
                    "INSERT INTO http_services VALUES(?1,'fixture','fixture','fixture','{}')",
                    [root.to_string_lossy().as_ref()],
                )
                .unwrap();
            }
            "symlink" => {
                let dir = root.join("shared-resources/models");
                let moved = root.join("aliased-models");
                std::fs::rename(&dir, &moved).unwrap();
                std::os::unix::fs::symlink(moved, &dir).unwrap();
            }
            "hardlink" => {
                std::fs::hard_link(
                    root.join("shared-resources/models/models.db"),
                    root.join("alias.db"),
                )
                .unwrap();
            }
            _ => unreachable!(),
        };
        drop(conn);
        assert!(
            recover_catalog_owner(registry.clone(), &root, &expected)
                .await
                .is_err(),
            "{mode}"
        );
        assert!(registry.get_instance(&root).unwrap().is_some(), "{mode}");
    }
    let (_t, root, registry) = fixture();
    assert!(PumasApi::builder(&root)
        .with_registry(registry.clone())
        .with_instance_profile(InstanceProfile::CatalogQuery)
        .auto_create_dirs(true)
        .build()
        .await
        .is_err());
    assert!(registry.get_instance(&root).unwrap().is_none());
    assert!(PumasApi::builder(&root)
        .with_registry(registry.clone())
        .with_instance_profile(InstanceProfile::CatalogQuery)
        .build()
        .await
        .is_err());
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().status,
        pumas_library::registry::InstanceStatus::Claiming
    );
}

#[tokio::test]
async fn actual_killed_full_owner_remains_unqualified() {
    let (t, root, registry) = fixture();
    seed(&root);
    let mut owner = Owned::spawn(t.path(), "full", "full-owner", None);
    owner.marker("full_ready").await;
    let description: pumas_library::discovery::InstanceDescription =
        serde_json::from_slice(&std::fs::read(owner.control.join("full_ready")).unwrap()).unwrap();
    owner.kill();
    let expected = CatalogOwnerCheckpoint {
        contract_version: 1,
        generation: description.generation.clone(),
        library_id: description.registry_library_id,
        models_sha256: "0".repeat(64),
        checkpoint_id: uuid::Uuid::new_v4().to_string(),
    };
    assert!(recover_catalog_owner(registry.clone(), &root, &expected)
        .await
        .is_err());
    assert!(PumasApi::builder(&root)
        .with_registry(registry.clone())
        .with_instance_profile(InstanceProfile::CatalogQuery)
        .build()
        .await
        .is_err());
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().started_at,
        description.generation
    );
}

#[tokio::test]
async fn labelled_live_generation_change_fences_old_dispatch_and_changed_token() {
    let (t, root, registry) = fixture();
    seed(&root);
    let api = build(&root, registry.clone()).await;
    let owner = registry.get_instance(&root).unwrap().unwrap();
    let conn = rusqlite::Connection::open(t.path().join("registry.db")).unwrap();
    conn.execute("UPDATE instances SET started_at='fixture-successor',connection_token='fixture-successor-token'",[]).unwrap();
    for token in [
        owner.connection_token.as_ref().unwrap().as_str(),
        "fixture-successor-token",
    ] {
        assert!(wire(
            owner.port,
            token,
            "catalog_query",
            serde_json::json!({"request":{"operation":"list"}})
        )
        .await["error"]
            .is_object());
    }
    assert!(api.list_models().await.is_err());
    api.shutdown_instance().await.unwrap();
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().started_at,
        "fixture-successor"
    );
}
