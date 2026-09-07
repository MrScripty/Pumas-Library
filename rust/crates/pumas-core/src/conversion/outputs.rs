//! Exclusive per-attempt staging and non-replacing conversion publication.
//!
//! Parent directories must remain stable during an attempt. This module does
//! not establish native process cleanup or hostile-filesystem capability safety.
//! Dropping an attempt retains staging: automatic deletion could race a native
//! producer whose cancellation has not yet been observed.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::platform::filesystem::rename_directory_noreplace;
use crate::{PumasError, Result};

pub(super) struct OutputWorkspace {
    staging: PathBuf,
    parent: PathBuf,
    desired_name: OsString,
}

impl OutputWorkspace {
    /// Allocate private staging beside the desired output, never reuse or
    /// remove another attempt's directory. The parent must already exist.
    pub(super) async fn prepare(output_dir: &Path) -> Result<Self> {
        let desired_name = validated_name(output_dir)?;
        let parent = output_dir
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."))
            .to_path_buf();
        tokio::task::spawn_blocking(move || {
            let parent = parent.canonicalize().map_err(|error| {
                PumasError::io("resolving conversion output parent", &parent, error)
            })?;
            let staging = tempfile::Builder::new()
                .prefix(".pumas-conversion-")
                .tempdir_in(&parent)
                .map_err(|error| PumasError::io("allocating conversion staging", &parent, error))?
                .keep();
            Ok(Self {
                staging,
                parent,
                desired_name,
            })
        })
        .await
        .map_err(|error| PumasError::ConversionFailed {
            message: format!("Conversion staging allocation worker failed: {error}"),
        })?
    }

    pub(super) fn staging_path(&self) -> &Path {
        &self.staging
    }

    /// Publish without replacing any existing entry and return its actual
    /// identity. Failed or interrupted requests do not delete staging. A dropped
    /// waiter does not cancel the blocking move; its publication may complete.
    pub(super) async fn publish(self) -> Result<PathBuf> {
        tokio::task::spawn_blocking(move || {
            let mut target = self.parent.join(&self.desired_name);
            let mut version = 2_u64;
            loop {
                match rename_directory_noreplace(&self.staging, &target) {
                    Ok(()) => return Ok(target),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                        let mut name = self.desired_name.clone();
                        name.push(format!("-v{version}"));
                        target = self.parent.join(name);
                        version =
                            version
                                .checked_add(1)
                                .ok_or_else(|| PumasError::ConversionFailed {
                                    message: "Conversion output version counter exhausted".into(),
                                })?;
                    }
                    Err(error) => {
                        return Err(PumasError::io(
                            "publishing conversion output",
                            &target,
                            error,
                        ))
                    }
                }
            }
        })
        .await
        .map_err(|error| PumasError::ConversionFailed {
            message: format!("Conversion publication worker failed: {error}"),
        })?
    }
}

fn validated_name(output_dir: &Path) -> Result<OsString> {
    let raw = output_dir.as_os_str().as_encoded_bytes();
    let leaf = raw
        .rsplit(|byte| *byte == b'/' || (cfg!(windows) && *byte == b'\\'))
        .next()
        .unwrap_or_default();
    let name = output_dir.file_name();
    if leaf.is_empty()
        || leaf == b"."
        || leaf == b".."
        || raw.contains(&0)
        || name.is_none_or(|name| name.as_encoded_bytes() != leaf)
    {
        return Err(PumasError::InvalidParams {
            message: "Conversion output must have a nonempty normal final path component".into(),
        });
    }
    Ok(name.expect("validated output name").to_os_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[tokio::test]
    async fn separate_attempts_and_dropped_work_preserve_existing_staging_and_outputs() {
        let root = tempfile::tempdir().expect("temporary root");
        let desired = root.path().join("model-Q4_K_M");
        fs::create_dir(&desired).expect("old output");
        fs::write(desired.join("weights"), b"old model").expect("old payload");
        let old_stage = desired.with_extension("quantizing");
        fs::create_dir(&old_stage).expect("old staging");
        fs::write(old_stage.join("weights"), b"unfinished old model").expect("old stage payload");
        let first = OutputWorkspace::prepare(&desired)
            .await
            .expect("first staging");
        let second = OutputWorkspace::prepare(&desired)
            .await
            .expect("second staging");
        let first_path = first.staging_path().to_path_buf();
        let second_path = second.staging_path().to_path_buf();
        assert_ne!(first_path, second_path);
        assert!(first_path
            .file_name()
            .expect("stage name")
            .to_string_lossy()
            .starts_with(".pumas-conversion-"));
        drop(first);
        drop(second);
        assert!(first_path.is_dir());
        assert!(second_path.is_dir());
        assert_eq!(
            fs::read(desired.join("weights")).expect("old output retained"),
            b"old model"
        );
        assert_eq!(
            fs::read(old_stage.join("weights")).expect("old staging retained"),
            b"unfinished old model"
        );
    }

    #[tokio::test]
    async fn invalid_leaf_rejected_before_allocating_staging() {
        let root = tempfile::tempdir().expect("temporary root");
        for name in [
            "",
            ".",
            "..",
            "nested/.",
            "nested/..",
            "nested/",
            "bad\0name",
        ] {
            let desired = if name.is_empty() {
                PathBuf::new()
            } else {
                root.path().join(name)
            };
            assert!(
                matches!(
                    OutputWorkspace::prepare(&desired).await,
                    Err(PumasError::InvalidParams { .. })
                ),
                "{name:?}"
            );
        }
        assert_eq!(fs::read_dir(root.path()).expect("root entries").count(), 0);
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    #[tokio::test]
    async fn publication_returns_base_then_versioned_actual_identity() {
        let root = tempfile::tempdir().expect("root");
        let desired = root.path().join("model");
        for (payload, name) in [
            (b"first".as_slice(), "model"),
            (b"second".as_slice(), "model-v2"),
        ] {
            let attempt = OutputWorkspace::prepare(&desired).await.expect("staging");
            let staging = attempt.staging_path().to_path_buf();
            fs::write(staging.join("weights"), payload).expect("stage weights");
            let actual = attempt.publish().await.expect("publish");
            assert_eq!(
                actual,
                root.path()
                    .canonicalize()
                    .expect("canonical root")
                    .join(name)
            );
            assert_eq!(
                fs::read(actual.join("weights")).expect("published weights"),
                payload
            );
            assert!(!staging.exists());
        }
        assert_eq!(
            fs::read(desired.join("weights")).expect("first output unchanged"),
            b"first"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    #[tokio::test]
    async fn occupied_files_and_empty_directories_are_never_replaced() {
        let root = tempfile::tempdir().expect("root");
        let desired = root.path().join("model");
        fs::write(&desired, b"unrelated file").expect("occupied file");
        fs::create_dir(root.path().join("model-v2")).expect("occupied empty directory");
        let attempt = OutputWorkspace::prepare(&desired).await.expect("staging");
        fs::write(attempt.staging_path().join("weights"), b"new").expect("payload");
        assert_eq!(
            attempt
                .publish()
                .await
                .expect("publish")
                .file_name()
                .expect("actual leaf"),
            "model-v3"
        );
        assert_eq!(fs::read(desired).expect("old file"), b"unrelated file");
        assert_eq!(
            fs::read_dir(root.path().join("model-v2"))
                .expect("old empty directory")
                .count(),
            0
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn dangling_symlink_and_non_utf8_names_are_preserved() {
        use std::os::unix::ffi::{OsStrExt, OsStringExt};
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().expect("root");
        let name = OsString::from_vec(b"model-\xff".to_vec());
        let desired = root.path().join(&name);
        let absent = root.path().join("absent");
        symlink(&absent, &desired).expect("dangling link");
        let attempt = OutputWorkspace::prepare(&desired).await.expect("staging");
        fs::write(attempt.staging_path().join("weights"), b"new").expect("payload");
        let actual = attempt
            .publish()
            .await
            .expect("publish beside dangling link");
        assert_eq!(
            actual.file_name().expect("actual name").as_bytes(),
            b"model-\xff-v2"
        );
        assert_eq!(fs::read_link(desired).expect("link retained"), absent);
        assert!(!absent.exists());
    }

    #[cfg(any(target_os = "linux", target_os = "macos", windows))]
    #[tokio::test]
    async fn concurrent_publishers_keep_distinct_payloads_and_report_their_paths() {
        let root = tempfile::tempdir().expect("root");
        let desired = root.path().join("model");
        let first = OutputWorkspace::prepare(&desired).await.expect("first");
        let second = OutputWorkspace::prepare(&desired).await.expect("second");
        fs::write(first.staging_path().join("weights"), b"first").expect("first payload");
        fs::write(second.staging_path().join("weights"), b"second").expect("second payload");
        let (first, second) = tokio::join!(first.publish(), second.publish());
        let first = first.expect("first publication");
        let second = second.expect("second publication");
        assert_ne!(first, second);
        assert_eq!(
            fs::read(first.join("weights")).expect("first identity"),
            b"first"
        );
        assert_eq!(
            fs::read(second.join("weights")).expect("second identity"),
            b"second"
        );
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    #[tokio::test]
    async fn noncollision_error_is_propagated_with_staging_payload_intact() {
        let root = tempfile::tempdir().expect("root");
        let attempt = OutputWorkspace::prepare(&root.path().join("x".repeat(300)))
            .await
            .expect("short private staging name");
        let staging = attempt.staging_path().to_path_buf();
        fs::write(staging.join("weights"), b"recoverable").expect("payload");
        assert!(matches!(
            attempt.publish().await,
            Err(PumasError::Io { .. })
        ));
        assert_eq!(
            fs::read(staging.join("weights")).expect("stage retained"),
            b"recoverable"
        );
        assert_eq!(
            fs::read_dir(root.path()).expect("no retry outputs").count(),
            1
        );
    }
}
