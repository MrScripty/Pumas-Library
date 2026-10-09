//! Real local Pumas HTTP/IPC processes with inference plugins disabled. These
//! fixtures qualify the admission boundary, not models or crash reclamation.
use super::*;
use pumas_library::discovery::{CompatibilityRequirements, LocalDiscovery};
use pumas_library::registry::LibraryRegistry;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::Duration;

const CHILD_TEST: &str = "discovery::process_tests::local_owner_process_fixture";

struct OwnerProcess {
    child: Child,
    control: PathBuf,
    log: PathBuf,
}
impl OwnerProcess {
    fn spawn(root: &Path, registry: &Path, control: &Path, port: u16) -> Self {
        Self::spawn_with_gate(root, registry, control, port, None)
    }

    fn spawn_with_gate(
        root: &Path,
        registry: &Path,
        control: &Path,
        port: u16,
        gate: Option<&Path>,
    ) -> Self {
        std::fs::create_dir_all(control).unwrap();
        let log = control.join("process.log");
        let output = std::fs::File::create(&log).unwrap();
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", CHILD_TEST, "--ignored", "--nocapture"])
            .env("PUMAS_FIXTURE_ROOT", root)
            .env("PUMAS_FIXTURE_REGISTRY", registry)
            .env("PUMAS_FIXTURE_CONTROL", control)
            .env("PUMAS_FIXTURE_PORT", port.to_string())
            .stdout(Stdio::from(output.try_clone().unwrap()))
            .stderr(Stdio::from(output));
        if let Some(gate) = gate {
            command.env("PUMAS_FIXTURE_GATE", gate);
        }
        let child = command.spawn().unwrap();
        Self {
            child,
            control: control.to_owned(),
            log,
        }
    }

    async fn ready(&mut self) -> HttpServiceDescription {
        let path = self.control.join("ready.json");
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Ok(bytes) = std::fs::read(&path) {
                    if let Ok(description) = serde_json::from_slice(&bytes) {
                        return description;
                    }
                }
                if let Some(status) = self.child.try_wait().unwrap() {
                    panic!(
                        "owner exited {status}: {}",
                        std::fs::read_to_string(&self.log).unwrap()
                    );
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("actual owner readiness deadline")
    }

    async fn exit(&mut self) -> std::process::ExitStatus {
        tokio::time::timeout(Duration::from_secs(20), async {
            loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    return status;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "process did not settle: {}",
                std::fs::read_to_string(&self.log).unwrap()
            )
        })
    }

    async fn cancel_shutdown(&mut self) {
        std::fs::write(self.control.join("stop"), b"").unwrap();
        marker(&self.control.join("cancelled")).await;
        assert!(self.child.try_wait().unwrap().is_none());
    }

    async fn finish(&mut self) {
        self.cancel_shutdown().await;
        std::fs::write(self.control.join("release"), b"").unwrap();
        assert!(
            self.exit().await.success(),
            "{}",
            std::fs::read_to_string(&self.log).unwrap()
        );
    }
}
impl Drop for OwnerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

async fn marker(path: &Path) {
    tokio::time::timeout(Duration::from_secs(20), async {
        while !path.exists() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("marker deadline: {}", path.display()));
}

/// Invoked only as an exact child test by the native qualification tests.
#[test]
#[ignore = "owned helper process; requires explicit fixture environment"]
fn local_owner_process_fixture() {
    let root = PathBuf::from(std::env::var_os("PUMAS_FIXTURE_ROOT").unwrap());
    let registry_path = PathBuf::from(std::env::var_os("PUMAS_FIXTURE_REGISTRY").unwrap());
    let control = PathBuf::from(std::env::var_os("PUMAS_FIXTURE_CONTROL").unwrap());
    let port = std::env::var("PUMAS_FIXTURE_PORT")
        .unwrap()
        .parse()
        .unwrap();
    tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .unwrap()
        .block_on(async {
            if let Some(gate) = std::env::var_os("PUMAS_FIXTURE_GATE") {
                std::fs::write(control.join("armed"), b"").unwrap();
                marker(Path::new(&gate)).await;
            }
            let registry = LibraryRegistry::open_at(&registry_path).unwrap();
            let api = pumas_library::PumasApi::builder(&root)
                .auto_create_dirs(true)
                .with_registry(registry.clone())
                .with_hf_client(false)
                .with_process_manager(false)
                .with_connectivity_probe(false)
                .build()
                .await
                .unwrap();
            // Hold an existing external-custody receipt so cancellation deterministically
            // occurs while the actual core shutdown coordinator is still waiting.
            let mut held = api
                .prepare_http_service(
                    LoopbackHttpEndpoint::parse("http://127.0.0.1:1").unwrap(),
                    build_info(),
                )
                .unwrap();
            let server = Arc::new(
                crate::server::start_server(
                    api,
                    crate::server::LoopbackHost::parse("127.0.0.1").unwrap(),
                    port,
                    crate::http_transport::HttpShutdownPolicy::from_millis(1000).unwrap(),
                )
                .await
                .unwrap(),
            );
            let borrowed = LocalDiscovery::open_at(&registry_path)
                .unwrap()
                .borrow_http_service(&root, &CompatibilityRequirements::default())
                .await
                .unwrap();
            std::fs::write(
                control.join("ready.json"),
                serde_json::to_vec(borrowed.description()).unwrap(),
            )
            .unwrap();
            drop(borrowed);
            marker(&control.join("stop")).await;
            let waiter = tokio::spawn({
                let server = server.clone();
                async move { server.shutdown().await }
            });
            tokio::time::timeout(Duration::from_secs(10), async {
                while !registry.list_http_services().unwrap().is_empty() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await
            .unwrap();
            assert!(
                !waiter.is_finished(),
                "held external custody must delay core cessation"
            );
            waiter.abort();
            assert!(waiter.await.unwrap_err().is_cancelled());
            assert!(registry.get_instance(&root).unwrap().is_some());
            std::fs::write(control.join("cancelled"), b"").unwrap();
            marker(&control.join("release")).await;
            held.complete_shutdown(Ok(())).unwrap();
            server.wait().await.unwrap();
            assert!(registry.get_instance(&root).unwrap().is_none());
        });
}

fn http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .timeout(Duration::from_secs(5))
        .build()
        .unwrap()
}

fn fenced(request: reqwest::RequestBuilder, fence: &HttpAdmissionFence) -> reqwest::RequestBuilder {
    request
        .header(HTTP_INSTANCE_GENERATION_HEADER, &fence.instance_generation)
        .header(HTTP_SERVICE_GENERATION_HEADER, &fence.service_generation)
}

#[tokio::test]
async fn native_simultaneous_cold_bootstrap_has_one_owner() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    std::fs::create_dir(&root).unwrap();
    let db = temp.path().join("registry.db");
    drop(LibraryRegistry::open_at(&db).unwrap());
    let gate = temp.path().join("bootstrap-gate");
    let mut first =
        OwnerProcess::spawn_with_gate(&root, &db, &temp.path().join("first"), 0, Some(&gate));
    let mut second =
        OwnerProcess::spawn_with_gate(&root, &db, &temp.path().join("second"), 0, Some(&gate));
    marker(&first.control.join("armed")).await;
    marker(&second.control.join("armed")).await;
    std::fs::write(gate, b"").unwrap();
    let first_won = tokio::time::timeout(Duration::from_secs(20), async {
        loop {
            if first.control.join("ready.json").exists() {
                return true;
            }
            if second.control.join("ready.json").exists() {
                return false;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("one competing bootstrap must become ready");
    let (winner, loser) = if first_won {
        (&mut first, &mut second)
    } else {
        (&mut second, &mut first)
    };
    let description = winner.ready().await;
    assert!(!loser.exit().await.success());
    let registry = LibraryRegistry::open_read_only_at(&db).unwrap();
    assert_eq!(registry.list_instances().unwrap().len(), 1);
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().started_at,
        description.instance.generation
    );
    winner.finish().await;
    assert!(registry.get_instance(&root).unwrap().is_none());
}

#[tokio::test]
async fn native_fence_competing_bootstrap_cancellation_and_same_url_restart() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let db = temp.path().join("registry.db");
    let mut first = OwnerProcess::spawn(&root, &db, &temp.path().join("first"), 0);
    let description = first.ready().await;
    let old_fence = description.admission_fence().unwrap();
    let registry = LibraryRegistry::open_read_only_at(&db).unwrap();
    let before = registry.get_instance(&root).unwrap().unwrap();
    let client = http_client();
    let health = format!("{}/health", description.endpoint.as_str());
    let rpc = format!("{}/rpc", description.endpoint.as_str());
    let origin = "http://localhost:5173";
    assert_eq!(
        fenced(client.get(&health), &old_fence)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    assert_eq!(
        client.get(&health).send().await.unwrap().status(),
        StatusCode::OK
    );
    let mut wrong = old_fence.clone();
    wrong.service_generation.push_str("-stale");
    // A wrong fence blocks even a shutdown command before domain admission.
    let shutdown = serde_json::json!({"jsonrpc":"2.0", "id":1, "method":"shutdown", "params":{}});
    assert_eq!(
        fenced(client.post(&rpc).json(&shutdown), &wrong)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::PRECONDITION_FAILED
    );
    let browser = fenced(client.get(&health).header(header::ORIGIN, origin), &wrong)
        .send()
        .await
        .unwrap();
    assert_eq!(browser.status(), StatusCode::PRECONDITION_FAILED);
    assert_eq!(
        browser.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        origin
    );
    let partial = client
        .get(&health)
        .header(header::ORIGIN, origin)
        .header(
            HTTP_INSTANCE_GENERATION_HEADER,
            &old_fence.instance_generation,
        )
        .send()
        .await
        .unwrap();
    assert_eq!(partial.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        partial.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        origin
    );
    let preflight = client
        .request(reqwest::Method::OPTIONS, &rpc)
        .header(header::ORIGIN, origin)
        .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
        .header(
            header::ACCESS_CONTROL_REQUEST_HEADERS,
            format!(
                "content-type,{HTTP_INSTANCE_GENERATION_HEADER},{HTTP_SERVICE_GENERATION_HEADER}"
            ),
        )
        .send()
        .await
        .unwrap();
    assert!(preflight.status().is_success());
    let allowed = preflight.headers()[header::ACCESS_CONTROL_ALLOW_HEADERS]
        .to_str()
        .unwrap();
    assert!(allowed.contains(HTTP_INSTANCE_GENERATION_HEADER));
    assert!(allowed.contains(HTTP_SERVICE_GENERATION_HEADER));

    // Inject only rendezvous metadata in a real running process fixture. A
    // successor row cannot rebind the listener's retained generation.
    let connection = rusqlite::Connection::open(&db).unwrap();
    let mut hostile = description.clone();
    hostile.service_generation.push_str("-replaced");
    fn replace(connection: &rusqlite::Connection, description: &HttpServiceDescription) {
        connection.execute(
            "UPDATE http_services SET service_generation=?1, description_json=?2 WHERE library_path=?3",
            rusqlite::params![description.service_generation, serde_json::to_string(description).unwrap(),
                description.instance.library_root.to_string_lossy()],
        ).unwrap();
    }
    replace(&connection, &hostile);
    let unavailable = fenced(
        client.get(&health).header(header::ORIGIN, origin),
        &hostile.admission_fence().unwrap(),
    )
    .send()
    .await
    .unwrap();
    assert_eq!(unavailable.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        unavailable.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        origin
    );
    replace(&connection, &description);
    wrong = old_fence.clone();
    wrong.instance_generation.push_str("-stale");
    assert_eq!(
        fenced(client.post(&rpc).body("malformed JSON"), &wrong)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::PRECONDITION_FAILED
    );
    assert_eq!(
        client
            .get(&health)
            .header(
                HTTP_INSTANCE_GENERATION_HEADER,
                &old_fence.instance_generation
            )
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        fenced(client.get(&health), &old_fence)
            .header(HTTP_SERVICE_GENERATION_HEADER, "duplicate")
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let borrowed = LocalDiscovery::open_at(&db)
        .unwrap()
        .borrow_http_service(&root, &CompatibilityRequirements::default())
        .await
        .unwrap();
    assert_eq!(borrowed.description(), &description);
    drop(borrowed);
    let mut competitor = OwnerProcess::spawn(&root, &db, &temp.path().join("competitor"), 0);
    assert!(!competitor.exit().await.success());
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().started_at,
        before.started_at
    );
    assert_eq!(
        fenced(client.get(&health), &old_fence)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    first.cancel_shutdown().await;
    assert_eq!(
        registry.get_instance(&root).unwrap().unwrap().started_at,
        before.started_at
    );
    assert!(registry.list_http_services().unwrap().is_empty());
    std::fs::write(first.control.join("release"), b"").unwrap();
    assert!(first.exit().await.success());
    assert!(registry.get_instance(&root).unwrap().is_none());
    let port = url::Url::parse(description.endpoint.as_str())
        .unwrap()
        .port()
        .unwrap();
    let mut second = OwnerProcess::spawn(&root, &db, &temp.path().join("second"), port);
    let successor = second.ready().await;
    assert_eq!(successor.endpoint, description.endpoint);
    assert_ne!(successor.admission_fence().unwrap(), old_fence);
    assert_eq!(
        fenced(client.get(&health), &old_fence)
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::PRECONDITION_FAILED
    );
    assert_eq!(
        fenced(client.get(&health), &successor.admission_fence().unwrap())
            .send()
            .await
            .unwrap()
            .status(),
        StatusCode::OK
    );
    second.finish().await;
}

#[tokio::test]
async fn native_dead_owner_remains_unresolved_after_process_loss_and_registry_reopen() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("root");
    let db = temp.path().join("registry.db");
    let mut first = OwnerProcess::spawn(&root, &db, &temp.path().join("first"), 0);
    let description = first.ready().await;
    let before = LibraryRegistry::open_read_only_at(&db)
        .unwrap()
        .get_instance(&root)
        .unwrap()
        .unwrap();
    first.child.kill().unwrap();
    assert!(!first.child.wait().unwrap().success());
    let observer = LocalDiscovery::open_at(&db).unwrap();
    assert!(observer
        .borrow_http_service(&root, &CompatibilityRequirements::default())
        .await
        .is_err());
    let mut denied = OwnerProcess::spawn(&root, &db, &temp.path().join("denied"), 0);
    assert!(!denied.exit().await.success());
    let registry = LibraryRegistry::open_read_only_at(&db).unwrap();
    let retained = registry.get_instance(&root).unwrap().unwrap();
    assert_eq!(retained.started_at, before.started_at);
    assert_eq!(retained.connection_token, before.connection_token);
    assert_eq!(registry.list_http_services().unwrap(), vec![description]);
}
