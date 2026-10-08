//! Controlled process qualification only: no installed Torch/model inference.
use super::super::audio_runtime::AudioRuntimeOwner;
use super::*;
use crate::index::ModelRecord;
use crate::model_library::{DownloadDestinationRoot, DownloadPersistence, ModelLibrary};
use crate::models::{AssetValidationState, ModelMetadata, StorageKind};
use crate::platform::managed_child::{ManagedChild, ManagedChildCustodySlot};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::future::Future;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::{AsyncBufReadExt, BufReader};

const MEMBERS: &[&str] = &[
    "config.json",
    "model.safetensors",
    "preprocessor_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
];
const CODE: &[&str] = &[
    "owned_audio.py",
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
    "loaders/__init__.py",
    "tests/owned_channel_fixture.py",
];

struct Supervisor {
    child: Mutex<Option<ManagedChild>>,
    custody: Arc<ManagedChildCustodySlot>,
    registry: Arc<AudioCustodyRegistry>,
    stopped: AtomicBool,
    changed: Notify,
}
impl Supervisor {
    fn stop(self: &Arc<Self>) {
        self.registry.close_admission();
        let supervisor = self.clone();
        tokio::task::spawn_blocking(move || {
            if let Some(mut child) = supervisor.child.lock().unwrap().take() {
                // Test cleanup budget, never a transport/RPC/drain policy.
                child.terminate_and_drain(Duration::from_secs(5)).unwrap();
                drop(child);
                supervisor.stopped.store(true, Ordering::Release);
                supervisor.changed.notify_waiters();
            }
        });
    }
    async fn wait(&self) {
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if self.stopped.load(Ordering::Acquire) {
                return;
            }
            changed.await;
        }
    }
}

struct Fixture {
    client: Arc<OwnedAudioClient>,
    prepared: Option<Arc<PreparedArtifactUse>>,
    root: DownloadDestinationRoot,
    scratch: PathBuf,
    runtime_copy: PathBuf,
    digest: String,
    stderr: BufReader<tokio::net::UnixStream>,
    supervisor: Arc<Supervisor>,
    _temp: tempfile::TempDir,
    _library: ModelLibrary,
}
impl Fixture {
    async fn launch(tokens: &[u8], flags: &[&str]) -> Self {
        let temp = tempfile::TempDir::new().unwrap();
        let library = ModelLibrary::new(temp.path().join("library"))
            .await
            .unwrap();
        let model_id = "library/speech";
        let artifact_id = "controlled-selected";
        let package = library.library_root().join(model_id);
        std::fs::create_dir_all(&package).unwrap();
        let mut digest = Sha256::new();
        for name in MEMBERS {
            let bytes:&[u8]=match *name {
                "config.json"=>br#"{"model_type":"cohere_asr","architectures":["CohereAsrForConditionalGeneration"]}"#,
                "model.safetensors"=>tokens,
                _=>b"{}",
            };
            std::fs::write(package.join(name), bytes).unwrap();
            digest.update(name.as_bytes());
            digest.update(b"\0");
            digest.update(bytes);
        }
        let digest = hex::encode(digest.finalize());
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
        library
            .install_mutation_authority(
                crate::api::RuntimeTasks::new(),
                root.clone(),
                Arc::new(DownloadPersistence::new(&temp.path().join("downloads"))),
            )
            .unwrap();
        let prepared = Arc::new(
            library
                .prepare_cohere_artifact_use(model_id, artifact_id)
                .unwrap(),
        );
        let scratch = prepared.read_source_path().to_owned();
        let source = prepared.clone_read_source_directory().unwrap();
        let fd = source.as_raw_fd();
        let original_code = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../torch-server");
        let runtime = AudioRuntimeOwner::controlled_fixture_for_selected(
            &original_code,
            CODE,
            MEMBERS,
            &prepared,
        )
        .unwrap();
        let runtime_copy = runtime.read_source_path().to_owned();
        runtime.validate_source().unwrap();
        let profile = RuntimeProfileId::parse("controlled-audio-private").unwrap();
        let registry = AudioCustodyRegistry::new(profile.clone(), 23);
        registry.retain_runtime(runtime).unwrap();
        let custody = ManagedChildCustodySlot::new();
        let (stderr, child_stderr) = UnixStream::pair().unwrap();
        stderr.set_nonblocking(true).unwrap();
        let mut command = Command::new("python3");
        command
            .args(["-I", "-B", "-S"])
            .arg(runtime_copy.join("tests/owned_channel_fixture.py"))
            .arg("--fixture-source-fd")
            .arg(fd.to_string())
            .args(flags)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(OwnedFd::from(child_stderr)));
        // SAFETY: the held directory descriptor stays live through spawn; this
        // child-only fcntl is async-signal-safe and only removes CLOEXEC.
        #[allow(unsafe_code)]
        unsafe {
            command.pre_exec(move || {
                let flags = libc::fcntl(fd, libc::F_GETFD);
                if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
        let mut child = ManagedChild::spawn(&mut command, custody.clone()).unwrap();
        registry.attach_to_child(&mut child).unwrap();
        let pid = child.id();
        let (input, output) = child.take_private_stdio().unwrap();
        let supervisor = Arc::new(Supervisor {
            child: Mutex::new(Some(child)),
            custody,
            registry: registry.clone(),
            stopped: AtomicBool::new(false),
            changed: Notify::new(),
        });
        let stopped = supervisor.clone();
        let channel =
            PrivateAudioChannel::from_pipes(input, output, Arc::new(move || stopped.stop()))
                .unwrap();
        let client = OwnedAudioClient::bind(channel, registry, profile, 23, pid)
            .await
            .unwrap();
        drop(source);
        Self {
            client,
            prepared: Some(prepared),
            root,
            scratch,
            runtime_copy,
            digest,
            stderr: BufReader::new(tokio::net::UnixStream::from_std(stderr).unwrap()),
            supervisor,
            _temp: temp,
            _library: library,
        }
    }
    async fn load(&mut self) -> OwnedAudioSlot {
        let prepared = self.prepared.take().unwrap();
        self.client
            .load(prepared, "fixture-selected")
            .await
            .unwrap()
    }
    async fn stop(&self) {
        self.client.channel.quarantine();
        self.supervisor.wait().await;
        assert!(!self.supervisor.custody.is_active());
        assert!(!self.supervisor.custody.has_parked_child());
        assert!(!self.runtime_copy.exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.supervisor.stop();
    }
}

fn request() -> Value {
    json!({"contract_version":1,"request_id":"parent-correlated-7","model":"library/speech","capability":"audio_transcription","input":{"kind":"audio","encoding":"pcm_s16le","sample_rate_hz":16000,"channels":1,"sample_count":1,"data_base64":"AAA="},"output":"text","options":{"kind":"audio","language":"en"}})
}
fn assert_busy(root: &DownloadDestinationRoot) {
    assert!(matches!(
        root.try_acquire_execution_grant(),
        Err(crate::PumasError::DownloadRootBusy)
    ));
}
async fn bounded<T>(future: impl std::future::Future<Output = T>) -> T {
    // Harness hang guard only. Production has no invented exchange timeout.
    tokio::time::timeout(Duration::from_secs(15), future)
        .await
        .expect("controlled fixture hung")
}

#[tokio::test]
async fn selected_model_and_runtime_bytes_survive_use_until_exact_unload_and_child_drain() {
    bounded(async {
        let mut fixture = Fixture::launch(&[0], &[]).await;
        let slot = fixture.load().await;
        assert_busy(&fixture.root);
        assert!(fixture.scratch.exists());
        let result = fixture.client.execute(&slot, request()).await.unwrap();
        assert_eq!(result.finish_reason, FinishReason::Stop);
        assert_eq!(
            result.text,
            format!("controlled:{};calls=1;pcm=0000", fixture.digest)
        );
        let result = fixture.client.execute(&slot, request()).await.unwrap();
        assert!(result.text.contains("calls=2"));
        fixture.client.unload(&slot).await.unwrap();
        assert!(!fixture.scratch.exists());
        drop(fixture.root.try_acquire_execution_grant().unwrap());
        assert!(fixture.runtime_copy.exists());
        drop(slot);
        fixture.stop().await;
    })
    .await;
}

#[tokio::test]
async fn confirmed_cancel_without_diagnostic_keeps_slot_reusable_and_does_not_release_model() {
    bounded(async {
        let mut fixture = Fixture::launch(&[0], &["--hold-use-until-cancel"]).await;
        let slot = fixture.load().await;
        let operation = fixture.client.start(&slot, request()).await.unwrap();
        operation.cancel();
        assert_eq!(
            operation.wait().await.unwrap_err(),
            ChannelError::NativeRejected
        );
        drop(operation);
        assert_busy(&fixture.root);
        assert!(fixture.scratch.exists());
        let next = fixture.client.start(&slot, request()).await.unwrap();
        next.cancel();
        assert_eq!(next.wait().await.unwrap_err(), ChannelError::NativeRejected);
        drop(next);
        fixture.client.unload(&slot).await.unwrap();
        assert!(!fixture.scratch.exists());
        drop(slot);
        fixture.stop().await;
    })
    .await;
}

#[tokio::test]
async fn aborted_wait_requests_cancel_while_observer_retains_original_settlement() {
    bounded(async {
        let mut fixture = Fixture::launch(&[0], &["--hold-use-until-cancel"]).await;
        let slot = fixture.load().await;
        let operation = fixture.client.start(&slot, request()).await.unwrap();
        let waiter = operation.clone();
        let (entered, entry) = tokio::sync::oneshot::channel();
        let task = tokio::spawn(async move {
            let wait = waiter.wait();
            tokio::pin!(wait);
            // Poll wait once before signaling cancellation boundary.
            std::future::poll_fn(|cx| {
                assert!(wait.as_mut().poll(cx).is_pending());
                std::task::Poll::Ready(())
            })
            .await;
            entered.send(()).unwrap();
            wait.await
        });
        entry.await.unwrap();
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        assert_eq!(
            operation.wait().await.unwrap_err(),
            ChannelError::NativeRejected
        );
        drop(operation);
        fixture.client.unload(&slot).await.unwrap();
        drop(slot);
        fixture.stop().await;
    })
    .await;
}

#[tokio::test]
async fn lost_last_operation_caller_cancels_and_retains_slot_until_native_settlement() {
    bounded(async {
        let mut fixture = Fixture::launch(&[0], &["--hold-use-until-cancel"]).await;
        let slot = fixture.load().await;
        let operation = fixture.client.start(&slot, request()).await.unwrap();
        let retained = operation.operation.clone();
        drop(operation);
        loop {
            let changed = retained.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            if retained.is_settled() {
                break;
            }
            changed.await;
        }
        assert_eq!(
            retained
                .result
                .lock()
                .unwrap()
                .as_ref()
                .unwrap()
                .as_ref()
                .unwrap_err(),
            &ChannelError::NativeRejected
        );
        assert_busy(&fixture.root);
        drop(retained);
        fixture.client.unload(&slot).await.unwrap();
        drop(slot);
        fixture.stop().await;
    })
    .await;
}

#[tokio::test]
async fn lost_admitted_load_caller_cancels_exact_load_and_releases_only_original_clean_receipt() {
    bounded(async {
        let mut fixture = Fixture::launch(&[0], &["--hold-load-until-cancel"]).await;
        let prepared = fixture.prepared.take().unwrap();
        let client = fixture.client.clone();
        let task = tokio::spawn(async move { client.load(prepared, "fixture-selected").await });
        let mut line = String::new();
        fixture.stderr.read_line(&mut line).await.unwrap();
        assert_eq!(line.trim(), "controlled load entered");
        assert_busy(&fixture.root);
        assert!(fixture.scratch.exists());
        task.abort();
        assert!(task.await.unwrap_err().is_cancelled());
        // A refused repeated hello is only a channel barrier; model release must
        // be observed after the retained original load receipt, not cancel ack.
        let barrier = fixture
            .client
            .channel
            .exchange::<Value>(
                "hello",
                json!({}),
                Box::new(|| Ok(Box::new(super::super::audio_channel::ValueCustody))),
            )
            .await;
        assert!(matches!(barrier, Err(ChannelError::NativeRejected)));
        // Acquire exclusion eventually by driving the original receipt task;
        // no polling sleeps or arbitrary cleanup timeout is imposed.
        while fixture.scratch.exists() {
            tokio::task::yield_now().await;
        }
        drop(fixture.root.try_acquire_execution_grant().unwrap());
        fixture.stop().await;
    })
    .await;
}

#[tokio::test]
async fn explicit_length_evidence_and_unknown_terminal_failure_are_not_fabricated_stop() {
    bounded(async {
        for (tokens, expected) in [(vec![1; 512], Some(FinishReason::Length)), (vec![1], None)] {
            let mut fixture = Fixture::launch(&tokens, &[]).await;
            let slot = fixture.load().await;
            let result = fixture.client.execute(&slot, request()).await;
            match expected {
                Some(reason) => assert_eq!(result.unwrap().finish_reason, reason),
                None => assert_eq!(result.unwrap_err(), ChannelError::NativeRejected),
            }
            fixture.client.unload(&slot).await.unwrap();
            drop(slot);
            fixture.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn pre_native_rpc_error_reuses_same_channel_and_slot_for_clean_next_request() {
    bounded(async {
        let mut fixture = Fixture::launch(&[0], &[]).await;
        let slot = fixture.load().await;
        let mut invalid = request();
        invalid["input"]["sample_count"] = json!(2);
        assert!(matches!(
            fixture.client.execute(&slot, invalid).await,
            Err(ChannelError::NativeRejected)
        ));
        let result = fixture.client.execute(&slot, request()).await.unwrap();
        assert_eq!(result.finish_reason, FinishReason::Stop);
        fixture.client.unload(&slot).await.unwrap();
        drop(slot);
        fixture.stop().await;
    })
    .await;
}

#[tokio::test]
async fn another_root_with_same_model_and_members_cannot_replace_inherited_source_custody() {
    bounded(async {
        let mut original = Fixture::launch(&[0], &[]).await;
        let mut other = Fixture::launch(&[1; 512], &[]).await;
        let replacement = other.prepared.as_ref().unwrap().clone();
        assert!(matches!(
            original.client.load(replacement, "fixture-selected").await,
            Err(ChannelError::NotAdmitted)
        ));
        assert_busy(&original.root);
        assert_busy(&other.root);
        let slot = original.load().await;
        let result = original.client.execute(&slot, request()).await.unwrap();
        assert_eq!(result.finish_reason, FinishReason::Stop);
        assert!(result.text.contains(&original.digest));
        assert!(!result.text.contains(&other.digest));
        original.client.unload(&slot).await.unwrap();
        drop(slot);
        drop(original.root.try_acquire_execution_grant().unwrap());
        original.stop().await;
        other.stop().await;
        drop(other.prepared.take());
        assert!(!other.scratch.exists());
        drop(other.root.try_acquire_execution_grant().unwrap());
    })
    .await;
}

#[tokio::test]
async fn partial_or_incoherent_private_protocol_recovers_loaded_custody_only_after_exact_child_drain(
) {
    bounded(async {
        let mut fixture = Fixture::launch(&[0], &[]).await;
        let slot = fixture.load().await;
        assert_busy(&fixture.root);
        assert!(fixture.scratch.exists());
        // The controlled child's closed request schema rejects this operation,
        // closes its stream, and cannot attest native cleanup with an RPC result.
        let result = fixture
            .client
            .channel
            .exchange::<Value>(
                "unrecognized_operation",
                json!({}),
                Box::new(|| Ok(Box::new(super::super::audio_channel::ValueCustody))),
            )
            .await;
        assert!(matches!(result, Err(ChannelError::Unknown)));
        fixture.supervisor.wait().await;
        assert!(!fixture.supervisor.custody.is_active());
        assert!(!fixture.scratch.exists());
        assert!(!fixture.runtime_copy.exists());
        drop(fixture.root.try_acquire_execution_grant().unwrap());
        assert!(matches!(
            fixture.client.execute(&slot, request()).await,
            Err(ChannelError::NotAdmitted)
        ));
        drop(slot);
    })
    .await;
}
