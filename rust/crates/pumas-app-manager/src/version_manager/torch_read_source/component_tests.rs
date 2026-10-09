//! Controlled inert archive assembly; no actual wheel, Python or native load.
use super::*;
use crate::version_manager::{
    torch_runtime_byte_manifest_sha256, TorchComponentAssemblyRequest, TorchComponentFile,
    TorchComponentManifest,
};
use pumas_library::acquisition::*;
use std::io::Write;

const WHEEL: &str = "fixture_provider-1.0-cp39-abi3-linux_x86_64.whl";
const RECORD: &str = "fixture_provider-1.0.dist-info/METADATA,sha256=_mpgRO97JLQqMJVOkapdSEsPTOjfZV0HAQ9dQd5koZQ,59\nfixture_provider-1.0.dist-info/WHEEL,sha256=_FFLaua4oDdbUGXIfp4uAO1enDZFaXWxW0uyN5oqUWA,71\nfixture_provider/__init__.py,sha256=0rFVOrs4zc7Ng6Z2YHMPBFzRXPc1i6NJX-aCxciGbXk,24\nfixture_provider/provider.abi3.so,sha256=kPZVcDl6FZXqWRgjsoDhCtOOR1_m6pt2DaZ95XgSMfg,42\nfixture_provider-1.0.dist-info/RECORD,,\n";

struct Fixture {
    installed: Installed,
    manager: VersionManager,
    service: Arc<AcquisitionService>,
}

impl Fixture {
    async fn new() -> Self {
        let installed = Installed::new_at_tag("v2.9.1");
        let previous = registered_manager(&installed).await;
        let client = previous.github_client.clone();
        previous.shutdown_installations().await.unwrap();
        std::fs::create_dir(installed.launcher.path().join("acquisition-store")).unwrap();
        let service = Arc::new(AcquisitionService::new(Arc::new(AcquisitionStore::new(
            &installed.launcher.path().join("acquisition-store"),
        ))));
        let manager = VersionManager::new_with_acquisition_client(
            installed.launcher.path().to_owned(),
            AppId::Torch,
            service.clone(),
            Some(client),
        )
        .await
        .unwrap();
        Self {
            installed,
            manager,
            service,
        }
    }
    async fn request(&self, variant: &str, attempt: &str) -> TorchComponentAssemblyRequest {
        if variant == "recipe-bound" {
            let path = self.manager.versions_dir().join("v2.9.1/runtime.json");
            let mut recipe: serde_json::Value =
                serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
            recipe["assembly_fixture_padding"] = serde_json::json!("");
            let empty_size = serde_json::to_vec(&recipe).unwrap().len();
            recipe["assembly_fixture_padding"] =
                serde_json::json!("x".repeat(1024 * 1024 - 64 - empty_size));
            let bytes = serde_json::to_vec(&recipe).unwrap();
            assert_eq!(bytes.len(), 1024 * 1024 - 64);
            std::fs::write(path, bytes).unwrap();
        }
        let base = self
            .manager
            .retain_torch_runtime_bytes("v2.9.1")
            .await
            .unwrap();
        let base_digest = torch_runtime_byte_manifest_sha256(&base).unwrap();
        let mut entries = vec![
            (
                "fixture_provider/__init__.py".to_string(),
                b"# inert component bytes\n".to_vec(),
            ),
            (
                "fixture_provider/provider.abi3.so".to_string(),
                b"inert synthetic native bytes; never loaded".to_vec(),
            ),
            (
                "fixture_provider-1.0.dist-info/METADATA".to_string(),
                b"Metadata-Version: 2.1\nName: fixture-provider\nVersion: 1.0\n\n".to_vec(),
            ),
            (
                "fixture_provider-1.0.dist-info/WHEEL".to_string(),
                b"Wheel-Version: 1.0\nRoot-Is-Purelib: false\nTag: cp39-abi3-linux_x86_64\n\n"
                    .to_vec(),
            ),
            (
                "fixture_provider-1.0.dist-info/RECORD".to_string(),
                if variant == "record" {
                    RECORD.replace("sha256=kPZVc", "sha256=aPZVc").into_bytes()
                } else {
                    RECORD.as_bytes().to_vec()
                },
            ),
        ];
        let mut archive_files: Vec<_> = entries
            .iter()
            .map(|(path, bytes)| TorchComponentFile {
                path: path.clone(),
                size: bytes.len() as u64,
                sha256: format!("{:x}", Sha256::digest(bytes)),
            })
            .collect();
        let mut final_dependency_files: Vec<_> = base
            .manifest()
            .filter(|(role, _)| {
                *role == pumas_library::runtime_read_source::RuntimeReadRole::Dependencies
            })
            .map(|(_, file)| TorchComponentFile {
                path: file.path().into(),
                size: file.size(),
                sha256: file.sha256().into(),
            })
            .collect();
        final_dependency_files.extend(archive_files.clone());
        if variant == "unlisted" {
            entries.push(("unlisted.py".into(), b"not selected".to_vec()));
        }
        if variant == "traversal" {
            entries.push(("../escaped".into(), b"must refuse".to_vec()));
        }
        if variant == "embedded-footer" {
            entries[0].1.extend_from_slice(b"PK\x05\x06");
        }
        if variant == "digest" {
            entries[0].1.push(b'!');
        }
        if variant == "closure" {
            final_dependency_files[0].sha256 = "0".repeat(64);
        }
        if variant == "hook" {
            archive_files[0].path = "unexpected.pth".into();
        }
        let input = self
            .installed
            .launcher
            .path()
            .join(format!("input-{attempt}.whl"));
        let file = File::create(&input).unwrap();
        let mut zip = zip::ZipWriter::new(file);
        for (index, (path, bytes)) in entries.iter().enumerate() {
            let options = zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored);
            if variant == "symlink" && index == 0 {
                zip.add_symlink(path, "outside", options).unwrap();
            } else {
                zip.start_file(path, options).unwrap();
                zip.write_all(bytes).unwrap();
            }
        }
        zip.finish().unwrap();
        let mut bytes = std::fs::read(&input).unwrap();
        if variant == "zip64" || variant == "central-size" {
            let footer = bytes.windows(4).rposition(|b| b == b"PK\x05\x06").unwrap();
            if variant == "zip64" {
                bytes[footer + 8..footer + 12].copy_from_slice(&[255; 4]);
            } else {
                bytes[footer + 12..footer + 16]
                    .copy_from_slice(&(65_u32 * 1024 * 1024).to_le_bytes());
            }
            std::fs::write(&input, &bytes).unwrap();
        }
        let manifest = ArtifactManifest::new(
            ArtifactSourceIdentity::new(
                "controlled-fixture",
                "inert-provider",
                ArtifactRevisionEvidence::new(
                    "fixture.source",
                    "inert-archive-v1",
                    RevisionStrength::Immutable,
                )
                .unwrap(),
            )
            .unwrap(),
            vec![ArtifactFile::new(
                WHEEL,
                "selected-wheel",
                Some(bytes.len() as u64),
                Some(
                    Sha256Evidence::new("fixture.sha256", format!("{:x}", Sha256::digest(&bytes)))
                        .unwrap(),
                ),
                FileVerificationRequirement::Sha256,
            )
            .unwrap()],
        )
        .unwrap();
        let stage = self
            .installed
            .launcher
            .path()
            .join(format!("input-stage-{attempt}"));
        std::fs::create_dir(&stage).unwrap();
        let workspace = ReservedDirectory::capture(
            self.installed.launcher.path(),
            Path::new(&format!("input-stage-{attempt}")),
            Arc::new(()),
            || Ok(()),
        )
        .unwrap()
        .acquisition_workspace()
        .unwrap();
        TorchComponentAssemblyRequest {
            base_tag: "v2.9.1".into(),
            expected_base_manifest_sha256: base_digest,
            component: TorchComponentManifest {
                distribution: "fixture-provider".into(),
                version: "1.0".into(),
                wheel_name: WHEEL.into(),
                wheel_tag: "cp39-abi3-linux_x86_64".into(),
                python: "python3.12".into(),
                platform: "linux-x86_64".into(),
                archive_files,
                replace_base_members: vec![],
                final_dependency_files,
            },
            input: AcquisitionLocalRequest {
                demand: AcquisitionDemand {
                    consumer: "runtime.torch.components".into(),
                    operation: format!("assemble-{attempt}"),
                },
                manifest,
                workspace,
                sources: vec![AcquisitionLocalSource::new(
                    File::open(input).unwrap(),
                    Arc::new(()),
                )
                .unwrap()],
                retry: AcquisitionRetryPolicy {
                    attempts: Some(1),
                    elapsed: Duration::from_secs(5),
                    backoff: pumas_library::network::RetryConfig::new(),
                },
            },
        }
    }
    async fn shutdown(&self) {
        let _ = self.manager.shutdown_installations().await;
        self.service.shutdown().await.unwrap();
    }
}

async fn completion_result(
    mut progress: mpsc::Receiver<ProgressUpdate>,
) -> std::result::Result<(), String> {
    tokio::time::timeout(Duration::from_secs(10), async {
        while let Some(update) = progress.recv().await {
            match update {
                ProgressUpdate::Completed { success: true } => return Ok(()),
                ProgressUpdate::Completed { success: false } => {
                    return Err("unsuccessful completion".into())
                }
                ProgressUpdate::Error { message } => return Err(message),
                _ => {}
            }
        }
        Err("progress closed without terminal".into())
    })
    .await
    .unwrap()
}

async fn completion(progress: mpsc::Receiver<ProgressUpdate>) -> bool {
    completion_result(progress).await.is_ok()
}

#[tokio::test]
async fn component_assembly_publishes_closed_bytes_without_startup_or_inherited_qualification() {
    let fixture = Fixture::new().await;
    let sentinel = std::fs::read(fixture.installed.packages.join("dependency.py")).unwrap();
    let request = fixture.request("valid", "publish").await;
    let assembly = fixture
        .manager
        .assemble_torch_component_revision(request)
        .await
        .unwrap();
    let tag = assembly.revision_tag;
    assert!(completion(assembly.progress).await);
    let output = fixture.manager.versions_dir().join(&tag);
    assert!(!output.join("venv/bin").exists());
    assert!(!output.join("probe-results.json").exists());
    assert_eq!(
        std::fs::read(fixture.installed.packages.join("dependency.py")).unwrap(),
        sentinel
    );
    let metadata = fixture
        .manager
        .metadata_manager
        .get_installed_version(&tag, Some(AppId::Torch))
        .unwrap()
        .unwrap();
    assert_eq!(metadata.dependencies_installed, Some(false));
    let retained = fixture
        .manager
        .select_torch_component_revision(&tag)
        .await
        .unwrap();
    retained.validate().unwrap();
    assert_eq!(retained.revision_tag(), tag);
    assert!(Arc::ptr_eq(
        retained.retained_source(),
        retained.clone().retained_source()
    ));
    assert!(fixture.manager.verify_torch_identity(&tag).await.is_err());
    assert!(fixture
        .manager
        .torch_installed_probe_report(&tag)
        .await
        .is_err());
    assert!(fixture.manager.set_active_version(&tag).await.is_err());
    assert!(fixture.manager.check_dependencies(&tag).await.is_err());
    let options = fixture.manager.torch_runtime_options().await.unwrap();
    assert_eq!(
        options["installed"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["tag"] == tag)
            .unwrap()["qualification"],
        "assembled_unqualified"
    );
    let mut config = pumas_library::process::BinaryLaunchConfig::torch(&tag, &output);
    config.binary_path = PathBuf::from("/bin/sh");
    config.extra_args = vec!["-c".into(), "exit 97".into()];
    assert!(
        matches!(pumas_library::process::ProcessLauncher::launch_binary(&config),Err(PumasError::Validation{field,..}) if field=="runtime.component_start_unregistered")
    );
    let detached_file = retained
        .retained_source()
        .clone_member(RuntimeReadRole::Sidecar, "component-manifest.json")
        .unwrap();
    let retained_clone = retained.clone();
    assert_eq!(retained.qualification(), "assembled_unqualified");
    let matching = fixture
        .manager
        .select_torch_component_revision_matching(
            &tag,
            retained.manifest_sha256(),
            retained.interpreter_depot_manifest_sha256(),
        )
        .await
        .unwrap();
    drop(matching);
    assert!(
        fixture
            .manager
            .select_torch_component_revision_matching(
                &tag,
                retained.manifest_sha256(),
                &"0".repeat(64),
            )
            .await
            .is_err()
    );
    assert!(fixture.manager.remove_version(&tag).await.is_err());
    drop(retained);
    assert!(fixture.manager.remove_version(&tag).await.is_err());
    drop(retained_clone);
    fixture.manager.remove_version(&tag).await.unwrap();
    // File duplication alone does not preserve owner revision/depot leases.
    assert!(detached_file.metadata().unwrap().len() > 0);
    drop(detached_file);
    fixture.shutdown().await;
}

#[tokio::test]
async fn component_base_mismatch_and_import_hooks_refuse_before_store_admission() {
    let fixture = Fixture::new().await;
    let mut request = fixture.request("valid", "wrong-base").await;
    request.expected_base_manifest_sha256 = "0".repeat(64);
    assert!(fixture
        .manager
        .assemble_torch_component_revision(request)
        .await
        .is_err());
    assert!(fixture
        .manager
        .assemble_torch_component_revision(fixture.request("hook", "hook").await)
        .await
        .is_err());
    let mut request = fixture.request("valid", "directory-namespace-bound").await;
    request
        .component
        .final_dependency_files
        .extend((0..50_000).map(|index| TorchComponentFile {
            // Count original Linux directory spellings independently.
            path: format!(
                "{}-{}/one/two/{}.bin",
                if index % 2 == 0 { "Tree" } else { "tree" },
                index / 2,
                index % 2
            ),
            size: 0,
            sha256: "0".repeat(64),
        }));
    let error = fixture
        .manager
        .assemble_torch_component_revision(request)
        .await
        .err()
        .unwrap();
    assert!(error.to_string().contains("namespace exceeds bound"));
    assert!(fixture.service.store().acquisitions().unwrap().is_empty());
    fixture.shutdown().await;
}

#[tokio::test]
async fn component_archive_record_digest_namespace_and_final_closure_fail_closed() {
    for (index, variant) in [
        "record",
        "digest",
        "unlisted",
        "traversal",
        "symlink",
        "closure",
        "zip64",
        "central-size",
        "embedded-footer",
        "recipe-bound",
    ]
    .iter()
    .enumerate()
    {
        let fixture = Fixture::new().await;
        let assembly = fixture
            .manager
            .assemble_torch_component_revision(
                fixture.request(variant, &format!("refusal-{index}")).await,
            )
            .await
            .unwrap();
        let tag = assembly.revision_tag;
        let error = completion_result(assembly.progress).await.unwrap_err();
        if *variant == "embedded-footer" {
            assert!(error.contains("Ambiguous ZIP footer"), "{error}");
        }
        if *variant == "recipe-bound" {
            assert!(
                error.contains("recipe exceeds retained reader bound"),
                "{error}"
            );
        }
        if matches!(*variant, "zip64" | "central-size") {
            assert!(error.contains("Unsupported ZIP64"), "{error}");
        }
        assert!(
            !fixture.manager.versions_dir().join(&tag).exists(),
            "{variant}"
        );
        assert!(fixture
            .manager
            .metadata_manager
            .get_installed_version(&tag, Some(AppId::Torch))
            .unwrap()
            .is_none());
        assert_eq!(
            std::fs::read(fixture.installed.packages.join("dependency.py")).unwrap(),
            b"selected dependency"
        );
        let records = fixture.service.store().acquisitions().unwrap();
        assert_eq!(records.len(), 1);
        let record = records.values().next().unwrap();
        assert_eq!(record.phase, AcquisitionPhase::Withdrawn, "{variant}");
        assert!(fixture
            .service
            .consumer_receipt(record.id)
            .unwrap()
            .is_none());
        fixture.shutdown().await;
    }
}

#[tokio::test]
async fn component_caller_loss_keeps_base_depot_until_owned_publication_finishes() {
    let mut fixture = Fixture::new().await;
    let pause = Arc::new(installer::TorchPublicationPause::new());
    fixture.manager = fixture.manager.with_torch_publication_pause(pause.clone());
    let assembly = fixture
        .manager
        .assemble_torch_component_revision(fixture.request("valid", "caller-loss").await)
        .await
        .unwrap();
    let tag = assembly.revision_tag;
    drop(assembly.progress);
    tokio::time::timeout(Duration::from_secs(10), pause.reached.notified())
        .await
        .unwrap();
    assert!(
        super::super::super::managed_depot_lease::ManagedDepotLease::mutation(
            &fixture.installed.depot
        )
        .is_err()
    );
    pause.resume.add_permits(1);
    fixture.manager.shutdown_installations().await.unwrap();
    assert!(fixture
        .manager
        .metadata_manager
        .get_installed_version(&tag, Some(AppId::Torch))
        .unwrap()
        .is_some());
    assert!(
        super::super::super::managed_depot_lease::ManagedDepotLease::mutation(
            &fixture.installed.depot
        )
        .is_ok()
    );
    fixture.service.shutdown().await.unwrap();
}

#[tokio::test]
async fn component_metadata_failure_rolls_back_after_actual_atomic_rename() {
    let mut fixture = Fixture::new().await;
    let pause = Arc::new(installer::TorchPublicationPause::new());
    fixture.manager = fixture.manager.with_torch_publication_pause(pause.clone());
    let assembly = fixture
        .manager
        .assemble_torch_component_revision(fixture.request("valid", "rollback").await)
        .await
        .unwrap();
    let tag = assembly.revision_tag;
    tokio::time::timeout(Duration::from_secs(10), pause.reached.notified())
        .await
        .unwrap();
    let metadata = fixture
        .installed
        .launcher
        .path()
        .join("launcher-data/metadata/versions-torch.json");
    let saved = std::fs::read(&metadata).unwrap();
    std::fs::remove_file(&metadata).unwrap();
    std::fs::create_dir(&metadata).unwrap();
    pause.resume.add_permits(1);
    assert!(!completion(assembly.progress).await);
    assert!(!fixture.manager.versions_dir().join(&tag).exists());
    let records = fixture.service.store().acquisitions().unwrap();
    assert_eq!(records.len(), 1);
    let record = records.values().next().unwrap();
    assert!(matches!(record.phase, AcquisitionPhase::Adopted { .. }));
    let receipt = fixture
        .service
        .consumer_receipt(record.id)
        .unwrap()
        .unwrap();
    assert_eq!(receipt.payload["phase"], "component_bytes_prepared");
    assert_eq!(
        receipt.payload["manifest_sha256"],
        tag.strip_prefix("torch-component-").unwrap()
    );
    std::fs::remove_dir(&metadata).unwrap();
    std::fs::write(&metadata, saved).unwrap();
    assert!(fixture
        .manager
        .metadata_manager
        .get_installed_version("v2.9.1", Some(AppId::Torch))
        .unwrap()
        .is_some());
    assert!(fixture
        .manager
        .metadata_manager
        .get_installed_version(&tag, Some(AppId::Torch))
        .unwrap()
        .is_none());
    fixture.shutdown().await;
}
