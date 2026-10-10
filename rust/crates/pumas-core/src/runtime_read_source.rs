//! Retained selected runtime bytes, independent of execution qualification.
//!
//! A runtime installer supplies held directory capabilities and cooperative
//! mutation leases. Capture reads the actual closed selected tree; reports and
//! submitted hashes never create audio admission. Roots and leases remain owned
//! until the final Arc is dropped, normally after exact-child-tree drain.
//! External system libraries and hostile same-user mutation are outside this
//! byte-custody scope. Linux is the currently supported capture platform.

use crate::platform::capability_fs::open_pinned_directory_at;
use cap_std::fs::{Dir, OpenOptions, OpenOptionsExt};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::sync::Arc;

/// Source-fixed candidate bytes. Public metadata is not execution authority;
/// actual retained byte selections must match and be revalidated independently.
pub const AUDIO_RUNTIME_CANDIDATE_RECIPE: &str =
    include_str!("runtime_read_source/audio_candidate_recipe.json");

const MAX_MEMBERS: usize = 200_000;
const MAX_NAME: usize = 1024;

/// Roles describe selected owned roots, not complete native execution closure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum RuntimeReadRole {
    Interpreter,
    Dependencies,
    Sidecar,
    /// Source-pinned owned system loader/libraries, never ambient host paths.
    NativeLibraries,
}

/// A bounded expected installed member. Its actual bytes are always re-read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeReadFile {
    path: String,
    size: u64,
    sha256: String,
}

impl RuntimeReadFile {
    pub fn new(path: String, size: u64, sha256: String) -> io::Result<Self> {
        valid_name(&path)?;
        if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(refusal("runtime member digest is invalid"));
        }
        Ok(Self {
            path,
            size,
            sha256: sha256.to_ascii_lowercase(),
        })
    }
    pub fn path(&self) -> &str {
        &self.path
    }
    pub fn size(&self) -> u64 {
        self.size
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

/// Ownership transferred by an installer, never decoded from request JSON.
/// Exclusions record explicitly unselected regular files, directories or aliases;
/// they do not attest its contents. Selected trees remain closed otherwise.
pub struct RuntimeReadRoot {
    role: RuntimeReadRole,
    directory: File,
    expected: Vec<RuntimeReadFile>,
    excluded: Vec<String>,
    lease: Arc<dyn Send + Sync>,
}

impl RuntimeReadRoot {
    pub fn new(
        role: RuntimeReadRole,
        directory: File,
        expected: Vec<RuntimeReadFile>,
        excluded: Vec<String>,
        lease: Arc<dyn Send + Sync>,
    ) -> io::Result<Self> {
        if !directory.metadata()?.is_dir() || expected.is_empty() || expected.len() > MAX_MEMBERS {
            return Err(refusal("runtime root or selected member count is invalid"));
        }
        let mut paths = BTreeSet::new();
        for member in &expected {
            if !paths.insert(member.path.clone()) {
                return Err(refusal("duplicate runtime member"));
            }
        }
        let mut exclusions = BTreeSet::new();
        for name in &excluded {
            valid_name(name)?;
            if !exclusions.insert(name.clone()) || paths.iter().any(|path| below(path, name)) {
                return Err(refusal("runtime exclusion overlaps selected bytes"));
            }
        }
        if exclusions.iter().any(|path| {
            exclusions
                .iter()
                .any(|other| path != other && below(path, other))
        }) {
            return Err(refusal("overlapping runtime exclusions"));
        }
        Ok(Self {
            role,
            directory,
            expected,
            excluded,
            lease,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Identity {
    device: u64,
    inode: u64,
}

struct Member {
    manifest: RuntimeReadFile,
    identity: Identity,
}
#[derive(Debug, PartialEq, Eq)]
struct Excluded {
    identity: Identity,
    link: Option<PathBuf>,
}
struct Root {
    role: RuntimeReadRole,
    directory: Dir,
    members: Vec<Member>,
    directories: BTreeMap<String, Identity>,
    excluded: BTreeMap<String, Excluded>,
    _lease: Arc<dyn Send + Sync>,
}

/// Actual selected-tree custody. This type has no availability/qualification
/// field and cannot be reconstructed from a manifest or a wire receipt.
pub struct RetainedRuntimeReadSource {
    roots: Vec<Root>,
}

impl std::fmt::Debug for RetainedRuntimeReadSource {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RetainedRuntimeReadSource")
            .field(
                "roles",
                &self.roots.iter().map(|root| root.role).collect::<Vec<_>>(),
            )
            .field(
                "selected_members",
                &self
                    .roots
                    .iter()
                    .map(|root| root.members.len())
                    .sum::<usize>(),
            )
            .finish_non_exhaustive()
    }
}

impl RetainedRuntimeReadSource {
    pub fn capture(selections: Vec<RuntimeReadRoot>) -> io::Result<Arc<Self>> {
        if !cfg!(target_os = "linux") {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "runtime byte capture supports Linux only",
            ));
        }
        if selections.is_empty() || selections.len() > 4 {
            return Err(refusal("invalid runtime root count"));
        }
        let mut roles = BTreeSet::new();
        let mut roots = Vec::new();
        for selection in selections {
            if !roles.insert(selection.role) {
                return Err(refusal("duplicate runtime root role"));
            }
            let directory = Dir::from_std_file(selection.directory);
            let exclusions: BTreeSet<_> = selection.excluded.into_iter().collect();
            let (files, directories, excluded) = namespace(&directory, &exclusions)?;
            let expected: BTreeSet<_> = selection
                .expected
                .iter()
                .map(|member| member.path.clone())
                .collect();
            if files != expected {
                return Err(refusal(
                    "runtime selected namespace differs from installed manifest",
                ));
            }
            let mut members = Vec::with_capacity(expected.len());
            for manifest in selection.expected {
                let mut file = open_member(&directory, &manifest.path)?;
                let identity = file_identity(&file)?;
                if digest(&mut file)? != (manifest.size, manifest.sha256.clone()) {
                    return Err(refusal(
                        "runtime selected bytes differ from installed manifest",
                    ));
                }
                members.push(Member { manifest, identity });
            }
            roots.push(Root {
                role: selection.role,
                directory,
                members,
                directories,
                excluded,
                _lease: selection.lease,
            });
        }
        let owner = Arc::new(Self { roots });
        owner.validate()?;
        Ok(owner)
    }

    /// Blocking pre-effect validation. Runtime operation borrows do not scan.
    pub fn validate(&self) -> io::Result<()> {
        for root in &self.roots {
            let exclusions = root.excluded.keys().cloned().collect();
            let (files, directories, excluded) = namespace(&root.directory, &exclusions)?;
            if files
                != root
                    .members
                    .iter()
                    .map(|member| member.manifest.path.clone())
                    .collect()
                || directories != root.directories
                || excluded != root.excluded
            {
                return Err(refusal("runtime retained namespace or identity changed"));
            }
            for member in &root.members {
                let mut file = open_member(&root.directory, &member.manifest.path)?;
                if file_identity(&file)? != member.identity
                    || digest(&mut file)? != (member.manifest.size, member.manifest.sha256.clone())
                {
                    return Err(refusal("runtime retained member identity or bytes changed"));
                }
            }
        }
        Ok(())
    }

    pub fn manifest(&self) -> impl Iterator<Item = (RuntimeReadRole, &RuntimeReadFile)> {
        self.roots.iter().flat_map(|root| {
            root.members
                .iter()
                .map(move |member| (root.role, &member.manifest))
        })
    }

    /// A direct duplicate of the held capability; ambient path replacement
    /// cannot substitute another root for this owner or its inherited child.
    pub fn clone_root(&self, role: RuntimeReadRole) -> io::Result<File> {
        Ok(self.root(role)?.directory.try_clone()?.into_std_file())
    }

    pub fn clone_member(&self, role: RuntimeReadRole, path: &str) -> io::Result<File> {
        let root = self.root(role)?;
        let member = root
            .members
            .iter()
            .find(|member| member.manifest.path == path)
            .ok_or_else(|| refusal("runtime member is outside retained selection"))?;
        let mut file = open_member(&root.directory, path)?;
        if file_identity(&file)? != member.identity
            || digest(&mut file)? != (member.manifest.size, member.manifest.sha256.clone())
        {
            return Err(refusal("runtime selected member changed"));
        }
        use std::io::{Seek, SeekFrom};
        file.seek(SeekFrom::Start(0))?;
        Ok(file)
    }

    /// Enumerate only identity-retained directories. Directory capabilities
    /// permit namespace traversal/enumeration, never recursive content reads.
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    pub(crate) fn directory_manifest(&self) -> impl Iterator<Item = (RuntimeReadRole, &str)> {
        self.roots.iter().flat_map(|root| {
            std::iter::once((root.role, "")).chain(
                root.directories
                    .keys()
                    .map(move |name| (root.role, name.as_str())),
            )
        })
    }

    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    pub(crate) fn clone_directory(&self, role: RuntimeReadRole, path: &str) -> io::Result<File> {
        if path.is_empty() {
            return self.clone_root(role);
        }
        let root = self.root(role)?;
        let expected = root
            .directories
            .get(path)
            .ok_or_else(|| refusal("runtime directory is outside retained selection"))?;
        let (parent, name) = open_parent(&root.directory, path)?;
        let directory =
            open_pinned_directory_at(&parent, std::ffi::OsStr::new(&name))?.into_std_file();
        if file_identity(&directory)? != *expected {
            return Err(refusal("runtime retained directory identity changed"));
        }
        Ok(directory)
    }

    fn root(&self, role: RuntimeReadRole) -> io::Result<&Root> {
        self.roots
            .iter()
            .find(|root| root.role == role)
            .ok_or_else(|| refusal("runtime role not retained"))
    }
}

fn valid_name(name: &str) -> io::Result<()> {
    if name.is_empty()
        || name.len() > MAX_NAME
        || name.contains('\\')
        || name
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..")
        || Path::new(name)
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err(refusal("runtime member name is not a closed relative path"));
    }
    Ok(())
}

fn below(path: &str, parent: &str) -> bool {
    path == parent
        || path
            .strip_prefix(parent)
            .is_some_and(|rest| rest.starts_with('/'))
}

fn open_parent(root: &Dir, name: &str) -> io::Result<(Dir, String)> {
    valid_name(name)?;
    let mut directory = root.try_clone()?;
    let mut parts = name.split('/').peekable();
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return Ok((directory, part.into()));
        }
        directory = open_pinned_directory_at(&directory, std::ffi::OsStr::new(part))?;
    }
    Err(refusal("runtime member name is empty"))
}

fn open_member(root: &Dir, name: &str) -> io::Result<File> {
    let (parent, leaf) = open_parent(root, name)?;
    let before = parent.symlink_metadata(&leaf)?;
    if !before.is_file() || before.is_symlink() {
        return Err(refusal("runtime member is not a regular non-link file"));
    }
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let file = parent.open_with(&leaf, &options)?.into_std();
    let metadata = file.metadata()?;
    if !metadata.is_file() {
        return Err(refusal("runtime member became a special file"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(refusal("runtime member has another hard-link owner"));
        }
    }
    Ok(file)
}

type Namespace = (
    BTreeSet<String>,
    BTreeMap<String, Identity>,
    BTreeMap<String, Excluded>,
);
fn namespace(root: &Dir, exclusions: &BTreeSet<String>) -> io::Result<Namespace> {
    let mut files = BTreeSet::new();
    let mut directories = BTreeMap::new();
    let mut excluded = BTreeMap::new();
    let mut queue = vec![(String::new(), root.try_clone()?)];
    while let Some((prefix, directory)) = queue.pop() {
        for entry in directory.entries()? {
            let entry = entry?;
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| refusal("runtime member name is not UTF-8"))?;
            let path = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            valid_name(&path)?;
            let metadata = directory.symlink_metadata(&name)?;
            if exclusions.contains(&path) {
                if !metadata.is_dir() && !metadata.is_symlink() && !metadata.is_file() {
                    return Err(refusal("runtime exclusions cannot contain special files"));
                }
                excluded.insert(
                    path,
                    Excluded {
                        identity: cap_identity(&metadata)?,
                        link: if metadata.is_symlink() {
                            // Record literal link contents without resolving an
                            // already-excluded alias or granting target reads.
                            Some(directory.read_link_contents(&name)?)
                        } else {
                            None
                        },
                    },
                );
            } else if metadata.is_dir() && !metadata.is_symlink() {
                let child = open_pinned_directory_at(&directory, std::ffi::OsStr::new(&name))?;
                directories.insert(
                    path.clone(),
                    file_identity(&child.try_clone()?.into_std_file())?,
                );
                queue.push((path, child));
            } else if metadata.is_file() && !metadata.is_symlink() {
                files.insert(path);
            } else {
                return Err(refusal(
                    "runtime selected namespace contains a link or special file",
                ));
            }
            if files.len() + directories.len() + excluded.len() > MAX_MEMBERS {
                return Err(refusal("runtime namespace exceeded member bound"));
            }
        }
    }
    if excluded.keys().cloned().collect::<BTreeSet<_>>() != *exclusions {
        return Err(refusal("runtime exclusion is missing"));
    }
    Ok((files, directories, excluded))
}

fn cap_identity(metadata: &cap_std::fs::Metadata) -> io::Result<Identity> {
    #[cfg(unix)]
    {
        use cap_std::fs::MetadataExt;
        Ok(Identity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "runtime identity unavailable",
        ))
    }
}
fn file_identity(file: &File) -> io::Result<Identity> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let metadata = file.metadata()?;
        Ok(Identity {
            device: metadata.dev(),
            inode: metadata.ino(),
        })
    }
    #[cfg(not(unix))]
    {
        let _ = file;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "runtime identity unavailable",
        ))
    }
}
fn digest(file: &mut File) -> io::Result<(u64, String)> {
    let mut sha = Sha256::new();
    let mut size = 0u64;
    let mut buffer = [0; 65536];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        size = size
            .checked_add(count as u64)
            .ok_or_else(|| refusal("runtime member length overflow"))?;
        sha.update(&buffer[..count]);
    }
    Ok((size, hex::encode(sha.finalize())))
}
fn refusal(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::PermissionDenied, message)
}

#[cfg(all(test, target_os = "linux"))]
#[path = "runtime_read_source/tests.rs"]
mod tests;

#[cfg(all(
    feature = "test-support",
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
#[path = "runtime_read_source/dependency_probe.rs"]
mod dependency_probe;

/// Explicit fixed, non-model qualification only. This cannot register an audio
/// endpoint or alter the shipping policy. Unsupported hosts refuse before spawn.
#[cfg(feature = "test-support")]
pub async fn qualify_audio_dependency_reads(
    source: Arc<RetainedRuntimeReadSource>,
) -> io::Result<serde_json::Value> {
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    {
        dependency_probe::run(source).await
    }
    #[cfg(not(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    )))]
    {
        let _ = source;
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "audio dependency qualification requires Linux x86_64",
        ))
    }
}

/// Read-only host preflight for the explicit dependency qualification driver.
#[cfg(feature = "test-support")]
pub fn audio_dependency_qualification_host_preflight() -> io::Result<()> {
    #[cfg(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    ))]
    {
        crate::platform::audio_read_boundary::AudioReadBoundary::supported_abi().map(|_| ())
    }
    #[cfg(not(all(
        target_os = "linux",
        target_arch = "x86_64",
        target_pointer_width = "64"
    )))]
    {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "audio dependency qualification requires Linux x86_64",
        ))
    }
}

#[path = "runtime_read_source/candidate_recipe.rs"]
mod candidate_recipe;

/// Check one retained role against source-fixed recipe bytes. This comparison
/// is never production admission. Relocated CPython source remains byte-held.
pub fn validate_audio_candidate_read_role(
    source: &RetainedRuntimeReadSource,
    role: RuntimeReadRole,
) -> io::Result<()> {
    candidate_recipe::validate(source, role)
}
