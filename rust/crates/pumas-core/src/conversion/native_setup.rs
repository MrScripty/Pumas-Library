//! Persistent negative knowledge for the current llama.cpp setup recipe.
//! Absence is not a provenance receipt. The environment lease and stable paths
//! exclude cooperating writers; this is not a hostile-path or power-loss protocol.

use std::io;
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub(super) struct NativeSetup {
    path: PathBuf,
}

impl NativeSetup {
    pub(super) fn new(base: &Path) -> Self {
        Self {
            path: base.join("setup-incomplete"),
        }
    }

    /// Called under the setup lease before source, build, or venv mutation.
    /// Existing supported markers authorize explicit repair, never overwrite.
    pub(super) fn begin(&self) -> io::Result<()> {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&self.path)
        {
            Ok(_) => Ok(()),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => self.require_marker(),
            Err(error) => Err(error),
        }
    }

    fn require_marker(&self) -> io::Result<()> {
        let metadata = std::fs::symlink_metadata(&self.path)?;
        if metadata.is_file() && metadata.len() == 0 {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Refusing occupied native setup marker {}: expected a zero-byte regular file",
                    self.path.display()
                ),
            ))
        }
    }

    /// Only after every recipe proof and its final cancellation check. Failure
    /// retains the marker; Drop must never clear incomplete setup knowledge.
    pub(super) fn complete(&self) -> io::Result<()> {
        self.require_marker()?;
        std::fs::remove_file(&self.path)
    }

    pub(super) fn incomplete(&self) -> io::Result<bool> {
        occupied(std::fs::symlink_metadata(&self.path))
    }

    pub(super) async fn incomplete_async(&self) -> io::Result<bool> {
        occupied(tokio::fs::symlink_metadata(&self.path).await)
    }
}

fn occupied(metadata: io::Result<std::fs::Metadata>) -> io::Result<bool> {
    match metadata {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn marker_writer_child() {
        if let Some(root) = std::env::var_os("PUMAS_TEST_NATIVE_SETUP_MARKER") {
            NativeSetup::new(Path::new(&root)).begin().unwrap();
        }
    }

    #[test]
    fn incomplete_marker_survives_producer_process_exit_and_explicit_repair_clears_it() {
        let root = tempfile::tempdir().unwrap();
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "conversion::native_setup::tests::marker_writer_child",
                "--nocapture",
            ])
            .env("PUMAS_TEST_NATIVE_SETUP_MARKER", root.path())
            .status()
            .unwrap();
        assert!(status.success());
        let reopened = NativeSetup::new(root.path());
        assert!(reopened.incomplete().unwrap());
        reopened.begin().unwrap();
        reopened.complete().unwrap();
        assert!(!NativeSetup::new(root.path()).incomplete().unwrap());
    }
}
