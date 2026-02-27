//! Atomic file operations for safe JSON persistence.
//!
//! Implements atomic writes using:
//! 1. Write to temp file with unique PID+TID suffix
//! 2. fsync to ensure data reaches disk
//! 3. Atomic rename to target path
//! 4. Optional backup creation

use crate::{PumasError, Result};
use serde::{de::DeserializeOwned, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::Path;
use std::process;
use std::thread;
use tracing::{debug, warn};

#[cfg(unix)]
use std::os::unix::io::AsRawFd;

/// Read and parse a JSON file.
///
/// Returns `None` if the file doesn't exist, or an error if parsing fails.
pub fn atomic_read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }

    let mut file = File::open(path).map_err(|e| PumasError::Io {
        message: format!("Failed to open {}", path.display()),
        path: Some(path.to_path_buf()),
        source: Some(e),
    })?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)
        .map_err(|e| PumasError::Io {
            message: format!("Failed to read {}", path.display()),
            path: Some(path.to_path_buf()),
            source: Some(e),
        })?;

    let data: T = serde_json::from_str(&contents).map_err(|e| PumasError::Json {
        message: format!("Failed to parse {}: {}", path.display(), e),
        source: Some(e),
    })?;

    Ok(Some(data))
}

/// Write data to a JSON file atomically.
///
/// This function:
/// 1. Serializes data to a temp file with PID+TID suffix
/// 2. Validates the JSON by re-parsing
/// 3. Calls fsync to ensure data reaches disk
/// 4. Optionally creates a .bak backup
/// 5. Atomically renames temp file to target
pub fn atomic_write_json<T: Serialize>(path: &Path, data: &T, keep_backup: bool) -> Result<()> {
    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            fs::create_dir_all(parent).map_err(|e| PumasError::Io {
                message: format!("Failed to create directory {}", parent.display()),
                path: Some(parent.to_path_buf()),
                source: Some(e),
            })?;
        }
    }

    // Generate unique temp file name
    let pid = process::id();
    let tid = thread_id();
    let temp_path = path.with_extension(format!("json.{}.{}.tmp", pid, tid));

    // Serialize to string with pretty printing
    let serialized = serde_json::to_string_pretty(data).map_err(|e| PumasError::Json {
        message: format!("Failed to serialize data: {}", e),
        source: Some(e),
    })?;

    // Validate JSON by re-parsing
    serde_json::from_str::<serde_json::Value>(&serialized).map_err(|e| PumasError::Json {
        message: format!("JSON validation failed: {}", e),
        source: Some(e),
    })?;

    // Write to temp file
    {
        let mut file = OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .open(&temp_path)
            .map_err(|e| PumasError::Io {
                message: format!("Failed to create temp file {}", temp_path.display()),
                path: Some(temp_path.clone()),
                source: Some(e),
            })?;

        file.write_all(serialized.as_bytes())
            .map_err(|e| PumasError::Io {
                message: format!("Failed to write temp file {}", temp_path.display()),
                path: Some(temp_path.clone()),
                source: Some(e),
            })?;

        file.flush().map_err(|e| PumasError::Io {
            message: format!("Failed to flush temp file {}", temp_path.display()),
            path: Some(temp_path.clone()),
            source: Some(e),
        })?;

        // fsync to ensure data reaches disk
        #[cfg(unix)]
        {
            unsafe {
                libc::fsync(file.as_raw_fd());
            }
        }

        #[cfg(not(unix))]
        {
            file.sync_all().map_err(|e| PumasError::Io {
                message: format!("Failed to sync temp file {}", temp_path.display()),
                path: Some(temp_path.clone()),
                source: Some(e),
            })?;
        }
    }

    // Create backup if requested and target exists
    if keep_backup && path.exists() {
        let backup_path = path.with_extension("json.bak");
        if let Err(e) = fs::copy(path, &backup_path) {
            warn!("Failed to create backup {}: {}", backup_path.display(), e);
            // Continue anyway - backup failure is not fatal
        } else {
            debug!("Created backup: {}", backup_path.display());
        }
    }

    // Atomic rename
    fs::rename(&temp_path, path).map_err(|e| PumasError::Io {
        message: format!(
            "Failed to rename {} to {}",
            temp_path.display(),
            path.display()
        ),
        path: Some(path.to_path_buf()),
        source: Some(e),
    })?;

    debug!("Atomically wrote {}", path.display());
    Ok(())
}

/// Get a unique thread identifier.
fn thread_id() -> u64 {
    // Use thread ID hash as a numeric identifier
    let id = thread::current().id();
    // Format as debug string and hash it
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    let mut hasher = DefaultHasher::new();
    format!("{:?}", id).hash(&mut hasher);
    hasher.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use tempfile::TempDir;

    #[derive(Debug, Serialize, Deserialize, PartialEq)]
    struct TestData {
        name: String,
        value: i32,
    }

    #[test]
    fn test_atomic_write_and_read() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");

        let data = TestData {
            name: "test".to_string(),
            value: 42,
        };

        // Write
        atomic_write_json(&path, &data, false).unwrap();
        assert!(path.exists());

        // Read
        let read_data: Option<TestData> = atomic_read_json(&path).unwrap();
        assert_eq!(read_data, Some(data));
    }

    #[test]
    fn test_atomic_write_creates_backup() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("test.json");

        let data1 = TestData {
            name: "first".to_string(),
            value: 1,
        };
        let data2 = TestData {
            name: "second".to_string(),
            value: 2,
        };

        // First write
        atomic_write_json(&path, &data1, true).unwrap();

        // Second write with backup
        atomic_write_json(&path, &data2, true).unwrap();

        // Check backup exists
        let backup_path = path.with_extension("json.bak");
        assert!(backup_path.exists());

        // Verify backup contains first data
        let backup_data: Option<TestData> = atomic_read_json(&backup_path).unwrap();
        assert_eq!(backup_data, Some(data1));

        // Verify current file contains second data
        let current_data: Option<TestData> = atomic_read_json(&path).unwrap();
        assert_eq!(current_data, Some(data2));
    }

    #[test]
    fn test_atomic_read_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("nonexistent.json");

        let result: Option<TestData> = atomic_read_json(&path).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_atomic_write_creates_directories() {
        let temp_dir = TempDir::new().unwrap();
        let path = temp_dir.path().join("nested").join("dir").join("test.json");

        let data = TestData {
            name: "nested".to_string(),
            value: 99,
        };

        atomic_write_json(&path, &data, false).unwrap();
        assert!(path.exists());
    }
}
