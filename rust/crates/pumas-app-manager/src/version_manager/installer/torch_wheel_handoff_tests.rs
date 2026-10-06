//! Real shared source -> local pip -> existing Torch publication controls.
//! Tiny synthetic distributions qualify handoff, not Torch inference/providers.
use super::*;
use pumas_library::acquisition::{AcquisitionPhase, AcquisitionStore};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Copy, PartialEq)]
enum Case {
    Success,
    ChangedWheel,
    MissingDependency,
    FailedPublication,
    PublishedWithoutAck,
    CancelChild,
    AbandonedChild,
    ChangedInstalledMember,
    ChangedProof,
    ChangedProvenance,
    ChangedPackageAncestor,
    ChangedProofAncestor,
}

struct InputDropProbe {
    _stage: Arc<TorchPendingStage>,
    store: Arc<AcquisitionStore>,
    phase: Arc<StdMutex<Option<AcquisitionPhase>>>,
    child_marker: PathBuf,
    running_at_drop: Arc<StdMutex<Option<bool>>>,
}
impl Drop for InputDropProbe {
    fn drop(&mut self) {
        if let Ok(pid) = std::fs::read_to_string(&self.child_marker) {
            *self.running_at_drop.lock().unwrap() = Some(descendant_running(&pid));
        }
        *self.phase.lock().unwrap() = self
            .store
            .acquisitions()
            .ok()
            .and_then(|rows| rows.into_values().next().map(|r| r.phase));
    }
}

fn descendant_running(pid: &str) -> bool {
    // Exited unreaped zombies cannot keep reading the held wheel descriptor.
    std::fs::read_to_string(format!("/proc/{pid}/stat"))
        .ok()
        .as_ref()
        .is_some_and(|state| {
            state
                .rsplit_once(") ")
                .is_none_or(|(_, tail)| !tail.starts_with("Z "))
        })
}

async fn fixture(case: Case) {
    let timeout = Duration::from_secs(30);
    let mut retained_root = Some(tempfile::tempdir().unwrap());
    let root = retained_root.as_ref().unwrap();
    let root_path = root.path().to_owned();
    let metadata = Arc::new(MetadataManager::new(root.path()));
    metadata.ensure_directories().unwrap();
    let progress = Arc::new(RwLock::new(InstallationProgressTracker::new(
        root.path().join("cache"),
    )));
    let cancel = Arc::new(AtomicBool::new(false));
    let installer = VersionInstaller::new(
        root.path().to_owned(),
        AppId::Torch,
        metadata.clone(),
        progress.clone(),
        cancel.clone(),
    );
    assert!(installer.torch_control.start());
    let versions = installer.versions_dir();
    std::fs::create_dir_all(&versions).unwrap();
    let stage = Arc::new(
        TorchPendingStage::new(
            &versions,
            "v2.9.1",
            TorchVersionsLock::try_acquire(&versions).unwrap(),
        )
        .unwrap(),
    );
    let stage_path = stage.path().to_owned();
    let runtime = stage_path.join("runtime");
    std::fs::create_dir(&runtime).unwrap();
    let source = root.path().join("fixture-source");
    std::fs::create_dir(&source).unwrap();
    let python_fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../torch-server/tests/test_install_verified_wheels.py");
    let code = "import importlib.util,json,pathlib,sys; s=importlib.util.spec_from_file_location('fixture',sys.argv[1]); m=importlib.util.module_from_spec(s); s.loader.exec_module(m); p=pathlib.Path(sys.argv[2]); a=[m.make_wheel(p,'root-wheel',requires=['dependency-wheel==1.0'])]; a += [] if sys.argv[3]=='missing' else [m.make_wheel(p,'dependency-wheel')]; print(json.dumps(a))";
    let generated = std::process::Command::new("python3")
        .args(["-I", "-c", code])
        .arg(&python_fixture)
        .arg(&source)
        .arg(if case == Case::MissingDependency {
            "missing"
        } else {
            "complete"
        })
        .output()
        .unwrap();
    assert!(
        generated.status.success(),
        "{}",
        String::from_utf8_lossy(&generated.stderr)
    );
    let mut artifacts: Vec<crate::version_manager::TorchArtifact> =
        serde_json::from_slice(&generated.stdout).unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let mut bodies = std::collections::BTreeMap::new();
    for artifact in &mut artifacts {
        let filename = torch_wheel_filename(&artifact.url).unwrap();
        let bytes = std::fs::read(source.join(&filename)).unwrap();
        artifact.url = format!("{endpoint}/{filename}");
        bodies.insert(format!("/{filename}"), bytes);
    }
    if case == Case::ChangedWheel {
        let key = format!("/{}", torch_wheel_filename(&artifacts[1].url).unwrap());
        bodies.insert(key, b"changed same-name/version wheel bytes".to_vec());
    }
    let request_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let requests = request_count.clone();
    let (stop_tx, mut stop_rx) = tokio::sync::oneshot::channel::<()>();
    let mut server = tokio::spawn(async move {
        loop {
            let accepted = tokio::select! { biased; _ = &mut stop_rx => break, result = listener.accept() => result };
            let (mut stream, _) = accepted.unwrap();
            let mut header = Vec::new();
            loop {
                let mut byte = [0];
                let count = tokio::select! { biased; _ = &mut stop_rx => return Ok(()), result = stream.read(&mut byte) => result.map_err(|_| "fixture read failed")? };
                if count == 0 {
                    break;
                }
                header.push(byte[0]);
                if header.ends_with(b"\r\n\r\n") {
                    break;
                }
                if header.len() > 8192 {
                    return Err("oversized fixture request");
                }
            }
            let header = String::from_utf8(header).unwrap();
            let path = header.split_whitespace().nth(1).unwrap();
            let body = bodies.get(path).ok_or("unselected fixture request")?;
            requests.fetch_add(1, Ordering::SeqCst);
            let response = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            stream.write_all(response.as_bytes()).await.unwrap();
            stream.write_all(body).await.unwrap();
            stream.shutdown().await.unwrap();
        }
        Ok(())
    });
    let store = Arc::new(AcquisitionStore::new(root.path()));
    let service = Arc::new(pumas_library::acquisition::AcquisitionService::new(
        store.clone(),
    ));
    let consumer = Arc::new(service.open_consumer("runtime.torch.wheels").unwrap());
    let wheels = versions.join(".torch-wheel-input-fixture");
    std::fs::create_dir(&wheels).unwrap();
    let phase_at_drop = Arc::new(StdMutex::new(None));
    let running_at_drop = Arc::new(StdMutex::new(None));
    let child_marker = stage_path.join("child-reading-wheel");
    let grant = ReservedDirectory::capture(
        &versions,
        Path::new(".torch-wheel-input-fixture"),
        Arc::new(InputDropProbe {
            _stage: stage.clone(),
            store: store.clone(),
            phase: phase_at_drop.clone(),
            child_marker: child_marker.clone(),
            running_at_drop: running_at_drop.clone(),
        }),
        || Ok(()),
    )
    .unwrap();
    let workspace = grant.acquisition_workspace().unwrap();
    drop(grant);
    let manifest = torch_wheel_manifest(&artifacts).unwrap();
    let request = AcquisitionHttpRequest {
        demand: AcquisitionDemand {
            consumer: consumer.owner().into(),
            operation: "fixture-wheel-install".into(),
        },
        manifest: manifest.clone(),
        workspace,
        sources: artifacts
            .iter()
            .map(|artifact| AcquisitionHttpSource {
                url: artifact.url.clone(),
                authorization: None,
            })
            .collect(),
        retry: AcquisitionRetryPolicy {
            attempts: Some(1),
            elapsed: Duration::from_secs(5),
            backoff: RetryConfig::new(),
        },
    };
    let resolution = serde_json::json!({"artifacts": artifacts});
    std::fs::write(
        runtime.join("resolution.json"),
        serde_json::to_vec(&resolution).unwrap(),
    )
    .unwrap();
    let log = root.path().join("installer.log");
    let destination = versions.join("v2.9.1");
    let pending = versions.join(".torch-pending-publish-v2.9.1");
    let prepared = Arc::new(AtomicBool::new(false));
    let published = Arc::new(AtomicBool::new(false));
    let observed_held = Arc::new(AtomicBool::new(false));
    let (progress_tx, _progress_rx) = mpsc::channel(32);
    let flow = with_verified_torch_wheels(
        &consumer,
        request,
        reqwest::Client::builder().no_proxy().build().unwrap(),
        Box::new(TorchWheelHost {
            cancel: cancel.clone(),
            shutdown: Arc::new(AtomicBool::new(false)),
            progress,
        }),
        |inputs| {
            let context = (
                &prepared,
                &manifest,
                &source,
                &runtime,
                &wheels,
                &child_marker,
                &installer,
                &log,
                &progress_tx,
                &stage,
            );
            async move {
                let (
                    prepared,
                    manifest,
                    source,
                    runtime,
                    wheels,
                    child_marker,
                    installer,
                    log,
                    progress_tx,
                    stage,
                ) = context;
                prepared.store(true, Ordering::SeqCst);
                if !matches!(inputs.record().phase, AcquisitionPhase::Using { .. })
                    || &inputs.record().manifest != manifest
                {
                    return Err(failed("Fixture input identity disagrees"));
                }
                for (index, file) in manifest.files().iter().enumerate() {
                    let verified = inputs.open_file(index).await?;
                    if verified.metadata().map_err(PumasError::from)?.len()
                        != std::fs::metadata(source.join(file.logical_path()))
                            .map_err(PumasError::from)?
                            .len()
                    {
                        return Err(failed("Fixture input size disagrees"));
                    }
                }
                let packages = runtime.join("packages");
                let output = runtime.join("local-proof");
                let mut command = Command::new("python3");
                if matches!(case, Case::CancelChild | Case::AbandonedChild) {
                    let child = "import pathlib,sys,time; f=open(sys.argv[1],'rb'); pathlib.Path(sys.argv[2]).write_text(str(__import__('os').getpid())); time.sleep(120)";
                    let parent = format!("import subprocess,sys,time; subprocess.Popen([sys.executable,'-I','-c',{},sys.argv[1],sys.argv[2]]); time.sleep(120)", serde_json::to_string(child).unwrap());
                    command
                        .args(["-I", "-c", &parent])
                        .arg(wheels.join(manifest.files()[0].logical_path()))
                        .arg(child_marker);
                } else {
                    command
                        .arg("-I")
                        .arg(
                            Path::new(env!("CARGO_MANIFEST_DIR"))
                                .join("../../../torch-server/install_verified_wheels.py"),
                        )
                        .arg("--resolution")
                        .arg(runtime.join("resolution.json"))
                        .arg("--wheels")
                        .arg(wheels)
                        .arg("--target")
                        .arg(&packages)
                        .arg("--output")
                        .arg(&output);
                }
                let status = installer
                    .run_runtime_command_status_with_custody(
                        command,
                        log,
                        "Fixture verified local wheel consumption",
                        progress_tx,
                        Some(TorchChildLease {
                            stage: stage.clone(),
                            _inputs: Some(inputs.clone()),
                        }),
                        None,
                    )
                    .await?;
                if !status.success() {
                    return Err(failed("Fixture local wheel consumption refused"));
                }
                let proof_file = output.join("installed-files.json");
                let installed: StagedFilesManifest =
                    serde_json::from_slice(&std::fs::read(&proof_file).map_err(PumasError::from)?)
                        .map_err(|_| failed("Invalid fixture proof"))?;
                validate_staged_files(&packages, &installed)?;
                let validated_manifest = std::fs::read(&proof_file).map_err(PumasError::from)?;
                let provenance = vec![(
                    runtime.join("resolution.json"),
                    std::fs::read(runtime.join("resolution.json")).map_err(PumasError::from)?,
                )];
                move_verified_packages(&packages, runtime, "3.fixture")?;
                let final_packages = torch_site_packages(runtime, "3.fixture");
                if matches!(
                    case,
                    Case::ChangedInstalledMember | Case::ChangedProof | Case::ChangedProvenance
                ) {
                    let changed = match case {
                        Case::ChangedInstalledMember => final_packages.join("root_wheel.py"),
                        Case::ChangedProof => proof_file.clone(),
                        _ => runtime.join("resolution.json"),
                    };
                    let mut probe = Command::new("python3");
                    probe.args(["-I", "-B", "-c", "import pathlib,sys; pathlib.Path(sys.argv[1]).write_bytes(b'probe mutation')"]).arg(changed);
                    let status = installer
                        .run_runtime_command_status_with_custody(
                            probe,
                            log,
                            "Fixture probe mutation",
                            progress_tx,
                            Some(TorchChildLease {
                                stage: stage.clone(),
                                _inputs: Some(inputs.clone()),
                            }),
                            None,
                        )
                        .await?;
                    if !status.success() {
                        return Err(failed("Fixture probe failed"));
                    }
                }
                if matches!(
                    case,
                    Case::ChangedPackageAncestor | Case::ChangedProofAncestor
                ) {
                    let relocated = if case == Case::ChangedPackageAncestor {
                        runtime.join("venv/lib")
                    } else {
                        output.clone()
                    };
                    let outside = runtime.parent().unwrap().join("relocated-proof-input");
                    let mut probe = Command::new("python3");
                    probe.args(["-I", "-B", "-c", "import pathlib,sys; p=pathlib.Path(sys.argv[1]); q=pathlib.Path(sys.argv[2]); p.rename(q); p.symlink_to(q,target_is_directory=True)"]).arg(relocated).arg(outside);
                    let status = installer
                        .run_runtime_command_status_with_custody(
                            probe,
                            log,
                            "Fixture proof ancestry escape",
                            progress_tx,
                            Some(TorchChildLease {
                                stage: stage.clone(),
                                _inputs: Some(inputs.clone()),
                            }),
                            None,
                        )
                        .await?;
                    if !status.success() {
                        return Err(failed("Fixture ancestry probe failed"));
                    }
                }
                validate_torch_final_proof(
                    runtime,
                    &final_packages,
                    &proof_file,
                    &validated_manifest,
                    &provenance,
                )?;
                Ok((
                    runtime.clone(),
                    serde_json::json!({"format":"fixture-wheel-publication-1", "installed_sha256":hash_regular_file(&proof_file)?}),
                ))
            }
        },
        |runtime, receipt| {
            let context = (
                &published,
                &store,
                &phase_at_drop,
                &manifest,
                &pending,
                &destination,
                &installer,
                &versions,
                &progress_tx,
                &stage,
            );
            async move {
                let (
                    published,
                    store,
                    phase_at_drop,
                    manifest,
                    pending,
                    destination,
                    installer,
                    versions,
                    progress_tx,
                    stage,
                ) = context;
                published.store(true, Ordering::SeqCst);
                let rows = store.acquisitions()?;
                let row = rows
                    .values()
                    .next()
                    .ok_or_else(|| failed("Fixture row absent"))?;
                if !matches!(row.phase, AcquisitionPhase::Using { .. })
                    || phase_at_drop.lock().unwrap().is_some()
                    || &receipt.manifest != manifest
                    || receipt.acquisition_id != row.id.to_string()
                {
                    return Err(failed("Fixture publication identity disagrees"));
                }
                std::fs::write(
                    runtime.join("acquisition-wheel-receipt.json"),
                    serde_json::to_vec(&receipt).map_err(|_| failed("Fixture receipt invalid"))?,
                )
                .map_err(PumasError::from)?;
                let release: GitHubRelease = serde_json::from_value(serde_json::json!({"tag_name":"v2.9.1", "name":"synthetic wheel consumer", "published_at":"2025-11-12T00:00:00Z", "html_url":"https://fixture.invalid/release"})).unwrap();
                if case == Case::FailedPublication {
                    std::fs::write(runtime.join(".pumas-publishing"), TORCH_PUBLISHING_MARKER)
                        .map_err(PumasError::from)?;
                    write_pending_publish_marker(pending, &runtime)?;
                    pumas_library::platform::filesystem::rename_directory_noreplace(
                        &runtime,
                        destination,
                    )
                    .map_err(PumasError::from)?;
                    return Err(failed("Fixture failed before metadata publication"));
                }
                installer
                    .publish_staged_torch_runtime(
                        runtime,
                        "v2.9.1",
                        &release,
                        destination,
                        versions,
                        progress_tx,
                        stage,
                        None,
                    )
                    .await?;
                if case == Case::PublishedWithoutAck {
                    return Err(failed("Fixture lost final publication acknowledgment"));
                }
                Ok(())
            }
        },
    );
    let (abandon_tx, mut abandon_rx) = tokio::sync::oneshot::channel();
    let controller = async {
        if matches!(case, Case::CancelChild | Case::AbandonedChild) {
            let observed = tokio::time::timeout(timeout, async {
                while !child_marker.exists() {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            })
            .await;
            observed_held.store(
                observed.is_ok()
                    && stage_path.exists()
                    && wheels.exists()
                    && phase_at_drop.lock().unwrap().is_none(),
                Ordering::SeqCst,
            );
            if case == Case::AbandonedChild {
                let _ = abandon_tx.send(());
            } else {
                cancel.store(true, Ordering::SeqCst);
            }
        }
    };
    let controlled_flow = async {
        tokio::select! {
            result = tokio::time::timeout(timeout, flow) => result,
            _ = &mut abandon_rx, if case == Case::AbandonedChild => Ok(Err(failed("Fixture consuming waiter abandoned"))),
        }
    };
    let (outcome, ()) = tokio::join!(controlled_flow, controller);
    // Always drain effects/children/source before evaluating fixture outcomes.
    cancel.store(true, Ordering::SeqCst);
    // On abandonment drain acquisition first: the detached shared worker cannot
    // remain the last input owner while the parked child is still being drained.
    let (cleanup, consumer_drain) = if case == Case::AbandonedChild {
        let acquired = tokio::time::timeout(timeout, consumer.shutdown()).await;
        let children = tokio::time::timeout(timeout, installer.shutdown_torch_cleanup()).await;
        (children, acquired)
    } else {
        let children = tokio::time::timeout(timeout, installer.shutdown_torch_cleanup()).await;
        let acquired = tokio::time::timeout(timeout, consumer.shutdown()).await;
        (children, acquired)
    };
    let service_drain = tokio::time::timeout(timeout, service.shutdown()).await;
    let _ = stop_tx.send(());
    let server_drain = tokio::time::timeout(timeout, &mut server).await;
    let source_joined = if server_drain.is_err() {
        server.abort();
        tokio::time::timeout(timeout, &mut server).await.is_ok()
    } else {
        true
    };
    let drained = cleanup.as_ref().is_ok_and(|r| r.is_ok())
        && consumer_drain.as_ref().is_ok_and(|r| r.is_ok())
        && service_drain.as_ref().is_ok_and(|r| r.is_ok())
        && source_joined
        && server_drain.as_ref().is_ok_and(|r| matches!(r, Ok(Ok(()))));
    if !drained {
        let _ = retained_root.take().unwrap().keep();
    }
    assert!(drained, "cleanup={cleanup:?}, consumer={consumer_drain:?}, service={service_drain:?}, server={server_drain:?}");
    let rows = store.acquisitions().unwrap();
    let row = rows.values().next().unwrap().clone();
    let receipt = service.consumer_receipt(row.id).unwrap();
    let source_requests = request_count.load(Ordering::SeqCst);
    let publication_marker = pending.exists();
    let output_present = destination.exists();
    let installed = metadata
        .get_installed_version("v2.9.1", Some(AppId::Torch))
        .unwrap();
    let drop_phase = phase_at_drop.lock().unwrap().clone();
    let result = outcome.expect("fixture caller timeout");
    assert_eq!(
        source_requests,
        if case == Case::MissingDependency {
            1
        } else {
            2
        }
    );
    if case == Case::Success {
        result.unwrap();
        assert!(matches!(row.phase, AcquisitionPhase::Adopted { .. }));
        assert!(matches!(drop_phase, Some(AcquisitionPhase::Adopted { .. })));
        assert!(receipt.is_some() && installed.is_some() && output_present && !publication_marker);
    } else {
        assert!(result.is_err());
        assert!(wheels.exists(), "retained inputs disappeared");
        if case == Case::ChangedWheel {
            assert!(!prepared.load(Ordering::SeqCst));
            assert!(matches!(row.phase, AcquisitionPhase::Transferring));
        } else {
            assert!(matches!(row.phase, AcquisitionPhase::Using { .. }));
        }
        assert_eq!(
            receipt.is_some(),
            matches!(case, Case::FailedPublication | Case::PublishedWithoutAck)
        );
        assert_eq!(
            published.load(Ordering::SeqCst),
            matches!(case, Case::FailedPublication | Case::PublishedWithoutAck)
        );
        assert_eq!(installed.is_some(), case == Case::PublishedWithoutAck);
        if matches!(case, Case::CancelChild | Case::AbandonedChild) {
            assert!(observed_held.load(Ordering::SeqCst));
            assert!(matches!(drop_phase, Some(AcquisitionPhase::Using { .. })));
            let pid = std::fs::read_to_string(&child_marker).unwrap();
            assert!(
                !descendant_running(&pid),
                "descendant remains running after successful cleanup"
            );
            assert_eq!(
                *running_at_drop.lock().unwrap(),
                Some(false),
                "last input released while descendant could still read"
            );
        }
    }
    drop(consumer);
    drop(service);
    drop(stage);
    drop(installer);
    if matches!(case, Case::FailedPublication | Case::PublishedWithoutAck) {
        let before = std::fs::read(destination.join("acquisition-wheel-receipt.json")).unwrap();
        let pending_before = std::fs::read(&pending).ok();
        let cold = Arc::new(pumas_library::acquisition::AcquisitionService::new(
            Arc::new(AcquisitionStore::new(&root_path)),
        ));
        let reopen = tokio::time::timeout(
            timeout,
            crate::version_manager::VersionManager::new_with_acquisition(
                &root_path,
                AppId::Torch,
                cold.clone(),
            ),
        )
        .await;
        let cold_drain = tokio::time::timeout(timeout, cold.shutdown()).await;
        let cold_drained = cold_drain.as_ref().is_ok_and(|r| r.is_ok());
        if !cold_drained || reopen.is_err() {
            let _ = retained_root.take().unwrap().keep();
        }
        assert!(cold_drained, "cold cleanup={cold_drain:?}");
        assert!(
            matches!(reopen, Ok(Err(_))),
            "uncertain use must refuse before ordinary startup cleanup"
        );
        assert_eq!(
            std::fs::read(destination.join("acquisition-wheel-receipt.json")).unwrap(),
            before
        );
        assert_eq!(std::fs::read(&pending).ok(), pending_before);
        assert!(wheels.exists());
    }
}

#[tokio::test]
async fn verified_wheels_install_then_publish_and_settle_exact_receipt() {
    fixture(Case::Success).await;
}
#[tokio::test]
async fn verified_wheels_changed_bytes_never_start_local_installation() {
    fixture(Case::ChangedWheel).await;
}
#[tokio::test]
async fn verified_wheels_missing_dependency_refuses_publication() {
    fixture(Case::MissingDependency).await;
}
#[tokio::test]
async fn verified_wheels_failed_publication_retains_inputs_and_cold_output() {
    fixture(Case::FailedPublication).await;
}
#[tokio::test]
async fn verified_wheels_published_without_ack_refuses_cold_replay() {
    fixture(Case::PublishedWithoutAck).await;
}
#[tokio::test]
async fn verified_wheels_cancel_child_retains_composite_custody_until_cleanup() {
    fixture(Case::CancelChild).await;
}

#[test]
fn verified_wheels_preserve_existing_qualified_nunchaku_source_and_digest() {
    let line = include_str!("../../../../../../torch-server/runtime/requirements.lock")
        .lines()
        .find_map(|line| line.strip_prefix("nunchaku @ "))
        .unwrap();
    let (url, hash) = line
        .split_whitespace()
        .next()
        .unwrap()
        .split_once("#sha256=")
        .unwrap();
    let mut artifact = crate::version_manager::TorchArtifact {
        name: "nunchaku".into(),
        version: "fixture".into(),
        url: url.into(),
        sha256: hash.into(),
    };
    assert!(qualified_nunchaku_wheel(&artifact));
    artifact.sha256 = "0".repeat(64);
    assert!(!qualified_nunchaku_wheel(&artifact));
    artifact.sha256 = hash.into();
    artifact.url.push_str(".changed");
    assert!(!qualified_nunchaku_wheel(&artifact));
}

#[tokio::test]
async fn verified_wheels_probe_member_mutation_refuses_publication() {
    fixture(Case::ChangedInstalledMember).await;
}
#[tokio::test]
async fn verified_wheels_probe_proof_mutation_refuses_publication() {
    fixture(Case::ChangedProof).await;
}
#[tokio::test]
async fn verified_wheels_probe_provenance_mutation_refuses_publication() {
    fixture(Case::ChangedProvenance).await;
}

#[tokio::test]
async fn verified_wheels_probe_package_ancestor_escape_refuses_publication() {
    fixture(Case::ChangedPackageAncestor).await;
}
#[tokio::test]
async fn verified_wheels_probe_proof_ancestor_escape_refuses_publication() {
    fixture(Case::ChangedProofAncestor).await;
}

#[tokio::test]
async fn verified_wheels_abandoned_waiter_holds_inputs_through_child_cleanup() {
    fixture(Case::AbandonedChild).await;
}
