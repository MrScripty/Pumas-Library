//! Real private protocol with a controlled Python backend, never native ASR.
use super::*;
use crate::index::ModelRecord;
use crate::model_library::{DownloadDestinationRoot, DownloadPersistence};
use crate::models::{AssetValidationState, ModelMetadata, StorageKind};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::os::fd::AsRawFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tokio::io::AsyncReadExt;

const MEMBERS: &[&str] = &[
    "config.json",
    "model.safetensors",
    "preprocessor_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
];
const CODE: &[&str] = &[
    "owned_audio.py",
    "installed_cohere_source.py",
    "model_manager.py",
    "device_manager.py",
    "private_owned_channel.py",
    "owned_model_operations.py",
    "speech_binding.py",
    "speech_operations.py",
    "native_speech_result.py",
    "audio_input.py",
    "audio_contract.py",
    "loaders/cohere_asr_loader.py",
    "loaders/owned_cohere_source.py",
    "loaders/__init__.py",
    "tests/owned_channel_fixture.py",
    "owned_worker.py",
    "tests/owned_worker_fixture.py",
    "tests/owned_session_fixture.py",
];

struct Inputs {
    temp: Arc<tempfile::TempDir>,
    library: Arc<ModelLibrary>,
    selected: Arc<PreparedArtifactUse>,
    runtime: Arc<AudioRuntimeOwner>,
    root: DownloadDestinationRoot,
    model_copy: PathBuf,
    code_copy: PathBuf,
    dependencies_copy: PathBuf,
    packages: Arc<tempfile::TempDir>,
}
impl Inputs {
    async fn new() -> Self {
        let standalone = true;
        let existing: Option<Arc<ModelLibrary>> = None;
        let tokens: &[u8] = &[0];
        let temp = tempfile::TempDir::new().unwrap();
        let library = match existing {
            Some(library) => library,
            None => Arc::new(
                ModelLibrary::new(temp.path().join("library"))
                    .await
                    .unwrap(),
            ),
        };
        let model_id = "library/speech";
        let artifact_id = "controlled-selected";
        let package = library.library_root().join(model_id);
        assert!(
            !package.exists(),
            "controlled fixture requires a fresh model selection"
        );
        std::fs::create_dir_all(&package).unwrap();
        for name in MEMBERS {
            let bytes:&[u8]=match *name {
                "config.json"=>br#"{"model_type":"cohere_asr","architectures":["CohereAsrForConditionalGeneration"]}"#,
                "model.safetensors"=>tokens,
                _=>b"{}",
            };
            std::fs::write(package.join(name), bytes).unwrap();
        }
        let metadata = serde_json::to_value(ModelMetadata {
            schema_version: Some(2),
            model_id: Some(model_id.into()),
            selected_artifact_id: Some(artifact_id.into()),
            selected_artifact_files: Some(MEMBERS.iter().map(|name| (*name).into()).collect()),
            storage_kind: Some(StorageKind::LibraryOwned),
            validation_state: Some(AssetValidationState::Valid),
            ..Default::default()
        })
        .unwrap();
        std::fs::write(
            package.join("metadata.json"),
            serde_json::to_vec(&metadata).unwrap(),
        )
        .unwrap();
        library
            .index()
            .upsert(&ModelRecord {
                id: model_id.into(),
                path: package.display().to_string(),
                cleaned_name: "controlled".into(),
                official_name: "Controlled selected byte fixture".into(),
                model_type: "audio".into(),
                tags: vec![],
                hashes: HashMap::new(),
                metadata,
                updated_at: "fixture".into(),
            })
            .unwrap();
        let root = DownloadDestinationRoot::open(library.library_root()).unwrap();
        if standalone {
            library
                .install_mutation_authority(
                    crate::api::RuntimeTasks::new(),
                    root.clone(),
                    Arc::new(DownloadPersistence::new(&temp.path().join("downloads"))),
                )
                .unwrap();
        }
        let prepared = Arc::new(
            library
                .prepare_cohere_artifact_use(model_id, artifact_id)
                .unwrap(),
        );
        let scratch = prepared.read_source_path().to_owned();
        let original_code = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../torch-server");
        let runtime = AudioRuntimeOwner::controlled_fixture_for_selected(
            &original_code,
            CODE,
            MEMBERS,
            &prepared,
        )
        .unwrap();
        let runtime_copy = runtime.read_source_path().to_owned();
        let packages = Arc::new(tempfile::TempDir::new().unwrap());
        let dependencies_copy = packages.path().to_owned();
        let dependency = b"CONTROLLED_DEPENDENCY = True\n";
        std::fs::write(packages.path().join("controlled_dependency.py"), dependency).unwrap();
        let packages_source =
            crate::platform::capability_fs::open_pinned_directory(packages.path()).unwrap();
        let installed_bytes = crate::runtime_read_source::RetainedRuntimeReadSource::capture(vec![
            crate::runtime_read_source::RuntimeReadRoot::new(
                crate::runtime_read_source::RuntimeReadRole::Dependencies,
                packages_source.try_clone().unwrap().into_std_file(),
                vec![crate::runtime_read_source::RuntimeReadFile::new(
                    "controlled_dependency.py".into(),
                    dependency.len() as u64,
                    hex::encode(Sha256::digest(dependency)),
                )
                .unwrap()],
                vec![],
                packages.clone(),
            )
            .unwrap(),
        ])
        .unwrap();
        let runtime =
            AudioRuntimeOwner::with_retained_installed_bytes(runtime, installed_bytes).unwrap();
        runtime.validate_source().unwrap();

        Self {
            temp: Arc::new(temp),
            library,
            selected: prepared,
            runtime,
            root,
            model_copy: scratch,
            code_copy: runtime_copy,
            dependencies_copy,
            packages,
        }
    }
}

struct Started {
    _temp: Arc<tempfile::TempDir>,
    waiter: tokio::task::JoinHandle<Result<InstalledAudioSession>>,
    control: SessionControl,
    phases: tokio::net::UnixStream,
    fault: oneshot::Receiver<Arc<AtomicBool>>,
    selected: std::sync::Weak<PreparedArtifactUse>,
    runtime: std::sync::Weak<AudioRuntimeOwner>,
    root: DownloadDestinationRoot,
    model_copy: PathBuf,
    code_copy: PathBuf,
    dependencies_copy: PathBuf,
}
impl Started {
    async fn new(hold: &'static str) -> Self {
        let inputs = Inputs::new().await;
        let profile = RuntimeProfileId::parse("controlled-session-protocol").unwrap();
        let service = super::super::RuntimeProfileService::with_provider_registry_and_adapters(
            inputs.temp.path(),
            crate::providers::ProviderRegistry::builtin(),
            super::super::RuntimeProviderAdapters::builtin(),
        );
        let guard = Arc::new(service.begin_profile_operation(profile.clone()).unwrap());
        let control = SessionControl::new(profile.clone(), 41);
        let worker_control = control.clone();
        let (phases, phase_child) = UnixStream::pair().unwrap();
        phases.set_nonblocking(true).unwrap();
        let (fault_send, fault) = oneshot::channel();
        let selected = Arc::downgrade(&inputs.selected);
        let runtime = Arc::downgrade(&inputs.runtime);
        let root = inputs.root.clone();
        let temp = inputs.temp.clone();
        let model_copy = inputs.model_copy.clone();
        let code_copy = inputs.code_copy.clone();
        let dependencies_copy = inputs.dependencies_copy.clone();
        let waiter = tokio::spawn(async move {
            let endpoints = AudioEndpoints::default();
            let worker_runtime = inputs.runtime.clone();
            let worker_selected = inputs.selected.clone();
            let worker_library = inputs.library.clone();
            InstalledAudioSession::launch_inner(
                inputs.library,
                &endpoints,
                inputs.runtime,
                inputs.selected,
                profile,
                41,
                worker_control,
                move |supervisor, send| {
                    let registry = supervisor.registry.clone();
                    let launch_runtime = worker_runtime.clone();
                    run_owned_worker(
                        supervisor,
                        Arc::new((worker_library, guard, inputs.temp)),
                        send,
                        move |custody| {
                            let code = launch_runtime
                                .clone_read_source_directory()?
                                .into_std_file();
                            let model = worker_selected.clone_read_source_directory()?;
                            let packages = crate::platform::capability_fs::open_pinned_directory(
                                inputs.packages.path(),
                            )?
                            .into_std_file();
                            let mut command = Command::new(
                                std::env::var_os("PUMAS_AUDIO_SESSION_PYTHON")
                                    .unwrap_or_else(|| "python3".into()),
                            );
                            command
                                .args(["-I", "-S", "-B"])
                                .arg(format!(
                                    "/proc/self/fd/{}/tests/owned_session_fixture.py",
                                    code.as_raw_fd()
                                ))
                                .arg("--code-root-fd")
                                .arg(code.as_raw_fd().to_string())
                                .arg("--model-root-fd")
                                .arg(model.as_raw_fd().to_string())
                                .arg("--packages-root-fd")
                                .arg(packages.as_raw_fd().to_string())
                                .arg("--phase-fd")
                                .arg(phase_child.as_raw_fd().to_string())
                                .arg("--hold")
                                .arg(hold)
                                .stdin(Stdio::piped())
                                .stdout(Stdio::piped())
                                .stderr(Stdio::piped());
                            let descriptors = [
                                code.as_raw_fd(),
                                model.as_raw_fd(),
                                packages.as_raw_fd(),
                                phase_child.as_raw_fd(),
                            ];
                            // SAFETY: test-only inherited held descriptors remain alive
                            // through spawn; fcntl is child-side async-signal-safe.
                            #[allow(unsafe_code)]
                            unsafe {
                                command.pre_exec(move || {
                                    for fd in descriptors {
                                        if libc::fcntl(fd, libc::F_SETFD, 0) < 0 {
                                            return Err(io::Error::last_os_error());
                                        }
                                    }
                                    Ok(())
                                });
                            }
                            let mut child = ManagedChild::spawn(&mut command, custody)?;
                            child.attach_cleanup_lease(worker_selected);
                            let fault = child.test_observation_failure_control();
                            let _ = fault_send.send(fault);
                            let diagnostics = child.take_private_stderr()?;
                            Ok((child, diagnostics))
                        },
                        move |child| {
                            registry
                                .retain_runtime_and_attach(worker_runtime, child)
                                .map_err(|_| {
                                    io::Error::other("controlled registry attachment refused")
                                })
                        },
                    );
                },
            )
            .await
        });
        Self {
            _temp: temp,
            waiter,
            control,
            phases: tokio::net::UnixStream::from_std(phases).unwrap(),
            fault,
            selected,
            runtime,
            root,
            model_copy,
            code_copy,
            dependencies_copy,
        }
    }
    async fn phase(&mut self, wanted: u8) {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let mut byte = [0];
                self.phases.read_exact(&mut byte).await.unwrap();
                if byte[0] == wanted {
                    break;
                }
            }
        })
        .await
        .unwrap();
    }
    async fn drained(&self) {
        tokio::time::timeout(Duration::from_secs(10), self.control.join())
            .await
            .unwrap()
            .unwrap();
        assert!(!self.control.0.custody.is_active());
        assert!(!self.control.0.custody.has_parked_child());
        assert!(self.selected.upgrade().is_none());
        assert!(self.runtime.upgrade().is_none());
        assert!(!self.model_copy.exists());
        assert!(!self.code_copy.exists());
        assert!(!self.dependencies_copy.exists());
        drop(self.root.try_acquire_execution_grant().unwrap());
    }
}

#[tokio::test]
async fn actual_private_hello_cancellation_drains_original_session() {
    let mut fixture = Started::new("hello").await;
    fixture.phase(b'H').await;
    fixture.waiter.abort();
    assert!(matches!((&mut fixture.waiter).await, Err(error) if error.is_cancelled()));
    fixture.drained().await;
}

#[tokio::test]
async fn actual_private_load_cancellation_retains_sources_until_retry_drains() {
    let mut fixture = Started::new("load").await;
    fixture.phase(b'L').await;
    let fault = (&mut fixture.fault).await.unwrap();
    let uncertain = FaultGuard(fault.clone());
    fault.store(true, Ordering::Release);
    fixture.waiter.abort();
    assert!(matches!((&mut fixture.waiter).await, Err(error) if error.is_cancelled()));
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(!fixture.control.0.done.load(Ordering::Acquire));
    assert!(fixture.selected.upgrade().is_some());
    assert!(fixture.runtime.upgrade().is_some());
    assert!(fixture.model_copy.exists());
    assert!(fixture.code_copy.exists());
    assert!(fixture.dependencies_copy.exists());
    assert!(fixture.root.try_acquire_execution_grant().is_err());
    drop(uncertain);
    fixture.drained().await;
}

#[tokio::test]
async fn completed_session_drop_closes_cloned_endpoint_before_uncertain_drain() {
    let mut fixture = Started::new("none").await;
    let session = tokio::time::timeout(Duration::from_secs(15), &mut fixture.waiter)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    let endpoint = session.endpoint();
    assert!(endpoint.available());
    let fault = (&mut fixture.fault).await.unwrap();
    let uncertain = FaultGuard(fault.clone());
    fault.store(true, Ordering::Release);
    drop(session);
    assert!(!endpoint.available());
    let admission = Arc::new(AtomicBool::new(false));
    assert!(endpoint
        .execute(
            serde_json::json!({"model":"library/speech","profile":"controlled-session-protocol"}),
            admission.clone()
        )
        .await
        .is_err());
    assert!(!admission.load(Ordering::Acquire));
    tokio::time::sleep(Duration::from_millis(150)).await;
    assert!(!fixture.control.0.done.load(Ordering::Acquire));
    assert!(fixture.selected.upgrade().is_some());
    assert!(fixture.runtime.upgrade().is_some());
    drop(uncertain);
    drop(endpoint);
    fixture.drained().await;
}

impl Drop for Started {
    fn drop(&mut self) {
        self.control.stop();
        self.waiter.abort();
    }
}
struct FaultGuard(Arc<AtomicBool>);
impl Drop for FaultGuard {
    fn drop(&mut self) {
        self.0.store(false, Ordering::Release);
    }
}
