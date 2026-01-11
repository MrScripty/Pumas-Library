"""Drive-aware I/O queue manager for model library operations.

Provides efficient file operations with automatic SSD/HDD detection
to optimize concurrency and disk access patterns.
"""

from __future__ import annotations

import os
import shutil
import subprocess
import threading
from contextlib import contextmanager
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from typing import Dict, Generator

from backend.logging_config import get_logger
from backend.model_library.io.hashing import StreamHasher

logger = get_logger(__name__)

# Default concurrency limits based on drive type
_DEFAULT_SSD_CONCURRENCY = 4  # SSDs handle parallel I/O well
_DEFAULT_HDD_CONCURRENCY = 2  # HDDs prefer sequential access

# Chunk size for streaming operations (8MB)
_CHUNK_SIZE = 8192 * 1024


class DriveType(Enum):
    """Type of storage drive."""

    SSD = "ssd"
    HDD = "hdd"
    UNKNOWN = "unknown"


@dataclass
class DriveInfo:
    """Information about a drive/filesystem.

    Attributes:
        path: Path that was queried
        drive_type: Type of drive (SSD, HDD, or UNKNOWN)
        device: Device path (e.g., /dev/sda)
        mount_point: Filesystem mount point
    """

    path: Path
    drive_type: DriveType = DriveType.UNKNOWN
    device: str = ""
    mount_point: Path | None = None


def _get_device_for_path(path: Path) -> str:
    """Get the block device for a given path.

    Args:
        path: Path to check

    Returns:
        Device name (e.g., 'sda') or empty string if not found
    """
    try:
        # Use df to get the source device
        result = subprocess.run(
            ["df", "--output=source", str(path)],
            capture_output=True,
            text=True,
            check=False,
            timeout=5,
        )
        if result.returncode == 0:
            lines = result.stdout.strip().split("\n")
            if len(lines) >= 2:
                device = lines[1].strip()
                # Extract base device name (e.g., /dev/sda1 -> sda)
                if device.startswith("/dev/"):
                    # Remove partition number
                    base_device = device[5:].rstrip("0123456789")
                    return base_device
    except (FileNotFoundError, subprocess.TimeoutExpired, OSError) as e:  # noqa: multi-exception
        logger.debug("Failed to get device for %s: %s", path, e)

    return ""


def _check_rotational_flag(device: str) -> DriveType | None:
    """Check the rotational flag in sysfs to determine drive type.

    Args:
        device: Device name (e.g., 'sda')

    Returns:
        DriveType.SSD if rotational=0, DriveType.HDD if rotational=1,
        None if check fails
    """
    rotational_path = Path(f"/sys/block/{device}/queue/rotational")

    try:
        if rotational_path.exists():
            rotational = rotational_path.read_text().strip()
            if rotational == "0":
                return DriveType.SSD
            elif rotational == "1":
                return DriveType.HDD
    except OSError as e:
        logger.debug("Failed to read rotational flag for %s: %s", device, e)

    return None


def get_drive_type(path: Path) -> DriveType:
    """Determine the type of drive (SSD or HDD) for a given path.

    Uses Linux sysfs to check the rotational flag when available.

    Args:
        path: Path to check

    Returns:
        DriveType.SSD, DriveType.HDD, or DriveType.UNKNOWN
    """
    # Get the closest existing path
    check_path = path
    while not check_path.exists() and check_path != check_path.parent:
        check_path = check_path.parent

    if not check_path.exists():
        return DriveType.UNKNOWN

    # Get device for path
    device = _get_device_for_path(check_path)
    if not device:
        return DriveType.UNKNOWN

    # Check rotational flag
    drive_type = _check_rotational_flag(device)
    if drive_type is not None:
        logger.debug("Detected %s as %s", path, drive_type.value)
        return drive_type

    return DriveType.UNKNOWN


def get_drive_info(path: Path) -> DriveInfo:
    """Get comprehensive drive information for a path.

    Args:
        path: Path to check

    Returns:
        DriveInfo with device, mount point, and drive type
    """
    drive_type = get_drive_type(path)
    device = _get_device_for_path(path)

    # Get mount point
    mount_point: Path | None = None
    try:
        result = subprocess.run(
            ["df", "--output=target", str(path)],
            capture_output=True,
            text=True,
            check=False,
            timeout=5,
        )
        if result.returncode == 0:
            lines = result.stdout.strip().split("\n")
            if len(lines) >= 2:
                mount_point = Path(lines[1].strip())
    except (FileNotFoundError, subprocess.TimeoutExpired, OSError) as e:  # noqa: multi-exception
        logger.debug("Failed to get mount point for %s: %s", path, e)

    return DriveInfo(
        path=path,
        drive_type=drive_type,
        device=f"/dev/{device}" if device else "",
        mount_point=mount_point,
    )


class IOManager:
    """Drive-aware I/O manager for optimized file operations.

    Provides file copy and move operations with:
    - Automatic SSD/HDD detection
    - Drive-based concurrency limits
    - Optional stream hashing during copy
    - Caching of drive detection results

    Example:
        manager = IOManager()
        result, hashes = manager.copy_file_with_hashing(src, dst)
        sha256 = hashes["sha256"]
    """

    def __init__(
        self,
        ssd_concurrency: int = _DEFAULT_SSD_CONCURRENCY,
        hdd_concurrency: int = _DEFAULT_HDD_CONCURRENCY,
    ) -> None:
        """Initialize the I/O manager.

        Args:
            ssd_concurrency: Maximum concurrent operations for SSDs
            hdd_concurrency: Maximum concurrent operations for HDDs
        """
        self.ssd_concurrency = ssd_concurrency
        self.hdd_concurrency = hdd_concurrency
        self._drive_cache: Dict[Path, DriveType] = {}
        self._semaphores: Dict[str, threading.Semaphore] = {}
        self._semaphore_lock = threading.Lock()

    def _get_cached_drive_type(self, path: Path) -> DriveType:
        """Get drive type with caching.

        Args:
            path: Path to check

        Returns:
            Cached or freshly detected DriveType
        """
        # Use path directly as cache key (simple but effective)
        # Resolve to canonical path to improve cache hits
        try:
            cache_key = path.resolve()
        except OSError:  # noqa: no-except-logging
            cache_key = path

        if cache_key not in self._drive_cache:
            self._drive_cache[cache_key] = get_drive_type(path)

        return self._drive_cache[cache_key]

    def get_concurrency_for_path(self, path: Path) -> int:
        """Get the recommended concurrency limit for operations on a path.

        Args:
            path: Path to check

        Returns:
            Recommended maximum concurrent operations
        """
        drive_type = self._get_cached_drive_type(path)

        if drive_type == DriveType.SSD:
            return self.ssd_concurrency
        else:
            # Default to HDD concurrency for HDD or UNKNOWN (conservative)
            return self.hdd_concurrency

    def clear_drive_cache(self) -> None:
        """Clear the drive type cache.

        Useful when drives are mounted/unmounted during runtime.
        """
        self._drive_cache.clear()

    def _get_mount_point(self, path: Path) -> str:
        """Get the mount point for a given path.

        Args:
            path: Path to check

        Returns:
            Mount point as string
        """
        try:
            check_path = path.resolve()
        except OSError:  # noqa: no-except-logging
            check_path = path

        while not check_path.is_mount() and check_path != check_path.parent:
            check_path = check_path.parent

        return str(check_path)

    def get_semaphore(self, path: Path) -> threading.Semaphore:
        """Get the appropriate semaphore for this path's drive.

        Semaphores limit concurrent I/O operations based on drive type:
        - SSD: Higher concurrency (default 4)
        - HDD: Lower concurrency (default 2) to prevent disk thrashing

        Args:
            path: Path for the I/O operation

        Returns:
            Semaphore for this path's mount point/drive
        """
        mount_point = self._get_mount_point(path)

        with self._semaphore_lock:
            if mount_point not in self._semaphores:
                concurrency = self.get_concurrency_for_path(path)
                self._semaphores[mount_point] = threading.Semaphore(concurrency)
                logger.debug(
                    "Created semaphore for %s with concurrency=%d",
                    mount_point,
                    concurrency,
                )

            return self._semaphores[mount_point]

    @contextmanager
    def io_slot(self, path: Path) -> Generator[None, None, None]:
        """Context manager for acquiring an I/O slot for a path.

        Use this to limit concurrent I/O operations on a drive.
        Blocks until a slot is available.

        Example:
            with io_manager.io_slot(dest_path):
                # Perform I/O operation
                shutil.copy(src, dest)

        Args:
            path: Path for the I/O operation

        Yields:
            None (just provides the context)
        """
        semaphore = self.get_semaphore(path)
        semaphore.acquire()
        try:
            yield
        finally:
            semaphore.release()

    def is_same_filesystem(self, path1: Path, path2: Path) -> bool:
        """Check if two paths are on the same filesystem.

        Useful for determining if atomic rename is possible or if
        a cross-filesystem copy is required.

        Args:
            path1: First path
            path2: Second path

        Returns:
            True if both paths are on the same filesystem
        """
        try:
            # Get the closest existing path for each
            check1 = path1
            while not check1.exists() and check1 != check1.parent:
                check1 = check1.parent

            check2 = path2
            while not check2.exists() and check2 != check2.parent:
                check2 = check2.parent

            if not check1.exists() or not check2.exists():
                return False

            return check1.stat().st_dev == check2.stat().st_dev
        except OSError as e:
            logger.debug("Failed to check filesystem: %s", e)
            return False

    def copy_file(
        self,
        src: Path,
        dst: Path,
        preserve_mtime: bool = False,
    ) -> Path:
        """Copy a file from source to destination.

        Args:
            src: Source file path
            dst: Destination file path
            preserve_mtime: Whether to preserve modification time

        Returns:
            Destination path

        Raises:
            FileNotFoundError: If source file doesn't exist
            OSError: If copy operation fails
        """
        if not src.exists():
            raise FileNotFoundError(f"Source file not found: {src}")

        # Create parent directories if needed
        dst.parent.mkdir(parents=True, exist_ok=True)

        # Use shutil for basic copy
        shutil.copy2(str(src), str(dst))

        # Handle mtime preservation explicitly if requested
        if preserve_mtime:
            src_stat = src.stat()
            os.utime(dst, (src_stat.st_atime, src_stat.st_mtime))

        return dst

    def copy_file_with_hashing(
        self,
        src: Path,
        dst: Path,
        algorithms: list[str] | None = None,
        preserve_mtime: bool = False,
    ) -> tuple[Path, Dict[str, str]]:
        """Copy a file while computing hashes in a single pass.

        More efficient than copying then hashing separately.

        Args:
            src: Source file path
            dst: Destination file path
            algorithms: Hash algorithms to compute (default: ["sha256", "blake3"])
            preserve_mtime: Whether to preserve modification time

        Returns:
            Tuple of (destination path, dict of hash name -> hex digest)

        Raises:
            FileNotFoundError: If source file doesn't exist
            OSError: If copy operation fails
        """
        if not src.exists():
            raise FileNotFoundError(f"Source file not found: {src}")

        # Create parent directories if needed
        dst.parent.mkdir(parents=True, exist_ok=True)

        # Initialize hasher
        hasher = StreamHasher(algorithms=algorithms)

        # Copy with streaming hash computation
        with src.open("rb") as src_file, dst.open("wb") as dst_file:
            for chunk in iter(lambda: src_file.read(_CHUNK_SIZE), b""):
                dst_file.write(chunk)
                hasher.update(chunk)

        # Preserve mtime if requested
        if preserve_mtime:
            src_stat = src.stat()
            os.utime(dst, (src_stat.st_atime, src_stat.st_mtime))

        return dst, hasher.hexdigest()

    def move_file(
        self,
        src: Path,
        dst: Path,
        preserve_mtime: bool = True,
    ) -> Path:
        """Move a file from source to destination.

        Uses rename if possible, falls back to copy+delete for cross-device moves.

        Args:
            src: Source file path
            dst: Destination file path
            preserve_mtime: Whether to preserve modification time (for cross-device)

        Returns:
            Destination path

        Raises:
            FileNotFoundError: If source file doesn't exist
            OSError: If move operation fails
        """
        if not src.exists():
            raise FileNotFoundError(f"Source file not found: {src}")

        # Create parent directories if needed
        dst.parent.mkdir(parents=True, exist_ok=True)

        try:
            # Try atomic rename first
            src.rename(dst)
            return dst
        except OSError:  # noqa: no-except-logging
            # Cross-device move: copy then delete
            self.copy_file(src, dst, preserve_mtime=preserve_mtime)
            src.unlink()
            return dst


# Global instance for shared I/O management across the application
io_manager = IOManager()
