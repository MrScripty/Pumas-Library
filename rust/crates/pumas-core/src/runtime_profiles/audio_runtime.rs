//! Retained, copied runtime-code read sources for an exact managed child.
//!
//! The shipping constructor refuses qualification. Existing installed Torch
//! version checks do not prove immutable interpreter/dependency/loader code or
//! a complete model read set. Only unit tests can qualify a fixed controlled
//! code snapshot; that scope is not real ASR/runtime execution qualification.
//! Neither paths, JSON, fingerprints nor a child handshake create authority.

#![allow(dead_code)] // The private channel consumes this opaque owner next.

#[path = "audio_runtime/installed.rs"]
pub(crate) mod installed;
#[cfg(all(
    target_os = "linux",
    target_arch = "x86_64",
    target_pointer_width = "64"
))]
#[path = "audio_runtime/installed_child.rs"]
mod installed_child;

use super::audio_custody::AudioCustodyError;
use crate::model_library::artifact_use::PreparedArtifactUse;
use crate::platform::capability_fs::{open_pinned_directory, open_pinned_directory_at};
use crate::platform::process::same_file_identity;
use crate::{PumasError, Result};
use cap_std::fs::{Dir, OpenOptions, OpenOptionsExt};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Weak};

struct Member {
    path: String,
    size: u64,
    sha256: String,
    file: File,
}

enum Qualification {
    Unavailable,
    Installed {
        model_read_set: BTreeSet<String>,
        selected: Weak<PreparedArtifactUse>,
    },
    #[cfg(any(test, feature = "test-support"))]
    ControlledProcess {
        model_read_set: BTreeSet<String>,
        selected: Option<Weak<PreparedArtifactUse>>,
    },
}

/// Opaque retained source ownership. No Clone/Deserialize or public constructor.
/// A caller may retain its Arc; the child composite guard owns the final copy.
pub(crate) struct AudioRuntimeOwner {
    qualification: Qualification,
    source_path: Option<PathBuf>,
    source_root: Dir,
    members: Vec<Member>,
    directories: Vec<(String, Dir)>,
    copied_root: Dir,
    read_source: tempfile::TempDir,
    manifest_sha256: String,
    installed_bytes: Option<Arc<crate::runtime_read_source::RetainedRuntimeReadSource>>,
    installed_interpreter: Option<String>,
}

impl std::fmt::Debug for AudioRuntimeOwner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AudioRuntimeOwner")
            .field("member_count", &self.members.len())
            .finish_non_exhaustive()
    }
}

impl AudioRuntimeOwner {
    /// Conditional installed-owner construction from concrete retained custody.
    /// The private shipping policy resolver currently refuses: no verified full
    /// native/model read-containment and lifecycle policy exists. Neither the
    /// candidate nor decoded evidence can grant that missing qualification.
    pub(crate) fn for_installed_runtime(
        candidate: installed::InstalledAudioRuntimeCandidate,
        selected: &Arc<PreparedArtifactUse>,
    ) -> std::result::Result<Arc<Self>, AudioCustodyError> {
        if !candidate.owns_selected(selected) {
            return Err(AudioCustodyError::StaleIdentity);
        }
        // Query only, before candidate reads, child spawn or native effects.
        // No weaker platform or old-kernel execution path exists.
        crate::platform::require_audio_read_confinement()
            .map_err(|_| AudioCustodyError::ReadConfinementUnavailable)?;
        candidate.into_runtime_owner(selected, installed::InstalledAudioPolicy::shipping())
    }

    /// The same conditional constructor with a fixed, source-owned policy for
    /// dummy bytes only. Absent from shipping AND test-support library builds.
    #[cfg(test)]
    fn for_fixed_installed_fixture(
        candidate: installed::InstalledAudioRuntimeCandidate,
        selected: &Arc<PreparedArtifactUse>,
    ) -> std::result::Result<Arc<Self>, AudioCustodyError> {
        candidate.into_runtime_owner(selected, installed::InstalledAudioPolicy::fixed_fixture())
    }

    /// Private copied code path for the admitted child, never a locator grant.
    pub(crate) fn read_source_path(&self) -> &Path {
        self.read_source.path()
    }

    /// Duplicate the held copied root, rather than re-opening its locator.
    pub(crate) fn clone_read_source_directory(&self) -> std::io::Result<Dir> {
        self.copied_root.try_clone()
    }

    /// Attach independently captured installed bytes before sharing/child
    /// attachment. Byte custody never changes this owner's qualification.
    pub(crate) fn with_retained_installed_bytes(
        owner: Arc<Self>,
        installed: Arc<crate::runtime_read_source::RetainedRuntimeReadSource>,
    ) -> Result<Arc<Self>> {
        installed.validate()?;
        let mut owner =
            Arc::try_unwrap(owner).map_err(|_| refusal("runtime owner already shared"))?;
        if owner.installed_bytes.is_some() {
            return Err(refusal("runtime installed bytes already retained"));
        }
        owner.installed_bytes = Some(installed);
        Ok(Arc::new(owner))
    }

    pub(crate) fn manifest_sha256(&self) -> &str {
        &self.manifest_sha256
    }

    /// In-memory qualification check, without callback or filesystem work.
    pub(crate) fn permits_selected(&self, prepared: &PreparedArtifactUse) -> bool {
        match &self.qualification {
            Qualification::Unavailable => false,
            Qualification::Installed {
                model_read_set,
                selected,
            } => {
                selected
                    .upgrade()
                    .is_some_and(|bound| std::ptr::eq(bound.as_ref(), prepared))
                    && prepared
                        .manifest()
                        .map(|member| member.relative_path.clone())
                        .collect::<BTreeSet<_>>()
                        == *model_read_set
            }
            #[cfg(any(test, feature = "test-support"))]
            Qualification::ControlledProcess {
                model_read_set,
                selected,
            } => {
                selected
                    .as_ref()
                    .and_then(Weak::upgrade)
                    .is_some_and(|bound| std::ptr::eq(bound.as_ref(), prepared))
                    && prepared
                        .manifest()
                        .map(|member| member.relative_path.clone())
                        .collect::<BTreeSet<_>>()
                        == *model_read_set
            }
        }
    }

    /// Blocking pre-effect scan. Native operation borrows never call this.
    pub(crate) fn validate_source(&self) -> Result<()> {
        if let Some(installed) = &self.installed_bytes {
            installed.validate()?;
        }
        if let Some(source_path) = &self.source_path {
            if !same_directory(&self.source_root, &open_pinned_directory(source_path)?)? {
                return Err(refusal("runtime source root identity changed"));
            }
        }
        if !same_directory(
            &self.copied_root,
            &open_pinned_directory(self.read_source.path())?,
        )? {
            return Err(refusal("runtime source root identity changed"));
        }
        let mut files = BTreeSet::new();
        let mut directories = BTreeSet::new();
        collect_members(&self.copied_root, "", &mut files, &mut directories)?;
        if files
            != self
                .members
                .iter()
                .map(|member| member.path.clone())
                .collect()
            || directories
                != self
                    .directories
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect()
        {
            return Err(refusal("runtime copied member set changed"));
        }
        for (path, directory) in &self.directories {
            let current = open_relative_directory(&self.copied_root, Path::new(path))?;
            if !same_directory(directory, &current)? {
                return Err(refusal("runtime copied directory identity changed"));
            }
        }
        for member in &self.members {
            let mut current = open_relative_file(&self.copied_root, &member.path)?;
            if !same_file_identity(&member.file, &current)?
                || current.metadata()?.len() != member.size
                || hash_reader(&mut current)? != (member.size, member.sha256.clone())
            {
                return Err(refusal("runtime copied bytes changed"));
            }
        }
        Ok(())
    }

    fn copied_identity_coherent(&self) -> Result<()> {
        if !same_directory(
            &self.copied_root,
            &open_pinned_directory(self.read_source.path())?,
        )? {
            return Err(refusal("runtime scratch root was replaced"));
        }
        let mut files = BTreeSet::new();
        let mut directories = BTreeSet::new();
        collect_members(&self.copied_root, "", &mut files, &mut directories)?;
        if files
            != self
                .members
                .iter()
                .map(|member| member.path.clone())
                .collect()
            || directories
                != self
                    .directories
                    .iter()
                    .map(|(path, _)| path.clone())
                    .collect()
        {
            return Err(refusal("runtime scratch membership changed"));
        }
        for (path, directory) in &self.directories {
            if !same_directory(
                directory,
                &open_relative_directory(&self.copied_root, Path::new(path))?,
            )? {
                return Err(refusal("runtime scratch directory was replaced"));
            }
        }
        for member in &self.members {
            if !same_file_identity(
                &member.file,
                &open_relative_file(&self.copied_root, &member.path)?,
            )? {
                return Err(refusal("runtime scratch file was replaced"));
            }
        }
        Ok(())
    }

    /// Actual-copy qualification of fixed controlled-process code only. The
    /// fixture's complete model read set is selected by trusted test code, never
    /// decoded from native replies. This factory is absent from shipping builds.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn controlled_fixture(
        source_path: &Path,
        selected_code: &[&str],
        model_read_set: &[&str],
    ) -> Result<Arc<Self>> {
        let selected = validated_names(selected_code)?;
        let read_set = validated_names(model_read_set)?;
        let mut owner = Self::snapshot(source_path, &selected)?;
        owner.qualification = Qualification::ControlledProcess {
            model_read_set: read_set,
            selected: None,
        };
        Ok(Arc::new(owner))
    }

    /// A fixed inherited model source must match this exact prepared owner,
    /// rather than another selection with equal names, bytes or a wire locator.
    /// Weak keeps allocation identity stable without extending its root grant
    /// after validated native unload. Code-only fixtures never admit model use.
    #[cfg(any(test, feature = "test-support"))]
    pub(crate) fn controlled_fixture_for_selected(
        source_path: &Path,
        selected_code: &[&str],
        model_read_set: &[&str],
        prepared: &Arc<PreparedArtifactUse>,
    ) -> Result<Arc<Self>> {
        let selected_code = validated_names(selected_code)?;
        let read_set = validated_names(model_read_set)?;
        if prepared
            .manifest()
            .map(|member| member.relative_path.clone())
            .collect::<BTreeSet<_>>()
            != read_set
        {
            return Err(refusal("controlled selected source read set differs"));
        }
        let mut owner = Self::snapshot(source_path, &selected_code)?;
        owner.qualification = Qualification::ControlledProcess {
            model_read_set: read_set,
            selected: Some(Arc::downgrade(prepared)),
        };
        Ok(Arc::new(owner))
    }

    fn snapshot(source_path: &Path, selected: &BTreeSet<String>) -> Result<Self> {
        Self::snapshot_directory(
            open_pinned_directory(source_path)?,
            Some(source_path.to_owned()),
            selected,
        )
    }

    /// Copy from the held directory capability, with no ambient locator grant.
    fn snapshot_directory(
        source_root: Dir,
        source_path: Option<PathBuf>,
        selected: &BTreeSet<String>,
    ) -> Result<Self> {
        let read_source = tempfile::Builder::new()
            .prefix("pumas-audio-code-")
            .tempdir()?;
        let copied_root = open_pinned_directory(read_source.path())?;
        let mut owner = Self {
            qualification: Qualification::Unavailable,
            source_path,
            source_root,
            members: Vec::with_capacity(selected.len()),
            directories: vec![],
            copied_root,
            read_source,
            manifest_sha256: String::new(),
            installed_bytes: None,
            installed_interpreter: None,
        };
        let mut created = BTreeSet::new();
        for path in selected {
            let mut original = open_relative_file(&owner.source_root, path)?;
            let before = original.metadata()?;
            if let Some(parent) = Path::new(path).parent() {
                let mut prefix = PathBuf::new();
                for component in parent.components() {
                    prefix.push(component.as_os_str());
                    let name = prefix
                        .to_str()
                        .ok_or_else(|| refusal("runtime name is not UTF-8"))?;
                    if created.insert(name.to_owned()) {
                        owner.copied_root.create_dir(&prefix)?;
                        owner.directories.push((
                            name.to_owned(),
                            open_relative_directory(&owner.copied_root, &prefix)?,
                        ));
                    }
                }
            }
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(unix)]
            options.mode(0o600);
            let mut copy = owner.copied_root.open_with(path, &options)?.into_std();
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
                    .ok_or_else(|| refusal("runtime size overflow"))?;
                digest.update(&buffer[..count]);
                copy.write_all(&buffer[..count])?;
            }
            let after = original.metadata()?;
            let current = open_relative_file(&owner.source_root, path)?;
            if size != before.len()
                || before.len() != after.len()
                || before.modified()? != after.modified()?
                || !same_file_identity(&original, &current)?
            {
                return Err(refusal("runtime source changed during copying"));
            }
            copy.flush()?;
            copy.sync_all()?;
            copy.seek(SeekFrom::Start(0))?;
            let mut permissions = copy.metadata()?.permissions();
            permissions.set_readonly(true);
            copy.set_permissions(permissions)?;
            drop(copy);
            owner.members.push(Member {
                path: path.clone(),
                size,
                sha256: hex::encode(digest.finalize()),
                file: open_relative_file(&owner.copied_root, path)?,
            });
        }
        let mut digest = Sha256::new();
        digest.update(b"pumas-audio-runtime-copied-code-v1\0");
        for member in &owner.members {
            digest.update((member.path.len() as u64).to_be_bytes());
            digest.update(member.path.as_bytes());
            digest.update(member.size.to_be_bytes());
            digest.update(member.sha256.as_bytes());
        }
        owner.manifest_sha256 = hex::encode(digest.finalize());
        owner.validate_source()?;
        for (_, directory) in &owner.directories {
            set_directory_mode(directory, false)?;
        }
        set_directory_mode(&owner.copied_root, false)?;
        Ok(owner)
    }
}

impl Drop for AudioRuntimeOwner {
    fn drop(&mut self) {
        // Restore held copies only, and do not recursively delete a replacement
        // scratch root. This is not protection from a hostile same-user race.
        let coherent = self.copied_identity_coherent().is_ok();
        self.read_source.disable_cleanup(!coherent);
        let _ = set_directory_mode(&self.copied_root, true);
        for (_, directory) in &self.directories {
            let _ = set_directory_mode(directory, true);
        }
        #[cfg(windows)]
        for member in &self.members {
            if let Ok(mut permissions) = member
                .file
                .metadata()
                .map(|metadata| metadata.permissions())
            {
                permissions.set_readonly(false);
                let _ = member.file.set_permissions(permissions);
            }
        }
    }
}

fn validated_names(names: &[&str]) -> Result<BTreeSet<String>> {
    let mut result = BTreeSet::new();
    if names.is_empty() {
        return Err(refusal("runtime selection is empty"));
    }
    for name in names {
        let components = Path::new(name).components().collect::<Vec<_>>();
        if components.is_empty()
            || name.contains('\\')
            || components
                .iter()
                .any(|component| !matches!(component, Component::Normal(_)))
            || name
                .split('/')
                .any(|part| part.is_empty() || part == "." || part == "..")
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"_/.-".contains(&byte))
            || !result.insert((*name).to_owned())
        {
            return Err(refusal("runtime member name is invalid or duplicated"));
        }
    }
    Ok(result)
}

fn open_relative_directory(root: &Dir, path: &Path) -> std::io::Result<Dir> {
    let mut current = root.try_clone()?;
    for component in path.components() {
        if !matches!(component, Component::Normal(_)) {
            return Err(std::io::Error::other("Invalid relative runtime directory"));
        }
        current = open_pinned_directory_at(&current, component.as_os_str())?;
    }
    Ok(current)
}

fn open_relative_file(root: &Dir, name: &str) -> std::io::Result<File> {
    let path = Path::new(name);
    let parent = open_relative_directory(root, path.parent().unwrap_or(Path::new("")))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| std::io::Error::other("Missing runtime filename"))?;
    let metadata = parent.symlink_metadata(file_name)?;
    if !metadata.is_file() || metadata.is_symlink() {
        return Err(std::io::Error::other(
            "Runtime member is not a regular non-link file",
        ));
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
        options
            .custom_flags(FILE_FLAG_OPEN_REPARSE_POINT)
            .access_mode(FILE_GENERIC_READ | FILE_WRITE_ATTRIBUTES);
    }
    let file = parent.open_with(file_name, &options)?.into_std();
    if !file.metadata()?.is_file() || file.metadata()?.file_type().is_symlink() {
        return Err(std::io::Error::other("Runtime member changed during open"));
    }
    Ok(file)
}

fn collect_members(
    root: &Dir,
    prefix: &str,
    files: &mut BTreeSet<String>,
    directories: &mut BTreeSet<String>,
) -> Result<()> {
    for entry in root.entries()? {
        let entry = entry?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| refusal("Runtime member is not UTF-8"))?;
        let relative = if prefix.is_empty() {
            name.clone()
        } else {
            format!("{prefix}/{name}")
        };
        let kind = root.symlink_metadata(&name)?;
        if kind.is_symlink() {
            return Err(refusal("runtime copied member became a link"));
        }
        if kind.is_file() {
            files.insert(relative);
        } else if kind.is_dir() {
            directories.insert(relative.clone());
            collect_members(
                &open_pinned_directory_at(root, std::ffi::OsStr::new(&name))?,
                &relative,
                files,
                directories,
            )?;
        } else {
            return Err(refusal("runtime copied member became a special file"));
        }
    }
    Ok(())
}

fn same_directory(first: &Dir, second: &Dir) -> std::io::Result<bool> {
    same_file_identity(
        &first.try_clone()?.into_std_file(),
        &second.try_clone()?.into_std_file(),
    )
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
            .ok_or_else(|| refusal("runtime size overflow"))?;
        digest.update(&buffer[..count]);
    }
    Ok((size, hex::encode(digest.finalize())))
}

fn set_directory_mode(directory: &Dir, writable: bool) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        directory
            .try_clone()?
            .into_std_file()
            .set_permissions(std::fs::Permissions::from_mode(if writable {
                0o700
            } else {
                0o500
            }))?;
    }
    #[cfg(not(unix))]
    let _ = (directory, writable);
    Ok(())
}

fn refusal(message: &str) -> PumasError {
    PumasError::Config {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> tempfile::TempDir {
        let root = tempfile::TempDir::new().unwrap();
        std::fs::create_dir(root.path().join("support")).unwrap();
        std::fs::write(root.path().join("fixture.py"), b"print('fixture')\n").unwrap();
        std::fs::write(root.path().join("support/helper.py"), b"VALUE = 7\n").unwrap();
        root
    }

    fn prepare(root: &tempfile::TempDir) -> Arc<AudioRuntimeOwner> {
        AudioRuntimeOwner::controlled_fixture(
            root.path(),
            &["fixture.py", "support/helper.py"],
            &["selected.bin"],
        )
        .unwrap()
    }

    #[test]
    fn code_snapshot_is_independent_of_originals() {
        let root = source();
        let owner = prepare(&root);
        assert_eq!(owner.manifest_sha256().len(), 64);
        let scratch = owner.read_source_path().to_owned();
        for member in &owner.members {
            let original = File::open(root.path().join(&member.path)).unwrap();
            assert!(!same_file_identity(&original, &member.file).unwrap());
            let bytes = std::fs::read(scratch.join(&member.path)).unwrap();
            assert_eq!(member.sha256, hex::encode(Sha256::digest(&bytes)));
        }
        std::fs::write(
            root.path().join("fixture.py"),
            b"raise RuntimeError('changed')\n",
        )
        .unwrap();
        owner.validate_source().unwrap();
        assert_eq!(
            std::fs::read(scratch.join("fixture.py")).unwrap(),
            b"print('fixture')\n"
        );
        drop(owner);
        assert!(!scratch.exists());
        assert!(root.path().join("fixture.py").exists());
    }

    #[test]
    fn missing_duplicate_and_escaping_code_members_are_refused() {
        let root = source();
        for selected in [
            vec!["fixture.py", "fixture.py"],
            vec!["missing.py"],
            vec!["../fixture.py"],
            vec!["/fixture.py"],
            vec!["support\\helper.py"],
            vec!["support/./helper.py"],
            vec![],
        ] {
            assert!(AudioRuntimeOwner::controlled_fixture(
                root.path(),
                &selected,
                &["selected.bin"]
            )
            .is_err());
        }
    }

    #[cfg(unix)]
    #[test]
    fn linked_members_and_linked_parent_directories_are_refused() {
        use std::os::unix::fs::symlink;
        let root = source();
        symlink(
            root.path().join("fixture.py"),
            root.path().join("linked.py"),
        )
        .unwrap();
        symlink(root.path().join("support"), root.path().join("linked-dir")).unwrap();
        for name in ["linked.py", "linked-dir/helper.py"] {
            assert!(
                AudioRuntimeOwner::controlled_fixture(root.path(), &[name], &["selected.bin"])
                    .is_err()
            );
        }
    }

    #[test]
    fn changed_copied_bytes_with_restored_timestamp_fail_actual_hash_validation() {
        let root = source();
        let owner = prepare(&root);
        let copy = owner.read_source_path().join("fixture.py");
        let metadata = std::fs::metadata(&copy).unwrap();
        let mut permissions = metadata.permissions();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            permissions.set_mode(0o600);
        }
        #[cfg(windows)]
        permissions.set_readonly(false);
        std::fs::set_permissions(&copy, permissions).unwrap();
        std::fs::write(&copy, vec![b'x'; metadata.len() as usize]).unwrap();
        File::options()
            .write(true)
            .open(&copy)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(metadata.modified().unwrap()))
            .unwrap();
        assert!(owner.validate_source().is_err());
    }

    #[cfg(unix)]
    #[test]
    fn replaced_source_root_and_copied_member_are_refused_without_deleting_replacements() {
        let root = source();
        let owner = prepare(&root);
        let original_root = root.path().to_owned();
        let moved = root.path().with_extension("held-original");
        std::fs::rename(&original_root, &moved).unwrap();
        std::fs::create_dir(&original_root).unwrap();
        assert!(owner.validate_source().is_err());
        std::fs::remove_dir(&original_root).unwrap();
        std::fs::rename(&moved, &original_root).unwrap();

        set_directory_mode(&owner.copied_root, true).unwrap();
        let replacement = owner.read_source_path().join("fixture.py");
        std::fs::remove_file(&replacement).unwrap();
        std::fs::write(&replacement, b"unowned replacement").unwrap();
        assert!(owner.validate_source().is_err());
        let scratch = owner.read_source_path().to_owned();
        drop(owner);
        assert_eq!(std::fs::read(&replacement).unwrap(), b"unowned replacement");
        std::fs::remove_dir_all(scratch).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn replaced_nested_copy_directory_is_preserved_on_disposal() {
        let root = source();
        let owner = prepare(&root);
        set_directory_mode(&owner.copied_root, true).unwrap();
        let original = owner.read_source_path().join("support");
        let held = owner.read_source_path().join("held-support");
        std::fs::rename(&original, &held).unwrap();
        std::fs::create_dir(&original).unwrap();
        std::fs::write(original.join("keep.txt"), b"not our directory").unwrap();
        assert!(owner.validate_source().is_err());
        let scratch = owner.read_source_path().to_owned();
        drop(owner);
        assert_eq!(
            std::fs::read(original.join("keep.txt")).unwrap(),
            b"not our directory"
        );
        std::fs::remove_dir_all(scratch).unwrap();
    }
}
