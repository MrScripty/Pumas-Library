//! Copied-import identity evidence and the Windows descendant-handle transition.
//! The root grant excludes cooperating writers. Identity evidence never grants
//! ambient pathname authority or permits adopting a replacement after release.

use super::*;
use crate::model_library::hashing::compute_dual_hash_reader;

pub(crate) const IMPORT_DOCUMENT_MAX_BYTES: u64 = 16 * 1024 * 1024;

pub(crate) fn require_import_document_size(value: &impl Serialize) -> Result<()> {
    if serde_json::to_vec_pretty(value)?.len() as u64 > IMPORT_DOCUMENT_MAX_BYTES {
        return Err(PumasError::Validation {
            field: "import_publication".into(),
            message: "Copied-import document exceeds the supported observation size limit".into(),
        });
    }
    Ok(())
}

pub(crate) const IMPORT_RECEIPT: &str = ".pumas_import_publication.json";
pub(crate) const IMPORT_METADATA_BACKUP: &str = "metadata.json.bak";
pub(crate) const IMPORT_MUTABLE_DOCUMENTS: [&str; 4] = [
    "metadata.json",
    IMPORT_METADATA_BACKUP,
    "overrides.json",
    IMPORT_RECEIPT,
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImportPayloadIdentity {
    library_root: FilesystemIdentity,
    stage: FilesystemIdentity,
    directories: BTreeMap<String, FilesystemIdentity>,
    files: BTreeMap<String, ImportFileIdentity>,
}

impl ImportPayloadIdentity {
    pub(crate) fn matches_single_file(&self, bytes: u64, sha256: &str) -> bool {
        self.files.len() == 1
            && self
                .files
                .values()
                .all(|file| file.size == bytes && file.sha256 == sha256)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ImportFileIdentity {
    identity: FilesystemIdentity,
    size: u64,
    sha256: String,
}

impl ImportFileIdentity {
    pub(crate) fn copied(file: &std::fs::File, size: u64, sha256: String) -> Result<Self> {
        let metadata = Metadata::from_file(file)?;
        if !metadata.is_file() || metadata.len() != size {
            return Err(invalid_capability_path().into());
        }
        Ok(Self {
            identity: filesystem_identity(&metadata).ok_or_else(invalid_capability_path)?,
            size,
            sha256,
        })
    }
}

impl ImportPayloadIdentity {
    fn same_bindings(&self, other: &Self) -> bool {
        self.library_root == other.library_root
            && self.stage == other.stage
            && self.directories == other.directories
            && self.files.len() == other.files.len()
            && self.files.iter().all(|(name, expected)| {
                other.files.get(name).is_some_and(|actual| {
                    actual.identity == expected.identity && actual.size == expected.size
                })
            })
    }
}

type HeldChildren = BTreeMap<PathBuf, Arc<HeldDestination>>;

impl DownloadDestinationRoot {
    /// Observation only: unlike owning composition, never initializes markers.
    pub(crate) fn open_import_read_only(path: &Path) -> Result<Self> {
        Ok(Self(Arc::new(
            RecoveryRoot::open(path)?.ok_or_else(invalid_capability_path)?,
        )))
    }
}

impl DownloadRecoveryDestination {
    /// Remove only the empty exclusive backup-name reservation we created for
    /// filesystem-equivalent collision checks, before payload evidence capture.
    pub(crate) fn finish_import_document_reservation(
        &self,
        name: &str,
        file: std::fs::File,
    ) -> Result<()> {
        let expected = filesystem_identity(&Metadata::from_file(&file)?)
            .ok_or_else(invalid_capability_path)?;
        let directory = self.directory(false)?;
        let current = directory.symlink_metadata(name)?;
        if !current.is_file()
            || current.is_symlink()
            || filesystem_identity(&current) != Some(expected)
        {
            return Err(invalid_capability_path().into());
        }
        drop(file);
        directory.remove_file(name)?;
        sync_directory(&directory)?;
        Ok(())
    }
    pub(crate) fn import_payload_root_matches(
        &self,
        expected: &ImportPayloadIdentity,
    ) -> Result<bool> {
        Ok(self.authority.root_identity == expected.library_root
            && directory_identity(&self.directory(false)?)? == expected.stage)
    }
    #[cfg(test)]
    pub(crate) fn inject_import_document_uncertainty(&self, name: &str) {
        *self.import_document_uncertainty.lock().unwrap() = Some(name.into());
    }
    pub(super) fn require_import_bound(&self) -> io::Result<()> {
        if *self
            .import_released
            .lock()
            .map_err(|_| io::Error::other("Import custody lock poisoned"))?
        {
            return Err(io::Error::other(
                "Import descendant handles released; exact identity rebind required",
            ));
        }
        Ok(())
    }

    /// Compare copy-time descriptor evidence with the complete held namespace.
    /// No payload bytes are reread here; only the final publication proof hashes.
    pub(crate) fn capture_copied_import_payload(
        &self,
        files: BTreeMap<String, ImportFileIdentity>,
    ) -> Result<ImportPayloadIdentity> {
        self.validate_import_stage_bindings()?;
        let (mut identity, _) = self.observe_import_payload(false)?;
        let mut expected = identity.clone();
        expected.files = files;
        if !expected.same_bindings(&identity) {
            *self
                .import_released
                .lock()
                .map_err(|_| invalid_capability_path())? = true;
            return Err(invalid_capability_path().into());
        }
        identity.files = expected.files;
        self.import_payload
            .set(identity.clone())
            .map_err(|_| invalid_capability_path())?;
        Ok(identity)
    }

    #[cfg(test)]
    pub(crate) fn capture_import_payload(&self, files: &[String]) -> Result<ImportPayloadIdentity> {
        let (identity, _) = self.observe_import_payload(true)?;
        if identity.files.keys().cloned().collect::<BTreeSet<_>>()
            != files.iter().cloned().collect()
        {
            return Err(invalid_capability_path().into());
        }
        self.capture_copied_import_payload(identity.files)
    }

    pub(crate) fn verify_import_bindings(&self, expected: &ImportPayloadIdentity) -> Result<()> {
        self.require_import_bound()?;
        let (observed, _) = self.observe_import_payload(false)?;
        if !expected.same_bindings(&observed) {
            return Err(invalid_capability_path().into());
        }
        Ok(())
    }

    /// A held lookup against exclusive reserved entries uses native filesystem
    /// equivalence without copying or adopting the caller's original filename.
    pub(crate) fn import_reserved_name_claimed(&self, name: &str) -> Result<bool> {
        if Path::new(name).components().count() != 1 {
            return Ok(false);
        }
        match self.directory(false)?.symlink_metadata(name) {
            Ok(_) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    pub(crate) fn verify_import_payload(&self, expected: &ImportPayloadIdentity) -> Result<()> {
        self.verify_import_bindings(expected)?;
        let (observed, _) = self.observe_import_payload(true)?;
        if &observed != expected {
            return Err(io::Error::other(
                "Copied import payload identity, contents or completeness changed",
            )
            .into());
        }
        Ok(())
    }

    /// Called only by the finite import producer after its last callback and
    /// complete proof. The stage/root capability and execution grant stay live.
    pub(crate) fn release_import_descendants(&self) -> Result<()> {
        self.require_import_bound()?;
        *self
            .import_released
            .lock()
            .map_err(|_| io::Error::other("Import custody lock poisoned"))? = true;
        self.file_parents
            .lock()
            .map_err(|_| io::Error::other("Import descendant authority lock poisoned"))?
            .clear();
        Ok(())
    }

    /// Rebind only the exact original tree through this held root. On any
    /// mismatch the released state remains closed to generic cleanup or use.
    pub(crate) fn rebind_import_payload(&self, expected: &ImportPayloadIdentity) -> Result<()> {
        *self
            .import_released
            .lock()
            .map_err(|_| io::Error::other("Import custody lock poisoned"))? = true;
        let (observed, children) = self.observe_import_payload(false)?;
        if !expected.same_bindings(&observed) {
            return Err(io::Error::other(
                "Copied import payload changed across descendant handle release; custody unknown",
            )
            .into());
        }
        if let Some(previous) = self.import_payload.get() {
            if previous != expected {
                return Err(invalid_capability_path().into());
            }
        } else {
            self.import_payload
                .set(expected.clone())
                .map_err(|_| invalid_capability_path())?;
        }
        *self
            .file_parents
            .lock()
            .map_err(|_| io::Error::other("Import descendant authority lock poisoned"))? = children;
        *self
            .import_released
            .lock()
            .map_err(|_| io::Error::other("Import custody lock poisoned"))? = false;
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn import_hash_pass_count(&self) -> usize {
        self.import_hash_passes
            .load(std::sync::atomic::Ordering::SeqCst)
    }

    fn observe_import_payload(
        &self,
        hash_contents: bool,
    ) -> Result<(ImportPayloadIdentity, HeldChildren)> {
        #[cfg(test)]
        if hash_contents {
            self.import_hash_passes
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
        let root = self.directory(false)?;
        let mut identity = ImportPayloadIdentity {
            library_root: self.authority.root_identity,
            stage: directory_identity(&root)?,
            directories: BTreeMap::new(),
            files: BTreeMap::new(),
        };
        let mut children = BTreeMap::new();
        observe_directory(
            &root,
            Path::new(""),
            &mut identity,
            &mut children,
            hash_contents,
        )?;
        // Confirm the bound root again, without reacquiring ambient authority.
        if directory_identity(&self.directory(false)?)? != identity.stage {
            return Err(invalid_capability_path().into());
        }
        Ok((identity, children))
    }

    /// Typed publication keeps document durability uncertainty distinct from
    /// payload confirmation. Display paths are diagnostics only.
    pub(crate) fn publish_import_document<T: Serialize>(
        &self,
        name: &str,
        value: &T,
    ) -> Result<crate::metadata::AtomicPublication> {
        require_import_document_size(value)?;
        if !matches!(name, "metadata.json" | IMPORT_RECEIPT) {
            return Err(invalid_capability_path().into());
        }
        let directory = self.directory(false)?;
        let expected = directory_identity(&directory)?;
        let destination = self.clone();
        let target = crate::metadata::AtomicJsonTarget::from_capability(
            directory,
            std::ffi::OsStr::new(name),
            self.display_path.join(name),
            move || Ok(directory_identity(&destination.directory(false)?)? == expected),
        )?;
        let outcome = target
            .publish_json(value)
            .map_err(|failure| failure.into_error())?;
        #[cfg(test)]
        {
            let mut injection = self.import_document_uncertainty.lock().unwrap();
            if injection.as_deref() == Some(name)
                && matches!(&outcome, crate::metadata::AtomicPublication::Durable)
            {
                injection.take();
                // The real atomic writer ran. Model an uncertain post-rename
                // durability report at the protocol boundary, conservatively.
                return Ok(
                    crate::metadata::AtomicPublication::PublishedDurabilityUnknown {
                        error: io::Error::other(format!("injected {name} durability uncertainty"))
                            .into(),
                    },
                );
            }
        }
        Ok(outcome)
    }
}

fn observe_directory(
    directory: &Dir,
    relative: &Path,
    identity: &mut ImportPayloadIdentity,
    children: &mut HeldChildren,
    hash_contents: bool,
) -> Result<()> {
    for entry in directory.entries()? {
        let name = entry?.file_name();
        let path = relative.join(&name);
        let portable = path
            .to_str()
            .ok_or_else(invalid_capability_path)?
            .replace(std::path::MAIN_SEPARATOR, "/");
        let metadata = directory.symlink_metadata(&name)?;
        if metadata.is_symlink() {
            return Err(invalid_capability_path().into());
        }
        if metadata.is_dir() {
            let child = open_directory_chain(directory, Path::new(&name), false)?;
            let child_identity = directory_identity(&child)?;
            identity.directories.insert(portable, child_identity);
            observe_directory(&child, &path, identity, children, hash_contents)?;
            children.insert(
                path,
                Arc::new(HeldDestination {
                    directory: child,
                    identity: child_identity,
                }),
            );
        } else if metadata.is_file() {
            if relative.as_os_str().is_empty()
                && name
                    .to_str()
                    .is_some_and(|name| IMPORT_MUTABLE_DOCUMENTS.contains(&name))
            {
                continue;
            }
            let mut options = OpenOptions::new();
            options.read(true);
            nofollow_options(&mut options);
            let file = directory.open_with(&name, &options)?;
            let metadata = file.metadata()?;
            if !metadata.is_file() || metadata.is_symlink() {
                return Err(invalid_capability_path().into());
            }
            let physical = filesystem_identity(&metadata).ok_or_else(invalid_capability_path)?;
            let sha256 = if hash_contents {
                compute_dual_hash_reader(&mut file.into_std())?.sha256
            } else {
                String::new()
            };
            identity.files.insert(
                portable,
                ImportFileIdentity {
                    identity: physical,
                    size: metadata.len(),
                    sha256,
                },
            );
        } else {
            return Err(invalid_capability_path().into());
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn copied_import_document_bounds_apply_before_publication() {
        let temp = tempfile::tempdir().unwrap();
        let root = DownloadDestinationRoot::open(temp.path()).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        let stage = root.create_import_stage().unwrap();
        let oversized = "x".repeat(IMPORT_DOCUMENT_MAX_BYTES as usize);
        for document in ["metadata.json", IMPORT_RECEIPT] {
            assert!(stage.publish_import_document(document, &oversized).is_err());
            assert!(!stage.display_path().join(document).exists());
        }
        stage.remove_import_stage_all().unwrap();
    }

    #[test]
    fn copied_import_nested_native_publication_rebinds_exact_payload() {
        let temp = tempfile::tempdir().unwrap();
        let root = DownloadDestinationRoot::open(temp.path()).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        let stage = root.create_import_stage().unwrap();
        stage.create_import_directory("empty").unwrap();
        stage.create_import_directory("component").unwrap();
        stage
            .create_import_file("component/weights")
            .unwrap()
            .write_all(b"weights")
            .unwrap();
        let proof = stage
            .capture_import_payload(&["component/weights".into()])
            .unwrap();
        stage.release_import_descendants().unwrap();
        assert!(stage.open_import_file("component/weights").is_err());
        let target = root.resolve(Path::new("vision/family/model")).unwrap();
        assert!(matches!(
            stage.publish_model_directory_noreplace(&target).unwrap(),
            crate::metadata::AtomicPublication::Durable
        ));
        target.rebind_import_payload(&proof).unwrap();
        assert!(target.import_component_exists("empty").unwrap());
        assert_eq!(target.file_len("component/weights").unwrap(), Some(7));
        target.verify_import_payload(&proof).unwrap();
    }

    #[test]
    fn copied_import_failed_native_rename_rebinds_before_cleanup() {
        let temp = tempfile::tempdir().unwrap();
        let root = DownloadDestinationRoot::open(temp.path()).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        let stage = root.create_import_stage().unwrap();
        stage.create_import_directory("component").unwrap();
        stage
            .create_import_file("component/weights")
            .unwrap()
            .write_all(b"weights")
            .unwrap();
        let proof = stage
            .capture_import_payload(&["component/weights".into()])
            .unwrap();
        let target = root.resolve(Path::new("vision/family/model")).unwrap();
        target.prepare().unwrap();
        target
            .create_import_file("sentinel")
            .unwrap()
            .write_all(b"existing")
            .unwrap();
        stage.release_import_descendants().unwrap();
        assert!(stage.publish_model_directory_noreplace(&target).is_err());
        assert!(stage.remove_import_stage_all().is_err());
        stage.rebind_import_payload(&proof).unwrap();
        stage.remove_import_stage_all().unwrap();
        assert!(!stage.display_path().exists());
        assert_eq!(
            std::fs::read(target.display_path().join("sentinel")).unwrap(),
            b"existing"
        );
    }

    #[test]
    fn copied_import_released_child_replacement_never_becomes_cleanup_authority() {
        let temp = tempfile::tempdir().unwrap();
        let root = DownloadDestinationRoot::open(temp.path()).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        let stage = root.create_import_stage().unwrap();
        stage.create_import_directory("component").unwrap();
        stage
            .create_import_file("component/weights")
            .unwrap()
            .write_all(b"weights")
            .unwrap();
        let proof = stage
            .capture_import_payload(&["component/weights".into()])
            .unwrap();
        stage.release_import_descendants().unwrap();
        let original = temp.path().join("original-child");
        std::fs::rename(stage.display_path().join("component"), &original).unwrap();
        std::fs::create_dir(stage.display_path().join("component")).unwrap();
        std::fs::write(stage.display_path().join("component/sentinel"), b"preserve").unwrap();
        assert!(stage.rebind_import_payload(&proof).is_err());
        assert!(stage.remove_import_stage_all().is_err());
        assert_eq!(std::fs::read(original.join("weights")).unwrap(), b"weights");
        assert_eq!(
            std::fs::read(stage.display_path().join("component/sentinel")).unwrap(),
            b"preserve"
        );
    }
}
