//! Actual shared HTTP -> held inspector -> catalog snapshot receipt controls.
use super::*;
use pumas_library::acquisition::AcquisitionStore;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Copy, Debug, PartialEq)]
enum Case {
    Success,
    FailedPage,
    MissingDependency,
    MissingSize,
    ChangedBody,
    ChangedTarget,
    ChangedExecutableAfterInspection,
    ChangedWheelAfterInspection,
    WrongSize,
    AbandonInspector,
    WrongDistInfo,
    Redirect,
    Selected,
    SelectedWrongSolver,
    SelectedSolverError,
    SelectedUnsupportedTarget,
    SelectedChangedExecutable,
    SelectedChangedWheel,
    SelectedProjectionMutation { file: &'static str, grow: bool },
    SelectedAbandonedChecker,
    Consumption(ConsumeCase),
    Lifecycle(LifecycleCase),
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum LifecycleCase {
    Success,
    FailedProbe,
    Mutation(&'static str),
    Publication(super::offline_selection::local_consumption::runtime::PublicationFault),
    Cancel,
    Abandon,
    AbandonPublication,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum ConsumeCase {
    Success,
    Mutation { file: &'static str, grow: bool },
    Abandon,
    Cancel,
}

async fn fixture(case: Case) {
    let root = Arc::new(tempfile::tempdir().unwrap());
    let metadata = Arc::new(MetadataManager::new(root.path()));
    metadata.ensure_directories().unwrap();
    let progress = Arc::new(RwLock::new(InstallationProgressTracker::new(
        root.path().join("cache"),
    )));
    let installer = VersionInstaller::new(
        root.path().to_owned(),
        AppId::Torch,
        metadata.clone(),
        progress,
        Arc::new(AtomicBool::new(false)),
    );
    assert!(installer.torch_control.start());
    let versions = installer.versions_dir();
    std::fs::create_dir_all(&versions).unwrap();
    let stage = Arc::new(
        TorchPendingStage::new(
            &versions,
            "v2.14.0",
            TorchVersionsLock::try_acquire(&versions).unwrap(),
        )
        .unwrap(),
    );
    let runtime = stage.path().join("runtime");
    write_embedded_torch_runtime(&runtime).unwrap();
    let log = root.path().join("fixture.log");
    let (progress_tx, _rx) = mpsc::channel(32);
    let mut create = Command::new("python3");
    create
        .args(["-I", "-m", "venv", "--copies"])
        .arg(runtime.join("venv"));
    installer
        .run_runtime_command(
            create,
            &log,
            "Create inert selected fixture",
            &progress_tx,
            Some(stage.clone()),
        )
        .await
        .unwrap();
    let python = pumas_library::platform::paths::venv_python(&runtime);
    let mut produced = installer
        .observe_qualified_torch_target(&runtime, &python, &stage, &log, &progress_tx)
        .await
        .unwrap();
    if matches!(
        case,
        Case::Selected
            | Case::SelectedSolverError
            | Case::SelectedChangedExecutable
            | Case::SelectedChangedWheel
            | Case::SelectedProjectionMutation { .. }
            | Case::SelectedAbandonedChecker
            | Case::Consumption(_)
            | Case::Lifecycle(_)
    ) {
        // Explicit test-only target declaration: uv 0.12.23 cannot represent
        // this executor's actual glibc 2.41. A synthetic 2.40 floor is validated
        // as a subset of actual native tags; production never changes approval.
        let mut observation: serde_json::Value =
            serde_json::from_str(&produced.observation).unwrap();
        observation["target"]["libc"] = serde_json::json!({"family":"glibc","version":"2.40"});
        produced.observation = serde_json::to_string(&observation).unwrap();
        std::fs::write(&produced.path, produced.observation.as_bytes()).unwrap();
    }
    let approved_hash = target_observation_digest(
        &serde_json::from_str::<serde_json::Value>(&produced.observation).unwrap(),
    )
    .unwrap();
    let source = root.path().join("source");
    std::fs::create_dir(&source).unwrap();
    let fixture_source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../torch-server/tests/test_install_verified_wheels.py");
    let code = r#"import importlib.util,json,pathlib,sys
s=importlib.util.spec_from_file_location('fixture',sys.argv[1]); m=importlib.util.module_from_spec(s); s.loader.exec_module(m)
content=b'VALUE = 7\n'
if sys.argv[3].startswith('lifecycle'):
 content=b"""from types import SimpleNamespace
__version__='2.14.0+cpu'
version=SimpleNamespace(cuda=None,hip=None)
cuda=SimpleNamespace(is_available=lambda:False)
backends=SimpleNamespace(mps=SimpleNamespace(is_available=lambda:False))
class Tensor:
 def __init__(self,values): self.values=values
 def __matmul__(self,other):
  return Tensor([[sum(a*b for a,b in zip(row,column)) for column in zip(*other.values)] for row in self.values])
 def tolist(self): return self.values
 def cpu(self): return self
def ones(shape): return Tensor([[1.0 for _ in range(shape[1])] for _ in range(shape[0])])
"""
 if sys.argv[3]=='lifecycle-failed': content=content.replace(b'self.values])',b'self.values]).bad()')
p=pathlib.Path(sys.argv[2]); rows=[m.make_wheel(p,'torch',version='2.14.0+cpu',content=content,requires=(['branch>=1','dependency<1'] if sys.argv[3]=='solver-error' else ['branch>=1'])),m.make_wheel(p,'branch',version='1',requires=['dependency<2']),m.make_wheel(p,'branch',version='2',requires=['dependency>=2']),m.make_wheel(p,'dependency',version='1'),m.make_wheel(p,'dependency',version='2')]
if sys.argv[3]=='wrong-info':
 import hashlib,zipfile
 a=rows[0]; path=p/a['url'].rsplit('/',1)[-1]
 with zipfile.ZipFile(path) as archive: members={m.filename:archive.read(m) for m in archive.infolist()}
 old='torch-2.14.0+cpu.dist-info'; new='wrong-1.0.dist-info'
 members={n.replace(old,new,1):body for n,body in members.items()}
 members[new+'/RECORD']=members[new+'/RECORD'].replace(old.encode(),new.encode())
 with zipfile.ZipFile(path,'w',zipfile.ZIP_DEFLATED) as archive:
  for name,body in members.items(): archive.writestr(name,body)
 a['sha256']=hashlib.sha256(path.read_bytes()).hexdigest()
for a in rows:
 filename=a['url'].rsplit('/',1)[-1]; a['size']=(p/filename).stat().st_size
 a['url']=('https://download.pytorch.org/whl/cpu/' if a['name']=='torch' else 'https://files.pythonhosted.org/packages/')+filename
print(json.dumps(rows))
"#;
    let generated = std::process::Command::new("python3")
        .args(["-I", "-c", code])
        .arg(fixture_source)
        .arg(&source)
        .arg(if case == Case::WrongDistInfo {
            "wrong-info"
        } else if case == Case::SelectedSolverError {
            "solver-error"
        } else if case == Case::Lifecycle(LifecycleCase::FailedProbe) {
            "lifecycle-failed"
        } else if matches!(case, Case::Lifecycle(_)) {
            "lifecycle"
        } else {
            "valid"
        })
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let artifacts: Vec<serde_json::Value> = serde_json::from_slice(&generated.stdout).unwrap();
    let mut observations = Vec::new();
    for name in ["torch", "branch", "dependency"] {
        if case == Case::MissingDependency && name == "dependency" {
            continue;
        }
        for repo in ["pytorch", "pypi"] {
            let base = if repo == "pytorch" {
                "https://download.pytorch.org/whl/cpu/"
            } else {
                "https://pypi.org/simple/"
            };
            let files: Vec<_> = artifacts.iter().filter(|a| a["name"] == name && repo == if name == "torch" { "pytorch" } else { "pypi" }).map(|a| {
                let mut row = serde_json::json!({"filename":torch_wheel_filename(a["url"].as_str().unwrap()).unwrap(),"url":a["url"],"hashes":{"sha256":a["sha256"]},"size":a["size"]});
                if case == Case::MissingSize { row.as_object_mut().unwrap().remove("size"); }
                if case == Case::WrongSize { row["size"] = serde_json::json!(a["size"].as_u64().unwrap() + 1); }
                row
            }).collect();
            let versions: std::collections::BTreeSet<_> = artifacts
                .iter()
                .filter(|a| {
                    a["name"] == name && repo == if name == "torch" { "pytorch" } else { "pypi" }
                })
                .map(|a| a["version"].as_str().unwrap().to_owned())
                .collect();
            observations.push(serde_json::json!({"repository":repo,"project":name,"url":format!("{base}{name}/"),
                "status":if case == Case::FailedPage && repo == "pypi" && name == "torch" { 503 } else { 200 },
                "observed_at":"2026-10-06T12:00:00Z",
                "body":serde_json::json!({"meta":{"api-version":"1.3"},"name":name,"files":files,"versions":versions}).to_string()}));
        }
    }
    let observed_path = produced.path.clone();
    let request = CatalogRequest::from_observed(
        "fixture-request-214",
        "2.14.0",
        "cpu",
        &["torch==2.14.0+cpu".into()],
        &[],
        serde_json::json!([]),
        serde_json::json!(observations),
        produced,
    )
    .unwrap();
    let request_hash = target_observation_digest(&request.wire).unwrap();
    if matches!(
        case,
        Case::ChangedExecutableAfterInspection
            | Case::ChangedWheelAfterInspection
            | Case::AbandonInspector
    ) {
        let helper = runtime.join("wheel_catalog_owner.py");
        let mut code = std::fs::read_to_string(&helper).unwrap();
        code.push_str(if case == Case::ChangedExecutableAfterInspection {
            "\n# Fixture-owned copied selected executable: replace after actual inspector exits.\nif '--wheels' in sys.argv:\n import os\n p=Path(sys.executable); q=p.with_name('catalog-executable-replacement'); q.write_bytes(b'changed selected catalog interpreter'); os.replace(q,p)\n"
        } else if case == Case::ChangedWheelAfterInspection {
            "\n# Fixture-only corruption after actual metadata inspection.\nif '--wheels' in sys.argv:\n p=next(Path(sys.argv[sys.argv.index('--wheels')+1]).glob('*/*.whl')); body=bytearray(p.read_bytes()); body[0]^=1; p.write_bytes(body)\n"
        } else {
            "\n# Fixture-only child that retains actual inspected inputs after output.\nif '--wheels' in sys.argv:\n import os,time\n Path(__file__).with_name('catalog-child-alive').write_text(str(os.getpid())); time.sleep(120)\n"
        });
        std::fs::write(helper, code).unwrap();
    }
    if case == Case::ChangedTarget {
        std::fs::write(observed_path, b"changed owned target evidence").unwrap();
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = requests.clone();
    let server_source = source.clone();
    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel();
    let server = tokio::spawn(async move {
        loop {
            let accepted = tokio::select! { biased; _ = &mut stop_rx => break, accepted = listener.accept() => accepted };
            let (mut socket, _) = accepted.unwrap();
            let mut raw = Vec::new();
            loop {
                let mut byte = [0];
                let count = tokio::select! { biased; _ = &mut stop_rx => return, count = socket.read(&mut byte) => count.unwrap() };
                if count == 0 {
                    break;
                }
                raw.push(byte[0]);
                if raw.ends_with(b"\r\n\r\n") {
                    break;
                }
                assert!(raw.len() <= 8192);
            }
            let header = String::from_utf8(raw).unwrap();
            let path = header.split_whitespace().nth(1).unwrap();
            let filename = path.rsplit('/').next().unwrap();
            let mut body = std::fs::read(server_source.join(filename)).unwrap();
            counter.fetch_add(1, Ordering::SeqCst);
            if case == Case::Redirect {
                socket.write_all(b"HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:9/unapproved\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
            } else {
                if case == Case::ChangedBody {
                    body[0] ^= 1;
                }
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                    body.len()
                );
                socket.write_all(response.as_bytes()).await.unwrap();
                socket.write_all(&body).await.unwrap();
            }
            let _ = socket.shutdown().await;
        }
    });
    let store = Arc::new(AcquisitionStore::new(root.path()));
    let service = Arc::new(AcquisitionService::new(store.clone()));
    let consumer = open_catalog_consumer(service.clone()).await.unwrap();
    let relative = Path::new(".torch-catalog-fixture");
    std::fs::create_dir(versions.join(relative)).unwrap();
    // A retained root/versions owner is independent of the pending runtime.
    let grant = ReservedDirectory::capture(&versions, relative, root.clone(), || Ok(())).unwrap();
    let context = CatalogContext {
        installer: &installer,
        consumer: &consumer,
        runtime: &runtime,
        python: &python,
        stage: stage.clone(),
        grant,
        log: &log,
        progress: &progress_tx,
        fixture_origin: Some(origin),
    };
    let mut retained_stage = Some(stage);
    let result = if case == Case::AbandonInspector {
        let mut flow = Box::pin(context.acquire(request));
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                tokio::select! {
                    _ = &mut flow => panic!("Inspector completed before abandonment control"),
                    _ = tokio::time::sleep(Duration::from_millis(25)) => {
                        if runtime.join("catalog-child-alive").exists() { break; }
                    }
                }
            }
        })
        .await
        .unwrap();
        drop(retained_stage.take());
        drop(flow);
        assert!(
            runtime.is_dir(),
            "Live inspector must pin its pending stage"
        );
        assert!(matches!(
            store.acquisitions().unwrap().values().next().unwrap().phase,
            AcquisitionPhase::Using { .. }
        ));
        Err(CatalogFailure::Incomplete)
    } else {
        tokio::time::timeout(Duration::from_secs(30), context.acquire(request))
            .await
            .unwrap()
    };
    let mut completion = match result {
        Ok(complete) => {
            assert!(matches!(
                case,
                Case::Success
                    | Case::Selected
                    | Case::SelectedWrongSolver
                    | Case::SelectedSolverError
                    | Case::SelectedUnsupportedTarget
                    | Case::SelectedChangedExecutable
                    | Case::SelectedChangedWheel
                    | Case::SelectedProjectionMutation { .. }
                    | Case::SelectedAbandonedChecker
                    | Case::Consumption(_)
                    | Case::Lifecycle(_)
            ));
            assert_eq!(complete.receipt.owner, CATALOG_OWNER);
            assert_eq!(complete.evidence["request_sha256"], request_hash);
            assert_eq!(
                complete.evidence["target_observation_sha256"],
                approved_hash
            );
            assert_eq!(
                complete.evidence["reachable"]["dependency"],
                if case == Case::SelectedSolverError {
                    serde_json::json!(["dependency<1", "dependency<2", "dependency>=2"])
                } else {
                    serde_json::json!(["dependency<2", "dependency>=2"])
                }
            );
            assert_eq!(complete.receipt.verified_files.len(), 5);
            for file in complete.receipt.manifest.files() {
                assert!(file.source_key().starts_with("https://"));
                assert!(!file.source_key().contains("127.0.0.1"));
                assert!(file.expected_size().is_some());
                assert_eq!(file.logical_path().split('/').count(), 2);
            }
            Some(complete)
        }
        Err(failure) => {
            assert_ne!(case, Case::Success);
            assert_eq!(
                failure,
                if matches!(
                    case,
                    Case::ChangedTarget
                        | Case::ChangedExecutableAfterInspection
                        | Case::ChangedWheelAfterInspection
                        | Case::WrongDistInfo
                ) {
                    CatalogFailure::Refused
                } else {
                    CatalogFailure::Incomplete
                }
            );
            None
        }
    };
    if matches!(
        case,
        Case::Selected
            | Case::SelectedWrongSolver
            | Case::SelectedSolverError
            | Case::SelectedUnsupportedTarget
            | Case::SelectedChangedExecutable
            | Case::SelectedChangedWheel
            | Case::SelectedProjectionMutation { .. }
            | Case::SelectedAbandonedChecker
            | Case::Consumption(_)
            | Case::Lifecycle(_)
    ) {
        use super::offline_selection::{
            QualifiedOfflineSolver, SelectionContext, SelectionFailure,
        };
        let complete = completion.take().unwrap();
        let granted_path = versions.join(relative);
        if matches!(
            case,
            Case::SelectedChangedExecutable
                | Case::SelectedChangedWheel
                | Case::SelectedProjectionMutation { .. }
                | Case::SelectedAbandonedChecker
        ) {
            let helper = runtime.join("offline_wheel_selection.py");
            let mut script = std::fs::read_to_string(&helper).unwrap();
            if let Case::SelectedProjectionMutation { file, grow } = case {
                script.push_str(&format!(
                    "\nif '--check' in sys.argv:\n import os\n p=Path(sys.argv[sys.argv.index('--directory')+1]) / {file:?}\n if {grow}:\n  with p.open('ab') as f: f.truncate(32 * 1024 * 1024)\n else:\n  q=p.with_name(p.name+'.replacement'); q.write_bytes(b'X'*p.stat().st_size); os.replace(q,p)\n",
                    grow = if grow { "True" } else { "False" },
                ));
                println!(
                    "post-checker projection fixture: file={} grow={grow}",
                    runtime.join("offline-selection").join(file).display()
                );
            } else {
                script.push_str(if case == Case::SelectedChangedExecutable {
                "\nif '--check' in sys.argv:\n import os\n p=Path(sys.executable); q=p.with_name('selection-python-replacement'); q.write_bytes(b'changed executable after selected proof'); os.replace(q,p)\n"
            } else if case == Case::SelectedChangedWheel {
                "\nif '--check' in sys.argv:\n p=next(Path(sys.argv[sys.argv.index('--wheels')+1]).glob('*/dependency-1-*.whl')); p.write_bytes(p.read_bytes()+b'changed unselected acquired input')\n"
            } else {
                "\nif '--check' in sys.argv:\n import os,time\n Path(__file__).with_name('selected-checker-alive').write_text(str(os.getpid())); time.sleep(120)\n"
            });
            }
            std::fs::write(helper, script).unwrap();
        }
        let solver = if case == Case::SelectedWrongSolver {
            python.clone()
        } else {
            PathBuf::from(
                std::env::var("PUMAS_QUALIFIED_UV")
                    .expect("Supply already qualified local uv; no downloader"),
            )
        };
        let context = SelectionContext {
            installer: &installer,
            consumer: &consumer,
            runtime: &runtime,
            python: &python,
            solver: QualifiedOfflineSolver { path: solver },
        };
        if case == Case::SelectedAbandonedChecker {
            let mut flow = Box::pin(complete.select(context));
            tokio::time::timeout(Duration::from_secs(15), async {
                loop {
                    tokio::select! {
                        _ = &mut flow => panic!("Checker completed before abandonment"),
                        _ = tokio::time::sleep(Duration::from_millis(25)) => {
                            if runtime.join("selected-checker-alive").exists() { break; }
                        }
                    }
                }
            })
            .await
            .unwrap();
            drop(retained_stage.take());
            drop(flow);
            assert!(
                runtime.is_dir(),
                "Managed selected checker must retain the pending runtime"
            );
            assert!(
                granted_path.is_dir(),
                "Managed selected checker must retain the catalog grant"
            );
            assert_eq!(std::fs::read_dir(&granted_path).unwrap().count(), 5);
        } else {
            match complete.select(context).await {
                Ok(packet) => {
                    assert!(matches!(
                        case,
                        Case::Selected | Case::Consumption(_) | Case::Lifecycle(_)
                    ));
                    assert_eq!(packet.evidence["schema"], "pumas.selected-wheel-packet.v1");
                    assert_eq!(packet.evidence["request_sha256"], request_hash);
                    assert_eq!(packet.evidence["target_observation_sha256"], approved_hash);
                    let selected = packet.evidence["selected"].as_array().unwrap();
                    assert_eq!(selected.len(), 3);
                    assert!(selected
                        .iter()
                        .all(|c| c["url"].as_str().unwrap().starts_with("https://")));
                    assert_eq!(packet.catalog.receipt.verified_files.len(), 5);
                    assert_eq!(
                        requests.load(Ordering::SeqCst),
                        5,
                        "Solver must not add source traffic"
                    );
                    println!("selected packet: {}", packet.evidence);
                    if let Case::Consumption(action) = case {
                        consume_fixture(
                            action,
                            packet,
                            &installer,
                            &consumer,
                            &runtime,
                            &python,
                            &granted_path,
                            &mut retained_stage,
                        )
                        .await;
                    } else if let Case::Lifecycle(action) = case {
                        lifecycle_fixture(
                            action,
                            packet,
                            &installer,
                            &consumer,
                            &runtime,
                            &python,
                            &granted_path,
                            &mut retained_stage,
                        )
                        .await;
                    } else {
                        packet.catalog._grant.validate().unwrap();
                        packet.catalog._grant.clear_contents().unwrap();
                    }
                }
                Err(refusal) => {
                    assert!(matches!(
                        case,
                        Case::SelectedWrongSolver
                            | Case::SelectedSolverError
                            | Case::SelectedUnsupportedTarget
                            | Case::SelectedChangedExecutable
                            | Case::SelectedChangedWheel
                            | Case::SelectedProjectionMutation { .. }
                    ));
                    assert_eq!(
                        refusal.kind,
                        if case == Case::SelectedSolverError {
                            SelectionFailure::SolverFailed
                        } else {
                            SelectionFailure::Refused
                        }
                    );
                    if let Case::SelectedProjectionMutation { file, grow } = case {
                        let selected = runtime.join("offline-selection/selected.json");
                        assert!(
                            selected.is_file(),
                            "Mutation must follow successful independent checking"
                        );
                        let modified = runtime.join("offline-selection").join(file);
                        assert!(std::fs::symlink_metadata(&modified)
                            .unwrap()
                            .file_type()
                            .is_file());
                        if grow {
                            assert_eq!(
                                std::fs::metadata(&modified).unwrap().len(),
                                32 * 1024 * 1024
                            );
                        } else {
                            assert!(std::fs::read(&modified).unwrap().iter().all(|b| *b == b'X'));
                        }
                        println!("post-checker projection refusal: file={file} grow={grow} kind={:?} checker_completed=true", refusal.kind);
                    }
                    refusal._catalog._grant.validate().unwrap();
                    assert_eq!(std::fs::read_dir(&granted_path).unwrap().count(), 5);
                    assert!(
                        case == Case::SelectedSolverError
                            || case == Case::SelectedUnsupportedTarget
                            || !runtime.join("offline-selection").exists()
                            || runtime.join("offline-selection/selected.json").exists()
                    );
                    assert!(runtime.is_dir());
                }
            }
        }
    }
    let consumer_pids: Vec<u32> = if matches!(
        case,
        Case::Consumption(ConsumeCase::Abandon | ConsumeCase::Cancel)
    ) {
        std::fs::read_to_string(runtime.join("consumer-child-alive"))
            .unwrap()
            .split_whitespace()
            .map(|pid| pid.parse().unwrap())
            .collect()
    } else if matches!(
        case,
        Case::Lifecycle(LifecycleCase::Cancel | LifecycleCase::Abandon)
    ) {
        std::fs::read_to_string(versions.join("lifecycle-child-pids"))
            .unwrap()
            .split_whitespace()
            .map(|pid| pid.parse().unwrap())
            .collect()
    } else {
        vec![]
    };
    installer.shutdown_torch_cleanup().await.unwrap();
    for pid in consumer_pids {
        if let Ok(stat) = std::fs::read_to_string(format!("/proc/{pid}/stat")) {
            assert_eq!(
                stat.rsplit_once(") ").unwrap().1.chars().next(),
                Some('Z'),
                "Consumer descendant must not remain live after drain"
            );
        }
    }
    if case == Case::Consumption(ConsumeCase::Abandon) {
        assert!(
            !runtime.exists(),
            "Stage can reclaim only after managed group drain"
        );
    }
    consumer.shutdown().await.unwrap();
    service.shutdown().await.unwrap();
    let _ = stop_tx.send(());
    tokio::time::timeout(Duration::from_secs(5), server)
        .await
        .unwrap()
        .unwrap();
    let rows = store.acquisitions().unwrap();
    if matches!(
        case,
        Case::FailedPage | Case::MissingSize | Case::ChangedTarget
    ) {
        assert!(rows.is_empty());
        assert_eq!(requests.load(Ordering::SeqCst), 0);
    } else {
        let row = rows.values().next().unwrap();
        let receipt = service.consumer_receipt(row.id).unwrap();
        assert_eq!(
            receipt.is_some(),
            matches!(
                case,
                Case::Success
                    | Case::Selected
                    | Case::SelectedWrongSolver
                    | Case::SelectedSolverError
                    | Case::SelectedUnsupportedTarget
                    | Case::SelectedChangedExecutable
                    | Case::SelectedChangedWheel
                    | Case::SelectedProjectionMutation { .. }
                    | Case::SelectedAbandonedChecker
                    | Case::Consumption(_)
                    | Case::Lifecycle(_)
            )
        );
        assert_eq!(
            matches!(row.phase, AcquisitionPhase::Adopted { .. }),
            matches!(
                case,
                Case::Success
                    | Case::Selected
                    | Case::SelectedWrongSolver
                    | Case::SelectedSolverError
                    | Case::SelectedUnsupportedTarget
                    | Case::SelectedChangedExecutable
                    | Case::SelectedChangedWheel
                    | Case::SelectedProjectionMutation { .. }
                    | Case::SelectedAbandonedChecker
                    | Case::Consumption(_)
                    | Case::Lifecycle(_)
            )
        );
        assert_eq!(
            requests.load(Ordering::SeqCst),
            if matches!(case, Case::ChangedBody | Case::Redirect | Case::WrongSize) {
                1
            } else if case == Case::MissingDependency {
                3
            } else {
                5
            }
        );
        if matches!(
            case,
            Case::MissingDependency
                | Case::ChangedExecutableAfterInspection
                | Case::ChangedWheelAfterInspection
                | Case::AbandonInspector
                | Case::WrongDistInfo
        ) {
            assert!(matches!(row.phase, AcquisitionPhase::Using { .. }));
            let cold = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
                root.path(),
            ))));
            assert!(open_catalog_consumer(cold.clone()).await.is_err());
            cold.shutdown().await.unwrap();
            assert!(versions.join(relative).exists());
        }
    }
    assert_eq!(metadata.get_installed_version("v2.14.0", Some(AppId::Torch)).unwrap().is_some(),
        matches!(case, Case::Lifecycle(LifecycleCase::Success | LifecycleCase::AbandonPublication |
            LifecycleCase::Publication(super::offline_selection::local_consumption::runtime::PublicationFault::AfterMetadata))));
    if let Some(complete) = completion.take() {
        complete._grant.validate().unwrap();
        complete._grant.clear_contents().unwrap();
    }
    drop(retained_stage);
    println!(
        "catalog control {case:?}: requests={} complete={} publication_controlled=true cleanup_drained=true",
        requests.load(Ordering::SeqCst),
        matches!(
            case,
            Case::Success
                | Case::Selected
                | Case::SelectedWrongSolver
                | Case::SelectedSolverError
                | Case::SelectedUnsupportedTarget
                | Case::SelectedChangedExecutable
                | Case::SelectedChangedWheel
                | Case::SelectedProjectionMutation { .. }
                | Case::SelectedAbandonedChecker
                | Case::Consumption(_)
                    | Case::Lifecycle(_)
        )
    );
}

#[allow(clippy::too_many_arguments)]
async fn consume_fixture(
    action: ConsumeCase,
    packet: super::offline_selection::SelectedWheelPacket,
    installer: &VersionInstaller,
    consumer: &AcquisitionConsumer,
    runtime: &Path,
    python: &Path,
    granted: &Path,
    stage: &mut Option<Arc<TorchPendingStage>>,
) {
    use super::offline_selection::{QualifiedOfflineSolver, SelectionContext};
    let original_receipt = serde_json::to_vec(&packet.catalog.receipt.payload).unwrap();
    if action != ConsumeCase::Success {
        let helper = runtime.join("consume_selected_wheels.py");
        let mut script = std::fs::read_to_string(&helper).unwrap();
        if let ConsumeCase::Mutation { file, grow } = action {
            let expression = match file {
                "report" => "args.output / 'local-pip-report.json'",
                "manifest" => "args.output / 'installed-files.json'",
                "requirements" => "args.output / 'local-requirements.txt'",
                "packet" => "args.packet",
                "lock" => "args.directory / 'pylock.toml'",
                "projection" => "args.directory / 'projection.json'",
                "roots" => "args.directory / 'roots.in'",
                "constraints" => "args.directory / 'constraints.in'",
                "target" => "args.observation",
                "producer" => "Path(__file__).with_name('selected-target-observation.json')",
                "wheel" => "next(args.wheels.glob('*/dependency-1-*.whl'))",
                "installed" => "args.target / 'torch.py'",
                "record" => "next(args.target.glob('torch-*.dist-info/RECORD'))",
                "executable" => "Path(sys.executable)",
                _ => panic!("Unknown controlled mutation"),
            };
            // Parse the same actual private helper arguments after successful
            // verification. These are fixture-only post-verifier mutations.
            script.push_str(&format!(
                "\nif '--verify-only' in sys.argv:\n import os\n Path(__file__).with_name('consumer-verified').write_text('actual verifier completed')\n from types import SimpleNamespace\n args=SimpleNamespace(**{{k[2:].replace('-','_'):Path(sys.argv[i+1]) for i,k in enumerate(sys.argv) if k.startswith('--') and k!='--verify-only'}})\n p={expression}\n if {grow}:\n  with p.open('ab') as f: f.truncate(32 * 1024 * 1024)\n else:\n  q=p.with_name(p.name+'.replacement'); q.write_bytes(b'X'*p.stat().st_size); os.replace(q,p)\n",
                grow = if grow { "True" } else { "False" },
            ));
        } else {
            script.push_str("\nif '--verify-only' in sys.argv:\n import os,time,subprocess\n child=subprocess.Popen([sys.executable,'-I','-c','import time; time.sleep(120)'])\n Path(__file__).with_name('consumer-child-alive').write_text(str(os.getpid())+' '+str(child.pid)); time.sleep(120)\n");
        }
        std::fs::write(helper, script).unwrap();
    }
    let context = SelectionContext {
        installer,
        consumer,
        runtime,
        python,
        solver: QualifiedOfflineSolver {
            path: PathBuf::from(std::env::var("PUMAS_QUALIFIED_UV").unwrap()),
        },
    };
    let mut flow = Box::pin(packet.consume(context));
    if matches!(action, ConsumeCase::Abandon | ConsumeCase::Cancel) {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                tokio::select! {
                    _ = &mut flow => panic!("Consumer completed before custody control"),
                    _ = tokio::time::sleep(Duration::from_millis(25)) => {
                        if runtime.join("consumer-child-alive").is_file() { break; }
                    }
                }
            }
        })
        .await
        .unwrap();
        assert!(runtime
            .join("selected-consumption/proof/local-pip-report.json")
            .is_file());
        if action == ConsumeCase::Abandon {
            drop(stage.take());
            drop(flow);
            assert!(
                runtime.is_dir() && granted.is_dir(),
                "Unjoined child must retain live packet and stage"
            );
        } else {
            installer.cancel_flag.store(true, Ordering::SeqCst);
            let refusal = flow.await.err().expect("Cancellation must refuse proof");
            assert_eq!(
                serde_json::to_vec(&refusal._packet.catalog.receipt.payload).unwrap(),
                original_receipt
            );
            assert!(runtime.is_dir() && granted.is_dir());
        }
    } else {
        match flow.await {
            Ok(installed) => {
                assert_eq!(action, ConsumeCase::Success);
                assert_eq!(
                    installed.evidence["schema"],
                    "pumas.selected-local-install.v1"
                );
                assert!(installed.evidence["installed_files"].as_u64().unwrap() > 0);
                assert!(installed.packages.join("torch.py").is_file());
                assert!(installed
                    .packages
                    .join("branch-2.dist-info/RECORD")
                    .is_file());
                assert!(!installed.packages.join("branch-1.dist-info").exists());
                assert_eq!(
                    serde_json::to_vec(&installed._packet.catalog.receipt.payload).unwrap(),
                    original_receipt
                );
                if let Ok(destination) = std::env::var("PUMAS_SELECTED_CONSUMER_EVIDENCE") {
                    let destination = PathBuf::from(destination);
                    std::fs::create_dir_all(&destination).unwrap();
                    for (name, source) in [
                        (
                            "installation-report.json",
                            runtime.join("selected-consumption/proof/local-pip-report.json"),
                        ),
                        (
                            "installed-files.json",
                            runtime.join("selected-consumption/proof/installed-files.json"),
                        ),
                        (
                            "local-requirements.txt",
                            runtime.join("selected-consumption/proof/local-requirements.txt"),
                        ),
                        (
                            "packet.json",
                            runtime.join("selected-consumption/packet.json"),
                        ),
                        ("pylock.toml", runtime.join("offline-selection/pylock.toml")),
                        ("catalog.json", runtime.join("catalog-evidence.json")),
                        ("request.json", runtime.join("catalog-request.json")),
                        ("target.json", runtime.join("catalog-approved-target.json")),
                    ] {
                        std::fs::copy(source, destination.join(name)).unwrap();
                    }
                    std::fs::write(
                        destination.join("installation-proof.json"),
                        serde_json::to_vec(&installed.evidence).unwrap(),
                    )
                    .unwrap();
                }
                println!("actual selected installation proof: {}", installed.evidence);
            }
            Err(refusal) => {
                assert!(matches!(action, ConsumeCase::Mutation { .. }));
                assert_eq!(
                    serde_json::to_vec(&refusal._packet.catalog.receipt.payload).unwrap(),
                    original_receipt
                );
                assert!(runtime.is_dir() && granted.is_dir());
                assert!(
                    runtime
                        .join("selected-consumption/proof/local-pip-report.json")
                        .is_file(),
                    "Mutation must follow actual installation"
                );
                assert!(
                    runtime.join("consumer-verified").is_file(),
                    "Mutation must follow actual successful verification"
                );
                if let ConsumeCase::Mutation { file, grow } = action {
                    let path = match file {
                        "report" => {
                            runtime.join("selected-consumption/proof/local-pip-report.json")
                        }
                        "manifest" => {
                            runtime.join("selected-consumption/proof/installed-files.json")
                        }
                        "requirements" => {
                            runtime.join("selected-consumption/proof/local-requirements.txt")
                        }
                        "packet" => runtime.join("selected-consumption/packet.json"),
                        "lock" => runtime.join("offline-selection/pylock.toml"),
                        "projection" => runtime.join("offline-selection/projection.json"),
                        "roots" => runtime.join("offline-selection/roots.in"),
                        "constraints" => runtime.join("offline-selection/constraints.in"),
                        "target" => runtime.join("catalog-approved-target.json"),
                        "producer" => runtime.join("selected-target-observation.json"),
                        "wheel" => walkdir::WalkDir::new(granted)
                            .into_iter()
                            .map(|e| e.unwrap())
                            .find(|e| e.file_name().to_string_lossy().starts_with("dependency-1-"))
                            .unwrap()
                            .path()
                            .to_owned(),
                        "installed" => runtime.join("selected-consumption/packages/torch.py"),
                        "record" => runtime.join(
                            "selected-consumption/packages/torch-2.14.0+cpu.dist-info/RECORD",
                        ),
                        "executable" => python.to_owned(),
                        _ => unreachable!(),
                    };
                    if grow {
                        assert_eq!(std::fs::metadata(path).unwrap().len(), 32 * 1024 * 1024);
                    } else {
                        let mut first = [0_u8; 1];
                        File::open(path).unwrap().read_exact(&mut first).unwrap();
                        assert_eq!(first, [b'X']);
                    }
                }
            }
        }
    }
    assert_eq!(std::fs::read_dir(granted).unwrap().count(), 5);
    println!("selected consumer control {action:?}: original_receipt_unchanged=true installation_executed=true");
}

#[tokio::test]
async fn complete_alternative_catalog_uses_shared_payloads_and_snapshot_receipt() {
    fixture(Case::Success).await;
}
#[tokio::test]
async fn failed_second_repository_never_acquires_or_completes() {
    fixture(Case::FailedPage).await;
}
#[tokio::test]
async fn missing_dependency_coverage_retains_using_and_refuses_cold_replay() {
    fixture(Case::MissingDependency).await;
}
#[tokio::test]
async fn unknown_size_refuses_before_any_payload() {
    fixture(Case::MissingSize).await;
}
#[tokio::test]
async fn changed_original_payload_never_publishes_snapshot() {
    fixture(Case::ChangedBody).await;
}
#[tokio::test]
async fn changed_owned_target_refuses_before_any_payload() {
    fixture(Case::ChangedTarget).await;
}
#[tokio::test]
async fn payload_redirect_cannot_extend_catalog_authority() {
    fixture(Case::Redirect).await;
}

#[tokio::test]
async fn changed_selected_executable_after_inspection_refuses_complete() {
    fixture(Case::ChangedExecutableAfterInspection).await;
}
#[tokio::test]
async fn changed_held_wheel_after_inspection_refuses_complete() {
    fixture(Case::ChangedWheelAfterInspection).await;
}

#[tokio::test]
async fn index_size_conflicting_with_body_refuses_complete() {
    fixture(Case::WrongSize).await;
}

#[tokio::test]
async fn abandoned_inspector_retains_child_stage_and_using_until_drain() {
    fixture(Case::AbandonInspector).await;
}

#[tokio::test]
async fn actual_wrong_dist_info_refuses_snapshot_and_cold_replay() {
    fixture(Case::WrongDistInfo).await;
}

#[tokio::test]
#[ignore = "Requires explicitly provisioned qualified public uv; synthetic wheels only"]
async fn complete_catalog_to_independently_checked_public_offline_selection() {
    fixture(Case::Selected).await;
}

#[tokio::test]
async fn complete_catalog_cannot_use_an_unqualified_solver_executable() {
    fixture(Case::SelectedWrongSolver).await;
}

#[tokio::test]
#[ignore = "Requires explicitly provisioned qualified public uv; copied fixture executable"]
async fn selected_proof_cannot_hide_final_selected_executable_replacement() {
    fixture(Case::SelectedChangedExecutable).await;
}

#[tokio::test]
#[ignore = "Requires explicitly provisioned qualified public uv; managed child abandonment"]
async fn abandoned_selected_checker_retains_catalog_grant_and_runtime_until_drain() {
    fixture(Case::SelectedAbandonedChecker).await;
}

#[tokio::test]
#[ignore = "Requires explicitly provisioned qualified public uv; bounded unsatisfiable catalog"]
async fn complete_unsatisfiable_catalog_solver_error_never_accepts_or_falls_back() {
    fixture(Case::SelectedSolverError).await;
}

#[tokio::test]
#[ignore = "Requires explicitly provisioned qualified public uv; actual native target projection"]
async fn actual_glibc_241_complete_catalog_refuses_without_target_downgrade() {
    fixture(Case::SelectedUnsupportedTarget).await;
}

#[tokio::test]
#[ignore = "Requires explicitly provisioned qualified public uv; final acquired-input fence"]
async fn selected_checker_cannot_hide_unselected_acquired_wheel_replacement() {
    fixture(Case::SelectedChangedWheel).await;
}

#[tokio::test]
#[ignore = "Requires explicitly provisioned qualified public uv; bounded projection growth"]
async fn selected_checker_cannot_hide_retained_projection_growth() {
    for file in ["projection.json", "roots.in", "constraints.in"] {
        fixture(Case::SelectedProjectionMutation { file, grow: true }).await;
    }
}

#[tokio::test]
#[ignore = "Requires explicitly provisioned qualified public uv; exact projection replacement"]
async fn selected_checker_cannot_hide_retained_projection_replacement() {
    for file in ["projection.json", "roots.in", "constraints.in"] {
        fixture(Case::SelectedProjectionMutation { file, grow: false }).await;
    }
}

#[tokio::test]
#[ignore = "Explicit qualified uv, actual public pip, synthetic compatible 2.40 native target"]
async fn complete_selected_packet_installs_exact_local_subset_with_real_report_and_record() {
    fixture(Case::Consumption(ConsumeCase::Success)).await;
}

#[tokio::test]
#[ignore = "Actual local install followed by verifier output/input mutations"]
async fn selected_consumer_final_mutations_refuse_without_receipt_rewrite() {
    for file in [
        "report",
        "manifest",
        "requirements",
        "packet",
        "lock",
        "projection",
        "roots",
        "constraints",
        "target",
        "producer",
        "wheel",
        "installed",
        "record",
        "executable",
    ] {
        fixture(Case::Consumption(ConsumeCase::Mutation {
            file,
            grow: false,
        }))
        .await;
    }
    for file in ["report", "manifest", "requirements"] {
        fixture(Case::Consumption(ConsumeCase::Mutation {
            file,
            grow: true,
        }))
        .await;
    }
}

#[tokio::test]
#[ignore = "Managed actual local verifier and descendant cleanup custody"]
async fn abandoned_selected_consumer_retains_live_packet_until_descendant_drain() {
    fixture(Case::Consumption(ConsumeCase::Abandon)).await;
}

#[tokio::test]
#[ignore = "Managed actual local verifier cancellation and descendant cleanup"]
async fn cancelled_selected_consumer_refuses_proof_and_preserves_catalog_receipt() {
    fixture(Case::Consumption(ConsumeCase::Cancel)).await;
}
#[allow(clippy::too_many_arguments)]
async fn lifecycle_fixture(
    action: LifecycleCase,
    packet: super::offline_selection::SelectedWheelPacket,
    installer: &VersionInstaller,
    consumer: &AcquisitionConsumer,
    runtime: &Path,
    python: &Path,
    granted: &Path,
    stage: &mut Option<Arc<TorchPendingStage>>,
) {
    use super::offline_selection::local_consumption::runtime::{
        ProviderApproval, PublicationFault, PublicationPause, RuntimeFailure,
        RuntimeLifecycleContext,
    };
    use super::offline_selection::{QualifiedOfflineSolver, SelectionContext};
    let original_receipt = serde_json::to_vec(&packet.catalog.receipt).unwrap();
    let context = || SelectionContext {
        installer,
        consumer,
        runtime,
        python,
        solver: QualifiedOfflineSolver {
            path: PathBuf::from(std::env::var("PUMAS_QUALIFIED_UV").unwrap()),
        },
    };
    let installed = packet
        .consume(context())
        .await
        .ok()
        .expect("Actual selected consumer must accept fixture");
    assert_eq!(
        serde_json::to_vec(&installed._packet.catalog.receipt).unwrap(),
        original_receipt
    );
    let provider = installer.versions_dir().join("fixture-provider-python");
    std::fs::copy(python, &provider).unwrap();
    let provider_hash = torch_interpreter_hash(&provider).unwrap();
    let serve = b"from types import SimpleNamespace\nasync def health(): return {'status':'ok','protocol':3}\ndef create_app(): return SimpleNamespace(routes=[SimpleNamespace(path='/health',endpoint=health)])\n".to_vec();
    let mut probe = std::fs::read_to_string(runtime.join("probe_runtime.py")).unwrap();
    // Appended actions run after the genuine probe computes and writes its result.
    probe.push_str("\nPath(__file__).with_name('lifecycle-probe-completed').write_text('actual probe completed')\n");
    if let LifecycleCase::Mutation(file) = action {
        let observation: serde_json::Value = serde_json::from_slice(
            &std::fs::read(runtime.join("catalog-approved-target.json")).unwrap(),
        )
        .unwrap();
        let minor = observation["target"]["markers"]["python_version"]
            .as_str()
            .unwrap();
        let path = match file {
            "member" => torch_site_packages(runtime, minor).join("torch.py"),
            "record" => {
                torch_site_packages(runtime, minor).join("torch-2.14.0+cpu.dist-info/RECORD")
            }
            "provider" => provider.clone(),
            "executable" => python.to_owned(),
            "producer" => runtime.join("selected-target-observation.json"),
            "target" => runtime.join("catalog-approved-target.json"),
            "catalog" => runtime.join("catalog-evidence.json"),
            "lock" => runtime.join("offline-selection/pylock.toml"),
            "projection" => runtime.join("offline-selection/projection.json"),
            "packet" => runtime.join("selected-consumption/packet.json"),
            "manifest" => runtime.join("selected-consumption/proof/installed-files.json"),
            "report" => runtime.join("selected-consumption/proof/local-pip-report.json"),
            "runtime" => runtime.join("runtime.json"),
            "profile" => runtime.join("selected-runtime-profile.json"),
            "probe" => runtime.join("probe-results.json"),
            "source" => runtime.join("serve.py"),
            "added-source" => runtime.join("unexpected.py"),
            _ => panic!("Unknown post-probe mutation"),
        };
        let path = serde_json::to_string(&path).unwrap();
        if file == "added-source" {
            probe.push_str(&format!(
                "\nPath({path}).write_bytes(b'unexpected source')\n"
            ));
        }
        probe.push_str(&format!("\nimport os\np=Path({path}); q=p.with_name(p.name+'.changed'); q.write_bytes(b'X'*p.stat().st_size); os.replace(q,p)\n"));
    }
    if matches!(action, LifecycleCase::Abandon | LifecycleCase::Cancel) {
        probe.push_str("\nimport os,time,subprocess\nchild=subprocess.Popen([sys.executable,'-I','-c','import time; time.sleep(120)'])\nPath(__file__).with_name('lifecycle-child-alive').write_text(f'{os.getpid()} {child.pid}')\ntime.sleep(120)\n");
    }
    let sources = vec![
        ("serve.py".into(), serve),
        ("probe_runtime.py".into(), probe.into_bytes()),
    ];
    for (name, bytes) in &sources {
        std::fs::write(runtime.join(name), bytes).unwrap();
    }
    let release = GitHubRelease {
        tag_name: "v2.14.0".into(),
        name: "Fixture 2.14.0".into(),
        published_at: "2026-10-06T00:00:00Z".into(),
        body: None,
        tarball_url: None,
        zipball_url: None,
        prerelease: false,
        assets: Vec::new(),
        html_url: "https://github.com/pytorch/pytorch/releases/tag/v2.14.0".into(),
        total_size: None,
        archive_size: None,
        dependencies_size: None,
    };
    let pause = (action == LifecycleCase::AbandonPublication)
        .then(|| Arc::new(PublicationPause::default()));
    let context = RuntimeLifecycleContext {
        selection: context(),
        provider: ProviderApproval::Fixture {
            path: provider,
            sha256: provider_hash,
        },
        tag: "v2.14.0",
        release: &release,
        adapter: "none",
        fixture_sources: sources,
        fault: if let LifecycleCase::Publication(fault) = action {
            fault
        } else {
            PublicationFault::None
        },
        publication_pause: pause.clone(),
    };
    let destination = installer.versions_dir().join("v2.14.0");
    let pending = installer
        .versions_dir()
        .join(".torch-pending-publish-v2.14.0");
    let mut flow = Box::pin(installed.publish(context));
    if matches!(
        action,
        LifecycleCase::Abandon | LifecycleCase::Cancel | LifecycleCase::AbandonPublication
    ) {
        tokio::time::timeout(Duration::from_secs(30), async {
            loop {
                tokio::select! {
                    _ = &mut flow => panic!("Lifecycle completed before abandonment boundary"),
                    _ = tokio::time::sleep(Duration::from_millis(25)) => {
                        if pause.as_ref().is_some_and(|p| p.entered.load(Ordering::SeqCst))
                            || runtime.join("lifecycle-child-alive").is_file() { break; }
                    }
                }
            }
        })
        .await
        .unwrap();
        if action == LifecycleCase::AbandonPublication {
            assert!(destination.is_dir() && pending.is_file() && granted.is_dir());
            assert!(installer
                .metadata_manager
                .get_installed_version("v2.14.0", Some(AppId::Torch))
                .unwrap()
                .is_none());
            drop(stage.take());
            drop(flow);
            assert!(
                destination.is_dir() && granted.is_dir(),
                "Registered publisher must retain packet through abandoned caller"
            );
            pause.unwrap().release();
            consumer.shutdown().await.unwrap();
            assert!(destination.is_dir() && !pending.exists());
            assert!(installer
                .metadata_manager
                .get_installed_version("v2.14.0", Some(AppId::Torch))
                .unwrap()
                .is_some());
        } else {
            std::fs::copy(
                runtime.join("lifecycle-child-alive"),
                installer.versions_dir().join("lifecycle-child-pids"),
            )
            .unwrap();
            if action == LifecycleCase::Abandon {
                drop(stage.take());
                drop(flow);
                assert!(runtime.is_dir() && granted.is_dir());
            } else {
                installer.cancel_flag.store(true, Ordering::SeqCst);
                let refusal = flow
                    .await
                    .err()
                    .expect("Cancellation must refuse publication");
                assert_eq!(refusal.kind, RuntimeFailure::Refused);
                assert_eq!(
                    serde_json::to_vec(&refusal._installed._packet.catalog.receipt).unwrap(),
                    original_receipt
                );
            }
            assert!(!destination.exists() && !pending.exists());
        }
    } else {
        match flow.await {
            Ok(published) => {
                assert_eq!(action, LifecycleCase::Success);
                assert_eq!(published.path, destination);
                assert_eq!(
                    published.record["schema"],
                    "pumas.selected-runtime-install.v1"
                );
                assert_eq!(published.record["probe"]["core_status"], "passed");
                assert_eq!(
                    published.record["provider"]["kind"],
                    "existing-local-fixture-provider"
                );
                assert_eq!(
                    published.record["catalog_receipt_sha256"],
                    target_observation_digest(
                        &serde_json::from_slice::<serde_json::Value>(&original_receipt).unwrap()
                    )
                    .unwrap()
                );
                assert!(!runtime.exists() && destination.is_dir() && !pending.exists());
                if let Ok(evidence) = std::env::var("PUMAS_SELECTED_LIFECYCLE_EVIDENCE") {
                    let evidence = PathBuf::from(evidence);
                    std::fs::create_dir_all(&evidence).unwrap();
                    for name in [
                        "selected-runtime-install.json",
                        "selected-runtime-profile.json",
                        "runtime.json",
                        "probe-results.json",
                        "catalog-evidence.json",
                        "catalog-request.json",
                        "catalog-approved-target.json",
                        "selected-target-observation.json",
                        "offline-selection/pylock.toml",
                        "offline-selection/projection.json",
                        "offline-selection/roots.in",
                        "offline-selection/constraints.in",
                        "selected-consumption/packet.json",
                        "selected-consumption/proof/local-pip-report.json",
                        "selected-consumption/proof/installed-files.json",
                        "selected-consumption/proof/local-requirements.txt",
                        "serve.py",
                        "probe_runtime.py",
                    ] {
                        let target = evidence.join(name);
                        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                        std::fs::copy(destination.join(name), target).unwrap();
                    }
                    std::fs::write(evidence.join("catalog-receipt.json"), &original_receipt)
                        .unwrap();
                    let manifest: StagedFilesManifest = serde_json::from_slice(
                        &std::fs::read(
                            destination.join("selected-consumption/proof/installed-files.json"),
                        )
                        .unwrap(),
                    )
                    .unwrap();
                    let profile: serde_json::Value = serde_json::from_slice(
                        &std::fs::read(destination.join("selected-runtime-profile.json")).unwrap(),
                    )
                    .unwrap();
                    let observed: serde_json::Value = serde_json::from_slice(
                        &std::fs::read(destination.join("catalog-approved-target.json")).unwrap(),
                    )
                    .unwrap();
                    let packages = torch_site_packages(
                        &destination,
                        observed["target"]["markers"]["python_version"]
                            .as_str()
                            .unwrap(),
                    );
                    for file in manifest.files {
                        let target = evidence.join("installed").join(&file.path);
                        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                        std::fs::copy(packages.join(file.path), target).unwrap();
                    }
                    for row in profile["artifacts"].as_array().unwrap() {
                        let filename = row["filename"].as_str().unwrap();
                        let source = walkdir::WalkDir::new(granted)
                            .into_iter()
                            .map(|e| e.unwrap())
                            .find(|e| e.file_name().to_string_lossy() == filename)
                            .unwrap()
                            .into_path();
                        std::fs::create_dir_all(evidence.join("wheels")).unwrap();
                        std::fs::copy(source, evidence.join("wheels").join(filename)).unwrap();
                    }
                }
            }
            Err(refusal) => {
                assert_ne!(action, LifecycleCase::Success);
                let expected = match action {
                    LifecycleCase::Publication(PublicationFault::AfterMetadata) => {
                        RuntimeFailure::CommittedWithoutAck
                    }
                    LifecycleCase::Publication(PublicationFault::ForeignDirectory) => {
                        RuntimeFailure::RecoveryRequired
                    }
                    _ => RuntimeFailure::Refused,
                };
                assert_eq!(refusal.kind, expected);
                assert_eq!(
                    serde_json::to_vec(&refusal._installed._packet.catalog.receipt).unwrap(),
                    original_receipt
                );
                if let LifecycleCase::Publication(PublicationFault::ForeignDirectory) = action {
                    assert_eq!(
                        std::fs::read(destination.join("foreign-keeper")).unwrap(),
                        b"foreign directory retained"
                    );
                    assert!(
                        pending.is_file()
                            && installer
                                .versions_dir()
                                .join("fixture-moved-original/selected-runtime-install.json")
                                .is_file()
                    );
                } else if action == LifecycleCase::Publication(PublicationFault::AfterMetadata) {
                    assert!(
                        destination.join("selected-runtime-install.json").is_file()
                            && pending.is_file()
                    );
                } else {
                    assert!(!destination.exists() && !pending.exists());
                }
                if matches!(action, LifecycleCase::Mutation(_)) {
                    assert!(
                        runtime.join("lifecycle-probe-completed").is_file(),
                        "Mutation must follow actual completed probe"
                    );
                }
                if action == LifecycleCase::FailedProbe {
                    let probe: serde_json::Value = serde_json::from_slice(
                        &std::fs::read(runtime.join("probe-results.json")).unwrap(),
                    )
                    .unwrap();
                    assert_eq!(probe["core_status"], "failed");
                    assert_eq!(probe["capabilities"]["cpu_tensor"]["status"], "failed");
                }
            }
        }
    }
    assert_eq!(std::fs::read_dir(granted).unwrap().count(), 5);
    println!("selected lifecycle control {action:?}: actual_probe=true catalog_receipt_unchanged=true publication_custody_checked=true");
}

#[tokio::test]
#[ignore = "Actual local selected installation and genuine probe with synthetic Torch/protocol sources"]
async fn selected_runtime_owned_fixture_publishes_actual_probe_and_bound_record() {
    fixture(Case::Lifecycle(LifecycleCase::Success)).await;
}
#[tokio::test]
#[ignore = "Actual selected local installation followed by CPU probe failure"]
async fn selected_runtime_actual_probe_failure_never_publishes() {
    fixture(Case::Lifecycle(LifecycleCase::FailedProbe)).await;
}
#[tokio::test]
#[ignore = "Actual completed probe followed by original input/output/provider mutations"]
async fn selected_runtime_post_probe_mutations_refuse_publication() {
    for file in [
        "member",
        "record",
        "provider",
        "executable",
        "producer",
        "target",
        "catalog",
        "lock",
        "projection",
        "packet",
        "manifest",
        "report",
        "runtime",
        "profile",
        "probe",
        "source",
        "added-source",
    ] {
        fixture(Case::Lifecycle(LifecycleCase::Mutation(file))).await;
    }
}
#[tokio::test]
#[ignore = "Owned publication failure, moved mutations, foreign directory and uncertain acknowledgment"]
async fn selected_runtime_publication_failure_and_movement_controls() {
    use super::offline_selection::local_consumption::runtime::PublicationFault;
    for fault in [
        PublicationFault::BeforeRename,
        PublicationFault::AfterRename,
        PublicationFault::MovedMember,
        PublicationFault::MovedRecord,
        PublicationFault::ForeignDirectory,
        PublicationFault::AfterMetadata,
    ] {
        fixture(Case::Lifecycle(LifecycleCase::Publication(fault))).await;
    }
}
#[tokio::test]
#[ignore = "Managed actual runtime probe and descendant cleanup custody"]
async fn selected_runtime_cancelled_and_abandoned_probe_keep_custody_until_drain() {
    for action in [LifecycleCase::Cancel, LifecycleCase::Abandon] {
        fixture(Case::Lifecycle(action)).await;
    }
}
#[tokio::test]
#[ignore = "Registered owned publisher completes while caller acknowledgment is abandoned"]
async fn selected_runtime_abandoned_publication_job_retains_custody_through_metadata() {
    fixture(Case::Lifecycle(LifecycleCase::AbandonPublication)).await;
}
