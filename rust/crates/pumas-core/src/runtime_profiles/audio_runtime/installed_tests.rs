use super::*;
use crate::index::ModelRecord;
use crate::model_library::{DownloadDestinationRoot, DownloadPersistence, ModelLibrary};
use crate::models::{AssetValidationState, ModelMetadata, StorageKind};
use crate::platform::capability_fs::open_pinned_directory;
use crate::runtime_read_source::{RuntimeReadFile, RuntimeReadRoot};
use std::collections::HashMap;
use std::path::PathBuf;

const MODEL: &str = "audio/cohere/installed-candidate-fixture";
const ARTIFACT: &str = "installed-candidate-fixture-hf";
const MODEL_FILES: &[&str] = &[
    "config.json",
    "model.safetensors",
    "preprocessor_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
];

struct ModelFixture {
    _temp: tempfile::TempDir,
    library: ModelLibrary,
    root: DownloadDestinationRoot,
}

impl ModelFixture {
    async fn new() -> Self {
        let temp = tempfile::tempdir().unwrap();
        let library = ModelLibrary::new(temp.path().join("library"))
            .await
            .unwrap();
        let package = library.library_root().join(MODEL);
        std::fs::create_dir_all(&package).unwrap();
        for name in MODEL_FILES {
            let bytes: &[u8] = match *name {
                "config.json" => br#"{"model_type":"cohere_asr","architectures":["CohereAsrForConditionalGeneration"]}"#,
                "model.safetensors" => b"synthetic unparsed weights; no native execution",
                _ => b"{}",
            };
            std::fs::write(package.join(name), bytes).unwrap();
        }
        let metadata = serde_json::to_value(ModelMetadata {
            schema_version: Some(2),
            model_id: Some(MODEL.into()),
            selected_artifact_id: Some(ARTIFACT.into()),
            selected_artifact_files: Some(MODEL_FILES.iter().map(|name| (*name).into()).collect()),
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
                id: MODEL.into(),
                path: package.display().to_string(),
                cleaned_name: "fixture".into(),
                official_name: "Synthetic ownership fixture".into(),
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
        Self {
            _temp: temp,
            library,
            root,
        }
    }

    fn prepare(&self) -> Arc<PreparedArtifactUse> {
        Arc::new(
            self.library
                .prepare_cohere_artifact_use(MODEL, ARTIFACT)
                .unwrap(),
        )
    }
}

struct RuntimeFixture {
    roots: Vec<(RuntimeReadRole, Arc<tempfile::TempDir>)>,
}

impl RuntimeFixture {
    fn new() -> Self {
        let roots = [
            RuntimeReadRole::Interpreter,
            RuntimeReadRole::Dependencies,
            RuntimeReadRole::Sidecar,
        ]
        .into_iter()
        .map(|role| (role, Arc::new(tempfile::tempdir().unwrap())))
        .collect();
        let fixture = Self { roots };
        // These are deliberately dummy runtime bytes, not an installed recipe.
        fixture.write(
            RuntimeReadRole::Interpreter,
            "bin/python3.12",
            b"not an executable interpreter",
        );
        fixture.write(
            RuntimeReadRole::Dependencies,
            "controlled_dependency.py",
            b"CONTROLLED = True\n",
        );
        for (name, bytes) in REQUIRED_CODE {
            fixture.write(RuntimeReadRole::Sidecar, name, bytes);
        }
        fixture
    }

    fn path(&self, role: RuntimeReadRole, name: &str) -> PathBuf {
        self.roots
            .iter()
            .find(|(candidate, _)| *candidate == role)
            .unwrap()
            .1
            .path()
            .join(name)
    }

    fn write(&self, role: RuntimeReadRole, name: &str, bytes: &[u8]) {
        let path = self.path(role, name);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    fn capture(&self, omit: Option<RuntimeReadRole>) -> Arc<RetainedRuntimeReadSource> {
        let selections = self
            .roots
            .iter()
            .filter(|(role, _)| Some(*role) != omit)
            .map(|(role, root)| {
                let mut files = Vec::new();
                fn collect(root: &Path, directory: &Path, files: &mut Vec<RuntimeReadFile>) {
                    for entry in std::fs::read_dir(directory).unwrap() {
                        let path = entry.unwrap().path();
                        if path.is_dir() {
                            collect(root, &path, files);
                        } else {
                            let bytes = std::fs::read(&path).unwrap();
                            files.push(
                                RuntimeReadFile::new(
                                    path.strip_prefix(root).unwrap().to_str().unwrap().into(),
                                    bytes.len() as u64,
                                    hex::encode(Sha256::digest(&bytes)),
                                )
                                .unwrap(),
                            );
                        }
                    }
                }
                collect(root.path(), root.path(), &mut files);
                RuntimeReadRoot::new(
                    *role,
                    open_pinned_directory(root.path()).unwrap().into_std_file(),
                    files,
                    vec![],
                    root.clone(),
                )
                .unwrap()
            })
            .collect();
        RetainedRuntimeReadSource::capture(selections).unwrap()
    }
}

#[tokio::test]
async fn candidate_retains_actual_roles_selected_bytes_and_mutation_leases() {
    let model = ModelFixture::new().await;
    let runtime = RuntimeFixture::new();
    let leases: Vec<_> = runtime
        .roots
        .iter()
        .map(|(_, root)| Arc::downgrade(root))
        .collect();
    let installed = runtime.capture(None);
    let installed_weak = Arc::downgrade(&installed);
    let selected = model.prepare();
    let selected_weak = Arc::downgrade(&selected);
    let scratch = selected.read_source_path().to_owned();
    let candidate =
        InstalledAudioRuntimeCandidate::capture(installed, "bin/python3.12", selected).unwrap();
    drop(runtime);
    candidate.validate().unwrap();
    assert!(installed_weak.upgrade().is_some());
    assert!(selected_weak.upgrade().is_some());
    assert!(leases.iter().all(|lease| lease.upgrade().is_some()));
    assert!(model.root.try_acquire_execution_grant().is_err());
    assert!(scratch.exists());
    drop(candidate);
    assert!(installed_weak.upgrade().is_none());
    assert!(selected_weak.upgrade().is_none());
    assert!(leases.iter().all(|lease| lease.upgrade().is_none()));
    assert!(!scratch.exists());
    drop(model.root.try_acquire_execution_grant().unwrap());
}

#[tokio::test]
async fn candidate_requires_every_runtime_role_and_a_retained_interpreter_member() {
    let model = ModelFixture::new().await;
    let runtime = RuntimeFixture::new();
    for role in [
        RuntimeReadRole::Interpreter,
        RuntimeReadRole::Dependencies,
        RuntimeReadRole::Sidecar,
    ] {
        assert!(InstalledAudioRuntimeCandidate::capture(
            runtime.capture(Some(role)),
            "bin/python3.12",
            model.prepare()
        )
        .is_err());
    }
    for name in ["missing", "../bin/python3.12", "controlled_dependency.py"] {
        assert!(InstalledAudioRuntimeCandidate::capture(
            runtime.capture(None),
            name,
            model.prepare()
        )
        .is_err());
    }
}

#[tokio::test]
async fn candidate_rejects_self_consistent_but_unshipped_worker_bytes() {
    let model = ModelFixture::new().await;
    let runtime = RuntimeFixture::new();
    runtime.write(
        RuntimeReadRole::Sidecar,
        "owned_audio.py",
        b"untrusted replacement\n",
    );
    let installed = runtime.capture(None);
    // A manifest that accurately describes those bytes still cannot select code.
    installed.validate().unwrap();
    assert!(
        InstalledAudioRuntimeCandidate::capture(installed, "bin/python3.12", model.prepare())
            .is_err()
    );
}

#[tokio::test]
async fn candidate_rejects_missing_worker_code_and_import_hooks() {
    let model = ModelFixture::new().await;
    let runtime = RuntimeFixture::new();
    std::fs::remove_file(runtime.path(RuntimeReadRole::Sidecar, "owned_worker.py")).unwrap();
    assert!(InstalledAudioRuntimeCandidate::capture(
        runtime.capture(None),
        "bin/python3.12",
        model.prepare()
    )
    .is_err());
    for name in [
        "hook.pth",
        "cached.pyc",
        "cached.pyo",
        "sitecustomize.py",
        "usercustomize.py",
    ] {
        let runtime = RuntimeFixture::new();
        runtime.write(
            RuntimeReadRole::Dependencies,
            name,
            b"unselected startup effect",
        );
        assert!(InstalledAudioRuntimeCandidate::capture(
            runtime.capture(None),
            "bin/python3.12",
            model.prepare()
        )
        .is_err());
    }
}

#[tokio::test]
async fn candidate_revalidation_observes_drift_in_each_held_role() {
    let model = ModelFixture::new().await;
    for (role, member) in [
        (RuntimeReadRole::Interpreter, "bin/python3.12"),
        (RuntimeReadRole::Dependencies, "controlled_dependency.py"),
        (RuntimeReadRole::Sidecar, "owned_audio.py"),
    ] {
        let runtime = RuntimeFixture::new();
        let candidate = InstalledAudioRuntimeCandidate::capture(
            runtime.capture(None),
            "bin/python3.12",
            model.prepare(),
        )
        .unwrap();
        runtime.write(role, member, b"changed actual held bytes");
        assert!(
            candidate.validate().is_err(),
            "{role:?} drift was not observed"
        );
    }
}

#[tokio::test]
async fn candidate_binds_exact_selected_allocation_and_observes_model_drift() {
    let model = ModelFixture::new().await;
    let runtime = RuntimeFixture::new();
    let selected = model.prepare();
    let other_model = ModelFixture::new().await;
    let equal_bytes = other_model.prepare();
    assert_eq!(selected.manifest_sha256(), equal_bytes.manifest_sha256());
    let candidate = InstalledAudioRuntimeCandidate::capture(
        runtime.capture(None),
        "bin/python3.12",
        selected.clone(),
    )
    .unwrap();
    assert!(candidate.owns_selected(&selected));
    assert!(!candidate.owns_selected(&equal_bytes));
    let copied = selected.read_source_path().join("model.safetensors");
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(&copied, std::fs::Permissions::from_mode(0o600)).unwrap();
    std::fs::write(copied, b"changed synthetic copied weights").unwrap();
    assert!(candidate.validate().is_err());
}

#[tokio::test]
async fn coherent_candidate_cannot_discharge_execution_gaps_or_enable_shipping_factory() {
    let model = ModelFixture::new().await;
    let runtime = RuntimeFixture::new();
    let candidate = InstalledAudioRuntimeCandidate::capture(
        runtime.capture(None),
        "bin/python3.12",
        model.prepare(),
    )
    .unwrap();
    candidate.validate().unwrap();
    assert_eq!(candidate.qualification_gaps(), GAPS);
    assert_eq!(candidate.qualification_gaps().len(), 4);
    assert!(matches!(
        super::super::AudioRuntimeOwner::for_installed_runtime(),
        Err(super::super::AudioCustodyError::UnqualifiedRuntime)
    ));
}
