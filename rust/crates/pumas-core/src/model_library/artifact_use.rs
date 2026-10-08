//! Prepared selected-byte custody for the first native Cohere ASR load path.
//!
//! This is private load-owner infrastructure, not production execution authority.
//! A future exact-child owner must retain this value together with independently
//! qualified recipe/code evidence through slot unload or confirmed process drain.
//! The package observation token is deliberately not accepted here. Native root
//! exclusion protects cooperating Pumas mutations; the copied read source keeps
//! later changes to the original package from changing the bytes handed to a
//! loader. Exclusion is cooperative, not protection from uncooperative writes
//! during copying. The manifest attests the actual copied bytes. Same-user
//! hostile modification of private scratch is not excluded by filesystem
//! permissions; revalidation is an observation, not a file lease.

// The next process/slot integration slice owns these private entry points.
#![allow(dead_code)]

use super::{
    DownloadDestinationRoot, DownloadRecoveryDestination, ModelLibrary, RootExecutionGrant,
};
use crate::models::{AssetValidationState, ModelMetadata, StorageKind};
use crate::platform::capability_fs::open_pinned_directory;
use crate::platform::process::same_file_identity;
use crate::{PumasError, Result};
use cap_std::fs::{Dir, OpenOptions, OpenOptionsExt};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
use tempfile::TempDir;

// A deliberately closed, unsharded native package variant. Unknown members are
// never copied, and an indexed selection containing one is refused. Expanding
// loader formats/member coverage requires corresponding qualification evidence.
const REQUIRED_MEMBERS: &[&str] = &[
    "config.json",
    "model.safetensors",
    "preprocessor_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
];
const OPTIONAL_MEMBERS: &[&str] = &[
    "added_tokens.json",
    "generation_config.json",
    "processor_config.json",
    "special_tokens_map.json",
];
const MAX_DESCRIPTOR_BYTES: u64 = 65_536;

/// Actual bytes in the copied load source. Neither this nor its digest grants
/// authority to construct another custody owner.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct SelectedByteManifestEntry {
    pub(crate) relative_path: String,
    pub(crate) size_bytes: u64,
    pub(crate) sha256: String,
}

struct CopiedMember {
    manifest: SelectedByteManifestEntry,
    file: File,
}

/// Non-cloneable, non-serializable custody; callers may retain it behind Arc.
/// It attests the prepared read source only, not a past model load or recipe.
pub(crate) struct PreparedArtifactUse {
    model_id: String,
    selected_artifact_id: String,
    root: DownloadDestinationRoot,
    grant: RootExecutionGrant,
    // Files and directory close before TempDir performs owned scratch cleanup.
    members: Vec<CopiedMember>,
    directory: Dir,
    read_source: TempDir,
    manifest_sha256: String,
}

impl ModelLibrary {
    /// Blocking preparation for an owning load task, before provider effects.
    /// Selectors must agree with the trusted index and canonical metadata. No
    /// caller path, digest, manifest JSON, or runtime identity can authorize it.
    pub(crate) fn prepare_cohere_artifact_use(
        &self,
        model_id: &str,
        selected_artifact_id: &str,
    ) -> Result<PreparedArtifactUse> {
        if !super::artifact_load_target::model_id_is_library_relative(model_id) {
            return Err(refusal("invalid model selector"));
        }
        let authority = self.mutation_authority()?;
        let root = authority.root().clone();
        let grant = root.try_acquire_execution_grant()?;
        let record = self
            .index()
            .get(model_id)?
            .ok_or_else(|| refusal("selected model is not indexed"))?;
        if Path::new(&record.path) != self.library_root().join(model_id)
            || !super::importer::publication::indexed_publication_ready(
                self.library_root(),
                model_id,
                &record.metadata,
            )
        {
            return Err(refusal(
                "selected publication is not a ready managed package",
            ));
        }
        let indexed: ModelMetadata = serde_json::from_value(record.metadata)?;
        let destination = root.resolve(Path::new(model_id))?;
        let canonical =
            super::importer::publication::read_held_canonical_import_metadata(&destination)?
                .ok_or_else(|| refusal("canonical selected metadata is unavailable"))?;
        let selected = selected_members(&indexed, model_id, selected_artifact_id)?;
        if selected_members(&canonical, model_id, selected_artifact_id)? != selected {
            return Err(refusal("canonical and indexed selections disagree"));
        }
        // A present supported optional member must be selected explicitly; a
        // loader must not silently consume unselected processor/tokenizer data.
        for member in OPTIONAL_MEMBERS {
            match destination.open_import_file(member) {
                Ok(_) if !selected.contains(*member) => {
                    return Err(refusal(
                        "loader member is absent from the indexed selection",
                    ));
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                Err(error) => return Err(error.into()),
            }
        }
        let read_source = tempfile::Builder::new()
            .prefix("pumas-artifact-use-")
            .tempdir()?;
        let directory = open_pinned_directory(read_source.path())?;
        let mut prepared = PreparedArtifactUse {
            model_id: model_id.to_owned(),
            selected_artifact_id: selected_artifact_id.to_owned(),
            root,
            grant,
            members: Vec::with_capacity(selected.len()),
            directory,
            read_source,
            manifest_sha256: String::new(),
        };
        for member in selected {
            prepared.grant.validate_root(&prepared.root)?;
            prepared
                .members
                .push(copy_member(&destination, &prepared.directory, &member)?);
        }
        validate_native_descriptors(&prepared.directory)?;
        prepared.manifest_sha256 = manifest_sha256(&prepared);
        prepared.validate_read_source()?;
        make_held_directory_private_read_only(&prepared.directory)?;
        Ok(prepared)
    }
}

impl PreparedArtifactUse {
    pub(crate) fn model_id(&self) -> &str {
        &self.model_id
    }

    pub(crate) fn selected_artifact_id(&self) -> &str {
        &self.selected_artifact_id
    }

    /// Private copied path for the qualified loader, never a locator authority.
    pub(crate) fn read_source_path(&self) -> &Path {
        self.read_source.path()
    }

    /// Duplicate the held copied directory for the exact managed child.
    /// A public locator cannot substitute for this retained source capability.
    pub(crate) fn clone_read_source_directory(&self) -> std::io::Result<File> {
        Ok(self.directory.try_clone()?.into_std_file())
    }

    pub(crate) fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    pub(crate) fn manifest(&self) -> impl Iterator<Item = &SelectedByteManifestEntry> {
        self.members.iter().map(|member| &member.manifest)
    }

    /// Blocking validation before a provider effect. Operation borrows later
    /// validate retained in-memory identity; they must not call this file scan.
    pub(crate) fn validate_read_source(&self) -> Result<()> {
        self.grant.validate_root(&self.root)?;
        let current = open_pinned_directory(self.read_source.path())?;
        if !same_file_identity(
            &self.directory.try_clone()?.into_std_file(),
            &current.into_std_file(),
        )? {
            return Err(refusal("copied read source was replaced"));
        }
        let actual = self
            .directory
            .entries()?
            .map(|entry| entry.map(|entry| entry.file_name()))
            .collect::<std::io::Result<BTreeSet<_>>>()?;
        let expected = self
            .members
            .iter()
            .map(|member| std::ffi::OsString::from(&member.manifest.relative_path))
            .collect::<BTreeSet<_>>();
        if actual != expected {
            return Err(refusal("copied read source member set changed"));
        }
        for member in &self.members {
            let mut current = open_member(&self.directory, &member.manifest.relative_path)?;
            if !same_file_identity(&member.file, &current)?
                || current.metadata()?.len() != member.manifest.size_bytes
                || hash_reader(&mut current)?.1 != member.manifest.sha256
            {
                return Err(refusal("copied read source bytes changed"));
            }
        }
        Ok(())
    }
}

impl Drop for PreparedArtifactUse {
    fn drop(&mut self) {
        // TempDir owns cleanup; permission restoration changes private copies
        // only. No original selected file is chmodded, renamed or removed.
        // A replaced scratch name is not ours to remove. Restore permissions
        // through the held descriptor, never through a potentially replaced
        // path. Leaking the original private copy is preferable to deleting a
        // different directory after an incoherent name observation.
        let coherent = open_pinned_directory(self.read_source.path())
            .and_then(|current| {
                same_file_identity(
                    &self.directory.try_clone()?.into_std_file(),
                    &current.into_std_file(),
                )
            })
            .unwrap_or(false);
        self.read_source.disable_cleanup(!coherent);
        let _ = make_held_directory_private_writable(&self.directory);
        #[cfg(windows)]
        for member in &self.members {
            if let Ok(mut permissions) = member.file.metadata().map(|value| value.permissions()) {
                permissions.set_readonly(false);
                let _ = member.file.set_permissions(permissions);
            }
        }
    }
}

fn selected_members(
    metadata: &ModelMetadata,
    model_id: &str,
    selected_artifact_id: &str,
) -> Result<BTreeSet<String>> {
    if selected_artifact_id.is_empty()
        || metadata.model_id.as_deref() != Some(model_id)
        || metadata.selected_artifact_id.as_deref() != Some(selected_artifact_id)
        || metadata.storage_kind != Some(StorageKind::LibraryOwned)
        || metadata.validation_state != Some(AssetValidationState::Valid)
        || !metadata.copied_import_ready()
    {
        return Err(refusal(
            "indexed selection is not a valid library-owned artifact",
        ));
    }
    let files = metadata
        .selected_artifact_files
        .as_ref()
        .ok_or_else(|| refusal("indexed selected members are unavailable"))?;
    if files.len() > REQUIRED_MEMBERS.len() + OPTIONAL_MEMBERS.len() {
        return Err(refusal("selected native member set is unsupported"));
    }
    let selected = files.iter().cloned().collect::<BTreeSet<_>>();
    if selected.len() != files.len()
        || !REQUIRED_MEMBERS
            .iter()
            .all(|member| selected.contains(*member))
        || !selected.iter().all(|member| {
            REQUIRED_MEMBERS.contains(&member.as_str())
                || OPTIONAL_MEMBERS.contains(&member.as_str())
        })
    {
        return Err(refusal(
            "selected native member set is unsupported or incomplete",
        ));
    }
    Ok(selected)
}

fn copy_member(
    source: &DownloadRecoveryDestination,
    target: &Dir,
    relative_path: &str,
) -> Result<CopiedMember> {
    let mut original = source.open_import_file(relative_path)?;
    let before = original.metadata()?;
    let mut options = OpenOptions::new();
    options.write(true).read(true).create_new(true);
    #[cfg(unix)]
    options.mode(0o600);
    let mut copy = target.open_with(relative_path, &options)?.into_std();
    let mut digest = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 65_536];
    loop {
        let count = original.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or_else(|| refusal("selected member size overflow"))?;
        digest.update(&buffer[..count]);
        copy.write_all(&buffer[..count])?;
    }
    let after = original.metadata()?;
    let current = source.open_import_file(relative_path)?;
    if size != before.len()
        || before.len() != after.len()
        || before.modified()? != after.modified()?
        || !same_file_identity(&original, &current)?
    {
        return Err(refusal("selected source member changed during copying"));
    }
    copy.flush()?;
    copy.sync_all()?;
    copy.seek(SeekFrom::Start(0))?;
    let manifest = SelectedByteManifestEntry {
        relative_path: relative_path.to_owned(),
        size_bytes: size,
        sha256: hex::encode(digest.finalize()),
    };
    let mut permissions = copy.metadata()?.permissions();
    permissions.set_readonly(true);
    copy.set_permissions(permissions)?;
    // Retain a read-only handle, not the writable copy descriptor.
    drop(copy);
    let file = open_member(target, relative_path)?;
    Ok(CopiedMember { manifest, file })
}

fn open_member(directory: &Dir, name: &str) -> Result<File> {
    let observed = directory.symlink_metadata(name)?;
    if !observed.is_file() || observed.is_symlink() {
        return Err(refusal("copied member is not a regular file"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    #[cfg(windows)]
    {
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_OPEN_REPARSE_POINT, FILE_GENERIC_READ, FILE_WRITE_ATTRIBUTES,
        };
        // Retain read-only byte access, with only the attribute permission
        // needed to clear our private copy's read-only flag during cleanup.
        options
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .access_mode(FILE_GENERIC_READ | FILE_WRITE_ATTRIBUTES);
    }
    let file = directory.open_with(name, &options)?.into_std();
    if !file.metadata()?.is_file() || file.metadata()?.file_type().is_symlink() {
        return Err(refusal("copied member is not a regular file"));
    }
    Ok(file)
}

fn hash_reader(reader: &mut impl Read) -> Result<(u64, String)> {
    let mut digest = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0u8; 65_536];
    loop {
        let count = reader.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or_else(|| refusal("selected member size overflow"))?;
        digest.update(&buffer[..count]);
    }
    Ok((size, hex::encode(digest.finalize())))
}

fn validate_native_descriptors(directory: &Dir) -> Result<()> {
    for name in [
        "config.json",
        "tokenizer_config.json",
        "preprocessor_config.json",
        "processor_config.json",
    ] {
        let mut file = match open_member(directory, name) {
            Ok(file) => file,
            Err(PumasError::Io {
                source: Some(error),
                ..
            }) if name == "processor_config.json"
                && error.kind() == std::io::ErrorKind::NotFound =>
            {
                continue
            }
            Err(error) => return Err(error),
        };
        let mut bytes = Vec::new();
        (&mut file)
            .take(MAX_DESCRIPTOR_BYTES + 1)
            .read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_DESCRIPTOR_BYTES {
            return Err(refusal(
                "native loader descriptor exceeds its supported bound",
            ));
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes)?;
        let object = value
            .as_object()
            .ok_or_else(|| refusal("native loader descriptor is not an object"))?;
        if object.contains_key("auto_map") || object.contains_key("custom_pipelines") {
            return Err(refusal("custom loader code is not supported"));
        }
        if name == "config.json"
            && (value["model_type"] != "cohere_asr"
                || value["architectures"]
                    != serde_json::json!(["CohereAsrForConditionalGeneration"]))
        {
            return Err(refusal("selected package is not native Cohere ASR"));
        }
    }
    Ok(())
}

fn manifest_sha256(prepared: &PreparedArtifactUse) -> String {
    let mut digest = Sha256::new();
    digest.update(b"pumas-selected-artifact-bytes-v1\0");
    for value in [&prepared.model_id, &prepared.selected_artifact_id] {
        digest.update((value.len() as u64).to_be_bytes());
        digest.update(value.as_bytes());
    }
    for member in &prepared.members {
        digest.update((member.manifest.relative_path.len() as u64).to_be_bytes());
        digest.update(member.manifest.relative_path.as_bytes());
        digest.update(member.manifest.size_bytes.to_be_bytes());
        digest.update(member.manifest.sha256.as_bytes());
    }
    hex::encode(digest.finalize())
}

fn make_held_directory_private_read_only(directory: &Dir) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        directory
            .try_clone()?
            .into_std_file()
            .set_permissions(std::fs::Permissions::from_mode(0o500))?;
    }
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

fn make_held_directory_private_writable(directory: &Dir) -> Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        directory
            .try_clone()?
            .into_std_file()
            .set_permissions(std::fs::Permissions::from_mode(0o700))?;
    }
    #[cfg(not(unix))]
    let _ = directory;
    Ok(())
}

fn refusal(message: &'static str) -> PumasError {
    PumasError::Validation {
        field: "artifact_use".into(),
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::ModelRecord;
    use std::collections::HashMap;
    use std::path::PathBuf;
    use std::sync::Arc;

    const MODEL_ID: &str = "audio/cohere/custody-fixture";
    const ARTIFACT_ID: &str = "cohere--custody-fixture__hf";

    struct Fixture {
        temp: TempDir,
        library: ModelLibrary,
        package: PathBuf,
        metadata: ModelMetadata,
    }

    impl Fixture {
        async fn new() -> Self {
            let temp = TempDir::new().unwrap();
            let library = ModelLibrary::new(temp.path().join("library"))
                .await
                .unwrap();
            let package = library.library_root().join(MODEL_ID);
            std::fs::create_dir_all(&package).unwrap();
            for name in REQUIRED_MEMBERS {
                let contents: &[u8] = match *name {
                    "config.json" => br#"{"model_type":"cohere_asr","architectures":["CohereAsrForConditionalGeneration"]}"#,
                    // Deliberately synthetic bytes: no tensors are parsed/loaded.
                    "model.safetensors" => b"synthetic selected bytes",
                    _ => b"{}",
                };
                std::fs::write(package.join(name), contents).unwrap();
            }
            let metadata = ModelMetadata {
                schema_version: Some(2),
                model_id: Some(MODEL_ID.into()),
                selected_artifact_id: Some(ARTIFACT_ID.into()),
                selected_artifact_files: Some(
                    REQUIRED_MEMBERS.iter().map(|name| (*name).into()).collect(),
                ),
                storage_kind: Some(StorageKind::LibraryOwned),
                validation_state: Some(AssetValidationState::Valid),
                ..Default::default()
            };
            let fixture = Self {
                temp,
                library,
                package,
                metadata,
            };
            fixture.publish_metadata();
            let root = DownloadDestinationRoot::open(fixture.library.library_root()).unwrap();
            let downloads = Arc::new(super::super::DownloadPersistence::new(
                &fixture.temp.path().join("downloads"),
            ));
            fixture
                .library
                .install_mutation_authority(crate::api::RuntimeTasks::new(), root, downloads)
                .unwrap();
            fixture
        }

        fn publish_metadata(&self) {
            let metadata = serde_json::to_value(&self.metadata).unwrap();
            std::fs::write(
                self.package.join("metadata.json"),
                serde_json::to_vec(&metadata).unwrap(),
            )
            .unwrap();
            self.library
                .index()
                .upsert(&ModelRecord {
                    id: MODEL_ID.into(),
                    path: self.package.display().to_string(),
                    cleaned_name: "custody-fixture".into(),
                    official_name: "Synthetic custody fixture".into(),
                    model_type: "audio".into(),
                    tags: vec![],
                    hashes: HashMap::new(),
                    metadata,
                    updated_at: "fixture".into(),
                })
                .unwrap();
        }

        fn prepare(&self) -> Result<PreparedArtifactUse> {
            self.library
                .prepare_cohere_artifact_use(MODEL_ID, ARTIFACT_ID)
        }

        fn assert_root_available(&self) {
            let root = self.library.mutation_authority().unwrap().root().clone();
            drop(root.try_acquire_execution_grant().unwrap());
        }
    }

    #[tokio::test]
    async fn copies_attest_actual_bytes_without_aliasing_originals_and_retain_root_exclusion() {
        let fixture = Fixture::new().await;
        let prepared = fixture.prepare().unwrap();
        assert_eq!(prepared.model_id(), MODEL_ID);
        assert_eq!(prepared.selected_artifact_id(), ARTIFACT_ID);
        assert_ne!(prepared.read_source_path(), fixture.package);
        assert_eq!(prepared.manifest().count(), REQUIRED_MEMBERS.len());
        assert_eq!(prepared.manifest_sha256().len(), 64);
        for entry in prepared.manifest() {
            let original = File::open(fixture.package.join(&entry.relative_path)).unwrap();
            let copied =
                File::open(prepared.read_source_path().join(&entry.relative_path)).unwrap();
            assert!(
                !same_file_identity(&original, &copied).unwrap(),
                "copy must not be a hardlink"
            );
            let bytes = std::fs::read(fixture.package.join(&entry.relative_path)).unwrap();
            assert_eq!(entry.size_bytes, bytes.len() as u64);
            assert_eq!(entry.sha256, hex::encode(Sha256::digest(bytes)));
            assert!(!std::fs::symlink_metadata(
                prepared.read_source_path().join(&entry.relative_path)
            )
            .unwrap()
            .file_type()
            .is_symlink());
        }
        prepared.validate_read_source().unwrap();
        assert!(matches!(
            fixture.prepare(),
            Err(PumasError::DownloadRootBusy)
        ));
        let copied_path = prepared.read_source_path().to_owned();
        drop(prepared);
        assert!(!copied_path.exists());
        fixture.assert_root_available();
    }

    #[tokio::test]
    async fn byte_manifest_changes_for_equal_length_edits_with_restored_mtime() {
        let fixture = Fixture::new().await;
        let first = fixture.prepare().unwrap();
        let original_digest = first.manifest_sha256().to_owned();
        let weight = fixture.package.join("model.safetensors");
        let original = std::fs::read(&weight).unwrap();
        let modified = std::fs::metadata(&weight).unwrap().modified().unwrap();
        std::fs::write(&weight, vec![b'x'; original.len()]).unwrap();
        File::options()
            .write(true)
            .open(&weight)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        first.validate_read_source().unwrap();
        assert_eq!(
            std::fs::read(first.read_source_path().join("model.safetensors")).unwrap(),
            original
        );
        assert_eq!(first.manifest_sha256(), original_digest);
        drop(first);
        let changed = fixture.prepare().unwrap();
        assert_ne!(
            changed.manifest_sha256(),
            original_digest,
            "actual bytes, not the package observation, own this digest"
        );
    }

    #[tokio::test]
    async fn every_required_member_must_be_selected_and_physically_present() {
        for member in REQUIRED_MEMBERS {
            let mut fixture = Fixture::new().await;
            fixture
                .metadata
                .selected_artifact_files
                .as_mut()
                .unwrap()
                .retain(|file| file != member);
            fixture.publish_metadata();
            assert!(
                fixture.prepare().is_err(),
                "unselected required member: {member}"
            );
            fixture.assert_root_available();

            let fixture = Fixture::new().await;
            std::fs::remove_file(fixture.package.join(member)).unwrap();
            assert!(
                fixture.prepare().is_err(),
                "missing required member: {member}"
            );
            fixture.assert_root_available();
        }
    }

    #[tokio::test]
    async fn refuses_unsupported_escaping_duplicate_and_unselected_optional_members() {
        for unsupported in [
            "../outside",
            "/outside",
            "nested/model.safetensors",
            "model.safetensors.index.json",
            "pytorch_model.bin",
            "model.safetensors",
        ] {
            let mut fixture = Fixture::new().await;
            fixture
                .metadata
                .selected_artifact_files
                .as_mut()
                .unwrap()
                .push(unsupported.into());
            fixture.publish_metadata();
            assert!(
                fixture.prepare().is_err(),
                "unsupported/duplicate member: {unsupported}"
            );
            fixture.assert_root_available();
        }
        let mut fixture = Fixture::new().await;
        std::fs::write(fixture.package.join("processor_config.json"), b"{}").unwrap();
        assert!(
            fixture.prepare().is_err(),
            "unselected processor data cannot be consumed"
        );
        fixture
            .metadata
            .selected_artifact_files
            .as_mut()
            .unwrap()
            .push("processor_config.json".into());
        fixture.publish_metadata();
        let prepared = fixture.prepare().unwrap();
        assert_eq!(prepared.manifest().count(), REQUIRED_MEMBERS.len() + 1);
    }

    #[tokio::test]
    async fn refuses_non_native_and_custom_code_descriptors_without_stranding_exclusion() {
        for (member, bytes) in [
            (
                "config.json",
                br#"{"model_type":"other","architectures":[]}"#.as_slice(),
            ),
            (
                "tokenizer_config.json",
                br#"{"auto_map":{"AutoTokenizer":"untrusted.Loader"}}"#.as_slice(),
            ),
            (
                "preprocessor_config.json",
                br#"{"custom_pipelines":{}}"#.as_slice(),
            ),
            ("config.json", b"not json".as_slice()),
        ] {
            let fixture = Fixture::new().await;
            std::fs::write(fixture.package.join(member), bytes).unwrap();
            assert!(fixture.prepare().is_err());
            fixture.assert_root_available();
        }
    }

    #[tokio::test]
    async fn selectors_require_canonical_indexed_owned_selection_and_configured_authority() {
        let fixture = Fixture::new().await;
        for model_id in ["../outside", "/outside", "", "audio/missing"] {
            assert!(fixture
                .library
                .prepare_cohere_artifact_use(model_id, ARTIFACT_ID)
                .is_err());
        }
        assert!(fixture
            .library
            .prepare_cohere_artifact_use(MODEL_ID, "wrong-artifact")
            .is_err());
        let mut indexed = fixture.library.index().get(MODEL_ID).unwrap().unwrap();
        indexed.metadata["selected_artifact_id"] = serde_json::json!("different-selection");
        fixture.library.index().upsert(&indexed).unwrap();
        assert!(fixture.prepare().is_err());
        fixture.publish_metadata();
        indexed = fixture.library.index().get(MODEL_ID).unwrap().unwrap();
        indexed.path = fixture.temp.path().join("outside").display().to_string();
        fixture.library.index().upsert(&indexed).unwrap();
        assert!(fixture.prepare().is_err());
        fixture.publish_metadata();
        let mut indexed = fixture.library.index().get(MODEL_ID).unwrap().unwrap();
        indexed.metadata["storage_kind"] = serde_json::json!("external_reference");
        fixture.library.index().upsert(&indexed).unwrap();
        assert!(fixture.prepare().is_err());
        fixture.assert_root_available();
        let standalone = ModelLibrary::new(fixture.temp.path().join("unconfigured"))
            .await
            .unwrap();
        assert!(standalone
            .prepare_cohere_artifact_use(MODEL_ID, ARTIFACT_ID)
            .is_err());
    }

    fn make_copy_writable(path: &Path) {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600)).unwrap();
        }
        #[cfg(not(unix))]
        {
            let mut permissions = std::fs::metadata(path).unwrap().permissions();
            permissions.set_readonly(false);
            std::fs::set_permissions(path, permissions).unwrap();
        }
    }

    #[tokio::test]
    async fn copied_source_revalidation_refuses_changed_bytes_and_member_additions() {
        let fixture = Fixture::new().await;
        let prepared = fixture.prepare().unwrap();
        let weight = prepared.read_source_path().join("model.safetensors");
        make_copy_writable(&weight);
        let modified = std::fs::metadata(&weight).unwrap().modified().unwrap();
        let length = std::fs::metadata(&weight).unwrap().len() as usize;
        std::fs::write(&weight, vec![b'x'; length]).unwrap();
        File::options()
            .write(true)
            .open(&weight)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(modified))
            .unwrap();
        assert!(
            prepared.validate_read_source().is_err(),
            "equal-size/mtime edit escaped SHA256 validation"
        );
        drop(prepared);
        let prepared = fixture.prepare().unwrap();
        make_held_directory_private_writable(&prepared.directory).unwrap();
        std::fs::write(prepared.read_source_path().join("unselected.json"), b"{}").unwrap();
        assert!(prepared.validate_read_source().is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn source_symlink_escape_and_copied_member_or_directory_replacement_are_refused() {
        let fixture = Fixture::new().await;
        let outside = fixture.temp.path().join("outside");
        std::fs::write(&outside, b"outside selected bytes").unwrap();
        let weight = fixture.package.join("model.safetensors");
        let bytes = std::fs::read(&weight).unwrap();
        std::fs::remove_file(&weight).unwrap();
        std::os::unix::fs::symlink(&outside, &weight).unwrap();
        assert!(fixture.prepare().is_err());
        std::fs::remove_file(&weight).unwrap();
        std::fs::write(&weight, bytes).unwrap();

        let prepared = fixture.prepare().unwrap();
        let copied = prepared.read_source_path().join("model.safetensors");
        let bytes = std::fs::read(&copied).unwrap();
        make_held_directory_private_writable(&prepared.directory).unwrap();
        std::fs::rename(&copied, fixture.temp.path().join("old-copy")).unwrap();
        std::fs::write(&copied, &bytes).unwrap();
        assert!(
            prepared.validate_read_source().is_err(),
            "equal bytes cannot replace the held file identity"
        );
        drop(prepared);

        let prepared = fixture.prepare().unwrap();
        let path = prepared.read_source_path().to_owned();
        let old = fixture.temp.path().join("old-read-source");
        // Simulate a same-user actor defeating advisory read-only permissions.
        make_held_directory_private_writable(&prepared.directory).unwrap();
        std::fs::rename(&path, &old).unwrap();
        std::fs::create_dir(&path).unwrap();
        std::fs::write(path.join("replacement-sentinel"), b"preserve").unwrap();
        assert!(prepared.validate_read_source().is_err());
        drop(prepared);
        assert_eq!(
            std::fs::read(path.join("replacement-sentinel")).unwrap(),
            b"preserve"
        );
        std::fs::remove_dir_all(path).unwrap();
    }

    #[tokio::test]
    async fn distinct_physical_roots_keep_independent_custody_and_exclusion() {
        let first = Fixture::new().await;
        let second = Fixture::new().await;
        let first_use = first.prepare().unwrap();
        let second_use = second.prepare().unwrap();
        assert_eq!(
            first_use.manifest_sha256(),
            second_use.manifest_sha256(),
            "byte digest is not root custody identity"
        );
        assert_ne!(first_use.read_source_path(), second_use.read_source_path());
        assert!(matches!(first.prepare(), Err(PumasError::DownloadRootBusy)));
        assert!(matches!(
            second.prepare(),
            Err(PumasError::DownloadRootBusy)
        ));
        drop(first_use);
        first.assert_root_available();
        assert!(matches!(
            second.prepare(),
            Err(PumasError::DownloadRootBusy)
        ));
        second_use.validate_read_source().unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn root_replacement_cannot_rebind_retained_or_new_custody() {
        let fixture = Fixture::new().await;
        let prepared = fixture.prepare().unwrap();
        let root = fixture.library.library_root();
        let original_root = fixture.temp.path().join("original-root");
        std::fs::rename(root, &original_root).unwrap();
        std::fs::create_dir(root).unwrap();
        assert!(prepared.validate_read_source().is_err());
        assert!(fixture.prepare().is_err());
        std::fs::remove_dir(root).unwrap();
        std::fs::rename(original_root, root).unwrap();
        prepared.validate_read_source().unwrap();
    }
}
