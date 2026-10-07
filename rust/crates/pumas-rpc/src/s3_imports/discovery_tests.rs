use super::tests::{gguf, rpc, terminal};
use super::*;
use crate::server::{start_server, LoopbackHost};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::sync::atomic::{AtomicBool, Ordering};

const ID: &str = "c3f7d104-1234-4321-abcd-aaaaaaaaaaaa";
const ACCESS: &str = "discovery-synthetic-key";
const SECRET: &str = "discovery-synthetic-secret";
const TOKEN: &str = "discovery-synthetic-token";
fn request(endpoint: &str, deadline: bool) -> Value {
    json!({"operation_id":ID,"endpoint":endpoint,"region":"fixture-region","bucket":"fixture-bucket","addressing":"path","prefix":"models/","timeout_ms":if deadline {100} else {3000}})
}
struct Source {
    endpoint: String,
    stop: Arc<AtomicBool>,
    thread: Option<std::thread::JoinHandle<Vec<String>>>,
    started: tokio::sync::oneshot::Receiver<()>,
}
impl Source {
    fn start(mode: u8) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!(
            "https://localhost:{}",
            listener.local_addr().unwrap().port()
        );
        let stop = Arc::new(AtomicBool::new(false));
        let stopping = stop.clone();
        let (started, waiting) = tokio::sync::oneshot::channel();
        let thread = std::thread::spawn(move || {
            let identity = native_tls::Identity::from_pkcs12(
                include_bytes!("../../../pumas-core/tests/fixtures/http-tls/localhost.p12"),
                "fixture",
            )
            .unwrap();
            let acceptor = native_tls::TlsAcceptor::new(identity).unwrap();
            let mut requests = vec![];
            let mut started = Some(started);
            let deadline = std::time::Instant::now() + Duration::from_secs(15);
            while !stopping.load(Ordering::Acquire) {
                let socket = match listener.accept() {
                    Ok((s, _)) => s,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        assert!(std::time::Instant::now() < deadline, "source deadline");
                        std::thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(_) => panic!("source accept failed"),
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                socket
                    .set_write_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut socket = acceptor.accept(socket).unwrap();
                let mut bytes = vec![];
                while !bytes.ends_with(b"\r\n\r\n") {
                    assert!(bytes.len() < 16 * 1024);
                    let mut b = [0];
                    socket.read_exact(&mut b).unwrap();
                    bytes.push(b[0]);
                }
                let req = String::from_utf8(bytes).unwrap();
                requests.push(req.clone());
                let listing = req.contains("list-type=2");
                if listing && matches!(mode, 5 | 6 | 9) {
                    started.take().unwrap().send(()).unwrap();
                    let mut b = [0];
                    assert_eq!(
                        socket.read(&mut b).unwrap(),
                        0,
                        "cancel/deadline/shutdown must close listing"
                    );
                    break;
                }
                let config = req.contains("config.json");
                let body = if config { b"{}".to_vec() } else { gguf() };
                if listing {
                    if mode == 8 || (mode == 2 && req.contains("continuation-token=")) {
                        let reflected = format!("{ACCESS} {SECRET} {TOKEN}");
                        write!(socket,"HTTP/1.1 403 Forbidden\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",reflected.len(),reflected).unwrap();
                        continue;
                    }
                    let second = req.contains("continuation-token=");
                    let xml = if mode == 1 {
                        "<ListBucketResult><Name>fixture-bucket</Name><Prefix>models/</Prefix><MaxKeys>16</MaxKeys><KeyCount>0</KeyCount><IsTruncated>false</IsTruncated></ListBucketResult>".into()
                    } else if mode == 10 {
                        let first = if second { 16 } else { 0 };
                        let contents = (first..first+16).map(|n| format!("<Contents><Key>models/file{n:02}</Key><ETag>\"selected\"</ETag><Size>24</Size></Contents>")).collect::<String>();
                        format!("<ListBucketResult><Name>fixture-bucket</Name><Prefix>models/</Prefix><MaxKeys>16</MaxKeys><KeyCount>16</KeyCount><IsTruncated>{}</IsTruncated>{}{contents}</ListBucketResult>", !second, if second {"<ContinuationToken>fixture-next</ContinuationToken>"} else {"<NextContinuationToken>fixture-next</NextContinuationToken>"})
                    } else if mode == 3 {
                        "x".repeat(64 * 1024 + 1)
                    } else {
                        let (key, size) = if second {
                            ("models/weights.gguf", gguf().len())
                        } else {
                            ("models/config.json", 2)
                        };
                        format!("<ListBucketResult><Name>fixture-bucket</Name><Prefix>models/</Prefix><MaxKeys>16</MaxKeys><KeyCount>1</KeyCount>{}{}<Contents><Key>{key}</Key><ETag>\"selected\"</ETag><Size>{size}</Size></Contents></ListBucketResult>",
                            if mode==4 {String::new()} else {format!("<IsTruncated>{}</IsTruncated>",!second)},
                            if second {"<ContinuationToken>fixture-next</ContinuationToken>"} else {"<NextContinuationToken>fixture-next</NextContinuationToken>"})
                    };
                    write!(socket,"HTTP/1.1 200 OK\r\nContent-Type: application/xml\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",xml.len(),xml).unwrap();
                } else {
                    let head = req.starts_with("HEAD ");
                    let long_version = "v".repeat(4096);
                    let version = if mode == 10 {
                        long_version.as_str()
                    } else if config {
                        "config-v1"
                    } else {
                        "weights-v1"
                    };
                    let etag = if mode == 7 { "changed" } else { "selected" };
                    write!(socket,"HTTP/1.1 {}\r\nContent-Length: {}\r\nETag: \"{}\"\r\nx-amz-version-id: {}\r\nLast-Modified: Wed, 01 Jan 2025 00:00:00 GMT\r\n{}Connection: close\r\n\r\n",if head {"200 OK"} else {"206 Partial Content"},body.len(),etag,version,
                        if head {String::new()} else {format!("Content-Range: bytes 0-{}/{}\r\n",body.len()-1,body.len())}).unwrap();
                    if !head {
                        socket.write_all(&body).unwrap();
                    }
                }
                socket.flush().unwrap();
            }
            requests
        });
        Self {
            endpoint,
            stop,
            thread: Some(thread),
            started: waiting,
        }
    }
    fn finish(&mut self) -> Vec<String> {
        self.stop.store(true, Ordering::Release);
        self.thread.take().unwrap().join().unwrap()
    }
}
impl Drop for Source {
    fn drop(&mut self) {
        if self.thread.is_some() {
            self.stop.store(true, Ordering::Release);
            let _ = self.thread.take().unwrap().join();
        }
    }
}
async fn discovery_terminal(server: &crate::server::ServerHandle) -> Value {
    tokio::time::timeout(Duration::from_secs(8), async {
        loop {
            let out = rpc(
                server,
                "get_s3_prefix_discovery",
                json!({"operation_id":ID}),
            )
            .await;
            assert!(out["error"].is_null());
            if out["result"]["status"] != "running" {
                break out["result"].clone();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap()
}
#[tokio::test]
async fn source_prefix_rpc_https_discovery_and_explicit_bundle() {
    const MARKER: &str = "PUMAS_PREFIX_RPC_TLS_CHILD";
    if std::env::var_os(MARKER).is_none() {
        let config = tempfile::TempDir::new().unwrap();
        let output=std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact","s3_imports::discovery_tests::source_prefix_rpc_https_discovery_and_explicit_bundle","--nocapture"])
            .env(MARKER,"1").env("XDG_CONFIG_HOME",config.path())
            .env("SSL_CERT_FILE",Path::new(env!("CARGO_MANIFEST_DIR")).join("../pumas-core/tests/fixtures/http-tls/localhost.pem"))
            .output().unwrap();
        print!("{}", String::from_utf8_lossy(&output.stdout));
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    for (mode, authenticated) in [
        (0, false),
        (0, true),
        (1, false),
        (2, false),
        (3, false),
        (4, false),
        (5, false),
        (6, false),
        (7, false),
        (8, true),
        (9, false),
        (10, false),
    ] {
        let root = tempfile::TempDir::new().unwrap();
        let api = pumas_library::PumasApi::builder(root.path())
            .auto_create_dirs(true)
            .with_hf_client(false)
            .with_process_manager(false)
            .build()
            .await
            .unwrap();
        let library = api.model_library().clone();
        let acquisition = api.acquisition().clone();
        let server = start_server(
            api,
            LoopbackHost::parse("127.0.0.1").unwrap(),
            0,
            crate::http_transport::HttpShutdownPolicy::default(),
        )
        .await
        .unwrap();
        let mut source = Source::start(mode);
        let input = request(&source.endpoint, mode == 6);
        let mut invalid = input.clone();
        invalid["timeout_ms"] = json!(0);
        assert_eq!(
            rpc(&server, "start_s3_prefix_discovery", invalid).await["error"]["code"],
            -32602
        );
        let started = if authenticated {
            rpc(&server,"start_authenticated_s3_prefix_discovery",json!({"source":input,"credentials":{"access_key_id":ACCESS,"secret_access_key":SECRET,"session_token":TOKEN}})).await
        } else {
            rpc(&server, "start_s3_prefix_discovery", input).await
        };
        assert_eq!(started["result"]["status"], "running");
        if matches!(mode, 5 | 6 | 9) {
            tokio::time::timeout(Duration::from_secs(3), &mut source.started)
                .await
                .unwrap()
                .unwrap();
            if mode == 5 {
                assert_eq!(
                    rpc(
                        &server,
                        "cancel_s3_prefix_discovery",
                        json!({"operation_id":ID})
                    )
                    .await["result"]["status"],
                    "running"
                );
            } else if mode == 9 {
                server.shutdown().await.unwrap();
                let reqs = source.finish();
                assert_eq!(reqs.len(), 1);
                assert!(acquisition.store().acquisitions().unwrap().is_empty());
                println!("prefix control mode={mode} shutdown drained");
                continue;
            }
        }
        let observed = discovery_terminal(&server).await;
        assert_eq!(
            observed["status"],
            match mode {
                0 | 1 => "complete",
                3 | 10 => "incomplete",
                5 => "cancelled",
                6 => "deadline",
                _ => "unavailable",
            }
        );
        assert!(acquisition.store().acquisitions().unwrap().is_empty());
        assert!(!root.path().join(format!(".s3-import-{ID}")).exists());
        for secret in [ACCESS, SECRET, TOKEN, "fixture-next"] {
            assert!(!observed.to_string().contains(secret));
        }
        let mut expected_model = None;
        if mode == 0 {
            let objects = observed["objects"].as_array().unwrap();
            assert_eq!(objects.len(), 2);
            assert_eq!(observed["pages"], 2);
            assert_eq!(objects[0]["key"], "models/config.json");
            assert_eq!(objects[1]["version_id"], "weights-v1");
            let import = json!({"operation_id":ID,"endpoint":source.endpoint,"region":"fixture-region","bucket":"fixture-bucket","addressing":"path","primary_logical_path":"weights.gguf","family":"fixture","official_name":"Discovered Bundle",
                "files":[{"key":objects[0]["key"],"version_id":objects[0]["version_id"],"logical_path":"config/data.json","sha256":"44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a"},
                {"key":objects[1]["key"],"version_id":objects[1]["version_id"],"logical_path":"weights.gguf","sha256":"a4e5e156ddec27e286f75328784d7106b60a4eb1d246e950a001a3f944fbda99"}]});
            let mut bad = import.clone();
            bad["files"][0]["sha256"] = json!("");
            assert_eq!(
                rpc(&server, "start_s3_model_bundle_import", bad).await["error"]["code"],
                -32602
            );
            assert!(acquisition.store().acquisitions().unwrap().is_empty());
            assert_eq!(
                rpc(&server, "start_s3_model_bundle_import", import).await["result"]["status"],
                "running"
            );
            let settled = terminal(&server).await;
            assert_eq!(settled["result"]["status"], "completed");
            let model = settled["result"]["model_id"].as_str().unwrap();
            assert_eq!(library.get_model(model).await.unwrap().unwrap().id, model);
            assert_eq!(
                library
                    .get_effective_metadata(model)
                    .unwrap()
                    .unwrap()
                    .import_state,
                Some(pumas_library::models::ImportState::Ready)
            );
            let requirement = pumas_library::intent::ModelRequirement {
                selector: pumas_library::intent::ModelSelector::LocalModel {
                    model_ref: pumas_library::models::PumasModelRef {
                        model_id: model.into(),
                        ..Default::default()
                    },
                },
                artifact: pumas_library::intent::ArtifactRequirement::default(),
                acquisition_policy: pumas_library::intent::AcquisitionPolicy::LocalOnly,
            };
            let get = rpc(
                &server,
                "intent_get_model",
                json!({"requirement":requirement}),
            )
            .await;
            assert!(get["error"].is_null());
            // This inert GGUF fixture is registered, but does not qualify a
            // runnable inference artifact. The intent observer remains honest.
            assert_eq!(get["result"]["state"], "incomplete");
            assert!(get["result"].to_string().contains(model));
            expected_model = Some(model.to_owned());
            assert_eq!(
                std::fs::read(library.library_root().join(model).join("weights.gguf")).unwrap(),
                gguf()
            );
            assert_eq!(
                std::fs::read(library.library_root().join(model).join("config/data.json")).unwrap(),
                b"{}"
            );
            let records = acquisition.store().acquisitions().unwrap();
            assert_eq!(records.len(), 1);
            let record = records.values().next().unwrap();
            let consumer = acquisition.open_consumer("model.s3.workflow").unwrap();
            let receipt = consumer.completion_receipt(record).unwrap().unwrap();
            assert_eq!(receipt.manifest, record.manifest);
            assert_eq!(receipt.verified_files, record.files);
            assert_eq!(receipt.demand, record.demand);
            assert_eq!(receipt.owner, "model.s3.workflow");
            for secret in [ACCESS, SECRET, TOKEN, "fixture-next"] {
                assert!(!serde_json::to_string(&receipt).unwrap().contains(secret));
            }
            consumer.shutdown().await.unwrap();
        } else if mode == 1 {
            assert_eq!(observed["objects"], json!([]));
        } else {
            assert!(observed.get("objects").is_none());
        }
        server.shutdown().await.unwrap();
        if let Some(model) = expected_model {
            let cold = pumas_library::PumasApi::builder(root.path())
                .auto_create_dirs(true)
                .with_hf_client(false)
                .with_process_manager(false)
                .build()
                .await
                .unwrap();
            assert_eq!(cold.get_model(&model).await.unwrap().unwrap().id, model);
            assert_eq!(
                cold.model_library()
                    .get_effective_metadata(&model)
                    .unwrap()
                    .unwrap()
                    .import_state,
                Some(pumas_library::models::ImportState::Ready)
            );
            let cold_records = cold.acquisition().store().acquisitions().unwrap();
            assert_eq!(cold_records, acquisition.store().acquisitions().unwrap());
            cold.shutdown_intent().await.unwrap();
            cold.shutdown_downloads().await.unwrap();
            cold.shutdown_acquisition().await.unwrap();
        }
        let reqs = source.finish();
        assert_eq!(
            reqs.len(),
            match mode {
                0 => 8,
                1 | 3 | 4 | 5 | 6 | 8 => 1,
                2 => 2,
                7 => 3,
                10 => 34,
                _ => unreachable!(),
            }
        );
        if authenticated {
            assert!(reqs[0].contains(ACCESS));
            assert!(reqs[0]
                .to_ascii_lowercase()
                .contains("x-amz-security-token:"));
        }
        if mode == 0 {
            assert!(reqs[2].starts_with("HEAD "));
            assert!(reqs[3].starts_with("HEAD "));
            assert!(reqs[4].contains("versionId="));
            assert!(reqs[6].contains("versionId="));
        }
        println!(
            "prefix control mode={mode} authenticated={authenticated} outcome={} requests={}",
            observed["status"],
            reqs.len()
        );
    }
}

#[test]
fn discovery_and_import_share_one_admission_and_cancel_ack_is_not_completion() {
    let (client, mut receiver) = S3Imports::channel();
    let input: S3DiscoveryParams =
        serde_json::from_value(request("https://source.invalid", false)).unwrap();
    assert!(matches!(
        client.start_discovery(input, None).unwrap(),
        S3DiscoveryOutcome::Running { .. }
    ));
    assert!(matches!(
        client
            .admit(serde_json::from_value(super::tests::params("https://source.invalid")).unwrap())
            .unwrap(),
        S3ImportOutcome::Rejected { .. }
    ));
    assert!(matches!(
        client.cancel_discovery(ID).unwrap(),
        S3DiscoveryOutcome::Running { .. }
    ));
    let Job::Discovery(job) = receiver.try_recv().unwrap() else {
        panic!("expected discovery")
    };
    drop(job);
    client.finish_discovery(
        ID,
        S3DiscoveryOutcome::Complete {
            operation_id: ID.into(),
            objects: vec![],
            pages: 1,
        },
    );
    assert!(matches!(
        client.discovery_snapshot(Some(ID)).unwrap(),
        S3DiscoveryOutcome::Cancelled { .. }
    ));
    assert!(matches!(
        client
            .start_discovery(
                serde_json::from_value(request("https://source.invalid", false)).unwrap(),
                None
            )
            .unwrap(),
        S3DiscoveryOutcome::Rejected { .. }
    ));
    assert!(matches!(
        client
            .admit(serde_json::from_value(super::tests::params("https://source.invalid")).unwrap())
            .unwrap(),
        S3ImportOutcome::Running { .. }
    ));
    let mut other = request("https://source.invalid", false);
    other["operation_id"] = json!("c3f7d104-1234-4321-abcd-bbbbbbbbbbbb");
    assert!(matches!(
        client
            .start_discovery(serde_json::from_value(other).unwrap(), None)
            .unwrap(),
        S3DiscoveryOutcome::Rejected { .. }
    ));
    client.close();
}
