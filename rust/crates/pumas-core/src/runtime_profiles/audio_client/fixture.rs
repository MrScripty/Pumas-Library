//! Controlled process qualification only: no installed Torch/model inference.
use super::super::audio_runtime::AudioRuntimeOwner;
use super::*;
use crate::index::ModelRecord;
use crate::model_library::{DownloadDestinationRoot, DownloadPersistence, ModelLibrary};
use crate::models::{AssetValidationState, ModelMetadata, StorageKind};
use crate::platform::managed_child::{ManagedChild, ManagedChildCustodySlot};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::os::fd::{AsRawFd, OwnedFd};
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::io::BufReader;

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
    "loaders/owned_cohere_source.py",
    "loaders/__init__.py",
    "tests/owned_channel_fixture.py",
    "owned_worker.py",
    "tests/owned_worker_fixture.py",
];

pub(crate) struct Supervisor {
    child: Mutex<Option<ManagedChild>>,
    pub(crate) custody: Arc<ManagedChildCustodySlot>,
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
    pub(crate) async fn wait(&self) {
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

pub(crate) struct Fixture {
    pub(crate) client: Arc<OwnedAudioClient>,
    pub(crate) prepared: Option<Arc<PreparedArtifactUse>>,
    pub(crate) root: DownloadDestinationRoot,
    pub(crate) scratch: PathBuf,
    pub(crate) runtime_copy: PathBuf,
    pub(crate) dependencies_copy: PathBuf,
    pub(crate) digest: String,
    pub(crate) stderr: BufReader<tokio::net::UnixStream>,
    pub(crate) supervisor: Arc<Supervisor>,
    pub(crate) _temp: tempfile::TempDir,
    pub(crate) _library: Arc<ModelLibrary>,
}
impl Fixture {
    pub(crate) async fn launch(tokens: &[u8], flags: &[&str]) -> Self {
        Self::launch_in_library(tokens, flags, None).await
    }
    pub(crate) async fn launch_in_library(
        tokens: &[u8],
        flags: &[&str],
        existing: Option<Arc<ModelLibrary>>,
    ) -> Self {
        let standalone = existing.is_none();
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
        let code_source = runtime.clone_read_source_directory().unwrap();
        let code_fd = code_source.as_raw_fd();
        let packages = Arc::new(tempfile::TempDir::new().unwrap());
        let dependencies_copy = packages.path().to_owned();
        let dependency = b"CONTROLLED_DEPENDENCY = True\n";
        std::fs::write(packages.path().join("controlled_dependency.py"), dependency).unwrap();
        let packages_source =
            crate::platform::capability_fs::open_pinned_directory(packages.path()).unwrap();
        let packages_fd = packages_source.as_raw_fd();
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
        let profile = RuntimeProfileId::parse("controlled-audio-private").unwrap();
        let registry = AudioCustodyRegistry::new(profile.clone(), 23);
        registry.retain_runtime(runtime).unwrap();
        let custody = ManagedChildCustodySlot::new();
        let (stderr, child_stderr) = UnixStream::pair().unwrap();
        stderr.set_nonblocking(true).unwrap();
        let mut command = Command::new("python3");
        command
            .args(["-I", "-B", "-S"])
            .arg(runtime_copy.join("tests/owned_worker_fixture.py"))
            .arg("--model-root-fd")
            .arg(fd.to_string())
            .arg("--code-root-fd")
            .arg(code_fd.to_string())
            .arg("--packages-root-fd")
            .arg(packages_fd.to_string())
            .args(flags)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::from(OwnedFd::from(child_stderr)));
        // SAFETY: the held directory descriptor stays live through spawn; this
        // child-only fcntl is async-signal-safe and only removes CLOEXEC.
        #[allow(unsafe_code)]
        unsafe {
            command.pre_exec(move || {
                for fd in [fd, code_fd, packages_fd] {
                    let flags = libc::fcntl(fd, libc::F_GETFD);
                    if flags < 0 || libc::fcntl(fd, libc::F_SETFD, flags & !libc::FD_CLOEXEC) < 0 {
                        return Err(std::io::Error::last_os_error());
                    }
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
            dependencies_copy,
            digest,
            stderr: BufReader::new(tokio::net::UnixStream::from_std(stderr).unwrap()),
            supervisor,
            _temp: temp,
            _library: library,
        }
    }
    pub(crate) async fn load(&mut self) -> OwnedAudioSlot {
        let prepared = self.prepared.take().unwrap();
        self.client
            .load(prepared, "fixture-selected")
            .await
            .unwrap()
    }
    pub(crate) async fn stop(&self) {
        self.client.channel.quarantine();
        self.supervisor.wait().await;
        assert!(!self.supervisor.custody.is_active());
        assert!(!self.supervisor.custody.has_parked_child());
        assert!(!self.runtime_copy.exists());
        assert!(!self.dependencies_copy.exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.supervisor.stop();
    }
}
