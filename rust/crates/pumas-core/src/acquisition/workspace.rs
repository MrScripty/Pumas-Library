//! Held, source-neutral filesystem authority. Persisted identity never opens it.
#![deny(unsafe_code)]

use super::{ArtifactFile, ArtifactManifest};
use crate::platform::capability_fs::sync_directory;
use crate::{PumasError, Result};
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::{Dir, Metadata, OpenOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, Weak};

/// Equality-only workspace locator. It conveys no filesystem permission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorkspaceIdentity {
    pub root_identity: String,
    pub relative_target: String,
}

/// Bounded verification receipt tied to a held regular file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct VerifiedFile {
    pub path: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Identity(u64, u64);

fn identity(metadata: &Metadata) -> Result<Identity> {
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        Ok(Identity(metadata.dev(), metadata.ino()))
    }
    #[cfg(windows)]
    {
        use cap_primitives::fs::_WindowsByHandle;
        Ok(Identity(
            u64::from(metadata.volume_serial_number().ok_or_else(changed)?),
            metadata.file_index().ok_or_else(changed)?,
        ))
    }
    #[cfg(not(any(unix, windows)))]
    {
        let _ = metadata;
        Err(changed())
    }
}

fn changed() -> PumasError {
    PumasError::Validation {
        field: "acquisition.workspace".into(),
        message: "Held acquisition workspace or file binding changed".into(),
    }
}

fn options() -> OpenOptions {
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    #[cfg(windows)]
    {
        use cap_std::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    options
}

struct HeldParent {
    directory: Dir,
    binding: Identity,
}

struct HeldWorkspace {
    directory: Dir,
    binding: Identity,
    validate: Box<dyn Fn() -> Result<()> + Send + Sync>,
    _execution_lease: Arc<dyn Send + Sync>,
    parents: Mutex<BTreeMap<PathBuf, Arc<HeldParent>>>,
    sealed: Mutex<bool>,
}

/// A runtime grant captured from an already-held, exclusively reserved directory.
/// Its validator must retain and revalidate the caller's root/execution grant.
/// It never opens a directory from its persisted or displayed locator.
#[derive(Clone)]
pub struct AcquisitionWorkspace {
    held: Arc<HeldWorkspace>,
    locator: WorkspaceIdentity,
}

/// A checkpoint must not keep a reservation alive after its owner drops it.
#[derive(Clone)]
pub(crate) struct WorkspaceCheckpointOwner(Weak<HeldWorkspace>);

impl WorkspaceCheckpointOwner {
    pub(crate) fn matches(&self, workspace: &AcquisitionWorkspace) -> bool {
        self.0.ptr_eq(&Arc::downgrade(&workspace.held)) && self.0.upgrade().is_some()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct PartialPrefix {
    binding: Identity,
    pub(crate) bytes: u64,
    pub(crate) sha256: String,
}

impl AcquisitionWorkspace {
    /// Reconstruct the equality-only identity for a reserved child without
    /// opening or creating that child. Consumers use this after durable
    /// adoption, when owned input cleanup may already have removed it.
    pub fn identity_for_reserved_directory(
        root: &Path,
        relative_target: &Path,
    ) -> Result<WorkspaceIdentity> {
        let relative = normalized_relative_path(relative_target)?;
        let root_directory = crate::platform::capability_fs::open_directory(root)?;
        let root_binding = identity(&root_directory.dir_metadata()?)?;
        Ok(WorkspaceIdentity {
            root_identity: format!("{:x}:{:x}", root_binding.0, root_binding.1),
            relative_target: relative,
        })
    }

    /// Open a previously reserved child of a consumer-owned root.
    ///
    /// `execution_lease` retains the consumer's reservation for the root, and
    /// `validate_root` rechecks its authority on every workspace operation.
    /// The locator is equality-only; the held directory capability, not the
    /// serialized locator, authorizes reads and writes.
    pub fn from_reserved_directory(
        root: &Path,
        relative_target: &Path,
        execution_lease: Arc<dyn Send + Sync>,
        validate_root: impl Fn() -> Result<()> + Send + Sync + 'static,
    ) -> Result<Self> {
        let relative = normalized_relative_path(relative_target)?;
        validate_root()?;
        let root_directory = crate::platform::capability_fs::open_directory(root)?;
        let root_binding = identity(&root_directory.dir_metadata()?)?;
        let directory = open_relative_directory(&root_directory, relative_target)?;
        let held_root = root_directory.try_clone()?;
        let validate_root = move || {
            validate_root()?;
            if identity(&held_root.dir_metadata()?)? != root_binding {
                return Err(changed());
            }
            Ok(())
        };
        let locator = WorkspaceIdentity {
            root_identity: format!("{:x}:{:x}", root_binding.0, root_binding.1),
            relative_target: relative,
        };
        Self::from_capability(directory, locator, execution_lease, validate_root)
    }

    pub(crate) fn from_capability(
        directory: Dir,
        locator: WorkspaceIdentity,
        execution_lease: Arc<dyn Send + Sync>,
        validate: impl Fn() -> Result<()> + Send + Sync + 'static,
    ) -> Result<Self> {
        validate()?;
        let binding = identity(&directory.dir_metadata()?)?;
        Ok(Self {
            held: Arc::new(HeldWorkspace {
                directory,
                binding,
                validate: Box::new(validate),
                _execution_lease: execution_lease,
                parents: Mutex::new(BTreeMap::new()),
                sealed: Mutex::new(false),
            }),
            locator,
        })
    }

    pub(crate) fn checkpoint_owner(&self) -> WorkspaceCheckpointOwner {
        WorkspaceCheckpointOwner(Arc::downgrade(&self.held))
    }

    pub fn identity(&self) -> &WorkspaceIdentity {
        &self.locator
    }

    pub(crate) fn validate(&self) -> Result<()> {
        (self.held.validate)()?;
        if identity(&self.held.directory.dir_metadata()?)? != self.held.binding {
            return Err(changed());
        }
        Ok(())
    }

    fn parent(&self, path: &str, create: bool) -> Result<(Dir, String)> {
        self.validate()?;
        let path = Path::new(path);
        if path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
        {
            return Err(changed());
        }
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(changed)?;
        let mut directory = self.held.directory.try_clone()?;
        let mut relative = PathBuf::new();
        for component in path.parent().unwrap_or(Path::new("")).components() {
            let Component::Normal(component) = component else {
                return Err(changed());
            };
            relative.push(component);
            let known = self
                .held
                .parents
                .lock()
                .map_err(|_| changed())?
                .get(&relative)
                .cloned();
            if create && known.is_none() {
                match directory.create_dir(component) {
                    Ok(()) => sync_directory(&directory)?,
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => return Err(error.into()),
                }
            }
            let metadata = directory.symlink_metadata(component)?;
            if !metadata.is_dir() || metadata.is_symlink() {
                return Err(changed());
            }
            let next = directory.open_dir(component)?;
            let binding = identity(&next.dir_metadata()?)?;
            if identity(&metadata)? != binding
                || known.as_ref().is_some_and(|held| held.binding != binding)
            {
                return Err(changed());
            }
            let candidate = Arc::new(HeldParent {
                directory: next,
                binding,
            });
            let held = {
                let mut parents = self.held.parents.lock().map_err(|_| changed())?;
                parents.entry(relative.clone()).or_insert(candidate).clone()
            };
            if held.binding != binding {
                return Err(changed());
            }
            directory = held.directory.try_clone()?;
        }
        self.validate()?;
        Ok((directory, name.to_owned()))
    }

    fn require_writable(&self) -> Result<()> {
        if *self.held.sealed.lock().map_err(|_| changed())? {
            return Err(changed());
        }
        self.validate()
    }

    pub(crate) fn prepare_file(&self, file: &ArtifactFile) -> Result<()> {
        self.require_writable()?;
        self.parent(file.logical_path(), true).map(|_| ())
    }

    pub(crate) fn file_len(&self, path: &str, partial: bool) -> Result<Option<u64>> {
        let (parent, name) = self.parent(path, false)?;
        let name = if partial {
            format!("{name}.part")
        } else {
            name
        };
        match parent.symlink_metadata(name) {
            Ok(metadata) if metadata.is_file() && !metadata.is_symlink() => {
                Ok(Some(metadata.len()))
            }
            Ok(_) => Err(changed()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }

    pub(crate) fn open_part(&self, path: &str, append: bool) -> Result<std::fs::File> {
        self.require_writable()?;
        let (parent, name) = self.parent(path, true)?;
        let mut options = options();
        options.read(true);
        if append {
            options.append(true);
        } else {
            options.write(true).create(true);
        }
        let file = parent
            .open_with(format!("{name}.part"), &options)?
            .into_std();
        if !file.metadata()?.is_file() {
            return Err(changed());
        }
        if !append {
            file.set_len(0)?;
        }
        self.validate()?;
        Ok(file)
    }

    /// Check through the held parent and keep this exact descriptor for append.
    pub(crate) fn open_checkpoint_part(
        &self,
        path: &str,
        expected: &PartialPrefix,
    ) -> Result<(std::fs::File, Sha256)> {
        let mut handle = self.open_part(path, true)?;
        let hash = self.validate_checkpoint_part(path, &mut handle, expected)?;
        Ok((handle, hash))
    }

    /// Recheck after the network wait without opening a new write descriptor.
    pub(crate) fn validate_checkpoint_part(
        &self,
        path: &str,
        handle: &mut std::fs::File,
        expected: &PartialPrefix,
    ) -> Result<Sha256> {
        let (observed, hash) = self.observe_part(path, handle)?;
        if &observed != expected {
            return Err(changed());
        }
        Ok(hash)
    }

    /// Observe a flushed prefix. Callers compare its digest to the bytes they
    /// actually streamed before admitting it as a continuation checkpoint.
    pub(crate) fn observe_part(
        &self,
        path: &str,
        handle: &mut std::fs::File,
    ) -> Result<(PartialPrefix, Sha256)> {
        self.require_writable()?;
        let (parent, name) = self.parent(path, false)?;
        let before = Metadata::from_file(handle)?;
        if !before.is_file() {
            return Err(changed());
        }
        let binding = identity(&before)?;
        handle.seek(SeekFrom::Start(0))?;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        let mut bytes = 0_u64;
        loop {
            let read = handle.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            bytes = bytes.checked_add(read as u64).ok_or_else(changed)?;
            hash.update(&buffer[..read]);
        }
        let after = Metadata::from_file(handle)?;
        let mut read_options = options();
        read_options.read(true);
        let current = Metadata::from_file(
            &parent
                .open_with(format!("{name}.part"), &read_options)?
                .into_std(),
        )?;
        if identity(&after)? != binding
            || identity(&current)? != binding
            || before.len() != bytes
            || after.len() != bytes
            || current.len() != bytes
            || before.modified()? != after.modified()?
            || before.modified()? != current.modified()?
        {
            return Err(changed());
        }
        self.validate()?;
        handle.seek(SeekFrom::End(0))?;
        Ok((
            PartialPrefix {
                binding,
                bytes,
                sha256: hex::encode(hash.clone().finalize()),
            },
            hash,
        ))
    }

    pub(crate) fn remove_part(&self, path: &str) -> Result<()> {
        self.require_writable()?;
        let (parent, name) = self.parent(path, false)?;
        match parent.remove_file(format!("{name}.part")) {
            Ok(()) => sync_directory(&parent)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        self.validate()
    }

    pub(crate) fn verify_file(&self, file: &ArtifactFile, partial: bool) -> Result<VerifiedFile> {
        let (parent, name) = self.parent(file.logical_path(), false)?;
        let name = if partial {
            format!("{name}.part")
        } else {
            name
        };
        let mut options = options();
        options.read(true);
        let mut handle = parent.open_with(&name, &options)?.into_std();
        let before = Metadata::from_file(&handle)?;
        if !before.is_file()
            || file
                .expected_size()
                .is_some_and(|size| size != before.len())
        {
            return Err(changed());
        }
        let binding = identity(&before)?;
        let mut hash = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let bytes = handle.read(&mut buffer)?;
            if bytes == 0 {
                break;
            }
            hash.update(&buffer[..bytes]);
        }
        let digest = hex::encode(hash.finalize());
        let after = Metadata::from_file(&handle)?;
        let (current_parent, _) = self.parent(file.logical_path(), false)?;
        let current = Metadata::from_file(&current_parent.open_with(&name, &options)?.into_std())?;
        if identity(&after)? != binding
            || identity(&current)? != binding
            || before.len() != after.len()
            || before.len() != current.len()
            || before.modified()? != after.modified()?
            || before.modified()? != current.modified()?
        {
            return Err(changed());
        }
        if let Some(expected) = file.expected_sha256() {
            if expected.value() != digest {
                return Err(PumasError::HashMismatch {
                    expected: expected.value().into(),
                    actual: digest,
                });
            }
        }
        handle.sync_all()?;
        sync_directory(&parent)?;
        self.validate()?;
        Ok(VerifiedFile {
            path: file.logical_path().into(),
            bytes: before.len(),
            sha256: digest,
        })
    }

    /// Open the exact verified file through the held directory capability.
    /// Hashing and identity checks use the returned file handle itself, so a
    /// consumer can move it to a blocking worker without reopening a path.
    pub(crate) fn open_verified_readonly(
        &self,
        file: &ArtifactFile,
        expected: &VerifiedFile,
    ) -> Result<std::fs::File> {
        if expected.path != file.logical_path() {
            return Err(changed());
        }
        self.validate()?;
        let (parent, name) = self.parent(file.logical_path(), false)?;
        let mut open_options = options();
        open_options.read(true);
        let mut handle = parent.open_with(&name, &open_options)?.into_std();
        let before = Metadata::from_file(&handle)?;
        if !before.is_file() || before.len() != expected.bytes {
            return Err(changed());
        }
        let binding = identity(&before)?;
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = handle.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let digest = hex::encode(hasher.finalize());
        let after = Metadata::from_file(&handle)?;
        let current = Metadata::from_file(&parent.open_with(&name, &open_options)?.into_std())?;
        if digest != expected.sha256
            || file
                .expected_sha256()
                .is_some_and(|selected| selected.value() != digest)
            || identity(&after)? != binding
            || identity(&current)? != binding
            || before.len() != after.len()
            || before.len() != current.len()
            || before.modified()? != after.modified()?
            || before.modified()? != current.modified()?
        {
            return Err(changed());
        }
        handle.seek(SeekFrom::Start(0))?;
        self.validate()?;
        Ok(handle)
    }

    pub(crate) fn publish_part(
        &self,
        file: &ArtifactFile,
        compare_existing: bool,
    ) -> Result<VerifiedFile> {
        self.require_writable()?;
        let staged = self.verify_file(file, true)?;
        let (parent, name) = self.parent(file.logical_path(), false)?;
        if compare_existing {
            let current = self.verify_file(file, false)?;
            if current != staged {
                return Err(PumasError::Validation {
                    field: "download.integrity".into(),
                    message: format!("Existing selected file {} differs from the immutable source artifact; preserving both files for reconciliation", file.logical_path()),
                });
            }
            self.remove_part(file.logical_path())?;
        } else {
            if self.file_len(file.logical_path(), false)?.is_some() {
                return Err(changed());
            }
            parent.rename(format!("{name}.part"), &parent, name)?;
            sync_directory(&parent)?;
        }
        self.verify_file(file, false)
    }

    pub(crate) fn seal(&self, manifest: &ArtifactManifest) -> Result<Vec<VerifiedFile>> {
        self.validate()?;
        *self.held.sealed.lock().map_err(|_| changed())? = true;
        manifest
            .files()
            .iter()
            .map(|file| self.verify_file(file, false))
            .collect()
    }

    pub(crate) fn verify_receipts(
        &self,
        manifest: &ArtifactManifest,
        receipts: &[VerifiedFile],
    ) -> Result<()> {
        let observed: Vec<_> = manifest
            .files()
            .iter()
            .map(|file| self.verify_file(file, false))
            .collect::<Result<_>>()?;
        if observed != receipts {
            return Err(changed());
        }
        Ok(())
    }
}

fn normalized_relative_path(path: &Path) -> Result<String> {
    let mut components = Vec::new();
    for component in path.components() {
        let Component::Normal(part) = component else {
            return Err(changed());
        };
        let part = part.to_str().ok_or_else(changed)?;
        if part.is_empty() {
            return Err(changed());
        }
        components.push(part);
    }
    if components.is_empty() {
        return Err(changed());
    }
    Ok(components.join("/"))
}

fn open_relative_directory(root: &Dir, relative: &Path) -> Result<Dir> {
    let mut current = root.try_clone()?;
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return Err(changed());
        };
        let metadata = current.symlink_metadata(component)?;
        if !metadata.is_dir() || metadata.is_symlink() {
            return Err(changed());
        }
        let next = current.open_dir(component)?;
        if identity(&metadata)? != identity(&next.dir_metadata()?)? {
            return Err(changed());
        }
        current = next;
    }
    Ok(current)
}

pub(crate) fn write_chunk(file: &mut std::fs::File, bytes: &[u8]) -> Result<()> {
    file.write_all(bytes)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::acquisition::FileVerificationRequirement;

    #[test]
    fn identity_only_lookup_matches_a_reserved_child_without_creating_it() {
        let temp = tempfile::TempDir::new().unwrap();
        let relative = Path::new(".native-attempt");
        let identity =
            AcquisitionWorkspace::identity_for_reserved_directory(temp.path(), relative).unwrap();
        assert_eq!(identity.relative_target, ".native-attempt");
        assert!(!temp.path().join(relative).exists());

        std::fs::create_dir(temp.path().join(relative)).unwrap();
        let workspace = AcquisitionWorkspace::from_reserved_directory(
            temp.path(),
            relative,
            Arc::new(()),
            || Ok(()),
        )
        .unwrap();
        assert_eq!(workspace.identity(), &identity);
    }

    #[test]
    fn checkpoint_descriptor_rejects_mutated_and_replaced_prefixes() {
        let temp = tempfile::TempDir::new().unwrap();
        let workspace = AcquisitionWorkspace::from_capability(
            crate::platform::capability_fs::open_directory(temp.path()).unwrap(),
            WorkspaceIdentity {
                root_identity: "fixture".into(),
                relative_target: "stage".into(),
            },
            Arc::new(()),
            || Ok(()),
        )
        .unwrap();
        let mut initial = workspace.open_part("payload.bin", false).unwrap();
        initial.write_all(b"DATA").unwrap();
        initial.sync_all().unwrap();
        let (prefix, _) = workspace.observe_part("payload.bin", &mut initial).unwrap();
        let (mut checked, _) = workspace
            .open_checkpoint_part("payload.bin", &prefix)
            .unwrap();
        std::fs::write(temp.path().join("payload.bin.part"), b"EVIL").unwrap();
        assert!(workspace
            .validate_checkpoint_part("payload.bin", &mut checked, &prefix)
            .is_err());
        assert!(workspace
            .open_checkpoint_part("payload.bin", &prefix)
            .is_err());
        std::fs::write(temp.path().join("payload.bin.part"), b"DATA").unwrap();
        std::fs::rename(
            temp.path().join("payload.bin.part"),
            temp.path().join("retired"),
        )
        .unwrap();
        std::fs::write(temp.path().join("payload.bin.part"), b"DATA").unwrap();
        assert!(workspace
            .validate_checkpoint_part("payload.bin", &mut checked, &prefix)
            .is_err());
        assert!(workspace
            .open_checkpoint_part("payload.bin", &prefix)
            .is_err());
        assert_eq!(
            std::fs::read(temp.path().join("payload.bin.part")).unwrap(),
            b"DATA"
        );
    }

    #[test]
    fn checkpoint_owner_does_not_match_reopened_workspace_or_retain_lease() {
        let temp = tempfile::TempDir::new().unwrap();
        let lease = Arc::new(());
        let make = || {
            AcquisitionWorkspace::from_capability(
                crate::platform::capability_fs::open_directory(temp.path()).unwrap(),
                WorkspaceIdentity {
                    root_identity: "fixture".into(),
                    relative_target: "stage".into(),
                },
                lease.clone(),
                || Ok(()),
            )
            .unwrap()
        };
        let first = make();
        let owner = first.checkpoint_owner();
        assert!(owner.matches(&first.clone()));
        let reopened = make();
        assert_eq!(first.identity(), reopened.identity());
        assert!(!owner.matches(&reopened));
        drop(first);
        assert_eq!(Arc::strong_count(&lease), 2);
        assert!(!owner.matches(&reopened));
    }

    #[test]
    fn concurrent_parent_creation_is_coalesced_and_replacement_is_refused() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = crate::platform::capability_fs::open_directory(temp.path()).unwrap();
        let workspace = AcquisitionWorkspace::from_capability(
            root,
            WorkspaceIdentity {
                root_identity: "physical-fixture-root".into(),
                relative_target: "stage".into(),
            },
            Arc::new(()),
            || Ok(()),
        )
        .unwrap();
        let file = ArtifactFile::new(
            "nested/payload.bin",
            "payload",
            Some(4),
            None,
            FileVerificationRequirement::SizeAndImmutableRevision,
        )
        .unwrap();
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let handles: Vec<_> = (0..2)
            .map(|_| {
                let grant = workspace.clone();
                let selected = file.clone();
                let ready = barrier.clone();
                std::thread::spawn(move || {
                    ready.wait();
                    grant.prepare_file(&selected)
                })
            })
            .collect();
        barrier.wait();
        for handle in handles {
            handle.join().unwrap().unwrap();
        }
        std::fs::rename(temp.path().join("nested"), temp.path().join("retired")).unwrap();
        std::fs::create_dir(temp.path().join("nested")).unwrap();
        assert!(workspace.open_part("nested/payload.bin", false).is_err());
        assert!(!temp.path().join("nested/payload.bin.part").exists());
        assert!(!temp.path().join("retired/payload.bin.part").exists());
    }
}
