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
p=pathlib.Path(sys.argv[2]); rows=[m.make_wheel(p,'torch',version='2.14.0+cpu',requires=(['branch>=1','dependency<1'] if sys.argv[3]=='solver-error' else ['branch>=1'])),m.make_wheel(p,'branch',version='1',requires=['dependency<2']),m.make_wheel(p,'branch',version='2',requires=['dependency>=2']),m.make_wheel(p,'dependency',version='1'),m.make_wheel(p,'dependency',version='2')]
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
                    assert_eq!(case, Case::Selected);
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
                    packet.catalog._grant.validate().unwrap();
                    packet.catalog._grant.clear_contents().unwrap();
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
    installer.shutdown_torch_cleanup().await.unwrap();
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
    assert!(metadata
        .get_installed_version("v2.14.0", Some(AppId::Torch))
        .unwrap()
        .is_none());
    if let Some(complete) = completion.take() {
        complete._grant.validate().unwrap();
        complete._grant.clear_contents().unwrap();
    }
    drop(retained_stage);
    println!(
        "catalog control {case:?}: requests={} complete={} installed=false cleanup_drained=true",
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
        )
    );
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
