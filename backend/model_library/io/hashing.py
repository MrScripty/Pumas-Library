"""Stream hashing utilities for efficient file hashing.

Provides utilities to compute BLAKE3 and SHA256 hashes during file I/O operations,
avoiding the need to read files multiple times.
"""

from __future__ import annotations

import hashlib
from pathlib import Path
from typing import Any, Dict

from backend.logging_config import get_logger

logger = get_logger(__name__)

# Chunk size for reading files: 8MB (optimal for most SSDs)
_CHUNK_SIZE = 8192 * 1024


class StreamHasher:
    """Compute multiple hashes simultaneously during streaming I/O.

    This class allows computing multiple hash algorithms (SHA256, BLAKE3, MD5, etc.)
    in a single pass over the data, which is more efficient than computing each
    hash separately.

    Example:
        >>> hasher = StreamHasher(algorithms=["sha256", "blake3"])
        >>> with open("large_file.bin", "rb") as f:
        ...     for chunk in iter(lambda: f.read(8192 * 1024), b""):
        ...         hasher.update(chunk)
        >>> hashes = hasher.hexdigest()
        >>> sha256_hash = hashes["sha256"]
        >>> blake3_hash = hashes["blake3"]
    """

    def __init__(self, algorithms: list[str] | None = None) -> None:
        """Initialize the stream hasher.

        Args:
            algorithms: List of hash algorithm names (e.g., ["sha256", "blake3"]).
                       Defaults to ["sha256", "blake3"] if not specified.
        """
        if algorithms is None:
            algorithms = ["sha256", "blake3"]

        self.hashers: Dict[str, Any] = {}

        for algo in algorithms:
            if algo == "blake3":
                try:
                    import blake3

                    self.hashers[algo] = blake3.blake3()
                except ImportError:
                    logger.debug("blake3 not available, skipping BLAKE3 hash")
            else:
                self.hashers[algo] = hashlib.new(algo)

    def update(self, data: bytes) -> None:
        """Update all hash computations with new data.

        Args:
            data: Bytes to add to the hash computation
        """
        for hasher in self.hashers.values():
            hasher.update(data)

    def hexdigest(self) -> Dict[str, str]:
        """Get hex digests for all configured hash algorithms.

        Returns:
            Dictionary mapping algorithm names to their hex digest strings
        """
        return {algo: hasher.hexdigest() for algo, hasher in self.hashers.items()}


def hash_file_sha256(file_path: Path, chunk_size: int = _CHUNK_SIZE) -> str:
    """Compute SHA256 hash of a file.

    Args:
        file_path: Path to the file to hash
        chunk_size: Size of chunks to read (default: 8MB)

    Returns:
        Lowercase hexadecimal SHA256 hash

    Raises:
        FileNotFoundError: If the file does not exist
        OSError: If there's an error reading the file
    """
    hasher = hashlib.sha256()

    with file_path.open("rb") as f:
        for chunk in iter(lambda: f.read(chunk_size), b""):
            hasher.update(chunk)

    return hasher.hexdigest()


def hash_file_blake3(file_path: Path, chunk_size: int = _CHUNK_SIZE) -> str:
    """Compute BLAKE3 hash of a file.

    Args:
        file_path: Path to the file to hash
        chunk_size: Size of chunks to read (default: 8MB)

    Returns:
        Lowercase hexadecimal BLAKE3 hash, or empty string if blake3 not available

    Raises:
        FileNotFoundError: If the file does not exist
        OSError: If there's an error reading the file
    """
    try:
        import blake3
    except ImportError:
        logger.debug("blake3 not available for hashing %s", file_path)
        return ""

    hasher = blake3.blake3()

    with file_path.open("rb") as f:
        for chunk in iter(lambda: f.read(chunk_size), b""):
            hasher.update(chunk)

    return hasher.hexdigest()


def compute_dual_hash(file_path: Path) -> tuple[str, str]:
    """Compute both SHA256 and BLAKE3 hashes in a single pass.

    This is more efficient than calling hash_file_sha256 and hash_file_blake3
    separately, as it only reads the file once.

    Args:
        file_path: Path to the file to hash

    Returns:
        Tuple of (sha256_hex, blake3_hex). blake3_hex will be empty string
        if blake3 is not available.

    Raises:
        FileNotFoundError: If the file does not exist
        OSError: If there's an error reading the file
    """
    hasher = StreamHasher(algorithms=["sha256", "blake3"])

    with file_path.open("rb") as f:
        for chunk in iter(lambda: f.read(_CHUNK_SIZE), b""):
            hasher.update(chunk)

    hashes = hasher.hexdigest()
    return hashes.get("sha256", ""), hashes.get("blake3", "")
