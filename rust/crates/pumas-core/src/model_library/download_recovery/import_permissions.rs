//! Ordinary Unix creation-mode compatibility for private copied imports.
//! Both the historical temporary directory and the empty observation template
//! are created directly under the library root, not the eventual model parent.
//! This is a mode/umask contract, not custom ACL or group-policy portability.

use super::*;

pub(crate) struct ImportDirectoryPermissions {
    #[cfg(unix)]
    root: FilesystemIdentity,
    #[cfg(unix)]
    stage: FilesystemIdentity,
    #[cfg(unix)]
    mode: u32,
    #[cfg(unix)]
    uid: u32,
    #[cfg(unix)]
    gid: u32,
}

impl DownloadRecoveryDestination {
    pub(crate) fn observe_import_directory_permissions(
        &self,
    ) -> Result<ImportDirectoryPermissions> {
        #[cfg(unix)]
        {
            self.observe_import_directory_permissions_with(|_| Ok(()))
        }
        #[cfg(not(unix))]
        {
            Ok(ImportDirectoryPermissions {})
        }
    }

    #[cfg(unix)]
    fn observe_import_directory_permissions_with(
        &self,
        after_observation: impl FnOnce(&DownloadRecoveryDestination) -> Result<()>,
    ) -> Result<ImportDirectoryPermissions> {
        use std::os::unix::fs::MetadataExt;
        self.authority.require_current()?;
        let stage = self.directory(false)?;
        let stage_metadata = stage.open(".")?.into_std().metadata()?;
        let name = format!(
            "{}mode-{}",
            super::super::importer::TEMP_IMPORT_PREFIX,
            uuid::Uuid::new_v4()
        );
        let template = self.execution_root().resolve(Path::new(&name))?;
        // The template is always empty: no payload, metadata, receipt or secret
        // is written under its ordinary inherited creation permissions.
        self.authority.root.create_dir(&name)?;
        let observed = (|| -> Result<ImportDirectoryPermissions> {
            let directory = template.directory(false)?;
            let metadata = directory.open(".")?.into_std().metadata()?;
            if metadata.uid() != stage_metadata.uid() || metadata.gid() != stage_metadata.gid() {
                return Err(io::Error::other(
                    "Import stage and normal-creation template ownership differ",
                )
                .into());
            }
            let permissions = ImportDirectoryPermissions {
                root: self.authority.root_identity,
                stage: directory_identity(&stage)?,
                mode: metadata.mode() & 0o7777,
                uid: metadata.uid(),
                gid: metadata.gid(),
            };
            after_observation(&template)?;
            // Revalidate observed identity, then unlink only an empty directory.
            // Never recursively delete a changed or unexpectedly populated template.
            template.directory(false)?;
            self.authority.root.remove_dir(&name)?;
            sync_directory(&self.authority.root)?;
            self.authority.require_current()?;
            Ok(permissions)
        })();
        observed.map_err(|error| PumasError::ImportFailed {
            message: format!("Could not settle empty import permission template {}: {error}; no pathname cleanup retry is authorized", template.display_path.display()),
        })
    }

    pub(crate) fn finalize_import_directory_permissions(
        &self,
        permissions: ImportDirectoryPermissions,
    ) -> Result<()> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::{MetadataExt, PermissionsExt};
            self.require_import_bound()?;
            let directory = self.directory(false)?;
            if permissions.root != self.authority.root_identity
                || permissions.stage != directory_identity(&directory)?
            {
                return Err(invalid_capability_path().into());
            }
            let file = directory.open(".")?.into_std();
            let metadata = file.metadata()?;
            if metadata.uid() != permissions.uid || metadata.gid() != permissions.gid {
                return Err(io::Error::other(
                    "Published import ownership changed before permission finalization",
                )
                .into());
            }
            #[cfg(test)]
            if self
                .import_permission_failure
                .load(std::sync::atomic::Ordering::SeqCst)
            {
                return Err(
                    io::Error::other("injected import directory permission failure").into(),
                );
            }
            file.set_permissions(std::fs::Permissions::from_mode(permissions.mode))?;
            file.sync_all()?;
            if file.metadata()?.mode() & 0o7777 != permissions.mode {
                return Err(io::Error::other(
                    "Published import directory mode did not settle as observed",
                )
                .into());
            }
            // A successful fchmod is not permission to adopt a rebound name.
            self.directory(false)?;
        }
        #[cfg(not(unix))]
        let _ = permissions;
        Ok(())
    }

    #[cfg(all(test, unix))]
    pub(crate) fn inject_import_permission_failure(&self) {
        self.import_permission_failure
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;

    #[test]
    fn copied_import_permission_template_replacement_is_retained_and_never_unlinked() {
        let temp = tempfile::tempdir().unwrap();
        let root = DownloadDestinationRoot::open(temp.path()).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        let stage = root.create_import_stage().unwrap();
        let original = temp.path().join("retained-template");
        let result = stage.observe_import_directory_permissions_with(|template| {
            std::fs::rename(template.display_path(), &original)?;
            std::fs::create_dir(template.display_path())?;
            std::fs::write(template.display_path().join("sentinel"), b"replacement")?;
            Ok(())
        });
        assert!(result.is_err());
        assert!(original.is_dir());
        let sentinel = std::fs::read_dir(temp.path())
            .unwrap()
            .map(|entry| entry.unwrap().path().join("sentinel"))
            .find(|path| path.is_file())
            .unwrap();
        assert_eq!(std::fs::read(sentinel).unwrap(), b"replacement");
    }

    #[test]
    fn copied_import_permission_template_unexpected_child_is_retained() {
        let temp = tempfile::tempdir().unwrap();
        let root = DownloadDestinationRoot::open(temp.path()).unwrap();
        let _grant = root.try_acquire_execution_grant().unwrap();
        let stage = root.create_import_stage().unwrap();
        let result = stage.observe_import_directory_permissions_with(|template| {
            std::fs::write(template.display_path().join("sentinel"), b"unexpected")?;
            Ok(())
        });
        assert!(result.is_err());
        assert!(std::fs::read_dir(temp.path()).unwrap().any(|entry| entry
            .unwrap()
            .path()
            .join("sentinel")
            .is_file()));
    }
}
