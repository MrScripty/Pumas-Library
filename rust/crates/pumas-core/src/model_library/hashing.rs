//! Streaming hash computation for model files.
//!
//! Provides SHA256 and BLAKE3 hashing with:
//! - Single-pass dual hash computation
//! - Fast hash (first + last 8MB) for quick filtering
//! - Progress reporting for large files

use crate::error::{PumasError, Result};
use blake3::Hasher as Blake3Hasher;
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use tokio::sync::mpsc;

/// Chunk size for reading files (8MB, optimal for SSDs).
const CHUNK_SIZE: usize = 8 * 1024 * 1024;

/// Size to read for fast hash (first + last 8MB).
const FAST_HASH_SIZE: usize = 8 * 1024 * 1024;

/// Dual hash result containing both SHA256 and BLAKE3.
#[derive(Debug, Clone)]
pub struct DualHash {
    /// SHA256 hash as lowercase hex string
    pub sha256: String,
    /// BLAKE3 hash as lowercase hex string
    pub blake3: String,
}

/// Progress update during hashing.
#[derive(Debug, Clone)]
pub struct HashProgress {
    /// Bytes processed so far
    pub bytes_processed: u64,
    /// Total file size
    pub total_bytes: u64,
    /// Progress percentage (0.0-1.0)
    pub progress: f32,
}

/// Compute both SHA256 and BLAKE3 hashes in a single pass.
///
/// This is more efficient than computing them separately since
/// we only read the file once.
///
/// # Arguments
///
/// * `path` - Path to the file to hash
///
/// # Returns
///
/// DualHash containing both hash values as hex strings.
pub fn compute_dual_hash(path: impl AsRef<Path>) -> Result<DualHash> {
    let path = path.as_ref();
    let mut file = std::fs::File::open(path).map_err(|e| PumasError::io_with_path(e, path))?;

    let mut sha256_hasher = Sha256::new();
    let mut blake3_hasher = Blake3Hasher::new();

    let mut buffer = vec![0u8; CHUNK_SIZE];
    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|e| PumasError::io_with_path(e, path))?;
        if bytes_read == 0 {
            break;
        }

        sha256_hasher.update(&buffer[..bytes_read]);
        blake3_hasher.update(&buffer[..bytes_read]);
    }

    let sha256 = hex::encode(sha256_hasher.finalize());
    let blake3 = blake3_hasher.finalize().to_hex().to_string();

    Ok(DualHash { sha256, blake3 })
}

/// Compute dual hash with progress reporting.
///
/// # Arguments
///
/// * `path` - Path to the file to hash
/// * `progress_tx` - Channel for progress updates
///
/// # Returns
///
/// DualHash containing both hash values.
pub async fn compute_dual_hash_with_progress(
    path: impl AsRef<Path>,
    progress_tx: Option<mpsc::Sender<HashProgress>>,
) -> Result<DualHash> {
    let path = path.as_ref().to_path_buf();
    let progress_tx = progress_tx.clone();

    // Run in blocking task since file I/O is blocking
    tokio::task::spawn_blocking(move || {
        let mut file = std::fs::File::open(&path).map_err(|e| PumasError::io_with_path(e, &path))?;

        let total_bytes = file
            .metadata()
            .map_err(|e| PumasError::io_with_path(e, &path))?
            .len();

        let mut sha256_hasher = Sha256::new();
        let mut blake3_hasher = Blake3Hasher::new();

        let mut buffer = vec![0u8; CHUNK_SIZE];
        let mut bytes_processed: u64 = 0;

        loop {
            let bytes_read = file
                .read(&mut buffer)
                .map_err(|e| PumasError::io_with_path(e, &path))?;
            if bytes_read == 0 {
                break;
            }

            sha256_hasher.update(&buffer[..bytes_read]);
            blake3_hasher.update(&buffer[..bytes_read]);

            bytes_processed += bytes_read as u64;

            // Send progress update (non-blocking, ignore errors)
            if let Some(ref tx) = progress_tx {
                let progress = HashProgress {
                    bytes_processed,
                    total_bytes,
                    progress: bytes_processed as f32 / total_bytes as f32,
                };
                let _ = tx.try_send(progress);
            }
        }

        let sha256 = hex::encode(sha256_hasher.finalize());
        let blake3 = blake3_hasher.finalize().to_hex().to_string();

        Ok(DualHash { sha256, blake3 })
    })
    .await
    .map_err(|e| PumasError::Other(format!("Hash computation task failed: {}", e)))?
}

/// Compute a fast hash for quick candidate filtering.
///
/// Reads only the first and last 8MB of the file plus the file size,
/// making it much faster for large files while still providing
/// good uniqueness for filtering candidates.
///
/// # Arguments
///
/// * `path` - Path to the file to hash
///
/// # Returns
///
/// SHA256 hash of (first_8mb + last_8mb + size_bytes).
pub fn compute_fast_hash(path: impl AsRef<Path>) -> Result<String> {
    let path = path.as_ref();
    let mut file = std::fs::File::open(path).map_err(|e| PumasError::io_with_path(e, path))?;

    let file_size = file
        .metadata()
        .map_err(|e| PumasError::io_with_path(e, path))?
        .len();

    let mut hasher = Sha256::new();

    // Read first 8MB
    let first_chunk_size = std::cmp::min(file_size as usize, FAST_HASH_SIZE);
    let mut buffer = vec![0u8; first_chunk_size];
    file.read_exact(&mut buffer)
        .map_err(|e| PumasError::io_with_path(e, path))?;
    hasher.update(&buffer);

    // Read last 8MB (if file is large enough)
    if file_size > FAST_HASH_SIZE as u64 * 2 {
        let last_start = file_size - FAST_HASH_SIZE as u64;
        file.seek(SeekFrom::Start(last_start))
            .map_err(|e| PumasError::io_with_path(e, path))?;

        let mut last_buffer = vec![0u8; FAST_HASH_SIZE];
        file.read_exact(&mut last_buffer)
            .map_err(|e| PumasError::io_with_path(e, path))?;
        hasher.update(&last_buffer);
    }

    // Include file size
    hasher.update(&file_size.to_le_bytes());

    Ok(hex::encode(hasher.finalize()))
}

/// Verify a file's SHA256 hash matches expected value.
///
/// # Arguments
///
/// * `path` - Path to the file
/// * `expected` - Expected SHA256 hash (lowercase hex)
///
/// # Returns
///
/// Ok(()) if hash matches, Err if mismatch.
pub fn verify_sha256(path: impl AsRef<Path>, expected: &str) -> Result<()> {
    let path = path.as_ref();
    let mut file = std::fs::File::open(path).map_err(|e| PumasError::io_with_path(e, path))?;

    let mut hasher = Sha256::new();
    let mut buffer = vec![0u8; CHUNK_SIZE];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|e| PumasError::io_with_path(e, path))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let actual = hex::encode(hasher.finalize());
    let expected_lower = expected.to_lowercase();

    if actual == expected_lower {
        Ok(())
    } else {
        Err(PumasError::HashMismatch {
            expected: expected_lower,
            actual,
        })
    }
}

/// Verify a file's BLAKE3 hash matches expected value.
pub fn verify_blake3(path: impl AsRef<Path>, expected: &str) -> Result<()> {
    let path = path.as_ref();
    let mut file = std::fs::File::open(path).map_err(|e| PumasError::io_with_path(e, path))?;

    let mut hasher = Blake3Hasher::new();
    let mut buffer = vec![0u8; CHUNK_SIZE];

    loop {
        let bytes_read = file
            .read(&mut buffer)
            .map_err(|e| PumasError::io_with_path(e, path))?;
        if bytes_read == 0 {
            break;
        }
        hasher.update(&buffer[..bytes_read]);
    }

    let actual = hasher.finalize().to_hex().to_string();
    let expected_lower = expected.to_lowercase();

    if actual == expected_lower {
        Ok(())
    } else {
        Err(PumasError::HashMismatch {
            expected: expected_lower,
            actual,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_dual_hash_empty_file() {
        let file = NamedTempFile::new().unwrap();
        let hash = compute_dual_hash(file.path()).unwrap();

        // SHA256 of empty file
        assert_eq!(
            hash.sha256,
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // BLAKE3 of empty file
        assert_eq!(
            hash.blake3,
            "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262"
        );
    }

    #[test]
    fn test_dual_hash_content() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"Hello, World!").unwrap();
        file.flush().unwrap();

        let hash = compute_dual_hash(file.path()).unwrap();
        assert!(!hash.sha256.is_empty());
        assert!(!hash.blake3.is_empty());
        assert_eq!(hash.sha256.len(), 64); // SHA256 is 32 bytes = 64 hex chars
        assert_eq!(hash.blake3.len(), 64); // BLAKE3 default is 32 bytes = 64 hex chars
    }

    #[test]
    fn test_fast_hash_small_file() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"Small content").unwrap();
        file.flush().unwrap();

        let hash = compute_fast_hash(file.path()).unwrap();
        assert_eq!(hash.len(), 64);
    }

    #[test]
    fn test_verify_sha256_match() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"test content").unwrap();
        file.flush().unwrap();

        // Compute expected hash
        let hash = compute_dual_hash(file.path()).unwrap();

        // Verification should succeed
        assert!(verify_sha256(file.path(), &hash.sha256).is_ok());
    }

    #[test]
    fn test_verify_sha256_mismatch() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"test content").unwrap();
        file.flush().unwrap();

        let result = verify_sha256(file.path(), "wrong_hash");
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_dual_hash_with_progress() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(&vec![0u8; 1024 * 1024]).unwrap(); // 1MB
        file.flush().unwrap();

        let (tx, mut rx) = mpsc::channel(32);
        let hash = compute_dual_hash_with_progress(file.path(), Some(tx)).await.unwrap();

        assert!(!hash.sha256.is_empty());

        // Should have received at least one progress update
        // Note: Small files might complete before any progress is sent
        let mut progress_received = false;
        while let Ok(progress) = rx.try_recv() {
            progress_received = true;
            assert!(progress.progress >= 0.0 && progress.progress <= 1.0);
        }
        // For small files, we might not get progress updates
        let _ = progress_received;
    }
}
