//! Held, source-neutral filesystem authority. Persisted identity never opens it.
#![deny(unsafe_code)]

use super::manifest::staging_path;
use super::{ArtifactFile, ArtifactManifest};
use crate::platform::capability_fs::{
    open_pinned_directory, open_pinned_directory_at, sync_directory,
};
use crate::{PumasError, Result};
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::{Dir, Metadata, OpenOptions};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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
    parents: Arc<Mutex<BTreeMap<PathBuf, Arc<HeldParent>>>>,
    sealed: Mutex<bool>,
}

/// Equality-only physical binding persisted by a reservation owner. Deserializing
/// this value never grants access; reopening must capture and compare a new grant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReservedDirectoryBinding {
    root: Identity,
    components: Vec<Identity>,
}

impl ReservedDirectoryBinding {
    /// Compare the current root for diagnostic persistence, without authorizing
    /// any child access or cleanup from this serialized value.
    pub fn matches_root(&self, root: &Path) -> Result<bool> {
        Ok(identity(&open_pinned_directory(root)?.dir_metadata()?)? == self.root)
    }
}

struct ReservedComponent {
    name: std::ffi::OsString,
    directory: Dir,
}

struct Reservation {
    root_path: PathBuf,
    root: Dir,
    components: Vec<ReservedComponent>,
    binding: ReservedDirectoryBinding,
    locator: WorkspaceIdentity,
    validate_owner: Box<dyn Fn() -> Result<()> + Send + Sync>,
    lease: Arc<dyn Send + Sync>,
    revoked: AtomicBool,
    parents: Arc<Mutex<BTreeMap<PathBuf, Arc<HeldParent>>>>,
}

impl Reservation {
    fn validate_root(&self) -> Result<()> {
        (self.validate_owner)()?;
        let current = open_pinned_directory(&self.root_path)?;
        if identity(&current.dir_metadata()?)? != self.binding.root {
            return Err(changed());
        }
        Ok(())
    }

    fn validate(&self) -> Result<()> {
        self.validate_root()?;
        let mut parent = &self.root;
        for (component, expected) in self.components.iter().zip(&self.binding.components) {
            let current = open_pinned_directory_at(parent, &component.name)?;
            if identity(&current.dir_metadata()?)? != *expected {
                return Err(changed());
            }
            parent = &component.directory;
        }
        Ok(())
    }

    fn directory(&self) -> &Dir {
        &self
            .components
            .last()
            .expect("nonempty reserved path")
            .directory
    }
}

/// One opaque reservation shared by acquisition and native cleanup.
///
/// The owner must serialize cooperating writers and settle all effects before
/// cleanup. Observed replacement is refused. This is not a defense against a
/// hostile concurrent writer with the same filesystem authority; Unix directory
/// unlink remains name-based. Windows handles pin directories during traversal.
#[derive(Clone)]
pub struct ReservedDirectory(Arc<Reservation>);

impl ReservedDirectory {
    /// Capture an existing child once, without creating or adopting missing work.
    pub fn capture(
        root: &Path,
        relative_target: &Path,
        lease: Arc<dyn Send + Sync>,
        validate_owner: impl Fn() -> Result<()> + Send + Sync + 'static,
    ) -> Result<Self> {
        let relative = normalized_relative_path(relative_target)?;
        validate_owner()?;
        let root_path = std::path::absolute(root)?;
        let root = open_pinned_directory(&root_path)?;
        let root_binding = identity(&root.dir_metadata()?)?;
        let mut parent = root.try_clone()?;
        let mut components = Vec::new();
        let mut bindings = Vec::new();
        for component in relative_target.components() {
            let Component::Normal(name) = component else {
                return Err(changed());
            };
            let directory = open_pinned_directory_at(&parent, name)?;
            bindings.push(identity(&directory.dir_metadata()?)?);
            parent = directory.try_clone()?;
            components.push(ReservedComponent {
                name: name.to_owned(),
                directory,
            });
        }
        let grant = Self(Arc::new(Reservation {
            root_path,
            root,
            components,
            binding: ReservedDirectoryBinding {
                root: root_binding,
                components: bindings,
            },
            locator: WorkspaceIdentity {
                root_identity: format!("{:x}:{:x}", root_binding.0, root_binding.1),
                relative_target: relative,
            },
            validate_owner: Box::new(validate_owner),
            lease,
            revoked: AtomicBool::new(false),
            parents: Arc::new(Mutex::new(BTreeMap::new())),
        }));
        grant.validate()?;
        Ok(grant)
    }

    pub fn workspace_identity(&self) -> &WorkspaceIdentity {
        &self.0.locator
    }

    pub fn binding(&self) -> &ReservedDirectoryBinding {
        &self.0.binding
    }

    /// Check the root before updating its owner's durable cleanup diagnostic.
    pub fn validate_root(&self) -> Result<()> {
        self.0.validate_root()
    }

    pub fn validate(&self) -> Result<()> {
        self.0.validate()
    }

    /// Clone the same authority, including revocation and observed descendants.
    pub fn acquisition_workspace(&self) -> Result<AcquisitionWorkspace> {
        let reservation = self.clone();
        let mut workspace = AcquisitionWorkspace::from_capability(
            self.0.directory().try_clone()?,
            self.0.locator.clone(),
            self.0.lease.clone(),
            move || {
                if reservation.0.revoked.load(Ordering::SeqCst) {
                    return Err(changed());
                }
                reservation.validate()
            },
        )?;
        // Newly constructed, so no workspace clone can exist yet.
        Arc::get_mut(&mut workspace.held)
            .expect("new workspace")
            .parents = self.0.parents.clone();
        Ok(workspace)
    }

    /// Revoke new use and clear through held directories after effects settle.
    /// Failure retains the reservation and must not authorize use withdrawal.
    pub fn clear_contents(&self) -> Result<()> {
        self.0.revoked.store(true, Ordering::SeqCst);
        self.validate()?;
        let observed = self.0.parents.lock().map_err(|_| changed())?;
        // Refuse all observed replacements before deleting any content.
        for (relative, expected) in observed.iter() {
            let current = open_relative_directory(self.0.directory(), relative)?;
            if identity(&current.dir_metadata()?)? != expected.binding {
                return Err(changed());
            }
        }
        drop(observed);
        self.clear_directory(self.0.directory(), Path::new(""))?;
        sync_directory(self.0.directory())?;
        self.validate()
    }

    fn clear_directory(&self, directory: &Dir, relative: &Path) -> Result<()> {
        for entry in directory.entries()? {
            self.validate()?;
            let entry = entry?;
            let name = entry.file_name();
            let metadata = directory.symlink_metadata(&name)?;
            if metadata.is_dir() && !metadata.is_symlink() {
                let child = open_pinned_directory_at(directory, &name)?;
                let expected = identity(&child.dir_metadata()?)?;
                if identity(&metadata)? != expected {
                    return Err(changed());
                }
                let child_relative = relative.join(&name);
                self.clear_directory(&child, &child_relative)?;
                sync_directory(&child)?;
                self.validate()?;
                if identity(&directory.symlink_metadata(&name)?)? != expected {
                    return Err(changed());
                }
                // Settled use handles are revoked. Release this directory's
                // acquisition pin only at its final empty unlink, not before
                // traversal (other observed descendants remain pinned).
                self.0
                    .parents
                    .lock()
                    .map_err(|_| changed())?
                    .remove(&child_relative);
                drop(child);
                // Empty-only removal. Never resolve recursive deletion by name.
                directory.remove_dir(&name)?;
            } else {
                // Unix unlinks symlinks without following their targets. Do
                // not guess Windows reparse-point deletion semantics.
                #[cfg(windows)]
                if metadata.is_symlink() {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "Windows staging reparse point retained; reconciliation required",
                    )
                    .into());
                }
                directory.remove_file(&name)?;
            }
        }
        sync_directory(directory)?;
        Ok(())
    }

    /// Remove only the empty shell, after every acquisition clone has dropped.
    /// The parent's execution lease remains held across final unlink and sync.
    /// On Unix the final compare/unlink pair is bounded by the caller's writer
    /// lock, not atomic against arbitrary same-authority filesystem mutation.
    pub fn remove_empty(self) -> Result<()> {
        let mut reservation = Arc::try_unwrap(self.0).map_err(|_| changed())?;
        if !reservation.revoked.load(Ordering::SeqCst) {
            return Err(changed());
        }
        reservation.validate()?;
        if reservation
            .directory()
            .entries()?
            .next()
            .transpose()?
            .is_some()
        {
            return Err(changed());
        }
        let leaf = reservation.components.pop().ok_or_else(changed)?;
        let parent = reservation
            .components
            .last()
            .map_or(&reservation.root, |entry| &entry.directory);
        let expected = reservation.binding.components.last().ok_or_else(changed)?;
        if identity(&parent.symlink_metadata(&leaf.name)?)? != *expected {
            return Err(changed());
        }
        drop(leaf.directory);
        parent.remove_dir(&leaf.name)?;
        sync_directory(parent)?;
        Ok(())
    }
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
    /// Observe expiry without acquiring a strong reference. Pruning under the
    /// checkpoint lock must never run the last workspace's destructor.
    pub(crate) fn is_live(&self) -> bool {
        self.0.strong_count() != 0
    }

    pub(crate) fn matches(&self, workspace: &AcquisitionWorkspace) -> bool {
        // The borrowed workspace already pins this allocation alive.
        self.0.ptr_eq(&Arc::downgrade(&workspace.held))
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
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
        let root_directory = open_pinned_directory(root)?;
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
        ReservedDirectory::capture(root, relative_target, execution_lease, validate_root)?
            .acquisition_workspace()
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
                parents: Arc::new(Mutex::new(BTreeMap::new())),
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
        let name = if partial { staging_path(&name) } else { name };
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
        let file = parent.open_with(staging_path(&name), &options)?.into_std();
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
                .open_with(staging_path(&name), &read_options)?
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
        match parent.remove_file(staging_path(&name)) {
            Ok(()) => sync_directory(&parent)?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(error.into()),
        }
        self.validate()
    }

    pub(crate) fn verify_file(&self, file: &ArtifactFile, partial: bool) -> Result<VerifiedFile> {
        let (parent, name) = self.parent(file.logical_path(), false)?;
        let name = if partial { staging_path(&name) } else { name };
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
            parent.rename(staging_path(&name), &parent, name)?;
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

    // Unix permits renaming an open directory. A reserved pathname must not
    // silently keep authorizing a detached inode or a replacement directory.
    #[cfg(unix)]
    #[test]
    fn reserved_workspace_refuses_replaced_root_or_child_before_file_effects() {
        for replace_root in [false, true] {
            let temp = tempfile::TempDir::new().unwrap();
            let root = temp.path().join("root");
            let relative = Path::new("stage");
            std::fs::create_dir_all(root.join(relative)).unwrap();
            let workspace = AcquisitionWorkspace::from_reserved_directory(
                &root,
                relative,
                Arc::new(()),
                || Ok(()),
            )
            .unwrap();
            let retired = temp.path().join("retired");
            let source = if replace_root {
                root.clone()
            } else {
                root.join(relative)
            };
            std::fs::rename(&source, &retired).unwrap();
            std::fs::create_dir_all(root.join(relative)).unwrap();
            let replacement = root.join(relative).join("sentinel");
            std::fs::write(&replacement, b"unrelated replacement").unwrap();
            let original = if replace_root {
                retired.join(relative)
            } else {
                retired
            };

            let outcome = workspace.open_part("payload.bin", false);
            let refused = outcome.is_err();
            drop(outcome);

            assert_eq!(
                std::fs::read(&replacement).unwrap(),
                b"unrelated replacement"
            );
            assert!(!root.join(relative).join("payload.bin.part").exists());
            assert!(
                refused,
                "replaced root={replace_root} must revoke path-bound use"
            );
            assert!(!original.join("payload.bin.part").exists());
        }
    }

    #[test]
    fn reserved_cleanup_revokes_use_then_requires_clone_release_for_empty_removal() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::create_dir(temp.path().join("stage")).unwrap();
        let lease = Arc::new(());
        let grant =
            ReservedDirectory::capture(temp.path(), Path::new("stage"), lease.clone(), || Ok(()))
                .unwrap();
        let workspace = grant.acquisition_workspace().unwrap();
        let mut file = workspace.open_part("nested/payload", false).unwrap();
        file.write_all(b"owned").unwrap();
        drop(file);
        grant.clear_contents().unwrap();
        assert!(temp.path().join("stage").is_dir());
        assert_eq!(
            std::fs::read_dir(temp.path().join("stage"))
                .unwrap()
                .count(),
            0
        );
        assert!(workspace.open_part("new", false).is_err());
        assert!(grant.acquisition_workspace().is_err());
        assert!(grant.clone().remove_empty().is_err());
        assert!(temp.path().join("stage").is_dir());
        drop(workspace);
        grant.remove_empty().unwrap();
        assert!(!temp.path().join("stage").exists());
        assert_eq!(Arc::strong_count(&lease), 1);
    }

    #[cfg(unix)]
    #[test]
    fn reserved_nested_component_replacement_preserves_both_trees() {
        for relative in ["outer/stage", "stage"] {
            let temp = tempfile::tempdir().unwrap();
            std::fs::create_dir_all(temp.path().join(relative)).unwrap();
            let grant =
                ReservedDirectory::capture(temp.path(), Path::new(relative), Arc::new(()), || {
                    Ok(())
                })
                .unwrap();
            let workspace = grant.acquisition_workspace().unwrap();
            let mut original = workspace.open_part("nested/input", false).unwrap();
            original.write_all(b"owned").unwrap();
            drop(original);
            let replaced = if relative == "stage" {
                temp.path().join("stage/nested")
            } else {
                temp.path().join("outer")
            };
            let retired = temp.path().join("retired");
            std::fs::rename(&replaced, &retired).unwrap();
            std::fs::create_dir_all(&replaced).unwrap();
            std::fs::write(replaced.join("sentinel"), b"replacement").unwrap();
            assert!(workspace.open_part("nested/new", false).is_err());
            assert!(grant.clear_contents().is_err());
            assert_eq!(
                std::fs::read(replaced.join("sentinel")).unwrap(),
                b"replacement"
            );
            let input = if relative == "stage" {
                retired.join("input.part")
            } else {
                retired.join("stage/nested/input.part")
            };
            assert_eq!(std::fs::read(input).unwrap(), b"owned");
        }
    }

    #[cfg(unix)]
    #[test]
    fn reserved_cleanup_refuses_observed_empty_replacement_before_final_unlink() {
        let temp = tempfile::tempdir().unwrap();
        let stage = temp.path().join("stage");
        std::fs::create_dir(&stage).unwrap();
        let grant =
            ReservedDirectory::capture(temp.path(), Path::new("stage"), Arc::new(()), || Ok(()))
                .unwrap();
        grant.clear_contents().unwrap();
        let retired = temp.path().join("retired");
        std::fs::rename(&stage, &retired).unwrap();
        std::fs::create_dir(&stage).unwrap();
        assert!(grant.remove_empty().is_err());
        assert!(stage.is_dir());
        assert!(retired.is_dir());
        // This tests replacement observed before unlink. It cannot prove safety
        // against hostile mutation between Unix's final comparison and unlink.
    }

    #[cfg(unix)]
    #[test]
    fn reserved_directory_refuses_a_symlink_even_to_the_original_child() {
        let temp = tempfile::tempdir().unwrap();
        let stage = temp.path().join("stage");
        let original = temp.path().join("original");
        std::fs::create_dir(&stage).unwrap();
        std::fs::write(stage.join("owned"), b"original").unwrap();
        let grant =
            ReservedDirectory::capture(temp.path(), Path::new("stage"), Arc::new(()), || Ok(()))
                .unwrap();
        std::fs::rename(&stage, &original).unwrap();
        std::os::unix::fs::symlink("original", &stage).unwrap();
        assert!(grant.validate().is_err());
        assert!(grant.clear_contents().is_err());
        assert!(
            ReservedDirectory::capture(temp.path(), Path::new("stage"), Arc::new(()), || Ok(()))
                .is_err()
        );
        assert_eq!(std::fs::read(original.join("owned")).unwrap(), b"original");
        assert!(std::fs::symlink_metadata(stage)
            .unwrap()
            .file_type()
            .is_symlink());
    }

    #[cfg(unix)]
    #[test]
    fn reserved_cleanup_unlinks_outside_symlink_without_visiting_target() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        std::fs::write(outside.path().join("sentinel"), b"outside").unwrap();
        std::fs::create_dir(temp.path().join("stage")).unwrap();
        std::os::unix::fs::symlink(outside.path(), temp.path().join("stage/link")).unwrap();
        let grant =
            ReservedDirectory::capture(temp.path(), Path::new("stage"), Arc::new(()), || Ok(()))
                .unwrap();
        grant.clear_contents().unwrap();
        grant.remove_empty().unwrap();
        assert_eq!(
            std::fs::read(outside.path().join("sentinel")).unwrap(),
            b"outside"
        );
    }

    #[cfg(windows)]
    #[test]
    fn reserved_windows_pins_root_and_child_until_acquisition_clones_drop() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("root");
        let stage = root.join("stage");
        std::fs::create_dir_all(&stage).unwrap();
        let grant =
            ReservedDirectory::capture(&root, Path::new("stage"), Arc::new(()), || Ok(())).unwrap();
        let workspace = grant.acquisition_workspace().unwrap();
        assert!(std::fs::rename(&root, temp.path().join("retired-root")).is_err());
        assert!(std::fs::rename(&stage, root.join("retired-stage")).is_err());
        assert!(std::fs::remove_dir(&stage).is_err());
        grant.clear_contents().unwrap();
        assert!(grant.clone().remove_empty().is_err());
        drop(workspace);
        grant.remove_empty().unwrap();
        assert!(!stage.exists());
        std::fs::rename(&root, temp.path().join("retired-root")).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn reserved_windows_cleanup_retains_junction_without_following_it() {
        use std::os::windows::process::CommandExt;
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::Builder::new()
            .prefix("pumas junction target & ! ")
            .tempdir()
            .unwrap();
        std::fs::write(outside.path().join("sentinel"), b"outside").unwrap();
        std::fs::create_dir(temp.path().join("stage")).unwrap();
        let junction = temp.path().join("stage").join("junction");
        // cmd has different quoting from the C runtime. Keep its command text
        // fixed; pass the dynamic target as a quoted, non-recursively expanded
        // environment value, with delayed expansion disabled for literal '!'.
        let output = std::process::Command::new("cmd.exe")
            .current_dir(temp.path())
            .env("PUMAS_TEST_JUNCTION_TARGET", outside.path())
            .args(["/d", "/v:off", "/c"])
            .raw_arg(r#"mklink /j "stage\junction" "%PUMAS_TEST_JUNCTION_TARGET%""#)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(std::fs::symlink_metadata(&junction)
            .unwrap()
            .file_type()
            .is_symlink());
        let grant =
            ReservedDirectory::capture(temp.path(), Path::new("stage"), Arc::new(()), || Ok(()))
                .unwrap();
        let error = grant.clear_contents().unwrap_err().to_string();
        assert!(error.contains("reparse point retained"), "{error}");
        assert_eq!(
            std::fs::read(outside.path().join("sentinel")).unwrap(),
            b"outside"
        );
        assert!(junction.exists());
        drop(grant);
        std::fs::remove_dir(junction).unwrap();
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
        let replaced = std::fs::rename(temp.path().join("nested"), temp.path().join("retired"));
        #[cfg(windows)]
        {
            // cap-std pins observed directories on Windows, so replacement is
            // refused by the OS rather than a later binding comparison.
            assert!(replaced.is_err());
            assert!(!temp.path().join("retired").exists());
        }
        #[cfg(not(windows))]
        {
            replaced.unwrap();
            std::fs::create_dir(temp.path().join("nested")).unwrap();
            assert!(workspace.open_part("nested/payload.bin", false).is_err());
            assert!(!temp.path().join("nested/payload.bin.part").exists());
            assert!(!temp.path().join("retired/payload.bin.part").exists());
        }
    }
}
