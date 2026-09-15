#![deny(unsafe_code)]

use crate::{ModelRecord, PumasError, Result};
use cap_std::ambient_authority;
#[cfg(unix)]
use cap_std::fs::OpenOptionsExt;
use cap_std::fs::{Dir, OpenOptions};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

const TOKEN_DOMAIN: &[u8] = b"pumas-download-recovery\0v1";
const TOKEN_PREFIX: &str = "v1:";
const TOKEN_HEX_BYTES: usize = 64;
const MAX_IDENTIFIER_BYTES: usize = 4 * 1024;
const MAX_COLLECTION_ITEMS: usize = 512;
const MAX_HF_REPO_ID_BYTES: usize = 96;
const MAX_PORTABLE_PATH_COMPONENT_BYTES: usize = 255;
const LIBRARY_ID_MARKER: &str = ".pumas-library-id.json";
#[cfg(unix)]
const LIBRARY_ID_LOCK: &str = ".pumas-library-id.lock";
const MAX_LIBRARY_ID_BYTES: u64 = 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LibraryIdDocument {
    schema_version: u32,
    library_id: String,
}

fn read_library_id(root: &Dir) -> io::Result<Option<uuid::Uuid>> {
    let metadata = match root.symlink_metadata(LIBRARY_ID_MARKER) {
        Ok(metadata) if metadata.is_file() && !metadata.is_symlink() => metadata,
        Ok(_) => return Err(invalid_library_id()),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error),
    };
    if metadata.len() > MAX_LIBRARY_ID_BYTES {
        return Err(invalid_library_id());
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let file = root.open_with(LIBRARY_ID_MARKER, &options)?.into_std();
    if !file.metadata()?.is_file() {
        return Err(invalid_library_id());
    }
    let mut bytes = Vec::new();
    file.take(MAX_LIBRARY_ID_BYTES + 1)
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_LIBRARY_ID_BYTES {
        return Err(invalid_library_id());
    }
    let document: LibraryIdDocument =
        serde_json::from_slice(&bytes).map_err(|_| invalid_library_id())?;
    let id = uuid::Uuid::parse_str(&document.library_id).map_err(|_| invalid_library_id())?;
    if document.schema_version != 1 || id.is_nil() {
        return Err(invalid_library_id());
    }
    Ok(Some(id))
}

fn invalid_library_id() -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidData,
        "Library identity marker is missing, changed, or invalid",
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
struct FilesystemIdentity {
    volume: u64,
    file: u64,
}

#[cfg(unix)]
fn filesystem_identity(metadata: &std::fs::Metadata) -> Option<FilesystemIdentity> {
    use std::os::unix::fs::MetadataExt;
    Some(FilesystemIdentity {
        volume: metadata.dev(),
        file: metadata.ino(),
    })
}

#[cfg(not(unix))]
fn filesystem_identity(_metadata: &std::fs::Metadata) -> Option<FilesystemIdentity> {
    // Download destination authority is unavailable outside Unix; read-only
    // inspection must also decline to issue a usable recovery capability.
    None
}

/// Opaque collision-resistant fingerprint of one recovery-relevant model state.
///
/// This is a stale-state precondition, not an authentication credential. The
/// recovery target and repository are always re-resolved by the core.
#[derive(Clone, PartialEq, Eq)]
pub struct DownloadRecoveryToken(String);

impl DownloadRecoveryToken {
    pub fn parse(value: &str) -> Option<Self> {
        let digest = value.strip_prefix(TOKEN_PREFIX)?;
        (digest.len() == TOKEN_HEX_BYTES
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)))
        .then(|| Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Validated platform-neutral model ID accepted by the recovery action.
#[derive(Clone, PartialEq, Eq)]
pub struct DownloadRecoveryModelId(String);

impl DownloadRecoveryModelId {
    pub fn parse(value: &str) -> Option<Self> {
        is_portable_relative_path(value).then(|| Self(value.to_string()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Producer-issued display/action precondition for one partial model.
#[derive(Clone)]
pub struct DownloadRecoveryTicket {
    token: DownloadRecoveryToken,
    repo_id: String,
    selected_artifact_id: Option<String>,
    selected_artifact_files: Vec<String>,
    selected_artifact_quant: Option<String>,
}

impl DownloadRecoveryTicket {
    pub fn token(&self) -> &str {
        self.token.as_str()
    }

    pub fn repo_id(&self) -> &str {
        &self.repo_id
    }

    pub fn selected_artifact_id(&self) -> Option<&str> {
        self.selected_artifact_id.as_deref()
    }

    pub fn selected_artifact_files(&self) -> &[String] {
        &self.selected_artifact_files
    }

    pub fn selected_artifact_quant(&self) -> Option<&str> {
        self.selected_artifact_quant.as_deref()
    }
}

#[derive(Clone)]
pub(crate) struct VerifiedDownloadRecovery {
    pub(crate) destination: DownloadRecoveryDestination,
    pub(crate) repo_id: String,
    pub(crate) files: Vec<String>,
}

/// Held library-root authority for one verified recovery destination.
///
/// Recovery file operations remain relative to this capability instead of
/// regaining ambient authority from the displayed absolute path.
#[derive(Clone)]
pub(crate) struct DownloadRecoveryDestination {
    authority: Arc<RecoveryRoot>,
    model_relative: PathBuf,
    display_path: PathBuf,
    held: Arc<OnceLock<HeldDestination>>,
    creation_anchor: Arc<CreationAnchor>,
    file_parents: Arc<Mutex<BTreeMap<PathBuf, Arc<HeldDestination>>>>,
    #[cfg(test)]
    cleanup_parent_sync: Option<Arc<CleanupParentSync>>,
}

#[cfg(test)]
type CleanupParentSync = dyn Fn(&Dir) -> io::Result<()> + Send + Sync;

impl super::partial_download::PartialDownloadFiles for DownloadRecoveryDestination {
    fn file_len(&self, filename: &str) -> Result<Option<u64>> {
        Ok(self.file_len(filename)?)
    }
    fn part_len(&self, filename: &str) -> Result<Option<u64>> {
        Ok(self.part_len(filename)?)
    }
    fn rename_part_to_file(&self, filename: &str) -> Result<()> {
        Ok(self.rename_part_to_file(filename)?)
    }
    fn remove_part(&self, filename: &str) -> Result<()> {
        Ok(self.remove_part(filename)?)
    }
    fn remove_marker(&self) -> Result<()> {
        Ok(self.remove_marker()?)
    }
}

struct CreationAnchor {
    directory: Dir,
    relative: PathBuf,
    tail: PathBuf,
    identity: FilesystemIdentity,
}

impl CreationAnchor {
    fn capture(root: &Dir, target: &Path) -> io::Result<Self> {
        for prefix in target.ancestors() {
            match open_directory_chain(root, prefix, false) {
                Ok(directory) => {
                    return Ok(Self {
                        identity: directory_identity(&directory)?,
                        directory,
                        relative: prefix.to_path_buf(),
                        tail: target
                            .strip_prefix(prefix)
                            .map_err(|_| invalid_capability_path())?
                            .to_path_buf(),
                    });
                }
                Err(error) if error.kind() == io::ErrorKind::NotFound => {}
                Err(error) => return Err(error),
            }
        }
        Err(invalid_capability_path())
    }
}

struct HeldDestination {
    directory: Dir,
    identity: FilesystemIdentity,
}

/// Equality information only. Effect authority lives in the held capability.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct DestinationIdentity {
    root: FilesystemIdentity,
    relative: String,
}

/// One configured root opened by the composition owner after directory setup.
#[derive(Clone)]
pub(crate) struct DownloadDestinationRoot(Arc<RecoveryRoot>);

/// Native exclusion retained by active download mutation, not configured roots.
pub(crate) struct RootExecutionGrant {
    file: std::fs::File,
    authority: DownloadDestinationRoot,
}

impl Drop for RootExecutionGrant {
    fn drop(&mut self) {
        // Closing our descriptor alone can leave flock held by a concurrent
        // fork until the child execs. Custody ends with this grant, so release
        // the lock explicitly on its shared open file description.
        let _ = fs2::FileExt::unlock(&self.file);
    }
}

impl RootExecutionGrant {
    pub(crate) fn validate_root(&self, root: &DownloadDestinationRoot) -> Result<()> {
        self.authority.0.require_current()?;
        root.0.require_current()?;
        if !self.authority.same_physical_root(root)
            || filesystem_identity(&self.file.metadata()?) != Some(root.0.root_identity)
            || self.authority.0.library_id != root.0.library_id
        {
            return Err(invalid_capability_path().into());
        }
        Ok(())
    }
}

impl DownloadDestinationRoot {
    pub(crate) fn same_physical_root(&self, other: &Self) -> bool {
        self.0.root_identity == other.0.root_identity
    }

    pub(crate) fn try_acquire_execution_grant(&self) -> Result<RootExecutionGrant> {
        self.0.require_current()?;
        // A fresh readable open has independent lock ownership. A cloned handle
        // shares ownership; an open_dir handle may not support native locking.
        let file = self.0.root.open(".")?.into_std();
        let metadata = file.metadata()?;
        if !metadata.is_dir() || filesystem_identity(&metadata) != Some(self.0.root_identity) {
            return Err(invalid_capability_path().into());
        }
        fs2::FileExt::try_lock_exclusive(&file).map_err(|error| {
            if error.kind() == io::ErrorKind::WouldBlock {
                PumasError::DownloadRootBusy
            } else {
                error.into()
            }
        })?;
        let grant = RootExecutionGrant {
            file,
            authority: self.clone(),
        };
        grant.validate_root(self)?;
        Ok(grant)
    }

    pub(crate) fn open(path: &Path) -> Result<Self> {
        #[cfg(not(unix))]
        {
            let _ = path;
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Download destination authority requires Unix",
            )
            .into());
        }
        #[cfg(unix)]
        {
            let mut root = RecoveryRoot::open(path)?.ok_or_else(invalid_capability_path)?;
            root.initialize_library_id()?;
            Ok(Self(Arc::new(root)))
        }
    }

    pub(crate) fn resolve(&self, path: &Path) -> io::Result<DownloadRecoveryDestination> {
        self.0.require_current()?;
        // Find a prefix which is the configured root itself. Missing descendants
        // never determine identity, and symlinks below this prefix are rejected.
        let mut relative = (!path.is_absolute()).then(|| path.to_path_buf());
        for ancestor in path.ancestors().skip(1).filter(|_| path.is_absolute()) {
            if let Ok(metadata) = std::fs::metadata(ancestor) {
                if filesystem_identity(&metadata) == Some(self.0.root_identity) {
                    relative = path.strip_prefix(ancestor).ok().map(Path::to_path_buf);
                    break;
                }
            }
        }
        let relative = relative.ok_or_else(invalid_capability_path)?;
        let text = relative.to_str().ok_or_else(invalid_capability_path)?;
        if !is_portable_relative_path(text) {
            return Err(invalid_capability_path());
        }
        let destination = DownloadRecoveryDestination {
            authority: self.0.clone(),
            display_path: self.0.root_canonical_path.join(&relative),
            creation_anchor: Arc::new(CreationAnchor::capture(&self.0.root, &relative)?),
            model_relative: relative,
            held: Arc::new(OnceLock::new()),
            file_parents: Arc::new(Mutex::new(BTreeMap::new())),
            #[cfg(test)]
            cleanup_parent_sync: None,
        };
        match destination.directory(false) {
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                destination.require_directory_chain(&destination.model_relative, true)?;
            }
            Err(error) => return Err(error),
        }
        self.0.require_current()?;
        Ok(destination)
    }
}

struct RecoveryRoot {
    root: Dir,
    root_source_path: PathBuf,
    root_canonical_path: PathBuf,
    root_identity: FilesystemIdentity,
    library_id: Option<uuid::Uuid>,
}

impl RecoveryRoot {
    fn open(library_root: &Path) -> Result<Option<Self>> {
        let root =
            Dir::open_ambient_dir(library_root, ambient_authority()).map_err(PumasError::from)?;
        let held_metadata = root
            .try_clone()
            .and_then(|root| root.into_std_file().metadata())
            .map_err(PumasError::from)?;
        let Some(root_identity) = filesystem_identity(&held_metadata) else {
            return Ok(None);
        };
        let root_canonical_path = std::fs::canonicalize(library_root).map_err(PumasError::from)?;
        let authority = Self {
            library_id: read_library_id(&root)?,
            root,
            root_source_path: library_root.to_path_buf(),
            root_canonical_path,
            root_identity,
        };
        if authority.require_physical_current().is_err() {
            return Ok(None);
        }
        Ok(Some(authority))
    }

    fn destination_for(
        self: &Arc<Self>,
        record: &ModelRecord,
    ) -> Option<DownloadRecoveryDestination> {
        if !is_portable_relative_path(&record.id) || self.require_current().is_err() {
            return None;
        }
        let model_relative = PathBuf::from(&record.id);
        let display_path = self.root_canonical_path.join(&model_relative);
        if Path::new(&record.path) != display_path {
            return None;
        }
        let model_metadata = self.root.symlink_metadata(&model_relative).ok()?;
        if !model_metadata.is_dir() || model_metadata.is_symlink() {
            return None;
        }
        if self.root.canonicalize(&model_relative).ok()? != model_relative
            || self.require_current().is_err()
        {
            return None;
        }
        Some(DownloadRecoveryDestination {
            authority: self.clone(),
            creation_anchor: Arc::new(CreationAnchor::capture(&self.root, &model_relative).ok()?),
            model_relative,
            display_path,
            held: Arc::new(OnceLock::new()),
            file_parents: Arc::new(Mutex::new(BTreeMap::new())),
            #[cfg(test)]
            cleanup_parent_sync: None,
        })
    }

    fn require_current(&self) -> io::Result<()> {
        self.require_physical_current()?;
        if read_library_id(&self.root)? != self.library_id {
            return Err(invalid_library_id());
        }
        Ok(())
    }

    fn require_physical_current(&self) -> io::Result<()> {
        let canonical = std::fs::canonicalize(&self.root_source_path)?;
        if canonical != self.root_canonical_path {
            return Err(invalid_capability_path());
        }
        let current = std::fs::metadata(&canonical)?;
        if filesystem_identity(&current) != Some(self.root_identity) {
            return Err(invalid_capability_path());
        }
        let held = self.root.try_clone()?.into_std_file().metadata()?;
        if filesystem_identity(&held) != Some(self.root_identity) {
            return Err(invalid_capability_path());
        }
        Ok(())
    }

    #[cfg(unix)]
    fn initialize_library_id(&mut self) -> Result<()> {
        self.require_physical_current()?;
        if self.library_id.is_some() {
            return self.reconfirm_library_id();
        }
        let mut options = OpenOptions::new();
        options
            .read(true)
            .write(true)
            .create_new(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        // Concurrent non-exclusive creation can return ENOENT on macOS. Elect
        // one creator atomically, then open the winner's stable lock file.
        let lock = match self.root.open_with(LIBRARY_ID_LOCK, &options) {
            Ok(lock) => lock,
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                options.create_new(false);
                self.root.open_with(LIBRARY_ID_LOCK, &options)?
            }
            Err(error) => return Err(error.into()),
        }
        .into_std();
        if !lock.metadata()?.is_file() {
            return Err(invalid_library_id().into());
        }
        fs2::FileExt::lock_exclusive(&lock)?;
        self.require_physical_current()?;
        // Another configured opener may have initialized it while we waited.
        if let Some(id) = read_library_id(&self.root)? {
            self.library_id = Some(id);
            return self.reconfirm_library_id();
        }
        let validator = Self {
            root: self.root.try_clone()?,
            root_source_path: self.root_source_path.clone(),
            root_canonical_path: self.root_canonical_path.clone(),
            root_identity: self.root_identity,
            library_id: None,
        };
        let target = crate::metadata::AtomicJsonTarget::from_capability(
            self.root.try_clone()?,
            std::ffi::OsStr::new(LIBRARY_ID_MARKER),
            self.root_canonical_path.join(LIBRARY_ID_MARKER),
            move || {
                validator
                    .require_physical_current()
                    .map(|()| true)
                    .map_err(Into::into)
            },
        )?;
        let id = uuid::Uuid::new_v4();
        match target.publish_json(&LibraryIdDocument {
            schema_version: 1,
            library_id: id.to_string(),
        }) {
            Ok(crate::metadata::AtomicPublication::Durable) => {}
            Ok(crate::metadata::AtomicPublication::PublishedDurabilityUnknown { error }) => {
                return Err(error)
            }
            Ok(crate::metadata::AtomicPublication::VisibilityUnknown { error, cleanup }) => {
                return Err(crate::metadata::AtomicPublishFailure {
                    stage: crate::metadata::AtomicPublishStage::Rename,
                    kind: crate::metadata::AtomicPublishFailureKind::Filesystem,
                    error,
                    cleanup,
                }
                .into_error());
            }
            Err(failure) => return Err(failure.into_error()),
        }
        self.library_id = Some(id);
        self.require_current()?;
        Ok(())
    }

    #[cfg(unix)]
    fn reconfirm_library_id(&self) -> Result<()> {
        self.require_current()?;
        let mut options = OpenOptions::new();
        options
            .read(true)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let marker = self.root.open_with(LIBRARY_ID_MARKER, &options)?.into_std();
        if !marker.metadata()?.is_file() {
            return Err(invalid_library_id().into());
        }
        marker.sync_all()?;
        self.require_current()?;
        self.root.open(".")?.into_std().sync_all()?;
        self.require_current()?;
        Ok(())
    }
}

impl DownloadRecoveryDestination {
    /// Observe durable destructive custody while the caller holds this root's
    /// execution grant. An absent index is allowed for standalone download roots;
    /// an existing index must be readable and authoritative.
    pub(crate) fn assert_no_intent_deletion_claim(&self) -> Result<()> {
        self.authority.require_current()?;
        let database = self.authority.root_canonical_path.join("models.db");
        let before = match std::fs::symlink_metadata(&database) {
            Ok(metadata) if metadata.is_file() => metadata,
            Ok(_) => return Err(invalid_capability_path().into()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                self.authority.require_current()?;
                return Ok(());
            }
            Err(error) => return Err(error.into()),
        };
        let index = crate::index::ModelIndex::open_read_only(&database)?;
        let claims = index.list_intent_model_deletion_claims()?;
        let after = std::fs::symlink_metadata(&database)?;
        self.authority.require_current()?;
        if !after.is_file() || filesystem_identity(&before) != filesystem_identity(&after) {
            return Err(invalid_capability_path().into());
        }
        if claims
            .iter()
            .any(|claim| claim.model_id == self.library_model_id())
        {
            return Err(PumasError::Config {
                message: "Download destination has unresolved intent deletion custody".into(),
            });
        }
        Ok(())
    }

    /// Observed catalog identity, not authority to perform filesystem effects.
    pub(crate) fn library_model_id(&self) -> String {
        self.model_relative
            .components()
            .map(|component| component.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/")
    }

    pub(crate) fn execution_root(&self) -> DownloadDestinationRoot {
        DownloadDestinationRoot(self.authority.clone())
    }

    /// Logical library identity survives remounts and clones; it grants no
    /// filesystem authority. Physical identity still governs every effect.
    pub(crate) fn persisted_identity(
        &self,
    ) -> Result<super::download_store::PersistedDestinationIdentity> {
        self.authority.require_current()?;
        let id = self.authority.library_id.ok_or_else(invalid_library_id)?;
        Ok(super::download_store::PersistedDestinationIdentity {
            library_root: format!("uuid:{id}"),
            relative_target: self.model_relative.to_string_lossy().into_owned(),
        })
    }

    pub(crate) fn identity(&self) -> DestinationIdentity {
        DestinationIdentity {
            root: self.authority.root_identity,
            relative: self.model_relative.to_string_lossy().into_owned(),
        }
    }

    fn directory(&self, create: bool) -> io::Result<Dir> {
        self.directory_if_present(create)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Download destination does not exist",
            )
        })
    }

    /// Absence is legitimate only before this capability has held a target.
    /// Root and creation-anchor failures never become successful empty cleanup.
    fn directory_if_present(&self, create: bool) -> io::Result<Option<Dir>> {
        self.authority.require_current()?;
        let anchor = &self.creation_anchor;
        if directory_identity(&open_directory_chain(
            &self.authority.root,
            &anchor.relative,
            false,
        )?)? != anchor.identity
        {
            return Err(invalid_capability_path());
        }
        let directory = match open_directory_chain(
            &anchor.directory,
            &anchor.tail,
            create && self.held.get().is_none(),
        ) {
            Ok(directory) => directory,
            Err(error)
                if !create
                    && self.held.get().is_none()
                    && error.kind() == io::ErrorKind::NotFound =>
            {
                self.authority.require_current()?;
                return Ok(None);
            }
            Err(error) => return Err(error),
        };
        let identity = directory_identity(&directory)?;
        if let Some(held) = self.held.get() {
            if held.identity != identity {
                return Err(invalid_capability_path());
            }
        } else {
            let _ = self.held.set(HeldDestination {
                directory: directory.try_clone()?,
                identity,
            });
        }
        let held = self.held.get().ok_or_else(invalid_capability_path)?;
        if held.identity != identity {
            return Err(invalid_capability_path());
        }
        self.authority.require_current()?;
        held.directory.try_clone().map(Some)
    }

    pub(crate) fn prepare(&self) -> io::Result<()> {
        self.directory(true).map(|_| ())
    }

    pub(crate) fn read_model_metadata(&self) -> Result<Option<crate::models::ModelMetadata>> {
        let Some(directory) = self.directory_if_present(false)? else {
            return Ok(None);
        };
        let metadata = Self::read_provenance_file(&directory, "metadata.json")?
            .map(serde_json::from_value)
            .transpose()?;
        self.directory(false)?;
        Ok(metadata)
    }

    pub(crate) fn write_model_metadata(
        &self,
        metadata: &crate::models::ModelMetadata,
    ) -> Result<()> {
        let directory = self.directory(false)?;
        let expected = directory_identity(&directory)?;
        let destination = self.clone();
        let target = crate::metadata::AtomicJsonTarget::from_capability(
            directory,
            std::ffi::OsStr::new("metadata.json"),
            self.display_path.join("metadata.json"),
            move || Ok(directory_identity(&destination.directory(false)?)? == expected),
        )?;
        match target.publish_json(metadata) {
            Ok(crate::metadata::AtomicPublication::Durable) => Ok(()),
            Ok(crate::metadata::AtomicPublication::PublishedDurabilityUnknown { error }) => {
                Err(error)
            }
            Ok(crate::metadata::AtomicPublication::VisibilityUnknown { error, cleanup }) => {
                Err(crate::metadata::AtomicPublishFailure {
                    stage: crate::metadata::AtomicPublishStage::Rename,
                    kind: crate::metadata::AtomicPublishFailureKind::Filesystem,
                    error,
                    cleanup,
                }
                .into_error())
            }
            Err(failure) => Err(failure.into_error()),
        }
    }

    /// Remove only the bound model directory, using held directory-relative
    /// operations. Symlinks inside the payload are unlinked, never traversed.
    /// The caller retains native exclusion and a durable deletion claim.
    pub(crate) fn remove_model_directory_all(&self) -> Result<()> {
        let directory = self.directory(false)?;
        let parent_relative = self
            .model_relative
            .parent()
            .ok_or_else(invalid_capability_path)?;
        let name = self
            .model_relative
            .file_name()
            .ok_or_else(invalid_capability_path)?;
        let parent = open_directory_chain(&self.authority.root, parent_relative, false)?;
        let expected = directory_identity(&directory)?;
        if directory_identity(&open_directory_chain(&parent, Path::new(name), false)?)? != expected
        {
            return Err(invalid_capability_path().into());
        }
        remove_held_directory_contents(&directory)?;
        self.authority.require_current()?;
        if directory_identity(&open_directory_chain(&parent, Path::new(name), false)?)? != expected
        {
            return Err(invalid_capability_path().into());
        }
        parent.remove_dir(name)?;
        parent.open(".")?.sync_all()?;
        self.authority.require_current()?;
        Ok(())
    }

    /// Relocate a bound directory between held roots without replacing a
    /// destination. The caller retains grants for both roots; cross-filesystem
    /// copying is not authorized by this capability.
    pub(crate) fn rename_model_directory_noreplace(&self, target: &Self) -> Result<()> {
        #[cfg(not(any(target_os = "linux", target_os = "macos")))]
        {
            let _ = target;
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Held model relocation requires an atomic no-replace rename",
            )
            .into())
        }
        #[cfg(any(target_os = "linux", target_os = "macos"))]
        {
            use std::os::unix::ffi::OsStrExt;

            self.authority.require_current()?;
            target.authority.require_current()?;
            let source = self.directory(false)?;
            let expected = directory_identity(&source)?;
            if target.directory_if_present(false)?.is_some() {
                return Err(io::Error::new(
                    io::ErrorKind::AlreadyExists,
                    "Model relocation destination already exists",
                )
                .into());
            }
            let source_parent = open_directory_chain(
                &self.authority.root,
                self.model_relative
                    .parent()
                    .ok_or_else(invalid_capability_path)?,
                false,
            )?;
            let target_parent = open_directory_chain(
                &target.authority.root,
                target
                    .model_relative
                    .parent()
                    .ok_or_else(invalid_capability_path)?,
                true,
            )?;
            let source_name = self
                .model_relative
                .file_name()
                .ok_or_else(invalid_capability_path)?;
            let target_name = target
                .model_relative
                .file_name()
                .ok_or_else(invalid_capability_path)?;
            if directory_identity(&open_directory_chain(
                &source_parent,
                Path::new(source_name),
                false,
            )?)? != expected
            {
                return Err(invalid_capability_path().into());
            }
            let source_c = std::ffi::CString::new(source_name.as_bytes())
                .map_err(|_| invalid_capability_path())?;
            let target_c = std::ffi::CString::new(target_name.as_bytes())
                .map_err(|_| invalid_capability_path())?;
            // A capability directory may use O_PATH, which supports renameat2
            // but rejects fsync. Open "." relative to each held parent to get
            // readable directory descriptors usable for both operations.
            let source_parent = source_parent.open(".")?.into_std();
            let target_parent = target_parent.open(".")?.into_std();
            self.authority.require_current()?;
            target.authority.require_current()?;
            rename_held_directories_noreplace(
                &source_parent,
                &source_c,
                &target_parent,
                &target_c,
            )?;
            source_parent.sync_all()?;
            target_parent.sync_all()?;
            self.authority.require_current()?;
            target.authority.require_current()?;
            if directory_identity(&target.directory(false)?)? != expected {
                return Err(invalid_capability_path().into());
            }
            Ok(())
        }
    }

    /// Observe existing download provenance through the held destination.
    /// The caller owns revision policy and must repeat this observation under
    /// its destination reservation before admitting effects.
    pub(crate) fn read_download_provenance(&self) -> Result<(Option<Value>, Option<Value>)> {
        let Some(directory) = self.directory_if_present(false)? else {
            return Ok((None, None));
        };
        let metadata = Self::read_provenance_file(&directory, "metadata.json")?;
        let marker = Self::read_provenance_file(&directory, ".pumas_download")?;
        // A replaced directory/root is a refusal, never an empty observation.
        self.directory(false)?;
        Ok((metadata, marker))
    }

    fn read_provenance_file(directory: &Dir, name: &str) -> Result<Option<Value>> {
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let file = match directory.open_with(name, &options) {
            Ok(file) => file.into_std(),
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };
        if !file.metadata()?.is_file() {
            return Err(invalid_capability_path().into());
        }
        let value: Value = serde_json::from_reader(file).map_err(|source| PumasError::Json {
            message: "Download provenance is not valid JSON".to_string(),
            source: Some(source),
        })?;
        if !value.is_object() {
            return Err(PumasError::Validation {
                field: "download.provenance".to_string(),
                message: "Download provenance must be a JSON object".to_string(),
            });
        }
        Ok(Some(value))
    }

    fn file_parent(&self, file: &str, create: bool) -> io::Result<(Dir, String)> {
        self.file_parent_if_present(file, create)?.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Download file parent does not exist",
            )
        })
    }

    fn file_parent_if_present(
        &self,
        file: &str,
        create: bool,
    ) -> io::Result<Option<(Dir, String)>> {
        if !is_portable_relative_path(file) {
            return Err(invalid_capability_path());
        }
        let path = Path::new(file);
        let name = path
            .file_name()
            .and_then(|s| s.to_str())
            .ok_or_else(invalid_capability_path)?;
        let Some(mut directory) = self.directory_if_present(false)? else {
            return Ok(None);
        };
        let mut relative = PathBuf::new();
        for component in path.parent().unwrap_or(Path::new("")).components() {
            let Component::Normal(component) = component else {
                return Err(invalid_capability_path());
            };
            relative.push(component);
            let known = {
                self.file_parents
                    .lock()
                    .map_err(|_| io::Error::other("Download parent authority lock poisoned"))?
                    .get(&relative)
                    .cloned()
            };
            let next = match open_directory_chain(
                &directory,
                Path::new(component),
                create && known.is_none(),
            ) {
                Ok(directory) => directory,
                Err(error)
                    if !create && known.is_none() && error.kind() == io::ErrorKind::NotFound =>
                {
                    return Ok(None);
                }
                Err(error) => return Err(error),
            };
            let identity = directory_identity(&next)?;
            let candidate = Arc::new(HeldDestination {
                directory: next,
                identity,
            });
            let held = {
                let mut parents = self
                    .file_parents
                    .lock()
                    .map_err(|_| io::Error::other("Download parent authority lock poisoned"))?;
                parents.entry(relative.clone()).or_insert(candidate).clone()
            };
            if held.identity != identity {
                return Err(invalid_capability_path());
            }
            directory = held.directory.try_clone()?;
        }
        Ok(Some((directory, name.to_owned())))
    }

    /// Publish the unchanged object schema through held directory authority.
    pub(crate) fn write_marker(&self, marker: &Value) -> crate::metadata::AtomicPublishResult {
        if !marker.is_object() {
            return Err(Box::new(crate::metadata::AtomicPublishFailure {
                stage: crate::metadata::AtomicPublishStage::Serialization,
                kind: crate::metadata::AtomicPublishFailureKind::InvalidData,
                error: PumasError::Other("Download marker must be a JSON object".into()),
                cleanup: crate::metadata::StagingCleanup::NotRequired,
            }));
        }
        let admitted = (|| -> Result<crate::metadata::AtomicJsonTarget> {
            let parent = self.directory(false)?;
            let expected = directory_identity(&parent)?;
            let destination = self.clone();
            crate::metadata::AtomicJsonTarget::from_capability(
                parent,
                std::ffi::OsStr::new(".pumas_download"),
                self.display_path.join(".pumas_download"),
                move || Ok(directory_identity(&destination.directory(false)?)? == expected),
            )
        })();
        match admitted {
            Ok(target) => target.publish_json(marker),
            Err(error) => Err(Box::new(crate::metadata::AtomicPublishFailure {
                stage: crate::metadata::AtomicPublishStage::TargetAdmission,
                kind: crate::metadata::AtomicPublishFailureKind::TargetUnavailable,
                error,
                cleanup: crate::metadata::StagingCleanup::NotRequired,
            })),
        }
    }
    pub(crate) fn display_path(&self) -> &Path {
        &self.display_path
    }

    #[cfg(test)]
    pub(crate) fn authority_strong_count(&self) -> usize {
        Arc::strong_count(&self.authority)
    }

    pub(crate) fn preflight(&self, files: &[String]) -> io::Result<()> {
        self.authority.require_current()?;
        self.require_directory_chain(&self.model_relative, false)?;
        for file in files {
            let file = Path::new(file);
            let relative = self.model_relative.join(file);
            if let Some(parent) = relative.parent() {
                self.require_directory_chain(parent, true)?;
            }
            self.require_regular_or_missing(&relative)?;
            self.require_regular_or_missing(&self.part_relative(file))?;
        }
        self.authority.require_current()?;
        Ok(())
    }

    pub(crate) fn create_parent(&self, file: &str) -> io::Result<()> {
        self.file_parent(file, true).map(|_| ())
    }

    pub(crate) fn file_len(&self, file: &str) -> io::Result<Option<u64>> {
        self.regular_file_len(&self.model_relative.join(file))
    }

    pub(crate) fn part_len(&self, file: &str) -> io::Result<Option<u64>> {
        self.regular_file_len(&self.part_relative(Path::new(file)))
    }

    /// Verify an admitted file through its held directory authority.
    /// Missing size/hash evidence remains unknown; it is never inferred here.
    /// The caller retains the execution reservation and owns this blocking read.
    pub(crate) fn verify_download_file(
        &self,
        filename: &str,
        is_partial: bool,
        expected_size: Option<u64>,
        expected_sha256: Option<&str>,
    ) -> Result<()> {
        if expected_sha256.is_some_and(|value| {
            value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
        }) {
            return Err(invalid_download_integrity(
                "Expected SHA256 is not a complete hexadecimal digest",
            ));
        }
        let (parent, name) = self.file_parent(filename, false)?;
        let name = if is_partial {
            format!(
                "{name}{}",
                crate::config::NetworkConfig::DOWNLOAD_TEMP_SUFFIX
            )
        } else {
            name
        };
        let mut options = OpenOptions::new();
        options.read(true);
        #[cfg(unix)]
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
        let mut file = parent.open_with(&name, &options)?.into_std();
        let before = file.metadata()?;
        if !before.is_file() {
            return Err(invalid_capability_path().into());
        }
        if expected_size.is_some_and(|expected| expected != before.len()) {
            return Err(invalid_download_integrity(
                "Downloaded file size does not match expected size",
            ));
        }
        let actual_sha256 = if expected_sha256.is_some() {
            let mut hasher = Sha256::new();
            let mut buffer = [0_u8; 64 * 1024];
            loop {
                let count = file.read(&mut buffer)?;
                if count == 0 {
                    break;
                }
                hasher.update(&buffer[..count]);
            }
            Some(hex::encode(hasher.finalize()))
        } else {
            None
        };
        let after = file.metadata()?;
        let (current_parent, _) = self.file_parent(filename, false)?;
        let current = current_parent.open_with(&name, &options)?.into_std();
        let current_metadata = current.metadata()?;
        if !current_metadata.is_file()
            || filesystem_identity(&before) != filesystem_identity(&current_metadata)
            || before.len() != after.len()
            || before.modified()? != after.modified()?
            || after.len() != current_metadata.len()
            || after.modified()? != current_metadata.modified()?
        {
            return Err(invalid_download_integrity(
                "Downloaded file changed during verification",
            ));
        }
        if let (Some(expected), Some(actual)) = (expected_sha256, actual_sha256) {
            if !actual.eq_ignore_ascii_case(expected) {
                return Err(PumasError::HashMismatch {
                    expected: expected.to_ascii_lowercase(),
                    actual,
                });
            }
        }
        Ok(())
    }

    pub(crate) fn open_part(&self, file: &str, append: bool) -> io::Result<std::fs::File> {
        let (parent, name) = self.file_parent(file, true)?;
        let name = format!(
            "{name}{}",
            crate::config::NetworkConfig::DOWNLOAD_TEMP_SUFFIX
        );
        let mut options = OpenOptions::new();
        #[cfg(unix)]
        {
            use cap_std::fs::OpenOptionsExt;
            options.custom_flags(libc::O_NOFOLLOW);
        }
        if append {
            options.append(true);
        } else {
            options.write(true).create(true).truncate(true);
        }
        parent.open_with(name, &options).map(|file| file.into_std())
    }

    pub(crate) fn remove_part(&self, file: &str) -> io::Result<()> {
        let Some((parent, name)) = self.file_parent_if_present(file, false)? else {
            return Ok(());
        };
        let name = format!(
            "{name}{}",
            crate::config::NetworkConfig::DOWNLOAD_TEMP_SUFFIX
        );
        self.remove_file_durable(&parent, &name)
    }

    pub(crate) fn rename_part_to_file(&self, file: &str) -> io::Result<()> {
        let (parent, name) = self.file_parent(file, false)?;
        let part = format!(
            "{name}{}",
            crate::config::NetworkConfig::DOWNLOAD_TEMP_SUFFIX
        );
        parent.rename(part, &parent, name)
    }

    pub(crate) fn remove_marker(&self) -> io::Result<()> {
        let Some(directory) = self.directory_if_present(false)? else {
            return Ok(());
        };
        self.remove_file_durable(&directory, ".pumas_download")
    }

    fn remove_file_durable(&self, parent: &Dir, name: &str) -> io::Result<()> {
        match parent.remove_file(name) {
            Ok(()) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        // An earlier unlink may have succeeded before its directory sync failed.
        // Absence alone is not durable cleanup proof, so retries sync again.
        #[cfg(test)]
        if let Some(sync) = &self.cleanup_parent_sync {
            return sync(parent);
        }
        parent.open(".")?.sync_all()
    }

    fn regular_file_len(&self, relative: &Path) -> io::Result<Option<u64>> {
        let file = relative
            .strip_prefix(&self.model_relative)
            .map_err(|_| invalid_capability_path())?;
        let file = file.to_str().ok_or_else(invalid_capability_path)?;
        let Some((parent, name)) = self.file_parent_if_present(file, false)? else {
            return Ok(None);
        };
        match parent.symlink_metadata(name) {
            Ok(metadata) if metadata.is_file() => Ok(Some(metadata.len())),
            Ok(_) => Err(invalid_capability_path()),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn require_regular_or_missing(&self, relative: &Path) -> io::Result<()> {
        self.regular_file_len(relative).map(|_| ())
    }

    fn require_directory_chain(&self, relative: &Path, allow_missing_tail: bool) -> io::Result<()> {
        let mut current = PathBuf::new();
        let mut missing = false;
        for component in relative.components() {
            let Component::Normal(component) = component else {
                return Err(invalid_capability_path());
            };
            current.push(component);
            if missing {
                continue;
            }
            match self.authority.root.symlink_metadata(&current) {
                Ok(metadata) if metadata.is_dir() => {}
                Ok(_) => return Err(invalid_capability_path()),
                Err(error) if allow_missing_tail && error.kind() == io::ErrorKind::NotFound => {
                    missing = true;
                }
                Err(error) => return Err(error),
            }
        }
        Ok(())
    }

    fn part_relative(&self, file: &Path) -> PathBuf {
        let mut relative = self.model_relative.join(file).into_os_string();
        relative.push(crate::config::NetworkConfig::DOWNLOAD_TEMP_SUFFIX);
        PathBuf::from(relative)
    }
}

fn directory_identity(directory: &Dir) -> io::Result<FilesystemIdentity> {
    filesystem_identity(&directory.try_clone()?.into_std_file().metadata()?)
        .ok_or_else(invalid_capability_path)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
#[allow(unsafe_code)]
fn rename_held_directories_noreplace(
    source_parent: &std::fs::File,
    source_name: &std::ffi::CStr,
    target_parent: &std::fs::File,
    target_name: &std::ffi::CStr,
) -> io::Result<()> {
    use std::os::fd::AsRawFd;
    // SAFETY: the borrowed files keep both directory descriptors open for the
    // entire synchronous syscall; the borrowed CStr arguments are valid,
    // NUL-terminated names for that same lifetime. The caller supplies one
    // validated basename per held no-follow parent. Neither syscall retains
    // these pointers; the exclusive flag forbids replacement of any target.
    #[cfg(target_os = "linux")]
    let result = unsafe {
        libc::syscall(
            libc::SYS_renameat2,
            source_parent.as_raw_fd(),
            source_name.as_ptr(),
            target_parent.as_raw_fd(),
            target_name.as_ptr(),
            libc::RENAME_NOREPLACE,
        )
    };
    // SAFETY: same descriptor and basename lifetime guarantees as above.
    #[cfg(target_os = "macos")]
    let result = unsafe {
        libc::renameatx_np(
            source_parent.as_raw_fd(),
            source_name.as_ptr(),
            target_parent.as_raw_fd(),
            target_name.as_ptr(),
            libc::RENAME_EXCL,
        )
    };
    if result == 0 {
        Ok(())
    } else {
        Err(io::Error::last_os_error())
    }
}

fn remove_held_directory_contents(directory: &Dir) -> io::Result<()> {
    for entry in directory.entries()? {
        let name = entry?.file_name();
        if directory.symlink_metadata(&name)?.is_dir() {
            let child = open_directory_chain(directory, Path::new(&name), false)?;
            let expected = directory_identity(&child)?;
            remove_held_directory_contents(&child)?;
            if directory_identity(&open_directory_chain(directory, Path::new(&name), false)?)?
                != expected
            {
                return Err(invalid_capability_path());
            }
            directory.remove_dir(&name)?;
        } else {
            directory.remove_file(&name)?;
        }
    }
    directory.open(".")?.sync_all()
}

fn invalid_download_integrity(message: &str) -> PumasError {
    PumasError::Validation {
        field: "download.integrity".to_string(),
        message: message.to_string(),
    }
}

/// Walk one component at a time without following symlinks. Each next operation
/// is anchored to the held preceding directory, including missing-tail creation.
fn open_directory_chain(root: &Dir, relative: &Path, create: bool) -> io::Result<Dir> {
    #[cfg(not(unix))]
    {
        let _ = (root, relative, create);
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "No-follow directory authority unavailable",
        ))
    }
    #[cfg(unix)]
    {
        use cap_std::fs::OpenOptionsExt;
        let mut directory = root.try_clone()?;
        for component in relative.components() {
            let Component::Normal(name) = component else {
                return Err(invalid_capability_path());
            };
            let mut options = OpenOptions::new();
            options
                .read(true)
                .custom_flags(libc::O_NOFOLLOW | libc::O_DIRECTORY);
            let file = match directory.open_with(name, &options) {
                Ok(file) => file,
                Err(error) if create && error.kind() == io::ErrorKind::NotFound => {
                    match directory.create_dir(name) {
                        Ok(()) => {}
                        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
                        Err(error) => return Err(error),
                    }
                    directory.open_with(name, &options)?
                }
                Err(error) => return Err(error),
            };
            if create {
                // Also sync existing entries: they may come from a previous
                // attempt whose directory-link durability was uncertain.
                file.sync_all()?;
                directory.open(".")?.sync_all()?;
            }
            directory = Dir::from_std_file(file.into_std());
        }
        Ok(directory)
    }
}

pub(crate) enum DownloadRecoveryVerification {
    Complete,
    Unavailable,
    Stale,
    Verified(VerifiedDownloadRecovery),
}

/// Atomic result of admitting one producer-verified recovery action.
pub(crate) enum RecoveryDownloadAdmission {
    Recovered {
        download_id: String,
    },
    Resumed {
        download_id: String,
    },
    Attached {
        download_id: String,
        status: crate::models::DownloadStatus,
    },
    AlreadyCompleted {
        download_id: String,
    },
    AlreadyCancelled {
        download_id: String,
    },
    ContextMismatch,
    BoundFilesUnavailable,
    CapabilityUnavailable,
}

#[derive(Clone)]
struct RecoverySnapshot {
    model_id: String,
    canonical_model_dir: String,
    root_identity: FilesystemIdentity,
    destination: DownloadRecoveryDestination,
    repo_id: String,
    selected_artifact_id: Option<String>,
    selected_artifact_files: Vec<String>,
    selected_artifact_quant: Option<String>,
}

impl RecoverySnapshot {
    fn token(&self) -> DownloadRecoveryToken {
        let mut hasher = blake3::Hasher::new();
        hasher.update(TOKEN_DOMAIN);
        hash_text(&mut hasher, &self.model_id);
        hash_text(&mut hasher, &self.canonical_model_dir);
        hasher.update(&self.root_identity.volume.to_be_bytes());
        hasher.update(&self.root_identity.file.to_be_bytes());
        hash_text(&mut hasher, &self.repo_id);
        hash_optional_text(&mut hasher, self.selected_artifact_id.as_deref());
        hash_optional_text(&mut hasher, self.selected_artifact_quant.as_deref());
        hasher.update(&(self.selected_artifact_files.len() as u64).to_be_bytes());
        for file in &self.selected_artifact_files {
            hash_text(&mut hasher, file);
        }
        DownloadRecoveryToken(format!("{TOKEN_PREFIX}{}", hasher.finalize().to_hex()))
    }

    fn into_ticket(self) -> DownloadRecoveryTicket {
        let token = self.token();
        DownloadRecoveryTicket {
            token,
            repo_id: self.repo_id,
            selected_artifact_id: self.selected_artifact_id,
            selected_artifact_files: self.selected_artifact_files,
            selected_artifact_quant: self.selected_artifact_quant,
        }
    }
}

fn hash_text(hasher: &mut blake3::Hasher, value: &str) {
    hasher.update(&(value.len() as u64).to_be_bytes());
    hasher.update(value.as_bytes());
}

fn hash_optional_text(hasher: &mut blake3::Hasher, value: Option<&str>) {
    match value {
        Some(value) => {
            hasher.update(&[1]);
            hash_text(hasher, value);
        }
        None => {
            hasher.update(&[0]);
        }
    }
}

/// Issue a recovery ticket from one core-owned projected model record.
///
/// Filesystem-ineligible partial records remain displayable and return no
/// ticket. Malformed recovery metadata is rejected when recovery is present.
pub fn issue_download_recovery_ticket(
    library_root: &Path,
    record: &ModelRecord,
) -> Result<Option<DownloadRecoveryTicket>> {
    Ok(recovery_snapshot(library_root, record)?.map(RecoverySnapshot::into_ticket))
}

pub(crate) fn verify_download_recovery_ticket(
    library_root: &Path,
    record: &ModelRecord,
    token: &DownloadRecoveryToken,
) -> Result<DownloadRecoveryVerification> {
    let incomplete = record
        .metadata
        .as_object()
        .and_then(|metadata| metadata.get("download_incomplete"))
        .and_then(Value::as_bool)
        .ok_or_else(invalid_recovery_metadata)?;
    if !incomplete {
        return Ok(DownloadRecoveryVerification::Complete);
    }
    let Some(snapshot) = recovery_snapshot(library_root, record)? else {
        return Ok(DownloadRecoveryVerification::Unavailable);
    };
    if snapshot.token() != *token {
        return Ok(DownloadRecoveryVerification::Stale);
    }
    Ok(DownloadRecoveryVerification::Verified(
        VerifiedDownloadRecovery {
            destination: snapshot.destination,
            repo_id: snapshot.repo_id,
            files: snapshot.selected_artifact_files,
        },
    ))
}

fn recovery_snapshot(
    library_root: &Path,
    record: &ModelRecord,
) -> Result<Option<RecoverySnapshot>> {
    let metadata = record
        .metadata
        .as_object()
        .ok_or_else(invalid_recovery_metadata)?;
    let incomplete = metadata
        .get("download_incomplete")
        .and_then(Value::as_bool)
        .ok_or_else(invalid_recovery_metadata)?;
    if !incomplete {
        return Ok(None);
    }

    // Tickets currently reconstruct a legacy-main request. Do not issue or
    // verify that authority for an artifact bound to another revision. Pinned
    // execution resumes through its persisted download snapshot instead.
    if optional_text(metadata, "upstream_revision")?.is_some_and(|revision| revision != "main") {
        return Ok(None);
    }

    let repo_id = optional_text(metadata, "repo_id")?;
    let Some(repo_id) = repo_id else {
        return Ok(None);
    };
    validate_repo_id(&repo_id)?;

    let selected_artifact_id = optional_text(metadata, "selected_artifact_id")?;
    let selected_artifact_quant = optional_text(metadata, "selected_artifact_quant")?;
    let selected_files = optional_file_set(metadata, "selected_artifact_files")?;
    let expected_files = optional_file_set(metadata, "expected_files")?;
    let files = if selected_files.is_empty() {
        expected_files
    } else {
        selected_files
    };

    if files.is_empty() {
        return Ok(None);
    }

    let Some(authority) = RecoveryRoot::open(library_root)? else {
        return Ok(None);
    };
    let authority = Arc::new(authority);
    let Some(destination) = authority.destination_for(record) else {
        return Ok(None);
    };
    if destination.preflight(&files).is_err() {
        return Ok(None);
    }

    let Some(canonical_model_dir) = destination.display_path.to_str().map(str::to_string) else {
        return Ok(None);
    };
    Ok(Some(RecoverySnapshot {
        model_id: record.id.clone(),
        canonical_model_dir,
        root_identity: authority.root_identity,
        destination,
        repo_id,
        selected_artifact_id,
        selected_artifact_files: files,
        selected_artifact_quant,
    }))
}

pub(crate) fn canonical_managed_model_dir(
    library_root: &Path,
    record: &ModelRecord,
) -> Result<Option<PathBuf>> {
    if !is_portable_relative_path(&record.id) {
        return Ok(None);
    }
    let canonical_root = std::fs::canonicalize(library_root).map_err(PumasError::from)?;
    let indexed_path = Path::new(&record.path);
    let indexed_metadata = match std::fs::symlink_metadata(indexed_path) {
        Ok(metadata) => metadata,
        Err(_) => return Ok(None),
    };
    if !indexed_metadata.is_dir() || indexed_metadata.file_type().is_symlink() {
        return Ok(None);
    }
    let canonical_model_dir = match std::fs::canonicalize(indexed_path) {
        Ok(path) => path,
        Err(_) => return Ok(None),
    };
    let Ok(relative) = canonical_model_dir.strip_prefix(&canonical_root) else {
        return Ok(None);
    };
    let relative_id = relative
        .components()
        .map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Option<Vec<_>>>()
        .map(|components| components.join("/"));
    if relative_id.as_deref() != Some(record.id.as_str()) {
        return Ok(None);
    }
    Ok(Some(canonical_model_dir))
}

fn optional_text(metadata: &Map<String, Value>, field: &str) -> Result<Option<String>> {
    match metadata.get(field) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => {
            let value = value.trim();
            if value.is_empty() || value.len() > MAX_IDENTIFIER_BYTES {
                Err(invalid_recovery_metadata())
            } else {
                Ok(Some(value.to_string()))
            }
        }
        Some(_) => Err(invalid_recovery_metadata()),
    }
}

fn optional_file_set(metadata: &Map<String, Value>, field: &str) -> Result<Vec<String>> {
    let values = match metadata.get(field) {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(values)) if values.len() <= MAX_COLLECTION_ITEMS => values,
        Some(Value::Array(_)) | Some(_) => return Err(invalid_recovery_metadata()),
    };
    let mut files = BTreeSet::new();
    for value in values {
        let value = value.as_str().ok_or_else(invalid_recovery_metadata)?;
        if !is_portable_relative_path(value) {
            return Err(invalid_recovery_metadata());
        }
        files.insert(value.to_string());
    }
    Ok(files.into_iter().collect())
}

fn validate_repo_id(value: &str) -> Result<()> {
    let mut segments = value.split('/');
    let owner = segments.next().unwrap_or_default();
    let name = segments.next().unwrap_or_default();
    let exact_shape = !owner.is_empty() && !name.is_empty() && segments.next().is_none();
    let valid_components = [owner, name].into_iter().all(|segment| {
        segment.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.')
        }) && segment
            .chars()
            .next()
            .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
            && segment
                .chars()
                .last()
                .is_some_and(|character| character.is_ascii_alphanumeric() || character == '_')
    });
    let forbidden = value.contains("--")
        || value.contains("..")
        || value.to_ascii_lowercase().ends_with(".git");
    if exact_shape && value.len() <= MAX_HF_REPO_ID_BYTES && valid_components && !forbidden {
        Ok(())
    } else {
        Err(invalid_recovery_metadata())
    }
}

fn is_portable_relative_path(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_IDENTIFIER_BYTES
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(character, '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
        })
        && value.split('/').all(is_portable_path_component)
}

fn is_portable_path_component(component: &str) -> bool {
    if component.is_empty()
        || component.len() > MAX_PORTABLE_PATH_COMPONENT_BYTES
        || matches!(component, "." | "..")
        || component.ends_with(['.', ' '])
    {
        return false;
    }
    let stem = component.split('.').next().unwrap_or_default();
    let upper = stem.to_ascii_uppercase();
    !matches!(
        upper.as_str(),
        "CON" | "PRN" | "AUX" | "NUL" | "CONIN$" | "CONOUT$"
    ) && !numbered_windows_device(&upper, "COM")
        && !numbered_windows_device(&upper, "LPT")
}

fn numbered_windows_device(value: &str, prefix: &str) -> bool {
    value
        .strip_prefix(prefix)
        .is_some_and(|suffix| matches!(suffix, "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9"))
}

fn invalid_recovery_metadata() -> PumasError {
    PumasError::Other("Model download recovery metadata is invalid".to_string())
}

fn invalid_capability_path() -> io::Error {
    io::Error::new(
        io::ErrorKind::PermissionDenied,
        "download recovery path is outside its verified filesystem authority",
    )
}

#[cfg(test)]
mod tests {
    #[cfg(unix)]
    #[test]
    fn held_model_metadata_preserves_substituted_destination() {
        let temp = tempfile::TempDir::new().unwrap();
        let library = temp.path().join("library");
        let victim = temp.path().join("victim");
        std::fs::create_dir_all(library.join("model")).unwrap();
        std::fs::create_dir_all(&victim).unwrap();
        std::fs::write(victim.join("metadata.json"), b"external metadata").unwrap();
        let root = super::DownloadDestinationRoot::open(&library).unwrap();
        let destination = root.resolve(std::path::Path::new("model")).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        let metadata = crate::models::ModelMetadata {
            model_id: Some("model".into()),
            ..Default::default()
        };
        destination.write_model_metadata(&metadata).unwrap();
        assert_eq!(
            destination.read_model_metadata().unwrap().unwrap().model_id,
            metadata.model_id
        );
        let original_bytes = std::fs::read(library.join("model/metadata.json")).unwrap();
        std::fs::rename(library.join("model"), library.join("original")).unwrap();
        std::os::unix::fs::symlink(&victim, library.join("model")).unwrap();
        assert!(destination.read_model_metadata().is_err());
        assert!(destination.write_model_metadata(&metadata).is_err());
        assert_eq!(
            std::fs::read(victim.join("metadata.json")).unwrap(),
            b"external metadata"
        );
        assert_eq!(
            std::fs::read(library.join("original/metadata.json")).unwrap(),
            original_bytes
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn held_rename_syscall_rejects_destination_created_after_preflight() {
        let temp = tempfile::TempDir::new().unwrap();
        let parent = std::fs::File::open(temp.path()).unwrap();
        std::fs::create_dir(temp.path().join("source")).unwrap();
        std::fs::write(temp.path().join("source/weights"), b"original").unwrap();
        assert!(!temp.path().join("target").exists());
        // An empty directory could be replaced by ordinary rename. Create it
        // after preflight and exercise the syscall's exclusive flag directly.
        std::fs::create_dir(temp.path().join("target")).unwrap();
        let error =
            super::rename_held_directories_noreplace(&parent, c"source", &parent, c"target")
                .unwrap_err();
        assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
        assert_eq!(
            std::fs::read(temp.path().join("source/weights")).unwrap(),
            b"original"
        );
        assert!(temp.path().join("target").is_dir());
        assert!(!temp.path().join("target/weights").exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn held_model_relocation_moves_between_roots_and_never_overwrites() {
        let temp = tempfile::TempDir::new().unwrap();
        let source_path = temp.path().join("source");
        let target_path = temp.path().join("target");
        std::fs::create_dir_all(source_path.join("model")).unwrap();
        std::fs::create_dir_all(&target_path).unwrap();
        std::fs::write(source_path.join("model/weights"), b"original").unwrap();
        let source_root = super::DownloadDestinationRoot::open(&source_path).unwrap();
        let target_root = super::DownloadDestinationRoot::open(&target_path).unwrap();
        let _source_grant = source_root.try_acquire_execution_grant().unwrap();
        let _target_grant = target_root.try_acquire_execution_grant().unwrap();
        let source = source_root.resolve(std::path::Path::new("model")).unwrap();
        let target = target_root
            .resolve(std::path::Path::new("family/model"))
            .unwrap();
        source.rename_model_directory_noreplace(&target).unwrap();
        assert!(!source_path.join("model").exists());
        assert_eq!(
            std::fs::read(target_path.join("family/model/weights")).unwrap(),
            b"original"
        );

        std::fs::create_dir_all(source_path.join("replacement")).unwrap();
        std::fs::write(source_path.join("replacement/weights"), b"replacement").unwrap();
        let replacement = source_root
            .resolve(std::path::Path::new("replacement"))
            .unwrap();
        assert!(replacement
            .rename_model_directory_noreplace(&target)
            .is_err());
        assert_eq!(
            std::fs::read(source_path.join("replacement/weights")).unwrap(),
            b"replacement"
        );
        assert_eq!(
            std::fs::read(target_path.join("family/model/weights")).unwrap(),
            b"original"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[test]
    fn held_model_relocation_rejects_a_substituted_destination_parent() {
        let temp = tempfile::TempDir::new().unwrap();
        let library = temp.path().join("library");
        let victim = temp.path().join("victim");
        std::fs::create_dir_all(library.join("source")).unwrap();
        std::fs::create_dir_all(library.join("family")).unwrap();
        std::fs::create_dir_all(&victim).unwrap();
        std::fs::write(library.join("source/weights"), b"original").unwrap();
        std::fs::write(victim.join("keep"), b"victim").unwrap();
        let root = super::DownloadDestinationRoot::open(&library).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        let source = root.resolve(std::path::Path::new("source")).unwrap();
        let target = root.resolve(std::path::Path::new("family/model")).unwrap();
        std::fs::rename(library.join("family"), library.join("original-family")).unwrap();
        std::os::unix::fs::symlink(&victim, library.join("family")).unwrap();
        assert!(source.rename_model_directory_noreplace(&target).is_err());
        assert_eq!(
            std::fs::read(library.join("source/weights")).unwrap(),
            b"original"
        );
        assert_eq!(std::fs::read(victim.join("keep")).unwrap(), b"victim");
        assert!(!victim.join("model").exists());
    }

    #[cfg(unix)]
    #[test]
    fn held_model_deletion_unlinks_payload_symlinks_without_following_them() {
        let temp = tempfile::TempDir::new().unwrap();
        let library = temp.path().join("library");
        let victim = temp.path().join("victim");
        std::fs::create_dir_all(library.join("model/nested")).unwrap();
        std::fs::create_dir_all(&victim).unwrap();
        std::fs::write(victim.join("keep"), b"external").unwrap();
        std::fs::write(library.join("model/nested/weights"), b"owned").unwrap();
        std::os::unix::fs::symlink(&victim, library.join("model/link")).unwrap();
        let root = super::DownloadDestinationRoot::open(&library).unwrap();
        let destination = root.resolve(std::path::Path::new("model")).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        destination.remove_model_directory_all().unwrap();
        assert!(!library.join("model").exists());
        assert_eq!(std::fs::read(victim.join("keep")).unwrap(), b"external");
    }

    #[cfg(unix)]
    #[test]
    fn held_model_deletion_rejects_substituted_destination_and_root() {
        for replace_root in [false, true] {
            let temp = tempfile::TempDir::new().unwrap();
            let library = temp.path().join("library");
            let victim = temp.path().join("victim");
            std::fs::create_dir_all(library.join("model")).unwrap();
            std::fs::create_dir_all(victim.join("model")).unwrap();
            std::fs::write(library.join("model/keep"), b"original").unwrap();
            std::fs::write(victim.join("model/keep"), b"victim").unwrap();
            let root = super::DownloadDestinationRoot::open(&library).unwrap();
            let destination = root.resolve(std::path::Path::new("model")).unwrap();
            let _grant = root.try_acquire_execution_grant().unwrap();
            let original = if replace_root {
                std::fs::rename(&library, temp.path().join("original")).unwrap();
                std::os::unix::fs::symlink(&victim, &library).unwrap();
                temp.path().join("original/model/keep")
            } else {
                std::fs::rename(library.join("model"), library.join("original")).unwrap();
                std::os::unix::fs::symlink(victim.join("model"), library.join("model")).unwrap();
                library.join("original/keep")
            };
            assert!(destination.remove_model_directory_all().is_err());
            assert_eq!(std::fs::read(original).unwrap(), b"original");
            assert_eq!(std::fs::read(victim.join("model/keep")).unwrap(), b"victim");
        }
    }

    #[cfg(unix)]
    #[test]
    fn intent_deletion_custody_check_is_read_only_and_survives_reopen() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let destination = root.resolve(std::path::Path::new("model")).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        let database = temp.path().join("models.db");
        destination.assert_no_intent_deletion_claim().unwrap();
        assert!(!database.exists());

        let token = uuid::Uuid::new_v4();
        {
            let index = crate::index::ModelIndex::new(&database).unwrap();
            index.claim_intent_model_deletion("model", token).unwrap();
        }
        assert!(destination.assert_no_intent_deletion_claim().is_err());
        let index = crate::index::ModelIndex::open_read_only(&database).unwrap();
        let claims = index.list_intent_model_deletion_claims().unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].claim_token, token);
        drop(index);
        let index = crate::index::ModelIndex::new(&database).unwrap();
        assert!(index.release_intent_model_deletion("model", token).unwrap());
        destination.assert_no_intent_deletion_claim().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn malformed_intent_database_is_not_absent_download_custody() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let destination = root.resolve(std::path::Path::new("model")).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        let database = temp.path().join("models.db");
        std::fs::write(&database, b"broken authoritative database").unwrap();
        assert!(destination.assert_no_intent_deletion_claim().is_err());
        assert_eq!(
            std::fs::read(&database).unwrap(),
            b"broken authoritative database"
        );
    }

    #[cfg(unix)]
    #[test]
    fn bound_download_integrity_verifies_final_and_partial_without_mutating_bytes() {
        const ABC_SHA256: &str = "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad";
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let destination = root.resolve(std::path::Path::new("model")).unwrap();
        std::fs::create_dir_all(temp.path().join("model/nested")).unwrap();
        for is_partial in [false, true] {
            let name = if is_partial {
                "weights.gguf.part"
            } else {
                "weights.gguf"
            };
            let path = temp.path().join("model/nested").join(name);
            std::fs::write(&path, b"abc").unwrap();
            destination
                .verify_download_file("nested/weights.gguf", is_partial, Some(3), Some(ABC_SHA256))
                .unwrap();
            assert!(matches!(
                destination.verify_download_file("nested/weights.gguf", is_partial, Some(4), Some(ABC_SHA256)),
                Err(crate::PumasError::Validation { field, .. }) if field == "download.integrity"
            ));
            assert!(matches!(
                destination.verify_download_file("nested/weights.gguf", is_partial, Some(3), Some("invalid")),
                Err(crate::PumasError::Validation { field, .. }) if field == "download.integrity"
            ));
            std::fs::write(&path, b"abd").unwrap();
            assert!(matches!(
                destination.verify_download_file(
                    "nested/weights.gguf",
                    is_partial,
                    Some(3),
                    Some(ABC_SHA256)
                ),
                Err(crate::PumasError::HashMismatch { .. })
            ));
            assert_eq!(std::fs::read(&path).unwrap(), b"abd");
            // Missing evidence permits only the regular-file observation.
            destination
                .verify_download_file("nested/weights.gguf", is_partial, None, None)
                .unwrap();
        }
    }

    #[cfg(unix)]
    #[test]
    fn bound_download_integrity_refuses_missing_symlink_directory_and_replaced_parent() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let destination = root.resolve(std::path::Path::new("model")).unwrap();
        std::fs::create_dir_all(temp.path().join("model/nested")).unwrap();
        assert!(destination
            .verify_download_file("missing.gguf", false, None, None)
            .is_err());
        assert!(destination
            .verify_download_file("nested", false, None, None)
            .is_err());
        std::fs::write(temp.path().join("outside.gguf"), b"abc").unwrap();
        std::os::unix::fs::symlink("../outside.gguf", temp.path().join("model/linked.gguf"))
            .unwrap();
        assert!(destination
            .verify_download_file("linked.gguf", false, Some(3), None)
            .is_err());
        std::fs::write(temp.path().join("model/nested/weights.gguf"), b"abc").unwrap();
        destination
            .verify_download_file("nested/weights.gguf", false, Some(3), None)
            .unwrap();
        std::fs::rename(
            temp.path().join("model/nested"),
            temp.path().join("model/original"),
        )
        .unwrap();
        std::fs::create_dir(temp.path().join("model/nested")).unwrap();
        std::fs::write(temp.path().join("model/nested/weights.gguf"), b"abc").unwrap();
        assert!(destination
            .verify_download_file("nested/weights.gguf", false, Some(3), None)
            .is_err());
    }

    #[cfg(unix)]
    #[test]
    fn download_provenance_is_read_only_and_preserves_marker_only_pins() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let destination = root.resolve(std::path::Path::new("model")).unwrap();
        assert_eq!(
            destination.read_download_provenance().unwrap(),
            (None, None)
        );
        assert!(!temp.path().join("model").exists());
        destination.prepare().unwrap();
        let marker = serde_json::json!({
            "selected_artifact": { "revision": "a".repeat(40) }
        });
        std::fs::write(
            temp.path().join("model/.pumas_download"),
            serde_json::to_vec(&marker).unwrap(),
        )
        .unwrap();
        assert_eq!(
            destination.read_download_provenance().unwrap(),
            (None, Some(marker))
        );
        assert!(!temp.path().join("model/metadata.json").exists());
    }

    #[cfg(unix)]
    #[test]
    fn download_provenance_refuses_symlinks_malformed_json_and_replaced_destination() {
        for name in ["metadata.json", ".pumas_download"] {
            let temp = tempfile::TempDir::new().unwrap();
            let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
            let destination = root.resolve(std::path::Path::new("model")).unwrap();
            destination.prepare().unwrap();
            let path = temp.path().join("model").join(name);
            std::fs::write(temp.path().join("elsewhere.json"), b"{}").unwrap();
            std::os::unix::fs::symlink("../elsewhere.json", &path).unwrap();
            assert!(destination.read_download_provenance().is_err());
            std::fs::remove_file(&path).unwrap();
            std::fs::write(&path, b"{").unwrap();
            assert!(destination.read_download_provenance().is_err());
            std::fs::write(&path, b"null").unwrap();
            assert!(destination.read_download_provenance().is_err());
            std::fs::write(&path, b"{}").unwrap();
            assert!(destination.read_download_provenance().is_ok());
            std::fs::rename(temp.path().join("model"), temp.path().join("original")).unwrap();
            std::fs::create_dir(temp.path().join("model")).unwrap();
            assert!(destination.read_download_provenance().is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn root_execution_grants_contend_across_aliases_and_release_on_close() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("library root");
        std::fs::create_dir(&path).unwrap();
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(&path, &alias).unwrap();
        let root = super::DownloadDestinationRoot::open(&path).unwrap();
        let contender = super::DownloadDestinationRoot::open(&alias).unwrap();
        let grant = root.try_acquire_execution_grant().unwrap();
        grant.validate_root(&contender).unwrap();
        let copied_path = temp.path().join("copied library");
        std::fs::create_dir(&copied_path).unwrap();
        std::fs::copy(
            path.join(super::LIBRARY_ID_MARKER),
            copied_path.join(super::LIBRARY_ID_MARKER),
        )
        .unwrap();
        let copied_root = super::DownloadDestinationRoot::open(&copied_path).unwrap();
        assert!(matches!(grant.validate_root(&copied_root),
            Err(crate::PumasError::Io { source: Some(ref error), .. })
                if error.kind() == std::io::ErrorKind::PermissionDenied));
        copied_root.try_acquire_execution_grant().unwrap();
        assert!(matches!(
            contender.try_acquire_execution_grant(),
            Err(crate::PumasError::DownloadRootBusy)
        ));
        drop(grant);
        contender.try_acquire_execution_grant().unwrap();
        assert_eq!(
            std::fs::read_dir(&path).unwrap().count(),
            2,
            "execution must not add a sidecar beyond existing identity files"
        );
    }

    #[cfg(unix)]
    #[test]
    fn root_execution_grant_release_is_not_delayed_by_inherited_handles() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let grant = root.try_acquire_execution_grant().unwrap();
        // A duplicate shares the open file description, just as a concurrent
        // fork does before the child exec closes CLOEXEC descriptors.
        let inherited = grant.file.try_clone().unwrap();
        assert!(matches!(
            root.try_acquire_execution_grant(),
            Err(crate::PumasError::DownloadRootBusy)
        ));
        drop(grant);
        let next = root
            .try_acquire_execution_grant()
            .expect("grant drop must release custody even while an inherited descriptor is open");
        drop(inherited);
        assert!(matches!(
            root.try_acquire_execution_grant(),
            Err(crate::PumasError::DownloadRootBusy)
        ));
        drop(next);
        root.try_acquire_execution_grant().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn root_execution_grants_reject_replaced_root_and_changed_uuid() {
        for replace_root in [false, true] {
            let temp = tempfile::TempDir::new().unwrap();
            let path = temp.path().join("library");
            std::fs::create_dir(&path).unwrap();
            let root = super::DownloadDestinationRoot::open(&path).unwrap();
            let grant = root.try_acquire_execution_grant().unwrap();
            if replace_root {
                std::fs::rename(&path, temp.path().join("old")).unwrap();
                std::fs::create_dir(&path).unwrap();
            } else {
                std::fs::write(
                    path.join(super::LIBRARY_ID_MARKER),
                    serde_json::to_vec(&serde_json::json!({
                        "schema_version": 1, "library_id": uuid::Uuid::new_v4().to_string()
                    }))
                    .unwrap(),
                )
                .unwrap();
            }
            let current = super::DownloadDestinationRoot::open(&path).unwrap();
            for result in [
                grant.validate_root(&root),
                grant.validate_root(&current),
                root.try_acquire_execution_grant().map(|_| ()),
            ] {
                assert!(
                    matches!(result, Err(crate::PumasError::Io { source: Some(ref error), .. })
                    if error.kind() == if replace_root { std::io::ErrorKind::PermissionDenied }
                        else { std::io::ErrorKind::InvalidData })
                );
            }
        }
    }

    #[cfg(unix)]
    #[test]
    #[ignore = "subprocess helper invoked by root_execution_grant_releases_after_process_death"]
    fn root_execution_grant_child_holder() {
        use std::io::Write;
        let Some(path) = std::env::var_os("PUMAS_ROOT_GRANT_CHILD_DIR") else {
            return;
        };
        let root = super::DownloadDestinationRoot::open(std::path::Path::new(&path)).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        println!("PUMAS_ROOT_GRANT_ACQUIRED");
        std::io::stdout().flush().unwrap();
        std::io::stdin().read_line(&mut String::new()).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn root_execution_grant_releases_after_process_death() {
        use std::io::BufRead;
        use std::process::{Command, Stdio};
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "model_library::download_recovery::tests::root_execution_grant_child_holder",
                "--nocapture",
            ])
            .env("PUMAS_ROOT_GRANT_CHILD_DIR", temp.path())
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let output = child.stdout.take().unwrap();
        let (ready_tx, ready_rx) = std::sync::mpsc::channel();
        let reader = std::thread::spawn(move || {
            for line in std::io::BufReader::new(output).lines() {
                if line.unwrap().contains("PUMAS_ROOT_GRANT_ACQUIRED") {
                    let _ = ready_tx.send(());
                    break;
                }
            }
        });
        let ready = ready_rx.recv_timeout(std::time::Duration::from_secs(5));
        let contended = ready.is_ok()
            && matches!(
                root.try_acquire_execution_grant(),
                Err(crate::PumasError::DownloadRootBusy)
            );
        child.kill().unwrap();
        child.wait().unwrap();
        reader.join().unwrap();
        ready.unwrap();
        assert!(contended, "independent process must hold native exclusion");
        root.try_acquire_execution_grant().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn readonly_recovery_inspection_does_not_initialize_library_identity() {
        let temp = tempfile::TempDir::new().unwrap();
        let record = partial_record(temp.path(), vec!["weights.gguf"]);
        assert!(super::issue_download_recovery_ticket(temp.path(), &record)
            .unwrap()
            .is_some());
        let root = std::sync::Arc::new(super::RecoveryRoot::open(temp.path()).unwrap().unwrap());
        let destination = root.destination_for(&record).unwrap();
        assert!(destination.persisted_identity().is_err());
        assert!(!temp.path().join(super::LIBRARY_ID_MARKER).exists());
        assert!(!temp.path().join(super::LIBRARY_ID_LOCK).exists());
        super::DownloadDestinationRoot::open(temp.path()).unwrap();
        assert!(
            root.require_current().is_err(),
            "an old markerless holder must not adopt a new ID"
        );
    }

    #[cfg(unix)]
    #[test]
    fn held_library_identity_rejects_missing_changed_or_invalid_marker() {
        for replacement in [
            "missing",
            "changed",
            "corrupt",
            "symlink",
            "directory",
            "oversized",
        ] {
            let temp = tempfile::TempDir::new().unwrap();
            let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
            let destination = root.resolve(std::path::Path::new("model")).unwrap();
            let marker = temp.path().join(super::LIBRARY_ID_MARKER);
            std::fs::remove_file(&marker).unwrap();
            match replacement {
                "missing" => {},
                "changed" => std::fs::write(&marker, serde_json::to_vec(&serde_json::json!({"schema_version":1,"library_id":uuid::Uuid::new_v4().to_string()})).unwrap()).unwrap(),
                "corrupt" => std::fs::write(&marker, b"not-json").unwrap(),
                "symlink" => std::os::unix::fs::symlink("missing-target", &marker).unwrap(),
                "directory" => std::fs::create_dir(&marker).unwrap(),
                _ => std::fs::write(&marker, vec![b' '; 1025]).unwrap(),
            }
            assert!(
                destination.persisted_identity().is_err(),
                "accepted {replacement}"
            );
            assert!(
                destination.prepare().is_err(),
                "effects survived {replacement}"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn concurrent_configured_roots_initialize_one_durable_library_identity() {
        for _ in 0..16 {
            let temp = tempfile::TempDir::new().unwrap();
            let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
            let handles = (0..8)
                .map(|_| {
                    let barrier = barrier.clone();
                    let path = temp.path().to_path_buf();
                    std::thread::spawn(move || {
                        barrier.wait();
                        super::DownloadDestinationRoot::open(&path)
                            .unwrap()
                            .resolve(std::path::Path::new("model"))
                            .unwrap()
                            .persisted_identity()
                            .unwrap()
                    })
                })
                .collect::<Vec<_>>();
            // Join every worker before propagating a panic so the temporary root
            // remains alive and does not turn one failure into unrelated ENOENTs.
            let results = handles
                .into_iter()
                .map(|handle| handle.join())
                .collect::<Vec<_>>();
            let identities = results
                .into_iter()
                .map(|result| result.unwrap())
                .collect::<Vec<_>>();
            assert!(identities.iter().all(|id| id == &identities[0]));
            let reopened = super::DownloadDestinationRoot::open(temp.path())
                .unwrap()
                .resolve(std::path::Path::new("model"))
                .unwrap();
            assert_eq!(reopened.persisted_identity().unwrap(), identities[0]);
        }
    }

    #[cfg(unix)]
    #[test]
    fn persisted_library_identity_survives_a_change_of_physical_root() {
        let temp = tempfile::TempDir::new().unwrap();
        let original = temp.path().join("original");
        let moved = temp.path().join("moved");
        std::fs::create_dir(&original).unwrap();
        std::fs::create_dir(&moved).unwrap();
        let first = super::DownloadDestinationRoot::open(&original)
            .unwrap()
            .resolve(std::path::Path::new("model"))
            .unwrap();
        // A clone retains logical library identity, not filesystem authority.
        std::fs::copy(
            original.join(".pumas-library-id.json"),
            moved.join(".pumas-library-id.json"),
        )
        .unwrap();
        let second = super::DownloadDestinationRoot::open(&moved)
            .unwrap()
            .resolve(std::path::Path::new("model"))
            .unwrap();
        assert_ne!(first.identity(), second.identity());
        assert_eq!(
            first.persisted_identity().unwrap(),
            second.persisted_identity().unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn destination_identity_survives_aliases_missing_tail_and_creation() {
        let temp = tempfile::TempDir::new().unwrap();
        let root_path = temp.path().join("library");
        std::fs::create_dir(&root_path).unwrap();
        let alias = temp.path().join("alias");
        std::os::unix::fs::symlink(&root_path, &alias).unwrap();
        let root = super::DownloadDestinationRoot::open(&root_path).unwrap();
        let destination = root.resolve(std::path::Path::new("llm/model")).unwrap();
        let identity = destination.identity();
        assert_eq!(
            identity,
            root.resolve(&alias.join("llm/model")).unwrap().identity()
        );
        destination.prepare().unwrap();
        assert_eq!(
            identity,
            root.resolve(&root_path.join("llm/model"))
                .unwrap()
                .identity()
        );
        assert_eq!(
            destination.persisted_identity().unwrap().relative_target,
            "llm/model"
        );
        let marker = serde_json::json!({"repo_id":"author/model", "files":["model.gguf"]});
        assert!(matches!(
            destination.write_marker(&marker).unwrap(),
            crate::metadata::AtomicPublication::Durable
        ));
        let actual: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root_path.join("llm/model/.pumas_download")).unwrap(),
        )
        .unwrap();
        assert_eq!(actual, marker);
    }

    #[cfg(unix)]
    #[test]
    fn destination_rejects_nested_symlinks_and_escape() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        std::fs::create_dir(temp.path().join("real")).unwrap();
        std::os::unix::fs::symlink("real", temp.path().join("alias")).unwrap();
        assert!(root.resolve(std::path::Path::new("alias/model")).is_err());
        assert!(root.resolve(std::path::Path::new("../escape")).is_err());
        assert!(root.resolve(std::path::Path::new(".")).is_err());
    }

    #[cfg(unix)]
    #[test]
    fn destination_rejects_replaced_model_and_missing_target_parent() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        std::fs::create_dir(temp.path().join("parent")).unwrap();
        let missing = root
            .resolve(std::path::Path::new("parent/missing"))
            .unwrap();
        std::fs::rename(temp.path().join("parent"), temp.path().join("old-parent")).unwrap();
        std::fs::create_dir(temp.path().join("parent")).unwrap();
        assert!(missing.prepare().is_err());
        assert!(!temp.path().join("parent/missing").exists());
        let destination = root.resolve(std::path::Path::new("model")).unwrap();
        destination.prepare().unwrap();
        std::fs::rename(temp.path().join("model"), temp.path().join("old-model")).unwrap();
        std::fs::create_dir(temp.path().join("model")).unwrap();
        assert!(destination.open_part("weights.gguf", false).is_err());
        assert!(destination
            .write_marker(&serde_json::json!({"files":[]}))
            .is_err());
        assert!(!temp.path().join("model/.pumas_download").exists());
    }

    #[cfg(unix)]
    #[test]
    fn destination_root_replacement_refuses_all_new_effects() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("root");
        std::fs::create_dir(&path).unwrap();
        let root = super::DownloadDestinationRoot::open(&path).unwrap();
        let destination = root.resolve(std::path::Path::new("missing")).unwrap();
        std::fs::rename(&path, temp.path().join("old-root")).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(destination.prepare().is_err());
        assert!(root.resolve(std::path::Path::new("missing")).is_err());
        assert!(!path.join("missing").exists());
    }

    #[cfg(unix)]
    #[test]
    fn file_probes_distinguish_uncreated_paths_from_lost_authority() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let destination = root.resolve(std::path::Path::new("model")).unwrap();
        assert_eq!(destination.file_len("weights.gguf").unwrap(), None);
        assert_eq!(destination.part_len("weights.gguf").unwrap(), None);

        destination.prepare().unwrap();
        assert_eq!(destination.file_len("nested/weights.gguf").unwrap(), None);
        assert_eq!(destination.part_len("nested/weights.gguf").unwrap(), None);
        destination.create_parent("nested/weights.gguf").unwrap();
        std::fs::rename(
            temp.path().join("model/nested"),
            temp.path().join("model/old-nested"),
        )
        .unwrap();
        assert!(destination.file_len("nested/weights.gguf").is_err());
        assert!(destination.part_len("nested/weights.gguf").is_err());

        std::fs::rename(temp.path().join("model"), temp.path().join("old-model")).unwrap();
        assert!(destination.file_len("weights.gguf").is_err());
        assert!(destination.part_len("weights.gguf").is_err());
    }

    #[cfg(unix)]
    #[test]
    fn partial_cleanup_requires_exact_parent_sync_even_on_absent_retry() {
        assert_cleanup_parent_sync(Some("nested/weights.gguf"));
    }

    #[cfg(unix)]
    #[test]
    fn marker_cleanup_requires_exact_parent_sync_even_on_absent_retry() {
        assert_cleanup_parent_sync(None);
    }

    #[cfg(unix)]
    fn assert_cleanup_parent_sync(partial: Option<&str>) {
        use std::os::unix::fs::MetadataExt;
        use std::sync::atomic::{AtomicUsize, Ordering};
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let mut destination = root.resolve(std::path::Path::new("model")).unwrap();
        destination.prepare().unwrap();
        let relative = match partial {
            Some(filename) => {
                destination.create_parent(filename).unwrap();
                format!("{filename}.part")
            }
            None => ".pumas_download".into(),
        };
        let path = temp.path().join("model").join(&relative);
        std::fs::write(&path, b"old download state").unwrap();
        let expected_parent = std::fs::metadata(path.parent().unwrap()).unwrap();
        let leaf = path.file_name().unwrap().to_owned();
        let sync_calls = std::sync::Arc::new(AtomicUsize::new(0));
        destination.cleanup_parent_sync = Some(std::sync::Arc::new({
            let sync_calls = sync_calls.clone();
            move |parent| {
                let actual = parent.open(".")?.into_std().metadata()?;
                assert_eq!(
                    (actual.dev(), actual.ino()),
                    (expected_parent.dev(), expected_parent.ino())
                );
                assert!(matches!(parent.symlink_metadata(&leaf), Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound));
                sync_calls.fetch_add(1, Ordering::SeqCst);
                Err(std::io::Error::from_raw_os_error(libc::EIO))
            }
        }));
        let remove = |destination: &super::DownloadRecoveryDestination| match partial {
            Some(filename) => destination.remove_part(filename),
            None => destination.remove_marker(),
        };
        let error =
            remove(&destination).expect_err("a real unlink must propagate parent sync failure");
        assert_eq!(error.raw_os_error(), Some(libc::EIO));
        assert!(!path.exists());
        let retry = remove(&destination)
            .expect_err("absent-file retry must still establish directory durability");
        assert_eq!(retry.raw_os_error(), Some(libc::EIO));
        assert_eq!(sync_calls.load(Ordering::SeqCst), 2);
        destination.cleanup_parent_sync = None;
        remove(&destination).unwrap();
        assert!(!path.exists());
    }

    #[cfg(unix)]
    #[test]
    fn failed_cleanup_unlink_preserves_target_and_does_not_claim_parent_sync() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let mut destination = root.resolve(std::path::Path::new("model")).unwrap();
        destination.prepare().unwrap();
        destination.create_parent("nested/weights.gguf").unwrap();
        let partial = temp.path().join("model/nested/weights.gguf.part");
        let marker = temp.path().join("model/.pumas_download");
        for path in [&partial, &marker] {
            std::fs::create_dir(path).unwrap();
            std::fs::write(path.join("retained"), b"not a download file").unwrap();
        }
        let sync_calls = std::sync::Arc::new(AtomicUsize::new(0));
        destination.cleanup_parent_sync = Some(std::sync::Arc::new({
            let sync_calls = sync_calls.clone();
            move |_| {
                sync_calls.fetch_add(1, Ordering::SeqCst);
                Ok(())
            }
        }));
        let partial_error = std::fs::remove_file(&partial).unwrap_err().raw_os_error();
        let marker_error = std::fs::remove_file(&marker).unwrap_err().raw_os_error();
        assert!(partial_error.is_some());
        assert!(marker_error.is_some());
        assert_eq!(
            destination
                .remove_part("nested/weights.gguf")
                .unwrap_err()
                .raw_os_error(),
            partial_error
        );
        assert_eq!(
            destination.remove_marker().unwrap_err().raw_os_error(),
            marker_error
        );
        assert_eq!(sync_calls.load(Ordering::SeqCst), 0);
        for path in [&partial, &marker] {
            assert_eq!(
                std::fs::read(path.join("retained")).unwrap(),
                b"not a download file"
            );
        }
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_accepts_uncreated_target_but_refuses_lost_authority() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let destination = root.resolve(std::path::Path::new("model")).unwrap();
        destination.remove_part("weights.gguf").unwrap();
        destination.remove_marker().unwrap();
        assert!(destination.remove_part("../escape").is_err());
        assert!(!temp.path().join("model").exists());

        destination.prepare().unwrap();
        std::fs::remove_dir(temp.path().join("model")).unwrap();
        assert!(destination.remove_part("weights.gguf").is_err());
        assert!(destination.remove_marker().is_err());

        std::fs::create_dir(temp.path().join("parent")).unwrap();
        let anchored = root.resolve(std::path::Path::new("parent/model")).unwrap();
        std::fs::rename(temp.path().join("parent"), temp.path().join("old-parent")).unwrap();
        std::fs::create_dir(temp.path().join("parent")).unwrap();
        assert!(anchored.remove_part("weights.gguf").is_err());
        assert!(anchored.remove_marker().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn cleanup_distinguishes_uncreated_nested_parent_from_lost_parent() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let destination = root.resolve(std::path::Path::new("model")).unwrap();
        destination.prepare().unwrap();
        destination.remove_part("nested/weights.gguf").unwrap();
        assert!(!temp.path().join("model/nested").exists());

        destination.create_parent("nested/weights.gguf").unwrap();
        std::fs::remove_dir(temp.path().join("model/nested")).unwrap();
        assert!(destination.remove_part("nested/weights.gguf").is_err());
        assert!(!temp.path().join("model/nested").exists());
    }

    #[cfg(unix)]
    #[test]
    fn ordinary_and_recovery_destinations_have_one_identity() {
        let temp = tempfile::TempDir::new().unwrap();
        let path = temp.path().join("library");
        let record = partial_record(&path, vec!["weights.gguf"]);
        let root = super::DownloadDestinationRoot::open(&path).unwrap();
        let ordinary = root.resolve(std::path::Path::new(&record.path)).unwrap();
        let recovery = std::sync::Arc::new(super::RecoveryRoot::open(&path).unwrap().unwrap())
            .destination_for(&record)
            .unwrap();
        assert_eq!(ordinary.identity(), recovery.identity());
        assert_eq!(ordinary.library_model_id(), record.id);
        assert_eq!(recovery.library_model_id(), record.id);
        assert_eq!(
            ordinary.persisted_identity().unwrap(),
            recovery.persisted_identity().unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn destination_rejects_nested_parent_replacement_between_file_effects() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = super::DownloadDestinationRoot::open(temp.path()).unwrap();
        let destination = root.resolve(std::path::Path::new("model")).unwrap();
        destination.prepare().unwrap();
        let _part = destination.open_part("nested/model.bin", false).unwrap();
        let nested = temp.path().join("model/nested");
        std::fs::rename(&nested, temp.path().join("model/old-nested")).unwrap();
        std::fs::create_dir(&nested).unwrap();
        let replacement = nested.join("model.bin.part");
        std::fs::write(&replacement, b"replacement").unwrap();
        assert!(destination.rename_part_to_file("nested/model.bin").is_err());
        assert!(destination.remove_part("nested/model.bin").is_err());
        assert_eq!(std::fs::read(replacement).unwrap(), b"replacement");
        assert!(!nested.join("model.bin").exists());
    }
    use crate::ModelRecord;
    use serde_json::{json, Value};
    use std::collections::HashMap;
    use std::sync::Arc;
    use tempfile::TempDir;

    use super::{
        issue_download_recovery_ticket, verify_download_recovery_ticket, DownloadRecoveryModelId,
        DownloadRecoveryToken, DownloadRecoveryVerification, RecoveryRoot,
    };

    fn partial_record(root: &std::path::Path, files: Vec<&str>) -> ModelRecord {
        let model_dir = root.join("llm/acme/model");
        std::fs::create_dir_all(&model_dir).unwrap();
        let model_dir = std::fs::canonicalize(model_dir).unwrap();
        ModelRecord {
            id: "llm/acme/model".to_string(),
            path: model_dir.display().to_string(),
            cleaned_name: "model".to_string(),
            official_name: "Model".to_string(),
            model_type: "llm".to_string(),
            tags: Vec::new(),
            hashes: HashMap::new(),
            metadata: json!({
                "download_incomplete": true,
                "repo_id": "acme/model",
                "selected_artifact_id": "acme/model::Q4_K_M",
                "selected_artifact_files": files,
                "selected_artifact_quant": "Q4_K_M"
            }),
            updated_at: "2026-09-03T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn recovery_fingerprint_is_canonical_and_stales_on_semantic_change() {
        let temp = TempDir::new().unwrap();
        let root = temp.path();
        let original = partial_record(root, vec!["weights-2.gguf", "weights-1.gguf"]);
        let ticket = issue_download_recovery_ticket(root, &original)
            .unwrap()
            .unwrap();

        assert_eq!(ticket.token().len(), 67);
        assert!(ticket.token().starts_with("v1:"));
        assert!(ticket.token()[3..]
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit()));
        assert_eq!(
            ticket.selected_artifact_files(),
            &["weights-1.gguf".to_string(), "weights-2.gguf".to_string()]
        );

        let reordered = partial_record(root, vec!["weights-1.gguf", "weights-2.gguf"]);
        assert_eq!(
            issue_download_recovery_ticket(root, &reordered)
                .unwrap()
                .unwrap()
                .token(),
            ticket.token(),
            "file order is not semantic"
        );
        let duplicated = partial_record(
            root,
            vec!["weights-2.gguf", "weights-1.gguf", "weights-1.gguf"],
        );
        assert_eq!(
            issue_download_recovery_ticket(root, &duplicated)
                .unwrap()
                .unwrap()
                .token(),
            ticket.token(),
            "duplicate metadata entries normalize to the same set"
        );

        for changed in [
            partial_record(root, vec!["weights-1.gguf"]),
            partial_record(root, vec!["weights-1.gguf", "weights-3.gguf"]),
        ] {
            assert_ne!(
                issue_download_recovery_ticket(root, &changed)
                    .unwrap()
                    .unwrap()
                    .token(),
                ticket.token()
            );
        }

        for (field, value) in [
            ("repo_id", json!("Acme/model")),
            ("repo_id", json!("acme/other")),
            ("selected_artifact_id", json!("acme/model::Q5_K_M")),
            ("selected_artifact_quant", json!("Q5_K_M")),
        ] {
            let mut changed = original.clone();
            changed.metadata[field] = value;
            assert_ne!(
                issue_download_recovery_ticket(root, &changed)
                    .unwrap()
                    .unwrap()
                    .token(),
                ticket.token()
            );
        }

        let moved_dir = root.join("llm/acme/moved");
        std::fs::create_dir_all(&moved_dir).unwrap();
        let mut moved = original.clone();
        moved.id = "llm/acme/moved".to_string();
        moved.path = std::fs::canonicalize(moved_dir)
            .unwrap()
            .display()
            .to_string();
        assert_ne!(
            issue_download_recovery_ticket(root, &moved)
                .unwrap()
                .unwrap()
                .token(),
            ticket.token()
        );

        let mut complete = original;
        complete.metadata["download_incomplete"] = json!(false);
        assert!(issue_download_recovery_ticket(root, &complete)
            .unwrap()
            .is_none());
    }

    #[test]
    fn recovery_action_identifiers_have_exact_portable_grammars() {
        assert!(DownloadRecoveryModelId::parse("llm/acme/model").is_some());
        for invalid in [
            "",
            "/llm/acme/model",
            "../model",
            "llm\\acme\\model",
            "C:/models/model",
            "llm//model",
            "llm/CON/model",
        ] {
            assert!(
                DownloadRecoveryModelId::parse(invalid).is_none(),
                "{invalid}"
            );
        }

        let valid = format!("v1:{}", "a".repeat(64));
        assert!(DownloadRecoveryToken::parse(&valid).is_some());
        for invalid in [
            "",
            "v1:abc",
            &format!("v2:{}", "a".repeat(64)),
            &format!("v1:{}", "A".repeat(64)),
            &format!("v1:{}", "g".repeat(64)),
            &format!("v1:{}", "a".repeat(65)),
        ] {
            assert!(DownloadRecoveryToken::parse(invalid).is_none(), "{invalid}");
        }
    }

    #[test]
    fn recovery_ticket_requires_managed_owned_path_and_provenance() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("library");
        std::fs::create_dir_all(&root).unwrap();
        let managed = partial_record(&root, vec!["weights.gguf"]);
        assert!(issue_download_recovery_ticket(&root, &managed)
            .unwrap()
            .is_some());

        let outside_root = temp.path().join("outside/llm/acme/model");
        std::fs::create_dir_all(&outside_root).unwrap();
        let mut outside = managed.clone();
        outside.path = outside_root.display().to_string();
        assert!(issue_download_recovery_ticket(&root, &outside)
            .unwrap()
            .is_none());

        let mut alias = managed.clone();
        alias.id = "llm/acme/alias".to_string();
        assert!(issue_download_recovery_ticket(&root, &alias)
            .unwrap()
            .is_none());

        let mut missing = managed.clone();
        missing.path = root.join("llm/acme/missing").display().to_string();
        assert!(issue_download_recovery_ticket(&root, &missing)
            .unwrap()
            .is_none());

        let mut no_provenance = managed.clone();
        no_provenance.metadata["repo_id"] = Value::Null;
        assert!(issue_download_recovery_ticket(&root, &no_provenance)
            .unwrap()
            .is_none());

        let ticket = issue_download_recovery_ticket(&root, &managed)
            .unwrap()
            .unwrap();
        let stale = DownloadRecoveryToken::parse(&format!("v1:{}", "b".repeat(64))).unwrap();
        assert!(matches!(
            verify_download_recovery_ticket(&root, &managed, &stale).unwrap(),
            DownloadRecoveryVerification::Stale
        ));
        let current = DownloadRecoveryToken::parse(ticket.token()).unwrap();
        assert!(matches!(
            verify_download_recovery_ticket(&root, &managed, &current).unwrap(),
            DownloadRecoveryVerification::Verified(_)
        ));
    }

    #[test]
    fn recovery_ticket_refuses_non_main_revision_without_mutation() {
        let temp = TempDir::new().unwrap();
        let mut record = partial_record(temp.path(), vec!["weights.gguf"]);
        let ticket = issue_download_recovery_ticket(temp.path(), &record)
            .unwrap()
            .unwrap();
        let token = DownloadRecoveryToken::parse(ticket.token()).unwrap();
        for revision in ["0123456789abcdef0123456789abcdef01234567", "release-tag"] {
            record.metadata["upstream_revision"] = json!(revision);
            assert!(issue_download_recovery_ticket(temp.path(), &record)
                .unwrap()
                .is_none());
            assert!(matches!(
                verify_download_recovery_ticket(temp.path(), &record, &token).unwrap(),
                DownloadRecoveryVerification::Unavailable
            ));
        }
        assert!(!temp.path().join(super::LIBRARY_ID_MARKER).exists());
        record.metadata["upstream_revision"] = json!("main");
        assert!(issue_download_recovery_ticket(temp.path(), &record)
            .unwrap()
            .is_some());
    }

    #[cfg(unix)]
    #[test]
    fn recovery_ticket_rejects_model_directory_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let root = temp.path().join("library");
        let target = root.join("llm/acme/target");
        let link = root.join("llm/acme/model");
        std::fs::create_dir_all(&target).unwrap();
        symlink(&target, &link).unwrap();
        let record = partial_record(&root, vec!["weights.gguf"]);
        assert!(issue_download_recovery_ticket(&root, &record)
            .unwrap()
            .is_none());
    }

    #[cfg(unix)]
    #[test]
    fn recovery_ticket_omits_authority_for_nested_and_partial_file_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let root = temp.path().join("library");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();

        let nested = partial_record(&root, vec!["nested/weights.gguf"]);
        symlink(&outside, root.join("llm/acme/model/nested")).unwrap();
        assert!(issue_download_recovery_ticket(&root, &nested)
            .unwrap()
            .is_none());

        std::fs::remove_file(root.join("llm/acme/model/nested")).unwrap();
        let partial = partial_record(&root, vec!["weights.gguf"]);
        symlink(
            outside.join("escaped.part"),
            root.join("llm/acme/model/weights.gguf.part"),
        )
        .unwrap();
        assert!(issue_download_recovery_ticket(&root, &partial)
            .unwrap()
            .is_none());
        assert!(!outside.join("escaped.part").exists());
    }

    #[cfg(unix)]
    #[test]
    fn verified_authority_rejects_target_replacement_without_outside_mutation() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let root = temp.path().join("library");
        let outside = temp.path().join("outside");
        std::fs::create_dir_all(&outside).unwrap();
        let record = partial_record(&root, vec!["weights.gguf"]);
        let ticket = issue_download_recovery_ticket(&root, &record)
            .unwrap()
            .unwrap();
        let token = DownloadRecoveryToken::parse(ticket.token()).unwrap();
        let DownloadRecoveryVerification::Verified(verified) =
            verify_download_recovery_ticket(&root, &record, &token).unwrap()
        else {
            panic!("recovery fixture must verify");
        };

        let original = root.join("llm/acme/model");
        let part = original.join("weights.gguf.part");
        symlink(outside.join("escaped.part"), &part).unwrap();
        assert!(verified
            .destination
            .open_part("weights.gguf", false)
            .is_err());
        assert!(!outside.join("escaped.part").exists());
        std::fs::remove_file(part).unwrap();

        std::fs::rename(&original, root.join("llm/acme/replaced-model")).unwrap();
        symlink(&outside, &original).unwrap();

        assert!(verified.destination.preflight(&verified.files).is_err());
        assert!(verified
            .destination
            .open_part("weights.gguf", false)
            .is_err());
        assert!(!outside.join("weights.gguf.part").exists());
    }

    #[test]
    fn recovery_token_rejects_library_root_replacement_with_same_display_path() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("library");
        let record = partial_record(&root, vec!["weights.gguf"]);
        std::fs::write(root.join("old-root-sentinel"), b"old").unwrap();
        let ticket = issue_download_recovery_ticket(&root, &record)
            .unwrap()
            .unwrap();
        let token = DownloadRecoveryToken::parse(ticket.token()).unwrap();

        let original_root = temp.path().join("original-library");
        std::fs::rename(&root, &original_root).unwrap();
        let replacement = partial_record(&root, vec!["weights.gguf"]);
        std::fs::write(root.join("replacement-root-sentinel"), b"replacement").unwrap();

        assert!(matches!(
            verify_download_recovery_ticket(&root, &replacement, &token).unwrap(),
            DownloadRecoveryVerification::Stale | DownloadRecoveryVerification::Unavailable
        ));
        assert_eq!(
            std::fs::read(original_root.join("old-root-sentinel")).unwrap(),
            b"old"
        );
        assert_eq!(
            std::fs::read(root.join("replacement-root-sentinel")).unwrap(),
            b"replacement"
        );
    }

    #[test]
    fn held_root_rejects_replacement_before_model_validation() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("library");
        let record = partial_record(&root, vec!["weights.gguf"]);
        std::fs::write(root.join("held-root-sentinel"), b"held").unwrap();
        let authority = Arc::new(RecoveryRoot::open(&root).unwrap().unwrap());

        let original_root = temp.path().join("original-library");
        std::fs::rename(&root, &original_root).unwrap();
        let replacement = partial_record(&root, vec!["weights.gguf"]);
        std::fs::write(root.join("replacement-root-sentinel"), b"replacement").unwrap();

        assert!(authority.destination_for(&replacement).is_none());
        assert_eq!(
            std::fs::read(original_root.join("held-root-sentinel")).unwrap(),
            b"held"
        );
        assert_eq!(record.path, replacement.path);
    }
}
