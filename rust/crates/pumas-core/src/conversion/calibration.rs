//! Shared supplied-calibration preflight, not retained execution custody.

use std::path::Path;

use tokio::io::AsyncReadExt;

use crate::{PumasError, Result};

/// Callers keep the path stable during inspection and subsequent execution.
/// A successful byte probe does not certify the entire dataset or its format.
pub(super) async fn validate_file(path: &Path) -> Result<()> {
    let metadata = tokio::fs::metadata(path)
        .await
        .map_err(|error| inspection_error("inspecting calibration file", path, error))?;
    require_nonempty_file(&metadata)?;

    // Reject static special files before opening; this does not protect against
    // a caller replacing a regular file with a special file during inspection.
    let mut file = tokio::fs::File::open(path)
        .await
        .map_err(|error| inspection_error("opening calibration file", path, error))?;
    let metadata = file
        .metadata()
        .await
        .map_err(|error| PumasError::io("inspecting opened calibration file", path, error))?;
    require_nonempty_file(&metadata)?;
    let mut byte = [0_u8; 1];
    if file
        .read(&mut byte)
        .await
        .map_err(|error| PumasError::io("reading calibration file", path, error))?
        == 0
    {
        return Err(invalid_file());
    }
    Ok(())
}

fn require_nonempty_file(metadata: &std::fs::Metadata) -> Result<()> {
    if !metadata.is_file() || metadata.len() == 0 {
        return Err(invalid_file());
    }
    Ok(())
}

fn invalid_file() -> PumasError {
    PumasError::InvalidParams {
        message: "Calibration must be a nonempty regular file".into(),
    }
}

fn inspection_error(operation: &str, path: &Path, error: std::io::Error) -> PumasError {
    if error.kind() == std::io::ErrorKind::NotFound {
        PumasError::InvalidParams {
            message: "Calibration file does not exist".into(),
        }
    } else {
        PumasError::io(operation, path, error)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn readable_bytes_are_not_mistaken_for_content_validation() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("calibration.bin");
        std::fs::write(&path, [0xff, 0x00]).unwrap();
        validate_file(&path).await.unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), [0xff, 0x00]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn symlinks_preserve_valid_targets_and_inspection_errors() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("calibration.txt");
        let link = root.path().join("link");
        std::fs::write(&path, "fixture text").unwrap();
        std::os::unix::fs::symlink(&path, &link).unwrap();
        validate_file(&link).await.unwrap();
        let cycle = root.path().join("cycle");
        std::os::unix::fs::symlink(&cycle, &cycle).unwrap();
        assert!(matches!(validate_file(&cycle).await,
            Err(PumasError::Io { message, .. }) if message.contains("inspecting calibration file")));
    }

    #[test]
    fn open_permission_errors_retain_operation_and_source() {
        let error = inspection_error(
            "opening calibration file",
            Path::new("fixture.txt"),
            std::io::Error::from(std::io::ErrorKind::PermissionDenied),
        );
        assert!(matches!(error,
            PumasError::Io { message, source: Some(source), .. }
                if message.contains("opening calibration file")
                    && source.kind() == std::io::ErrorKind::PermissionDenied));
    }
}
