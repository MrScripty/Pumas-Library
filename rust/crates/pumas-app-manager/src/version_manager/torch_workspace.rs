//! Operation-scoped scratch space for Torch resolver processes.

use pumas_library::{PumasError, Result};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::process::Command;

/// Create a unique workspace under the launcher's managed temporary directory.
pub(super) fn create_launcher_workspace(launcher_root: &Path) -> Result<Arc<tempfile::TempDir>> {
    let root = fs::canonicalize(launcher_root)
        .map_err(|err| PumasError::io_with_path(err, launcher_root))?;
    let mut parent = root.clone();
    for component in ["launcher-data", "tmp", "torch-preview"] {
        parent = checked_child_directory(&parent, component, &root)?;
    }

    tempfile::Builder::new()
        .prefix("resolver-")
        .tempdir_in(&parent)
        .map(Arc::new)
        .map_err(|err| PumasError::io_with_path(err, &parent))
}

/// Give one resolver process its own temporary directory without changing our environment.
pub(super) fn configure_resolver_command(command: &mut Command, workspace: &Path) -> Result<()> {
    let workspace =
        fs::canonicalize(workspace).map_err(|err| PumasError::io_with_path(err, workspace))?;
    let temporary = checked_child_directory(&workspace, "tmp", &workspace)?;
    for name in ["TMPDIR", "TMP", "TEMP"] {
        command.env(name, &temporary);
    }
    Ok(())
}

fn checked_child_directory(parent: &Path, name: &str, root: &Path) -> Result<PathBuf> {
    let child = parent.join(name);
    match fs::create_dir(&child) {
        Ok(()) => {}
        Err(err) if err.kind() == ErrorKind::AlreadyExists => {}
        Err(err) => return Err(PumasError::io_with_path(err, &child)),
    }
    let resolved = fs::canonicalize(&child).map_err(|err| PumasError::io_with_path(err, &child))?;
    if !resolved.starts_with(root) || resolved == root {
        return Err(PumasError::Config {
            message: format!(
                "Torch resolver temporary directory escapes its root: {}",
                child.display()
            ),
        });
    }
    if !resolved.is_dir() {
        return Err(PumasError::Config {
            message: format!(
                "Torch resolver temporary path is not a directory: {}",
                child.display()
            ),
        });
    }
    Ok(resolved)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspaces_are_unique_and_managed() {
        let root = tempfile::tempdir().unwrap();
        let first = create_launcher_workspace(root.path()).unwrap();
        let second = create_launcher_workspace(root.path()).unwrap();
        let parent = fs::canonicalize(root.path())
            .unwrap()
            .join("launcher-data/tmp/torch-preview");

        assert_ne!(first.path(), second.path());
        assert_eq!(first.path().parent(), Some(parent.as_path()));
        assert_eq!(second.path().parent(), Some(parent.as_path()));
        assert!(first.path().is_dir());
        assert!(second.path().is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn escaping_preview_symlink_is_rejected_without_touching_target() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        let managed_tmp = root.path().join("launcher-data/tmp");
        fs::create_dir_all(&managed_tmp).unwrap();
        let sentinel = outside.path().join("keep");
        fs::write(&sentinel, b"untouched").unwrap();
        symlink(outside.path(), managed_tmp.join("torch-preview")).unwrap();

        assert!(create_launcher_workspace(root.path()).is_err());
        assert_eq!(fs::read(&sentinel).unwrap(), b"untouched");
        assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
        assert!(managed_tmp.join("torch-preview").is_symlink());
    }

    #[test]
    fn command_uses_workspace_tmp_and_keeps_other_environment() {
        let root = tempfile::tempdir().unwrap();
        let workspace = create_launcher_workspace(root.path()).unwrap();
        let mut command = Command::new("resolver");
        command.env("RESOLVER_SENTINEL", "preserved");

        configure_resolver_command(&mut command, workspace.path()).unwrap();

        let environment: std::collections::HashMap<_, _> = command
            .as_std()
            .get_envs()
            .map(|(name, value)| (name.to_owned(), value.map(ToOwned::to_owned)))
            .collect();
        let temporary = workspace.path().join("tmp");
        assert!(temporary.is_dir());
        for name in ["TMPDIR", "TMP", "TEMP"] {
            assert_eq!(
                environment.get(std::ffi::OsStr::new(name)),
                Some(&Some(temporary.as_os_str().to_owned()))
            );
        }
        assert_eq!(
            environment.get(std::ffi::OsStr::new("RESOLVER_SENTINEL")),
            Some(&Some("preserved".into()))
        );
    }

    #[test]
    fn paths_with_spaces_work() {
        let parent = tempfile::tempdir().unwrap();
        let root = parent.path().join("launcher with spaces");
        fs::create_dir(&root).unwrap();
        let workspace = create_launcher_workspace(&root).unwrap();
        let mut command = Command::new("resolver");

        configure_resolver_command(&mut command, workspace.path()).unwrap();

        let temporary = workspace.path().join("tmp");
        assert!(temporary.is_dir());
        assert!(temporary.starts_with(fs::canonicalize(&root).unwrap()));
        assert!(command
            .as_std()
            .get_envs()
            .any(|(name, value)| { name == "TMPDIR" && value == Some(temporary.as_os_str()) }));
    }
}
