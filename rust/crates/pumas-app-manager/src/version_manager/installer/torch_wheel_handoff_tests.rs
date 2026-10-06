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
    TargetSuccess,
    TargetMismatch,
    TargetMissingContext,
    TargetUnsupportedContext,
    TargetChangedApproval,
    QualifiedTargetSuccess,
    QualifiedChangedProducerBeforeAcquisition,
    QualifiedConsumerChangedBeforeAcquisition,
    QualifiedChangedProducerBeforeConsumption,
    QualifiedChangedProducerAfterProbe,
    QualifiedConsumerChangedAfterProbe,
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
    let log = root.path().join("installer.log");
    let (progress_tx, _progress_rx) = mpsc::channel(32);
    let qualified_target = matches!(
        case,
        Case::QualifiedTargetSuccess
            | Case::QualifiedChangedProducerBeforeAcquisition
            | Case::QualifiedConsumerChangedBeforeAcquisition
            | Case::QualifiedChangedProducerBeforeConsumption
            | Case::QualifiedChangedProducerAfterProbe
            | Case::QualifiedConsumerChangedAfterProbe
    );
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
    let mut qualified_packet = if qualified_target {
        write_embedded_torch_runtime(&runtime).unwrap();
        let mut create = Command::new("python3");
        create
            .args(["-I", "-m", "venv", "--copies"])
            .arg(runtime.join("venv"));
        installer
            .run_runtime_command(
                create,
                &log,
                "Fixture selected venv",
                &progress_tx,
                Some(stage.clone()),
            )
            .await
            .unwrap();
        let python = pumas_library::platform::paths::venv_python(&runtime);
        let produced = installer
            .observe_qualified_torch_target(&runtime, &python, &stage, &log, &progress_tx)
            .await
            .unwrap();
        for artifact in &mut artifacts {
            artifact.url = format!(
                "https://files.pythonhosted.org/packages/{}",
                torch_wheel_filename(&artifact.url).unwrap()
            );
        }
        let lock = artifacts
            .iter()
            .map(|a| {
                format!(
                    "{}=={} \\\n    --hash=sha256:{}\n",
                    a.name, a.version, a.sha256
                )
            })
            .collect::<String>();
        std::fs::write(runtime.join("requirements.txt"), &lock).unwrap();
        let preview =
            serde_json::json!({"requirementsLock":lock, "directArtifacts":[]}).to_string();
        std::fs::write(runtime.join("qualified-preview.json"), &preview).unwrap();
        std::fs::write(
            runtime.join("fixture-rows.json"),
            serde_json::to_vec(&artifacts).unwrap(),
        )
        .unwrap();
        // Actual catalog CLI, public catalog API with controlled metadata fetch;
        // the production fetcher, endpoints and CLI receive no fixture bypass.
        let code = r#"import importlib.util,json,pathlib,sys
p=pathlib.Path(sys.argv[1])
s=importlib.util.spec_from_file_location('catalog',p/'qualified_wheel_catalog.py')
c=importlib.util.module_from_spec(s); s.loader.exec_module(c)
rows=json.loads((p/'fixture-rows.json').read_text())
called=[]
def fetch(url):
    assert url.startswith(c.INDEXES) and not url.endswith('.whl')
    called.append(url)
    return [{'url':a['url'],'hashes':{'sha256':a['sha256']}} for a in rows]
original=c.catalog
c.catalog=lambda lock,roots,*,target_observation: original(lock,roots,fetch,target_observation=target_observation)
sys.argv=['catalog','--lock',str(p/'requirements.txt'),'--preview',str(p/'qualified-preview.json'),'--output',str(p/'resolution.json'),'--target-observation',str(p/'selected-target-observation.json')]
c.main()
assert len(called)==len(rows)*2
"#;
        let mut catalog = Command::new(&python);
        catalog.args(["-I", "-c", code]).arg(&runtime);
        installer
            .run_runtime_command(
                catalog,
                &log,
                "Fixture finite catalog CLI",
                &progress_tx,
                Some(stage.clone()),
            )
            .await
            .unwrap();
        let resolution_json = std::fs::read_to_string(runtime.join("resolution.json")).unwrap();
        let document: serde_json::Value = serde_json::from_str(&resolution_json).unwrap();
        let mut resolution: DirectTorchResolution = serde_json::from_str(&resolution_json).unwrap();
        assert_eq!(
            serde_json::to_value(&resolution.artifacts).unwrap(),
            serde_json::to_value(&artifacts).unwrap()
        );
        validate_qualified_artifacts(&lock, &[], &resolution.artifacts).unwrap();
        resolution.accepted_target =
            Some(accepted_qualified_torch_target(&runtime, &document, produced, &python).unwrap());
        Some(PreparedTorchWheelInstall {
            runtime: runtime.clone(),
            resolution,
            resolution_json,
            report: preview,
            requirements: lock,
            interpreter_hash: "b".repeat(64),
            provider_label: None,
            qualified_recipe: true,
        })
    } else {
        None
    };
    if matches!(
        case,
        Case::QualifiedChangedProducerBeforeAcquisition
            | Case::QualifiedConsumerChangedBeforeAcquisition
    ) {
        if case == Case::QualifiedConsumerChangedBeforeAcquisition {
            let provider = native_observation_fixture();
            let provider_path = Path::new(provider["interpreter"].as_str().unwrap());
            let provider_hash = torch_interpreter_hash(provider_path).unwrap();
            std::fs::write(
                pumas_library::platform::paths::venv_python(&runtime),
                b"changed selected consumer bytes",
            )
            .unwrap();
            assert_eq!(
                torch_interpreter_hash(provider_path).unwrap(),
                provider_hash
            );
        } else {
            std::fs::write(
                runtime.join("selected-target-observation.json"),
                b"changed after catalog",
            )
            .unwrap();
        }
        assert!(revalidate_prepared_torch_target(qualified_packet.as_ref().unwrap()).is_err());
        assert!(AcquisitionStore::new(root.path())
            .acquisitions()
            .unwrap()
            .is_empty());
        assert!(!runtime.join("approved-target-observation.json").exists());
        installer.shutdown_torch_cleanup().await.unwrap();
        return;
    }
    if let Some(packet) = &qualified_packet {
        revalidate_prepared_torch_target(packet).unwrap();
        assert_eq!(packet.interpreter_hash, "b".repeat(64));
        assert_ne!(
            packet.interpreter_hash,
            packet
                .resolution
                .accepted_target
                .as_ref()
                .unwrap()
                .interpreter_sha256
        );
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let mut bodies = std::collections::BTreeMap::new();
    let mut acquisition_sources = Vec::new();
    for artifact in &mut artifacts {
        let filename = torch_wheel_filename(&artifact.url).unwrap();
        let bytes = std::fs::read(source.join(&filename)).unwrap();
        let fixture_url = format!("{endpoint}/{filename}");
        acquisition_sources.push(AcquisitionHttpSource {
            url: fixture_url.clone(),
            authorization: None,
        });
        if !qualified_target {
            artifact.url = fixture_url;
        }
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
        sources: acquisition_sources,
        retry: AcquisitionRetryPolicy {
            attempts: Some(1),
            elapsed: Duration::from_secs(5),
            backoff: RetryConfig::new(),
        },
    };
    let bound_target = matches!(
        case,
        Case::TargetSuccess
            | Case::TargetMismatch
            | Case::TargetMissingContext
            | Case::TargetUnsupportedContext
            | Case::TargetChangedApproval
    );
    let mut resolution = qualified_packet
        .as_ref()
        .map(|packet| serde_json::from_str(&packet.resolution_json).unwrap())
        .unwrap_or_else(|| serde_json::json!({"artifacts": artifacts}));
    let approved_target = if let Some(packet) = &mut qualified_packet {
        let accepted = packet.resolution.accepted_target.take().unwrap();
        std::fs::write(
            runtime.join("approved-target-observation.json"),
            &accepted.observation,
        )
        .unwrap();
        Some(accepted)
    } else if bound_target {
        let observation = native_observation_fixture();
        resolution["wheel_target"] = observation["target"].clone();
        resolution["wheel_target_observation_sha256"] =
            serde_json::json!(target_observation_digest(&observation).unwrap());
        resolution["interpreter"] = observation["interpreter"].clone();
        resolution["python"] = observation["target"]["markers"]["python_version"].clone();
        resolution["implementation"] = serde_json::json!("cpython");
        resolution["machine"] = observation["target"]["markers"]["platform_machine"].clone();
        let raw = observation.to_string();
        let accepted = accepted_torch_target(
            &resolution,
            &observation["target"]["markers"],
            Some(&raw),
            Path::new(observation["interpreter"].as_str().unwrap()),
            observation["interpreter_sha256"].as_str(),
        )
        .unwrap()
        .unwrap();
        std::fs::write(
            runtime.join("approved-target-observation.json"),
            &accepted.observation,
        )
        .unwrap();
        Some(accepted)
    } else {
        None
    };
    let resolution_provenance = serde_json::to_vec(&resolution).unwrap();
    if case == Case::TargetMismatch {
        resolution["wheel_target"]["markers"]["platform_release"] =
            serde_json::json!("different-approved-target");
    }
    if case == Case::TargetUnsupportedContext {
        let mut changed: serde_json::Value =
            serde_json::from_str(&approved_target.as_ref().unwrap().observation).unwrap();
        changed["schema"] = serde_json::json!("future");
        std::fs::write(
            runtime.join("approved-target-observation.json"),
            changed.to_string(),
        )
        .unwrap();
    }
    std::fs::write(
        runtime.join("resolution.json"),
        serde_json::to_vec(&resolution).unwrap(),
    )
    .unwrap();
    let destination = versions.join("v2.9.1");
    let pending = versions.join(".torch-pending-publish-v2.9.1");
    let prepared = Arc::new(AtomicBool::new(false));
    let published = Arc::new(AtomicBool::new(false));
    let observed_held = Arc::new(AtomicBool::new(false));
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
                &approved_target,
                &resolution_provenance,
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
                    approved_target,
                    resolution_provenance,
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
                if let Some(target) = approved_target {
                    if case == Case::QualifiedChangedProducerBeforeConsumption {
                        std::fs::write(
                            runtime.join("selected-target-observation.json"),
                            b"changed after acquisition",
                        )
                        .unwrap();
                    }
                    validate_torch_target_evidence(
                        runtime,
                        target,
                        Path::new(resolution["interpreter"].as_str().unwrap()),
                    )?;
                }
                let mut command = if qualified_target {
                    Command::new(resolution["interpreter"].as_str().unwrap())
                } else {
                    Command::new("python3")
                };
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
                    if qualified_target {
                        command
                            .arg("--recipe-lock")
                            .arg(runtime.join("requirements.txt"))
                            .arg("--preview")
                            .arg(runtime.join("qualified-preview.json"));
                    }
                    if (bound_target || qualified_target) && case != Case::TargetMissingContext {
                        command
                            .arg("--target-observation")
                            .arg(runtime.join("approved-target-observation.json"));
                    }
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
                let mut provenance = vec![(
                    runtime.join("resolution.json"),
                    resolution_provenance.clone(),
                )];
                if let Some(target) = &approved_target {
                    provenance.push((
                        runtime.join("approved-target-observation.json"),
                        target.observation.as_bytes().to_vec(),
                    ));
                    if let Some(path) = &target.producer_path {
                        provenance.push((path.clone(), target.observation.as_bytes().to_vec()));
                        let packet = qualified_packet.as_ref().unwrap();
                        provenance.push((
                            runtime.join("requirements.txt"),
                            packet.requirements.as_bytes().to_vec(),
                        ));
                        provenance.push((
                            runtime.join("qualified-preview.json"),
                            packet.report.as_bytes().to_vec(),
                        ));
                    }
                }
                move_verified_packages(&packages, runtime, "3.fixture")?;
                let final_packages = torch_site_packages(runtime, "3.fixture");
                if matches!(
                    case,
                    Case::ChangedInstalledMember
                        | Case::ChangedProof
                        | Case::ChangedProvenance
                        | Case::TargetChangedApproval
                        | Case::QualifiedChangedProducerAfterProbe
                        | Case::QualifiedConsumerChangedAfterProbe
                ) {
                    let changed = match case {
                        Case::QualifiedConsumerChangedAfterProbe => {
                            pumas_library::platform::paths::venv_python(runtime)
                        }
                        Case::ChangedInstalledMember => final_packages.join("root_wheel.py"),
                        Case::ChangedProof => proof_file.clone(),
                        Case::QualifiedChangedProducerAfterProbe => {
                            runtime.join("selected-target-observation.json")
                        }
                        Case::TargetChangedApproval => {
                            runtime.join("approved-target-observation.json")
                        }
                        _ => runtime.join("resolution.json"),
                    };
                    let mut probe = if case == Case::QualifiedConsumerChangedAfterProbe {
                        Command::new(resolution["interpreter"].as_str().unwrap())
                    } else {
                        Command::new("python3")
                    };
                    let code = if case == Case::QualifiedConsumerChangedAfterProbe {
                        // Replace the running stage-owned copy atomically; never
                        // write the executing inode or the managed provider.
                        "import os,pathlib,sys; p=pathlib.Path(sys.argv[1]); q=p.with_name('probe-replacement'); q.write_bytes(b'changed selected consumer executable'); os.replace(q,p)"
                    } else {
                        "import pathlib,sys; pathlib.Path(sys.argv[1]).write_bytes(b'probe mutation')"
                    };
                    probe.args(["-I", "-B", "-c", code]).arg(changed);
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
                if case == Case::QualifiedConsumerChangedAfterProbe {
                    let target = approved_target.as_ref().unwrap();
                    assert_ne!(
                        torch_interpreter_hash(Path::new(
                            resolution["interpreter"].as_str().unwrap()
                        ))
                        .unwrap(),
                        target.interpreter_sha256
                    );
                    let provider = native_observation_fixture();
                    assert_eq!(provider["interpreter_sha256"], target.interpreter_sha256);
                    validate_torch_provenance(runtime, &provenance)?;
                    assert_eq!(
                        std::fs::read(&proof_file).map_err(PumasError::from)?,
                        validated_manifest
                    );
                }
                validate_torch_final_proof(
                    runtime,
                    &final_packages,
                    &proof_file,
                    &validated_manifest,
                    &provenance,
                    approved_target.as_ref().map(|target| {
                        (
                            target,
                            Path::new(resolution["interpreter"].as_str().unwrap()),
                        )
                    }),
                )?;
                let mut proof = serde_json::json!({"format":"fixture-wheel-publication-1", "installed_sha256":hash_regular_file(&proof_file)?});
                if let Some(target) = &approved_target {
                    proof["wheel_target_observation_sha256"] = serde_json::json!(target.sha256);
                }
                Ok((runtime.clone(), proof))
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
    if matches!(
        case,
        Case::Success | Case::TargetSuccess | Case::QualifiedTargetSuccess
    ) {
        result.unwrap();
        assert!(matches!(row.phase, AcquisitionPhase::Adopted { .. }));
        assert!(matches!(drop_phase, Some(AcquisitionPhase::Adopted { .. })));
        assert!(receipt.is_some() && installed.is_some() && output_present && !publication_marker);
        if matches!(case, Case::TargetSuccess | Case::QualifiedTargetSuccess) {
            let saved: serde_json::Value = serde_json::from_slice(
                &std::fs::read(destination.join("acquisition-wheel-receipt.json")).unwrap(),
            )
            .unwrap();
            assert_eq!(
                saved["payload"]["wheel_target_observation_sha256"],
                approved_target.as_ref().unwrap().sha256
            );
            assert_eq!(
                std::fs::read(destination.join("approved-target-observation.json")).unwrap(),
                approved_target.as_ref().unwrap().observation.as_bytes()
            );
            if qualified_target {
                assert_eq!(
                    std::fs::read(destination.join("selected-target-observation.json")).unwrap(),
                    approved_target.as_ref().unwrap().observation.as_bytes()
                );
            }
        }
    } else {
        if case == Case::QualifiedConsumerChangedAfterProbe {
            eprintln!("Executable-only probe control: result_success={} adopted={} receipt={} installed={} output={} cleanup_drained={drained}", result.is_ok(), matches!(row.phase, AcquisitionPhase::Adopted { .. }), receipt.is_some(), installed.is_some(), output_present);
        }
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
        if matches!(
            case,
            Case::TargetMismatch
                | Case::TargetMissingContext
                | Case::TargetUnsupportedContext
                | Case::QualifiedChangedProducerBeforeConsumption
        ) {
            assert!(!runtime.join("packages").exists());
            assert!(!runtime.join("local-proof").exists());
        }
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
async fn qualified_owned_producer_finite_catalog_shared_consumer_receipt() {
    fixture(Case::QualifiedTargetSuccess).await;
}
#[tokio::test]
async fn qualified_changed_producer_refuses_before_acquisition() {
    fixture(Case::QualifiedChangedProducerBeforeAcquisition).await;
}
#[tokio::test]
async fn qualified_changed_selected_consumer_refuses_without_changing_provider() {
    fixture(Case::QualifiedConsumerChangedBeforeAcquisition).await;
}
#[tokio::test]
async fn qualified_changed_producer_refuses_before_consumption() {
    fixture(Case::QualifiedChangedProducerBeforeConsumption).await;
}
#[tokio::test]
async fn qualified_changed_selected_consumer_probe_refuses_publication_and_receipt() {
    fixture(Case::QualifiedConsumerChangedAfterProbe).await;
}
#[tokio::test]
async fn qualified_changed_producer_probe_refuses_publication_and_receipt() {
    fixture(Case::QualifiedChangedProducerAfterProbe).await;
}

#[tokio::test]
async fn verified_wheels_install_then_publish_and_settle_exact_receipt() {
    fixture(Case::Success).await;
}

#[tokio::test]
async fn verified_target_wheels_valid_handoff_binds_receipt_and_settles() {
    fixture(Case::TargetSuccess).await;
}
#[tokio::test]
async fn verified_target_wheels_mismatch_refuses_before_local_stage() {
    fixture(Case::TargetMismatch).await;
}
#[tokio::test]
async fn verified_target_wheels_missing_context_refuses_before_local_stage() {
    fixture(Case::TargetMissingContext).await;
}
#[tokio::test]
async fn verified_target_wheels_unsupported_context_refuses_before_local_stage() {
    fixture(Case::TargetUnsupportedContext).await;
}
#[tokio::test]
async fn verified_target_wheels_changed_approval_retains_custody_without_receipt() {
    fixture(Case::TargetChangedApproval).await;
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

fn accepted_packet_fixture() -> PreparedTorchWheelInstall {
    let artifacts: Vec<_> = [
        "torch",
        "fastapi",
        "uvicorn",
        "psutil",
        "pillow",
        "safetensors",
    ]
    .into_iter()
    .map(|name| crate::version_manager::TorchArtifact {
        name: name.into(),
        version: if name == "torch" && !cfg!(target_os = "macos") {
            "2.14.0+cpu"
        } else if name == "torch" {
            "2.14.0"
        } else {
            "1.0"
        }
        .into(),
        url: if name == "torch" {
            "https://download.pytorch.org/whl/cpu/torch/torch-fixture.whl".into()
        } else {
            format!("https://files.pythonhosted.org/packages/{name}-fixture.whl")
        },
        sha256: "a".repeat(64),
    })
    .collect();
    let (platform, machine) = match std::env::consts::OS {
        "windows" => ("Windows-fixture", "AMD64"),
        "macos" => ("macOS-fixture", "arm64"),
        _ => ("Linux-fixture", "x86_64"),
    };
    let resolution_json = serde_json::json!({
        "release":"2.14.0", "torch":artifacts[0].version, "build":"cpu", "python":"3.12",
        "interpreter":"/owned/stage/venv/python", "implementation":"cpython",
        "platform":platform, "machine":machine, "adapter":"none", "artifacts":artifacts,
    })
    .to_string();
    let report =
        serde_json::json!({ "version":"1", "install": artifacts.iter().map(|a| serde_json::json!({
        "metadata":{"name":a.name,"version":a.version},
        "download_info":{"url":a.url,"archive_info":{"hashes":{"sha256":a.sha256}}}
    })).collect::<Vec<_>>() })
        .to_string();
    let requirements = artifacts
        .iter()
        .map(|a| format!("{} @ {} --hash=sha256:{}\n", a.name, a.url, a.sha256))
        .collect::<String>();
    let resolution = accepted_torch_resolution(
        &resolution_json,
        &report,
        &requirements,
        &DirectTorchSelection {
            version: "2.14.0",
            build: "cpu",
            minor: "3.12",
            adapter: "none",
            python: Path::new("/owned/stage/venv/python"),
            target_interpreter_hash: None,
            target_observation: None,
        },
    )
    .unwrap();
    PreparedTorchWheelInstall {
        runtime: PathBuf::from("/owned/stage"),
        resolution,
        resolution_json,
        report,
        requirements,
        interpreter_hash: "b".repeat(64),
        provider_label: Some("Python 3.12.14".into()),
        qualified_recipe: false,
    }
}

fn native_observation_fixture() -> serde_json::Value {
    let result = std::process::Command::new("python3")
        .arg("-I")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../../torch-server/wheel_target.py"))
        .arg("--observe")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    serde_json::from_slice(&result.stdout).unwrap()
}

#[test]
fn accepted_target_packet_refuses_mismatch_missing_context_and_late_downgrade() {
    let mut packet = accepted_packet_fixture();
    let observation = native_observation_fixture();
    let approved = observation.to_string();
    let markers = &observation["target"]["markers"];
    let mut resolution: serde_json::Value = serde_json::from_str(&packet.resolution_json).unwrap();
    resolution["wheel_target"] = observation["target"].clone();
    resolution["wheel_target_observation_sha256"] =
        serde_json::json!(target_observation_digest(&observation).unwrap());
    resolution["python"] = markers["python_version"].clone();
    resolution["interpreter"] = observation["interpreter"].clone();
    resolution["machine"] = markers["platform_machine"].clone();
    let mut report: serde_json::Value = serde_json::from_str(&packet.report).unwrap();
    report["environment"] = markers.clone();
    let python = Path::new(observation["interpreter"].as_str().unwrap());
    let selection = DirectTorchSelection {
        version: "2.14.0",
        build: "cpu",
        minor: markers["python_version"].as_str().unwrap(),
        adapter: "none",
        python,
        target_interpreter_hash: observation["interpreter_sha256"].as_str(),
        target_observation: Some(&approved),
    };
    let check = |resolution: &serde_json::Value,
                 report: &serde_json::Value,
                 selection: &DirectTorchSelection<'_>| {
        accepted_torch_resolution(
            &resolution.to_string(),
            &report.to_string(),
            &packet.requirements,
            selection,
        )
    };
    let accepted = check(&resolution, &report, &selection).unwrap();
    assert!(accepted.accepted_target.is_some());
    for key in [
        "wheel_target",
        "wheel_target_observation_sha256",
        "interpreter",
        "python",
        "machine",
    ] {
        let mut changed = resolution.clone();
        changed[key] = serde_json::Value::Null;
        assert!(check(&changed, &report, &selection).is_err(), "{key}");
    }
    let mut changed = resolution.clone();
    changed["wheel_target"]["markers"]["platform_release"] = serde_json::json!("changed-target");
    assert!(check(&changed, &report, &selection).is_err());
    let mut changed_report = report.clone();
    changed_report["environment"]["platform_release"] = serde_json::json!("changed-report-target");
    assert!(check(&resolution, &changed_report, &selection).is_err());
    for raw in [
        serde_json::json!({}),
        serde_json::json!({"schema":"future"}),
        {
            let mut missing = observation.clone();
            missing["target"]["markers"]
                .as_object_mut()
                .unwrap()
                .remove("platform_release");
            missing
        },
    ] {
        let raw = raw.to_string();
        let unsupported = DirectTorchSelection {
            target_observation: Some(&raw),
            ..selection
        };
        assert!(check(&resolution, &report, &unsupported).is_err());
    }
    let unapproved = DirectTorchSelection {
        target_observation: None,
        ..selection
    };
    assert!(check(&resolution, &report, &unapproved).is_err());
    let missing_binary_context = DirectTorchSelection {
        target_interpreter_hash: None,
        ..selection
    };
    assert!(check(&resolution, &report, &missing_binary_context).is_err());
    let replaced_binary = DirectTorchSelection {
        target_interpreter_hash: Some(&"0".repeat(64)),
        ..selection
    };
    assert!(check(&resolution, &report, &replaced_binary).is_err());
    packet.resolution = accepted;
    packet.resolution_json = resolution.to_string();
    packet.report = report.to_string();
    // Managed-provider identity stays independent of selected venv/redirector bytes.
    assert_eq!(packet.interpreter_hash, "b".repeat(64));
    assert_ne!(
        packet.interpreter_hash,
        observation["interpreter_sha256"].as_str().unwrap()
    );
    revalidate_prepared_torch_target(&packet).unwrap();
    resolution.as_object_mut().unwrap().remove("wheel_target");
    resolution
        .as_object_mut()
        .unwrap()
        .remove("wheel_target_observation_sha256");
    packet.resolution_json = resolution.to_string();
    assert!(
        revalidate_prepared_torch_target(&packet).is_err(),
        "accepted context cannot be downgraded to legacy"
    );
}

#[test]
fn target_observation_projection_matches_python_unicode_fixture() {
    let value = serde_json::json!({"z":"é", "a":{"b":true,"a":null}});
    assert_eq!(
        target_observation_digest(&value).unwrap(),
        "d10edb991ee049b4c7b97e508696c2d04ca2166fae721b870e3dd70457ff9841"
    );
}

#[test]
fn accepted_packet_preserves_venv_identity_and_refuses_changed_evidence() {
    let packet = accepted_packet_fixture();
    let selection = DirectTorchSelection {
        version: "2.14.0",
        build: "cpu",
        minor: "3.12",
        adapter: "none",
        python: Path::new("/owned/stage/venv/python"),
        target_interpreter_hash: None,
        target_observation: None,
    };
    assert_eq!(packet.resolution.interpreter, "/owned/stage/venv/python");
    assert_eq!(packet.interpreter_hash, "b".repeat(64));
    assert_eq!(packet.provider_label.as_deref(), Some("Python 3.12.14"));
    for version in [
        serde_json::Value::Null,
        serde_json::json!("2"),
        serde_json::json!(1),
    ] {
        let mut report: serde_json::Value = serde_json::from_str(&packet.report).unwrap();
        report["version"] = version;
        assert!(accepted_torch_resolution(
            &packet.resolution_json,
            &report.to_string(),
            &packet.requirements,
            &selection
        )
        .is_err());
    }
    let mut resolution: serde_json::Value = serde_json::from_str(&packet.resolution_json).unwrap();
    resolution["interpreter"] = serde_json::json!("/managed/provider/python");
    assert!(accepted_torch_resolution(
        &resolution.to_string(),
        &packet.report,
        &packet.requirements,
        &selection
    )
    .is_err());
    let mut report: serde_json::Value = serde_json::from_str(&packet.report).unwrap();
    report["install"][0]["download_info"]["archive_info"]["hashes"]["sha256"] =
        serde_json::json!("c".repeat(64));
    assert!(accepted_torch_resolution(
        &packet.resolution_json,
        &report.to_string(),
        &packet.requirements,
        &selection
    )
    .is_err());
    assert!(accepted_torch_resolution(
        &packet.resolution_json,
        &packet.report,
        &packet.requirements.replace("sha256:a", "sha256:c"),
        &selection
    )
    .is_err());
}

#[tokio::test]
async fn automatic_candidate_loop_stops_at_first_accepted_packet() {
    let calls = Arc::new(StdMutex::new(Vec::new()));
    let capture = calls.clone();
    let prepared = resolve_torch_candidates(
        &["cpu".into(), "cu130".into()],
        &["3.14".into(), "3.12".into()],
        move |build, minor| {
            capture.lock().unwrap().push((build, minor.clone()));
            async move {
                if minor == "3.14" {
                    Ok(DirectTorchAttempt::Retry)
                } else {
                    Ok(DirectTorchAttempt::Resolved(Box::new(
                        accepted_packet_fixture(),
                    )))
                }
            }
        },
    )
    .await
    .unwrap();
    assert_eq!(
        *calls.lock().unwrap(),
        vec![("cpu".into(), "3.14".into()), ("cpu".into(), "3.12".into())]
    );
    assert_eq!(prepared.resolution.python, "3.12");
    assert_eq!(prepared.resolution.interpreter, "/owned/stage/venv/python");
}

#[tokio::test]
async fn automatic_candidate_loop_propagates_inconclusive_or_validation_errors() {
    let calls = Arc::new(StdMutex::new(0));
    let capture = calls.clone();
    let result = resolve_torch_candidates(
        &["cu130".into(), "cpu".into()],
        &["3.14".into(), "3.12".into()],
        move |_, _| {
            *capture.lock().unwrap() += 1;
            async { Err(failed("inconclusive resolution or refused evidence")) }
        },
    )
    .await;
    assert!(result.is_err());
    assert_eq!(*calls.lock().unwrap(), 1);
}

#[test]
fn qualified_recipe_catalog_preserves_complete_pins_and_selected_direct_identity() {
    let root = crate::version_manager::TorchArtifact {
        name: "torch".into(), version: "2.9.1+cu130".into(),
        url: "https://download-r2.pytorch.org/whl/cu130/torch-2.9.1%2Bcu130-cp312-cp312-manylinux_2_28_x86_64.whl".into(),
        sha256: "a".repeat(64),
    };
    let dependency = crate::version_manager::TorchArtifact {
        name: "dependency".into(),
        version: "1.0".into(),
        url: "https://files.pythonhosted.org/packages/dependency-1.0-py3-none-any.whl".into(),
        sha256: "b".repeat(64),
    };
    let lock = format!(
        "torch @ {} \\\n    --hash=sha256:{}\ndependency==1.0 \\\n    --hash=sha256:{}\n",
        root.url, root.sha256, dependency.sha256
    );
    let roots = vec![root.clone()];
    let artifacts = vec![root, dependency];
    validate_qualified_artifacts(&lock, &roots, &artifacts).unwrap();
    assert!(validate_qualified_artifacts(&lock, &roots, &artifacts[..1]).is_err());
    let mut changed = artifacts.clone();
    changed[1].sha256 = "c".repeat(64);
    assert!(validate_qualified_artifacts(&lock, &roots, &changed).is_err());
    changed = artifacts.clone();
    changed[0].url = changed[0]
        .url
        .replace("download-r2.pytorch.org", "download.pytorch.org");
    assert!(validate_qualified_artifacts(&lock, &roots, &changed).is_err());
    changed = artifacts;
    changed[1].url = changed[1]
        .url
        .replace("files.pythonhosted.org", "fixture.invalid");
    assert!(validate_qualified_artifacts(&lock, &roots, &changed).is_err());
}

#[test]
fn embedded_qualified_runtime_contains_public_tooling_and_single_record_owner() {
    let root = tempfile::tempdir().unwrap();
    write_embedded_torch_runtime(root.path()).unwrap();
    assert_eq!(
        std::fs::read(root.path().join("packaging-tooling.zip")).unwrap(),
        include_bytes!("../../../../../../torch-server/tooling/packaging.zip")
    );
    for file in [
        "qualified_wheel_catalog.py",
        "install_verified_wheels.py",
        "wheel_records.py",
    ] {
        let source = std::fs::read_to_string(root.path().join(file)).unwrap();
        assert!(!source.contains("pip._internal"));
        assert!(!source.contains("pip._vendor"));
    }
    assert_eq!(
        std::fs::read_to_string(root.path().join("requirements.txt")).unwrap(),
        include_str!("../../../../../../torch-server/runtime/requirements.lock")
    );
}
