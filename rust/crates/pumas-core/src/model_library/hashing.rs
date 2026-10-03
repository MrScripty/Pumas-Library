//! Streaming hash computation for model files.
//!
//! Provides SHA256 and BLAKE3 hashing with:
//! - Single-pass dual hash computation
//! - Fast hash (first + last 8MB) for quick filtering
//! - Progress reporting for large files

use crate::error::{PumasError, Result};
use blake3::Hasher as Blake3Hasher;
use sha2::{Digest, Sha256};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;
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

    compute_dual_hash_reader(&mut file).map_err(|error| match error {
        PumasError::Io {
            message, source, ..
        } => PumasError::Io {
            message,
            path: Some(path.to_path_buf()),
            source,
        },
        error => error,
    })
}

pub(super) fn compute_dual_hash_reader(file: &mut impl Read) -> Result<DualHash> {
    let mut sha256_hasher = Sha256::new();
    let mut blake3_hasher = Blake3Hasher::new();

    let mut buffer = vec![0u8; CHUNK_SIZE];
    loop {
        let bytes_read = read_chunk(file, &mut buffer)?;
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

/// Hash precisely the bytes written during a copied import, without a second
/// source read. The caller owns both descriptors and their final synchronization.
pub(super) fn copy_and_hash(
    input: &mut impl Read,
    output: &mut impl Write,
) -> Result<(u64, DualHash)> {
    let mut sha256 = Sha256::new();
    let mut blake3 = Blake3Hasher::new();
    let mut size = 0_u64;
    let mut buffer = vec![0_u8; CHUNK_SIZE];
    loop {
        let count = read_chunk(input, &mut buffer)?;
        if count == 0 {
            break;
        }
        output.write_all(&buffer[..count])?;
        sha256.update(&buffer[..count]);
        blake3.update(&buffer[..count]);
        size += count as u64;
    }
    Ok((
        size,
        DualHash {
            sha256: hex::encode(sha256.finalize()),
            blake3: blake3.finalize().to_hex().to_string(),
        },
    ))
}

// Interrupted reads transfer no bytes. Retrying only that structured OS kind
// preserves EOF, partial output and all other input errors.
fn read_chunk(input: &mut impl Read, buffer: &mut [u8]) -> std::io::Result<usize> {
    loop {
        match input.read(buffer) {
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            result => return result,
        }
    }
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
    hasher.update(file_size.to_le_bytes());

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

/// Verify a file's BLAKE3 hash matches an expected value.
///
/// Returns `Ok(())` if the hash matches, or `Err(HashMismatch)` if it does not.
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

    struct InterruptedInput {
        input: std::io::Cursor<Vec<u8>>,
        interrupt_next: bool,
    }

    impl Read for InterruptedInput {
        fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
            self.interrupt_next = !self.interrupt_next;
            if self.interrupt_next {
                return Err(std::io::ErrorKind::Interrupted.into());
            }
            let limit = buffer.len().min(3);
            self.input.read(&mut buffer[..limit])
        }
    }

    #[test]
    fn copied_import_stream_and_verification_retry_interrupted_reads_without_changing_evidence() {
        let bytes = b"synthetic model bytes";
        let interrupted = || InterruptedInput {
            input: std::io::Cursor::new(bytes.to_vec()),
            interrupt_next: false,
        };
        let mut output = Vec::new();
        let (size, copied) = copy_and_hash(&mut interrupted(), &mut output).unwrap();
        let verified = compute_dual_hash_reader(&mut interrupted()).unwrap();
        let expected = compute_dual_hash_reader(&mut bytes.as_slice()).unwrap();
        assert_eq!(size, bytes.len() as u64);
        assert_eq!(output, bytes);
        assert_eq!(copied.sha256, expected.sha256);
        assert_eq!(copied.blake3, expected.blake3);
        assert_eq!(verified.sha256, expected.sha256);
        assert_eq!(verified.blake3, expected.blake3);
    }

    #[test]
    fn copied_import_stream_preserves_non_interruption_errors_and_partial_output() {
        struct FailingInput(bool);
        impl Read for FailingInput {
            fn read(&mut self, buffer: &mut [u8]) -> std::io::Result<usize> {
                if self.0 {
                    return Err(std::io::ErrorKind::InvalidData.into());
                }
                self.0 = true;
                buffer[..3].copy_from_slice(b"one");
                Ok(3)
            }
        }
        let mut output = Vec::new();
        let error = copy_and_hash(&mut FailingInput(false), &mut output).unwrap_err();
        assert!(matches!(error, PumasError::Io { source: Some(source), .. }
            if source.kind() == std::io::ErrorKind::InvalidData));
        assert_eq!(output, b"one");
    }

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
}
