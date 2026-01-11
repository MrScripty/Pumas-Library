# Performance & Data Integrity Strategy

**Version**: 3.1

---

## Table of Contents

- [Overview](#overview)
- [Core Principles](#core-principles)
- [Performance Optimizations](#performance-optimizations)
  - [Smart I/O Queue Manager](#1-smart-io-queue-manager)
  - [Stream Hashing](#2-stream-hashing-hash-while-copy)
  - [Incremental Sync Strategy](#3-incremental-sync-strategy)
  - [HuggingFace API Throttling](#4-huggingface-api-throttling)
- [Data Integrity Safeguards](#data-integrity-safeguards)
  - [Atomic Imports](#1-atomic-imports-with-tmp-extension)
  - [Atomic Directory Imports](#2-atomic-directory-imports-diffusers)
  - [SQLite Write-Ahead Logging](#3-sqlite-write-ahead-logging-wal)
  - [WAL Checkpointing](#4-wal-checkpointing)
  - [Deep Scan Rebuild](#5-deep-scan-rebuild-capability)
- [Network & API Resilience](#network--api-resilience)
  - [Offline-First Import](#1-offline-first-import-strategy)
  - [Metadata Caching](#2-aggressive-metadata-caching)
- [Filesystem Validation](#filesystem-validation)
  - [Pre-Flight Validation](#filesystem-pre-flight-validation)
  - [NTFS Filename Normalization](#ntfs-compatible-filename-normalization)
  - [Cross-Filesystem Detection](#cross-filesystem-detection)
  - [NVMe Device Handling](#nvme-device-handling)
- [Platform Compatibility](#platform-compatibility)
  - [Sandbox Detection](#1-sandboxflatpak-detection)
  - [Dynamic Directory Scanning](#2-dynamic-directory-scanning)
- [Import Modes](#import-modes)
  - [Fast Import (Move)](#fast-import-move)
  - [Safe Import (Copy)](#safe-import-copy)
  - [Space Management](#space-management-delete-originals)

---

## Overview

This document outlines the architectural decisions for handling large model files (often 20GB+) efficiently while maintaining data integrity and system responsiveness.

---

## Core Principles

1. **Metadata as Single Source of Truth (SSoT)**: The `metadata.json` file inside each model directory is the authoritative record. SQLite is treated as a **disposable cache** that can be perfectly reconstructed via a "Deep Scan" of the library folders.

2. **Atomic Operations**: All file operations use temporary extensions during processing to prevent partial indexing by watchers or crashes mid-operation.

3. **Offline-First Design**: Network failures or API rate limits never block local operations.

4. **Platform-Aware I/O**: Optimize disk access patterns based on drive type (SSD vs HDD) to prevent thrashing.

5. **Incremental Processing**: Only process what changed, not the entire library.

---

## Performance Optimizations

### 1. Smart I/O Queue Manager

**Problem**: Parallel file operations on HDDs cause severe disk thrashing, degrading performance.

**Solution**: Drive-aware I/O queueing system that detects drive type and limits concurrency accordingly.

**Implementation**: New module `backend/model_library/io_manager.py`

```python
import psutil
from pathlib import Path
from queue import Queue
from threading import Lock, Semaphore
from typing import Literal

class IOManager:
    """
    Manages disk I/O operations with drive-type awareness.

    Strategy:
    - SSD: Allow 2 concurrent operations
    - HDD: Force strictly sequential (1 operation at a time)
    - Unknown: Assume HDD (safe default)
    """

    def __init__(self):
        self._drive_cache: dict[str, Literal["ssd", "hdd"]] = {}
        self._semaphores: dict[str, Semaphore] = {}
        self._lock = Lock()

    def detect_drive_type(self, path: Path) -> dict:
        """
        Detect if path is on SSD or HDD.

        Uses psutil to check disk type. Falls back to HDD if uncertain.

        Returns:
            {
                'drive_type': 'ssd' | 'hdd',
                'is_restricted_environment': bool,
                'confidence': 'high' | 'low'  # 'low' when /sys/block unavailable
            }
        """
        mount_point = self._get_mount_point(path)

        with self._lock:
            if mount_point in self._drive_cache:
                cached = self._drive_cache[mount_point]
                return {
                    'drive_type': cached['drive_type'],
                    'is_restricted_environment': cached.get('is_restricted_environment', False),
                    'confidence': cached.get('confidence', 'high')
                }

        # Detect via psutil
        is_restricted = False
        try:
            partitions = psutil.disk_partitions()
            for partition in partitions:
                if mount_point.startswith(partition.mountpoint):
                    # Check if rotational (Linux-specific via /sys/block/)
                    device = partition.device.replace('/dev/', '')

                    # Extract base device name (handles NVMe, MMC, etc.)
                    base_device = self._extract_base_device(device)
                    rotational_path = f"/sys/block/{base_device}/queue/rotational"

                    try:
                        with open(rotational_path, 'r') as f:
                            is_rotational = f.read().strip() == '1'
                            drive_type = "hdd" if is_rotational else "ssd"

                        result = {
                            'drive_type': drive_type,
                            'is_restricted_environment': False,
                            'confidence': 'high'
                        }
                    except (FileNotFoundError, PermissionError) as e:
                        # Sandboxed environment: /sys/block not accessible
                        logger.warning(
                            f"Cannot access {rotational_path}: {e}. "
                            f"Running in restricted environment (Flatpak/Docker?). "
                            f"Defaulting to HDD (safe mode)."
                        )
                        drive_type = "hdd"  # Safe default
                        is_restricted = True

                        result = {
                            'drive_type': drive_type,
                            'is_restricted_environment': True,
                            'confidence': 'low'
                        }

                    with self._lock:
                        self._drive_cache[mount_point] = result

                    return result
        except Exception as e:
            logger.error(f"Error detecting drive type: {e}")

        # Fallback: Assume HDD with low confidence
        result = {
            'drive_type': "hdd",
            'is_restricted_environment': True,
            'confidence': 'low'
        }

        with self._lock:
            self._drive_cache[mount_point] = result

        return result

    def get_semaphore(self, path: Path) -> Semaphore:
        """
        Get the appropriate semaphore for this path's drive.

        Returns:
            Semaphore with count=2 for SSD, count=1 for HDD
        """
        mount_point = self._get_mount_point(path)

        with self._lock:
            if mount_point not in self._semaphores:
                drive_info = self.detect_drive_type(path)
                drive_type = drive_info['drive_type']
                max_concurrent = 2 if drive_type == "ssd" else 1
                self._semaphores[mount_point] = Semaphore(max_concurrent)

        return self._semaphores[mount_point]

    def _get_mount_point(self, path: Path) -> str:
        """Get the mount point for a given path."""
        path = path.resolve()
        while not path.is_mount() and path != path.parent:
            path = path.parent
        return str(path)

    def _extract_base_device(self, device: str) -> str:
        """
        Extract base block device name from partition path.

        Examples:
        - sda1 → sda
        - nvme0n1p1 → nvme0n1
        - mmcblk0p1 → mmcblk0
        - loop0 → loop0
        - dm-0 → dm-0 (LVM/LUKS)
        - mapper/vg-lv → mapper/vg-lv
        """
        # Handle LVM/LUKS devices (dm-0, mapper/vg-lv)
        if device.startswith('dm-') or 'mapper/' in device:
            # Try to resolve to underlying device via /sys/block
            try:
                sys_path = f"/sys/block/{device.replace('mapper/', 'dm-')}/slaves"
                if Path(sys_path).exists():
                    slaves = list(Path(sys_path).iterdir())
                    if slaves:
                        # Use first slave device (usually the physical disk)
                        return self._extract_base_device(slaves[0].name)
            except Exception:
                pass
            # Fallback: treat as HDD (safe default)
            return device

        # Handle NVMe partitions (nvme0n1p1 → nvme0n1)
        if device.startswith('nvme'):
            parts = device.split('p')
            if len(parts) > 1 and parts[-1].isdigit():
                return 'p'.join(parts[:-1])
            return device

        # Handle MMC partitions (mmcblk0p1 → mmcblk0)
        if device.startswith('mmcblk'):
            parts = device.split('p')
            if len(parts) > 1 and parts[-1].isdigit():
                return 'p'.join(parts[:-1])
            return device

        # Handle SATA/SAS partitions (sda1 → sda)
        if device[:-1].isalpha() and device[-1].isdigit():
            return device[:-1]

        # Loop devices, etc. - return as-is
        return device

# Global instance
io_manager = IOManager()
```

**Usage in Importer**:
```python
from .io_manager import io_manager

def import_model(...):
    semaphore = io_manager.get_semaphore(source_path)

    with semaphore:  # Blocks if drive is at capacity
        # Perform copy + hash operation
        ...
```

---

### 2. Stream Hashing (Hash-While-Copy)

**Problem**: Computing hashes after copying reads each 20GB file twice (once for copy, once for hash).

**Solution**: Compute BLAKE3/SHA256 hash during the copy stream in a single pass.

**Implementation**: Update `backend/model_library/importer.py`

```python
import hashlib
from blake3 import blake3

def copy_and_hash(
    source: Path,
    destination: Path,
    progress_callback=None
) -> tuple[str, str]:
    """
    Copy file while computing BLAKE3 and SHA256 hashes in a single pass.

    Args:
        source: Source file path
        destination: Destination file path (should have .tmp extension)
        progress_callback: Optional callback(bytes_copied, total_bytes)

    Returns:
        (blake3_hash, sha256_hash) as hex strings
    """
    blake3_hasher = blake3()
    sha256_hasher = hashlib.sha256()

    total_size = source.stat().st_size
    bytes_copied = 0

    with open(source, 'rb') as src, open(destination, 'wb') as dst:
        while True:
            chunk = src.read(8192)  # 8KB chunks
            if not chunk:
                break

            # Write to destination
            dst.write(chunk)

            # Update both hashers
            blake3_hasher.update(chunk)
            sha256_hasher.update(chunk)

            # Progress tracking
            bytes_copied += len(chunk)
            if progress_callback:
                progress_callback(bytes_copied, total_size)

    return (
        blake3_hasher.hexdigest(),
        sha256_hasher.hexdigest()
    )


def _compute_hashes_in_place(self, file_path: Path) -> tuple[str, str]:
    """
    Compute hashes of file already in destination (for move operations).

    Args:
        file_path: Path to file to hash

    Returns:
        (blake3_hash, sha256_hash) as hex strings
    """
    blake3_hasher = blake3()
    sha256_hasher = hashlib.sha256()

    with open(file_path, 'rb') as f:
        while chunk := f.read(8192):
            blake3_hasher.update(chunk)
            sha256_hasher.update(chunk)

    return blake3_hasher.hexdigest(), sha256_hasher.hexdigest()
```

**Performance Impact**: Reduces import time by ~40% for large files (avoids second disk read).

---

### 3. Incremental Sync Strategy

**Problem**: Full tree validation on every import causes "stop-the-world" pauses (5 versions × 300 models = 1500+ path checks).

**Solution**: Incremental syncing that only processes newly imported models.

**Implementation**:

```python
# In backend/api/core.py

def _auto_sync_all_apps_incremental(
    self,
    model_ids: Optional[List[str]] = None
) -> dict:
    """
    Incrementally sync only specified models to all apps.

    Args:
        model_ids: List of model IDs to sync. If None, performs full sync.

    Returns:
        {
            'versions_synced': int,
            'links_created': int,
            'links_updated': int,
            'errors': list
        }
    """
    if model_ids is None:
        # Full sync fallback
        return self._auto_sync_all_apps()

    versions = self.version_manager.list_installed_versions()
    total_links_created = 0
    total_links_updated = 0
    errors = []

    for version_tag in versions:
        models_root = Path(f"comfyui-versions/{version_tag}/models")
        if not models_root.exists():
            continue

        try:
            result = self.mapper.sync_models_incremental(
                app_id="comfyui",
                version=version_tag,
                models_root=models_root,
                model_ids=model_ids
            )

            total_links_created += result.get('links_created', 0)
            total_links_updated += result.get('links_updated', 0)

        except Exception as e:
            error_msg = f"Failed to sync {version_tag}: {e}"
            logger.error(error_msg, exc_info=True)
            errors.append(error_msg)

    return {
        'versions_synced': len(versions),
        'links_created': total_links_created,
        'links_updated': total_links_updated,
        'errors': errors
    }
```

**Performance Impact**:
- **Before**: 5 versions × 300 models × 22 subdirs = 33,000 path checks per import
- **After**: 5 versions × 1 model × ~3 matching subdirs = 15 path checks per import
- **Speedup**: ~2200× faster for single model import

---

### 4. HuggingFace API Throttling

**Problem**: Importing 50 models could trigger 200+ API calls, hitting rate limits.

**Solution**: Global API throttle with intelligent candidate prioritization.

**Implementation**:

```python
import time
from threading import Lock

class HFAPIThrottle:
    """
    Global rate limiter for HuggingFace API calls.

    Strategy:
    - Max 60 calls per minute (HF's documented rate limit)
    - Sliding window tracking
    - Automatic backoff on 429 responses
    """

    def __init__(self, max_calls_per_minute: int = 60):
        self.max_calls = max_calls_per_minute
        self.window_seconds = 60
        self.call_timestamps: list[float] = []
        self.lock = Lock()
        self.backoff_until: Optional[float] = None

    def acquire(self):
        """Wait until API call is allowed under rate limit."""
        with self.lock:
            now = time.time()

            # Check if in backoff period
            if self.backoff_until and now < self.backoff_until:
                wait_time = self.backoff_until - now
                logger.info(f"API rate limit: Waiting {wait_time:.1f}s")
                time.sleep(wait_time)
                now = time.time()

            # Remove timestamps outside window
            cutoff = now - self.window_seconds
            self.call_timestamps = [ts for ts in self.call_timestamps if ts > cutoff]

            # Wait if at capacity
            if len(self.call_timestamps) >= self.max_calls:
                oldest = self.call_timestamps[0]
                wait_time = (oldest + self.window_seconds) - now
                if wait_time > 0:
                    logger.debug(f"API throttle: Waiting {wait_time:.1f}s")
                    time.sleep(wait_time)
                    now = time.time()

            # Record this call
            self.call_timestamps.append(now)

    def set_backoff(self, retry_after: int):
        """Set backoff period from 429 response Retry-After header."""
        self.backoff_until = time.time() + retry_after
        logger.warning(f"HF API rate limit hit. Backing off for {retry_after}s")

# Global instance
hf_throttle = HFAPIThrottle(max_calls_per_minute=60)
```

**API Call Budget (Optimized)**:
- Filename search: 1-2 calls
- Hash verification: **2 calls max** (top 2 candidates only)
- **Total**: 3-4 calls per file
- **Batch of 50 models**: ~200 calls spread over ~3 minutes (throttled)

---

## Data Integrity Safeguards

### 1. Atomic Imports with `.tmp` Extension

**Problem**: If import crashes mid-copy, partially written files might be indexed.

**Solution**: Write files with `.tmp` extension, rename atomically only on success.

**Implementation**:

```python
def import_model(self, source_path: Path, ...) -> dict:
    """
    Import model with atomic file operations.

    Process:
    1. Copy file to destination with .tmp extension
    2. Compute hashes during copy (stream hashing)
    3. Write metadata.json
    4. Atomically rename .tmp to final extension
    5. Refresh SQLite index
    6. Incremental sync to installed apps
    """
    # Determine destination
    dest_dir = self._get_model_directory(family, cleaned_name)
    ensure_directory(dest_dir)

    # Generate temporary filename
    final_filename = source_path.name
    temp_filename = f"{final_filename}.tmp"

    temp_path = dest_dir / temp_filename
    final_path = dest_dir / final_filename

    try:
        # Step 1: Copy with stream hashing
        blake3_hash, sha256_hash = copy_and_hash(
            source_path,
            temp_path,
            progress_callback=lambda copied, total: self._report_progress(
                source_path.name, copied, total
            )
        )

        # Step 2: Write metadata
        metadata = self._build_metadata(
            source_path=source_path,
            blake3_hash=blake3_hash,
            sha256_hash=sha256_hash,
            hf_metadata=hf_metadata,
            ...
        )

        metadata_path = dest_dir / "metadata.json"
        with open(metadata_path, 'w', encoding='utf-8') as f:
            json.dump(metadata, f, indent=2, ensure_ascii=False)

        # Step 3: Atomic rename (only on success)
        temp_path.rename(final_path)

        logger.info(f"Successfully imported: {final_path}")

        return {
            'success': True,
            'model_path': str(final_path.relative_to(self.library_root)),
            'model_id': metadata['model_id'],
            'hashes': {'blake3': blake3_hash, 'sha256': sha256_hash}
        }

    except Exception as e:
        # Cleanup on failure
        if temp_path.exists():
            temp_path.unlink()

        logger.error(f"Import failed: {e}", exc_info=True)
        raise
```

**Key Benefit**: FileSystem watchers will never see incomplete files. `.tmp` files can be safely ignored by the indexer.

---

### 2. Atomic Directory Imports (Diffusers)

**Problem**: Diffusers models are folders. `.tmp` strategy needs adaptation.

**Solution**: Copy entire folder to `.tmp`, then atomic rename.

**CRITICAL: Cross-Filesystem Limitation**

`os.rename()` and `Path.rename()` are only atomic when source and destination are on the **same filesystem**. If they are on different filesystems, an `OSError` is raised.

**Implementation**:

```python
def import_model(self, source_path: Path, ...) -> dict:
    """Import with support for both files and directories."""

    # Determine if source is file or directory
    is_directory = source_path.is_dir()

    # ... destination path calculation ...

    # CRITICAL: Check if cross-filesystem operation
    same_fs = source_path.stat().st_dev == dest_path.parent.stat().st_dev

    if is_directory:
        # Directory import (Diffusers format)
        temp_dest = dest_path.with_name(dest_path.name + '.tmp')

        if not same_fs:
            # Cross-filesystem directory move is NOT atomic
            # Warn user that interruption could leave partial folder
            logger.warning(
                f"Cross-filesystem directory import detected. "
                f"This operation is NOT atomic - interruption may leave partial data."
            )
            # Use shutil.move instead of rename (it handles cross-fs)
            # But note: this is a copy+delete, not atomic

        try:
            # Copy directory with progress tracking
            shutil.copytree(
                source_path,
                temp_dest,
                symlinks=False,
                copy_function=shutil.copy2,
                dirs_exist_ok=False
            )

            # Compute hashes for all model files in directory
            hashes = self._compute_directory_hashes(temp_dest)

            # Write metadata
            metadata_path = temp_dest / "metadata.json"
            with open(metadata_path, 'w') as f:
                json.dump(metadata, f, indent=2)

            # Atomic rename
            temp_dest.rename(dest_path)

            logger.info(f"Imported directory: {dest_path}")

        except Exception as e:
            # Cleanup on failure
            if temp_dest.exists():
                shutil.rmtree(temp_dest)
            raise
    else:
        # File import (existing logic)
        ...

    return {'success': True, 'model_path': str(dest_path), ...}


def _compute_directory_hashes(self, dir_path: Path) -> dict:
    """
    Compute hashes for all model files in directory.

    Returns:
        {
            'model.safetensors': {'blake3': '...', 'sha256': '...'},
            'config.json': {'blake3': '...', 'sha256': '...'},
        }
    """
    hashes = {}

    for file_path in dir_path.rglob('*'):
        if not file_path.is_file():
            continue

        # Skip metadata files
        if file_path.name in ('metadata.json', 'overrides.json', 'preview.png'):
            continue

        # Compute hashes
        blake3_hash, sha256_hash = self._compute_hashes_in_place(file_path)

        hashes[file_path.name] = {
            'blake3': blake3_hash,
            'sha256': sha256_hash,
            'size': file_path.stat().st_size
        }

    return hashes
```

---

### 3. SQLite Write-Ahead Logging (WAL)

**Problem**: Background hash operations need to write to SQLite, but UI reads would block.

**Solution**: Enable WAL mode for concurrent read/write access with busy timeout.

### Concurrent Write Handling

**Problem**: In PyWebView, multiple background threads (hashing, downloading, sync) may attempt concurrent writes to `models.db` or `registry.db`, potentially causing "Database is locked" errors even with WAL mode.

**Solution**: Use SQLite's built-in `busy_timeout` pragma. This is simpler than implementing a dedicated writer queue and sufficient for our use case. If issues arise during testing, we can upgrade to a writer queue pattern.

**Guidance**:
- Start with `busy_timeout=30.0` (30 seconds) on all connections
- Follow coding standards with explicit error handling
- Unit test concurrent access scenarios
- If "Database is locked" errors persist in testing, consider upgrading to `sqlitedict` or a dedicated writer queue

**Implementation**:

```python
def _initialize_database(self):
    """Initialize SQLite database with optimizations."""
    self.db_path.parent.mkdir(parents=True, exist_ok=True)

    conn = sqlite3.connect(str(self.db_path), timeout=30.0)  # 30-second busy timeout
    cursor = conn.cursor()

    # Enable WAL mode for concurrent access
    cursor.execute("PRAGMA journal_mode=WAL;")

    # Busy timeout as PRAGMA (redundant with connect timeout, but explicit)
    cursor.execute("PRAGMA busy_timeout=30000;")  # 30 seconds in milliseconds

    # Other optimizations
    cursor.execute("PRAGMA synchronous=NORMAL;")  # Faster than FULL, still safe
    cursor.execute("PRAGMA temp_store=MEMORY;")

    # Memory-mapped I/O (optimized for metadata DB size ~100-500MB)
    cursor.execute("PRAGMA mmap_size=268435456;")  # Cap at 256MB
    cursor.execute("PRAGMA cache_size=-2000;")      # Use 2MB of RAM for cache

    # Create tables
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS models (
            model_id TEXT PRIMARY KEY,
            family TEXT,
            model_type TEXT,
            official_name TEXT,
            cleaned_name TEXT,
            library_path TEXT,
            size_bytes INTEGER,
            blake3_hash TEXT,
            sha256_hash TEXT,
            added_date TEXT,
            last_updated TEXT,
            metadata_json TEXT
        )
    """)

    cursor.execute("CREATE INDEX IF NOT EXISTS idx_model_type ON models(model_type);")
    cursor.execute("CREATE INDEX IF NOT EXISTS idx_family ON models(family);")

    conn.commit()
    conn.close()

    logger.info(f"Database initialized with WAL mode: {self.db_path}")
```

**Benefits**:
- UI can read model list while background import updates database
- Multiple import operations can proceed concurrently
- Crash recovery is automatic (WAL journal replays on next open)

---

### 4. WAL Checkpointing

**Problem**: Long-running imports cause WAL file to grow unbounded, risking data loss on crash.

**Solution**: Aggressive checkpointing after batch operations and every 5 models during large imports.

**Implementation**:

```python
def checkpoint_wal(self):
    """
    Checkpoint WAL file to main database.

    Should be called after:
    - Large batch imports (always)
    - Every 5th model in a large queue (reduces recovery time)
    - Deep scan rebuilds
    - Periodic maintenance (daily at 3:00 AM)
    """
    conn = sqlite3.connect(str(self.db_path))
    cursor = conn.cursor()

    try:
        # TRUNCATE mode: Checkpoint and truncate WAL file
        cursor.execute("PRAGMA wal_checkpoint(TRUNCATE);")
        result = cursor.fetchone()

        # Result: (busy, log_pages, checkpointed_pages)
        busy, log_pages, checkpointed_pages = result

        if busy == 0:
            logger.info(
                f"WAL checkpoint complete: "
                f"{checkpointed_pages} pages written, "
                f"{log_pages} pages in log"
            )
        else:
            logger.warning(
                f"WAL checkpoint incomplete (busy): "
                f"{checkpointed_pages}/{log_pages} pages written"
            )

    except Exception as e:
        logger.error(f"WAL checkpoint failed: {e}", exc_info=True)
    finally:
        conn.close()


def import_model_batch(self, files: List[dict], ...) -> List[dict]:
    """
    Import multiple models with periodic WAL checkpointing.

    Checkpoints occur:
    - Every 5 models during import
    - Once at the end of the batch
    """
    results = []

    for i, file_data in enumerate(files):
        result = self._import_single_model(file_data)
        results.append(result)

        # Checkpoint every 5 models to limit WAL growth
        if (i + 1) % 5 == 0:
            logger.debug(f"Checkpointing WAL after {i + 1} models")
            self.library.checkpoint_wal()

    # Final checkpoint after batch
    self.library.checkpoint_wal()

    return results
```

---

### 5. Deep Scan Rebuild with File Verification

**Problem**: If SQLite database is corrupted or deleted, there's no way to recover the index. Additionally, metadata.json may reference files that were manually moved/deleted.

**Solution**: Implement "Deep Scan" that reconstructs database from `metadata.json` files while verifying physical file existence.

**Implementation**:

```python
def rebuild_index_from_metadata(self, progress_callback=None) -> dict:
    """
    Reconstruct SQLite database from metadata.json files (Deep Scan).

    This treats metadata.json as the Single Source of Truth.

    NEW: Verifies that actual model weight files exist on disk.
    Prevents "phantom models" where metadata.json exists but user manually
    deleted the .safetensors files.

    Returns:
        {
            'success': bool,
            'models_found': int,
            'models_indexed': int,
            'models_skipped_missing_files': int,
            'errors': list[str]
        }
    """
    logger.info("Starting deep scan to rebuild index with file verification...")

    # Clear existing database
    conn = sqlite3.connect(str(self.db_path))
    cursor = conn.cursor()
    cursor.execute("DELETE FROM models;")
    conn.commit()

    models_found = 0
    models_indexed = 0
    models_skipped = 0
    errors = []

    # Walk the library directory
    for metadata_path in self.library_root.rglob("metadata.json"):
        models_found += 1

        try:
            with open(metadata_path, 'r', encoding='utf-8') as f:
                metadata = json.load(f)

            # VERIFY: Check that actual model files exist
            model_dir = metadata_path.parent
            model_files_list = metadata.get('files', [])

            if not model_files_list:
                # Legacy: Infer from directory contents
                model_files = list(model_dir.glob('*.safetensors')) + \
                              list(model_dir.glob('*.ckpt')) + \
                              list(model_dir.glob('*.gguf'))
            else:
                # Modern: Check files listed in metadata
                model_files = []
                for file_entry in model_files_list:
                    file_path = model_dir / file_entry['name']
                    if file_path.exists():
                        model_files.append(file_path)

            # Skip if no model weights found (manual deletion)
            if not model_files:
                models_skipped += 1
                logger.warning(
                    f"Skipping {metadata_path}: No model weight files found "
                    f"(manually deleted?)"
                )
                continue

            # Insert into database
            cursor.execute("""
                INSERT OR REPLACE INTO models (
                    model_id, family, model_type, official_name,
                    cleaned_name, library_path, size_bytes,
                    blake3_hash, sha256_hash, added_date,
                    last_updated, metadata_json
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            """, (
                metadata.get('model_id'),
                metadata.get('family'),
                metadata.get('model_type'),
                metadata.get('official_name'),
                metadata.get('cleaned_name'),
                str(metadata_path.parent.relative_to(self.library_root)),
                metadata.get('size_bytes'),
                metadata.get('hashes', {}).get('blake3'),
                metadata.get('hashes', {}).get('sha256'),
                metadata.get('added_date'),
                metadata.get('updated_date'),
                json.dumps(metadata)
            ))

            models_indexed += 1

            if progress_callback:
                progress_callback(models_indexed, models_found)

        except Exception as e:
            error_msg = f"Failed to index {metadata_path}: {e}"
            logger.warning(error_msg)
            errors.append(error_msg)

    conn.commit()
    conn.close()

    logger.info(
        f"Deep scan complete: {models_indexed}/{models_found} models indexed, "
        f"{models_skipped} skipped (missing files), {len(errors)} errors"
    )

    return {
        'success': True,
        'models_found': models_found,
        'models_indexed': models_indexed,
        'models_skipped_missing_files': models_skipped,
        'errors': errors
    }
```

**UI Integration**: Add "Rebuild Index" button in Settings with warning dialog.

---

### 6. Link Registry Database (registry.db)

**Purpose**: Track every symlink/hardlink created by the mapping system for clean deletion, health validation, and path relocation.

**Location**: `launcher-data/db/registry.db`

**Implementation**: New module `backend/model_library/link_registry.py`

```python
import sqlite3
from pathlib import Path
from typing import List, Dict, Optional
from datetime import datetime

class LinkRegistry:
    """
    Persistent registry for all model links created by the mapping system.

    Enables:
    - Cascade deletion (delete model → find all links → unlink → purge registry → delete files)
    - Health checks (detect broken links, orphaned links, missing sources)
    - Hybrid path storage (relative for internal, absolute for external drives)
    - Drive relocation helper (bulk-update absolute paths when mount points change)
    """

    def __init__(self, db_path: Path, app_root: Path):
        self.db_path = db_path
        self.app_root = app_root  # For relative path calculations
        self._initialize_database()

    def _initialize_database(self):
        """Initialize link registry database."""
        self.db_path.parent.mkdir(parents=True, exist_ok=True)

        conn = sqlite3.connect(str(self.db_path))
        cursor = conn.cursor()

        # Enable WAL mode for concurrent access
        cursor.execute("PRAGMA journal_mode=WAL;")
        cursor.execute("PRAGMA synchronous=NORMAL;")

        # Create links table
        cursor.execute("""
            CREATE TABLE IF NOT EXISTS links (
                link_id INTEGER PRIMARY KEY AUTOINCREMENT,
                model_id TEXT NOT NULL,
                target_app_path TEXT NOT NULL,
                source_model_path TEXT NOT NULL,
                is_external BOOLEAN DEFAULT 0,
                link_type TEXT CHECK(link_type IN ('symlink', 'hardlink')) NOT NULL,
                created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
                UNIQUE(target_app_path)
            )
        """)

        cursor.execute("CREATE INDEX IF NOT EXISTS idx_model_id ON links(model_id);")
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_target_path ON links(target_app_path);")
        cursor.execute("CREATE INDEX IF NOT EXISTS idx_is_external ON links(is_external);")

        conn.commit()
        conn.close()

        logger.info(f"Link registry initialized: {self.db_path}")

    def register_link(
        self,
        model_id: str,
        target_path: Path,
        source_path: Path,
        is_external: bool,
        link_type: str = "symlink"
    ):
        """
        Register a newly created link in the database.

        Args:
            model_id: Model identifier from models.db
            target_path: Absolute path to link location (app directory)
            source_path: Absolute path to source file (library)
            is_external: True if source is on external drive (absolute path)
            link_type: "symlink" or "hardlink"
        """
        conn = sqlite3.connect(str(self.db_path))
        cursor = conn.cursor()

        # Convert to relative paths if internal
        if not is_external:
            target_rel = str(target_path.relative_to(self.app_root))
            source_rel = str(source_path.relative_to(self.app_root))
        else:
            target_rel = str(target_path)
            source_rel = str(source_path)

        try:
            cursor.execute("""
                INSERT INTO links (
                    model_id, target_app_path, source_model_path,
                    is_external, link_type
                ) VALUES (?, ?, ?, ?, ?)
            """, (model_id, target_rel, source_rel, is_external, link_type))

            conn.commit()
            logger.debug(f"Registered link: {target_rel} -> {source_rel}")
        except sqlite3.IntegrityError:
            # Link already exists (idempotent)
            logger.debug(f"Link already registered: {target_rel}")
        finally:
            conn.close()

    def get_links_for_model(self, model_id: str) -> List[Dict]:
        """
        Get all links for a given model.

        Returns:
            List of dicts with keys: link_id, target_app_path, source_model_path, is_external, link_type
        """
        conn = sqlite3.connect(str(self.db_path))
        cursor = conn.cursor()

        cursor.execute("""
            SELECT link_id, target_app_path, source_model_path, is_external, link_type
            FROM links
            WHERE model_id = ?
        """, (model_id,))

        rows = cursor.fetchall()
        conn.close()

        links = []
        for row in rows:
            link_id, target_rel, source_rel, is_external, link_type = row

            # Convert back to absolute paths if needed
            if not is_external:
                target_abs = self.app_root / target_rel
                source_abs = self.app_root / source_rel
            else:
                target_abs = Path(target_rel)
                source_abs = Path(source_rel)

            links.append({
                'link_id': link_id,
                'target_app_path': target_abs,
                'source_model_path': source_abs,
                'is_external': bool(is_external),
                'link_type': link_type
            })

        return links

    def delete_links_for_model(self, model_id: str) -> int:
        """
        Delete all registry entries for a model.

        Returns:
            Number of entries deleted
        """
        conn = sqlite3.connect(str(self.db_path))
        cursor = conn.cursor()

        cursor.execute("DELETE FROM links WHERE model_id = ?", (model_id,))
        deleted_count = cursor.rowcount

        conn.commit()
        conn.close()

        logger.info(f"Purged {deleted_count} registry entries for {model_id}")
        return deleted_count

    def find_broken_links(self) -> List[Dict]:
        """
        Find links where source model file no longer exists.

        Returns:
            List of broken link dicts
        """
        conn = sqlite3.connect(str(self.db_path))
        cursor = conn.cursor()

        cursor.execute("""
            SELECT link_id, model_id, target_app_path, source_model_path, is_external
            FROM links
        """)

        rows = cursor.fetchall()
        conn.close()

        broken_links = []

        for row in rows:
            link_id, model_id, target_rel, source_rel, is_external = row

            # Convert to absolute path
            if not is_external:
                source_abs = self.app_root / source_rel
            else:
                source_abs = Path(source_rel)

            # Check if source exists
            if not source_abs.exists():
                broken_links.append({
                    'link_id': link_id,
                    'model_id': model_id,
                    'target_app_path': target_rel,
                    'source_model_path': source_abs,
                    'reason': 'source_missing'
                })

        return broken_links

    def find_orphaned_links(self, app_models_root: Path) -> List[Path]:
        """
        Find symlinks on disk that are NOT in the registry (orphaned).

        These may be created manually or left behind from old mapping rules.

        Args:
            app_models_root: Path to app's models/ directory

        Returns:
            List of orphaned link paths
        """
        # Get all registered link paths
        conn = sqlite3.connect(str(self.db_path))
        cursor = conn.cursor()
        cursor.execute("SELECT target_app_path FROM links")
        registered_paths = {
            self.app_root / row[0] if not row[0].startswith('/') else Path(row[0])
            for row in cursor.fetchall()
        }
        conn.close()

        # Scan disk for all symlinks
        orphaned = []
        for subdir in app_models_root.iterdir():
            if not subdir.is_dir():
                continue
            for item in subdir.iterdir():
                if item.is_symlink():
                    if item not in registered_paths:
                        orphaned.append(item)

        return orphaned

    def find_ghost_apps(self) -> List[Dict]:
        """
        Find apps referenced in registry that no longer exist on disk.

        This happens when a user deletes a ComfyUI installation via file manager
        (bypassing our app), leaving stale registry entries.

        Returns:
            List of dicts: [{'app_path': str, 'link_count': int}]
        """
        conn = sqlite3.connect(str(self.db_path))
        cursor = conn.cursor()

        # Get unique app paths from registry
        cursor.execute("""
            SELECT DISTINCT
                CASE
                    WHEN target_app_path LIKE '/%' THEN target_app_path
                    ELSE target_app_path
                END as app_path
            FROM links
        """)

        ghost_apps = []
        seen_roots = set()

        for row in cursor.fetchall():
            target_path = row[0]

            # Extract app root (e.g., comfyui-versions/v1.0.0)
            # by finding the models/ parent
            path = Path(target_path) if target_path.startswith('/') else self.app_root / target_path
            app_root = None

            for parent in path.parents:
                if parent.name == 'models' and parent.parent.exists():
                    app_root = parent.parent
                    break

            if app_root and app_root not in seen_roots:
                seen_roots.add(app_root)

                # Check if app still exists
                if not app_root.exists():
                    # Count links to this ghost app
                    cursor.execute("""
                        SELECT COUNT(*) FROM links
                        WHERE target_app_path LIKE ?
                    """, (f"%{app_root.name}%",))
                    link_count = cursor.fetchone()[0]

                    ghost_apps.append({
                        'app_path': str(app_root),
                        'link_count': link_count
                    })

        conn.close()

        if ghost_apps:
            logger.warning(f"Found {len(ghost_apps)} ghost app(s) in registry")

        return ghost_apps

    def cleanup_ghost_app(self, app_path: str) -> int:
        """
        Remove all registry entries for a ghost (deleted) app.

        Args:
            app_path: Path to the deleted app installation

        Returns:
            Number of entries removed
        """
        conn = sqlite3.connect(str(self.db_path))
        cursor = conn.cursor()

        app_name = Path(app_path).name

        cursor.execute("""
            DELETE FROM links
            WHERE target_app_path LIKE ?
        """, (f"%{app_name}%",))

        deleted_count = cursor.rowcount
        conn.commit()
        conn.close()

        logger.info(f"Cleaned up {deleted_count} registry entries for ghost app: {app_path}")
        return deleted_count

    def bulk_update_external_paths(
        self,
        old_mount: str,
        new_mount: str
    ) -> int:
        """
        Bulk-update external absolute paths when drive mount point changes.

        Example: /media/drive_a -> /media/drive_b

        Args:
            old_mount: Old mount point prefix (e.g., "/media/usb")
            new_mount: New mount point prefix (e.g., "/media/usb2")

        Returns:
            Number of paths updated
        """
        conn = sqlite3.connect(str(self.db_path))
        cursor = conn.cursor()

        # Normalize mount points with trailing slash to prevent partial matches
        # e.g., /media/usb should not match /media/usb_backup
        old_mount_normalized = old_mount.rstrip('/') + '/'
        new_mount_normalized = new_mount.rstrip('/') + '/'

        # Find all external links with old mount
        # Using trailing slash ensures exact mount point match
        cursor.execute("""
            SELECT link_id, source_model_path
            FROM links
            WHERE is_external = 1 AND (
                source_model_path LIKE ? OR
                source_model_path = ?
            )
        """, (f"{old_mount_normalized}%", old_mount.rstrip('/')))

        rows = cursor.fetchall()
        updated = 0

        for link_id, old_path in rows:
            # Replace old mount with new mount
            if old_path.startswith(old_mount_normalized):
                # Path has trailing content after mount point
                new_path = new_mount_normalized + old_path[len(old_mount_normalized):]
            elif old_path == old_mount.rstrip('/'):
                # Path is exactly the mount point (edge case)
                new_path = new_mount.rstrip('/')
            else:
                # Should not happen due to WHERE clause, but handle gracefully
                continue

            cursor.execute("""
                UPDATE links
                SET source_model_path = ?
                WHERE link_id = ?
            """, (new_path, link_id))
            updated += 1

        conn.commit()
        conn.close()

        logger.info(f"Relocated {updated} external paths from {old_mount} to {new_mount}")
        return updated

# Global instance
link_registry = None  # Initialized on app startup
```

### Usage in Mapper

**Create Link**:
```python
def _create_link_with_registry(
    self,
    source: Path,
    target: Path,
    link_type: str,
    model_id: str,
    is_external: bool
) -> bool:
    """Create link and register in database."""
    # Create the link
    if link_type == 'symlink':
        success = make_relative_symlink(source, target)
    elif link_type == 'hardlink':
        target.hardlink_to(source)
        success = True
    else:
        raise ValueError(f"Unknown link type: {link_type}")

    if success:
        # Register in link registry
        link_registry.register_link(
            model_id=model_id,
            target_path=target,
            source_path=source,
            is_external=is_external,
            link_type=link_type
        )

    return success
```

**Cascade Delete**:
```python
def delete_model_with_cascade(self, model_id: str):
    """
    Delete model with cascade cleanup of all links.

    Steps:
    1. Query registry for all links
    2. Unlink all symlinks/hardlinks
    3. Purge registry entries
    4. Delete physical model files
    """
    # Step 1: Get all links from registry
    links = link_registry.get_links_for_model(model_id)

    logger.info(f"Found {len(links)} links for model {model_id}")

    # Step 2: Unlink all
    for link in links:
        try:
            Path(link['target_app_path']).unlink(missing_ok=True)
            logger.debug(f"Unlinked: {link['target_app_path']}")
        except Exception as e:
            logger.warning(f"Failed to unlink {link['target_app_path']}: {e}")

    # Step 3: Purge registry
    link_registry.delete_links_for_model(model_id)

    # Step 4: Delete physical files
    metadata = self.library.get_model_by_id(model_id)
    if metadata:
        model_dir = self.library_root / metadata['library_path']
        if model_dir.exists():
            shutil.rmtree(model_dir)
            logger.info(f"Deleted model directory: {model_dir}")
```

---

### 7. Ghost State Health Checks

**Purpose**: Detect and report broken links, orphaned symlinks, and missing sources on startup.

**Implementation**: Add to `backend/model_library/library.py`

```python
def perform_health_check(self) -> Dict:
    """
    Perform startup health check on model library and links.

    Detects:
    - Broken links: Link exists in registry but source file deleted
    - Orphaned links: Link exists on disk but not in registry
    - Missing source: Link exists on disk but source was manually moved
    - Ghost apps: Registry references apps that no longer exist on disk

    Returns:
        {
            'broken_links': List[Dict],
            'orphaned_links': List[Path],
            'ghost_apps': List[str],
            'total_issues': int,
            'status': 'healthy' | 'warnings' | 'errors'
        }
    """
    logger.info("Starting library health check...")

    broken_links = link_registry.find_broken_links()
    orphaned_links = []
    ghost_apps = []

    # Check all installed apps for orphaned links AND detect ghost apps
    for version_dir in Path("comfyui-versions").iterdir():
        if version_dir.is_dir():
            models_root = version_dir / "models"
            if models_root.exists():
                orphaned = link_registry.find_orphaned_links(models_root)
                orphaned_links.extend(orphaned)

    # Detect ghost apps: Apps referenced in registry but deleted from disk
    ghost_apps = link_registry.find_ghost_apps()

    total_issues = len(broken_links) + len(orphaned_links) + len(ghost_apps)

    if total_issues == 0:
        status = 'healthy'
    elif total_issues < 5:
        status = 'warnings'
    else:
        status = 'errors'

    logger.info(
        f"Health check complete: {len(broken_links)} broken, "
        f"{len(orphaned_links)} orphaned ({status})"
    )

    return {
        'broken_links': broken_links,
        'orphaned_links': [str(p) for p in orphaned_links],
        'total_issues': total_issues,
        'status': status
    }
```

**UI Integration**: Display health status in Settings with badges

```tsx
{healthStatus.status === 'healthy' ? (
  <div className="flex items-center gap-2 text-green-600">
    <CheckCircle className="w-5 h-5" />
    <span>Library Healthy</span>
  </div>
) : healthStatus.status === 'warnings' ? (
  <div className="flex items-center gap-2 text-yellow-600">
    <AlertTriangle className="w-5 h-5" />
    <span>{healthStatus.total_issues} issues detected</span>
    <button onClick={() => showHealthDetails()}>View Details</button>
  </div>
) : (
  <div className="flex items-center gap-2 text-red-600">
    <XCircle className="w-5 h-5" />
    <span>{healthStatus.total_issues} critical issues</span>
    <button onClick={() => showHealthDetails()}>View Details</button>
  </div>
)}
```

**Health Check Details Dialog**:

```tsx
<Dialog>
  <DialogTitle>Library Health Issues</DialogTitle>
  <DialogContent>
    {healthStatus.broken_links.length > 0 && (
      <Section>
        <h4>Broken Links ({healthStatus.broken_links.length})</h4>
        <p className="text-sm text-gray-600">
          These links point to models that no longer exist (manually deleted).
        </p>
        <ul className="mt-2 space-y-1">
          {healthStatus.broken_links.map(link => (
            <li key={link.link_id} className="text-sm">
              <span className="font-mono">{link.target_app_path}</span>
              <Badge variant="red">Source Missing</Badge>
            </li>
          ))}
        </ul>
        <button onClick={() => cleanBrokenLinks()}>
          Clean All Broken Links
        </button>
      </Section>
    )}

    {healthStatus.orphaned_links.length > 0 && (
      <Section>
        <h4>Orphaned Links ({healthStatus.orphaned_links.length})</h4>
        <p className="text-sm text-gray-600">
          These links exist on disk but are not tracked in the registry.
        </p>
        <ul className="mt-2 space-y-1">
          {healthStatus.orphaned_links.map(path => (
            <li key={path} className="text-sm font-mono">{path}</li>
          ))}
        </ul>
        <button onClick={() => removeOrphanedLinks()}>
          Remove All Orphaned Links
        </button>
      </Section>
    )}
  </DialogContent>
</Dialog>
```

---

### 8. Drive/Mount-Point Relocation Helper

**Purpose**: When an external drive's mount point changes, provide a tool to bulk-update all absolute paths.

**Scenario**: User has library on `/media/drive_a`, which changes to `/media/drive_b` after unplugging and replugging.

**Implementation**: Add to `backend/api/core.py`

```python
def relocate_external_drive(
    self,
    old_mount: str,
    new_mount: str
) -> Dict:
    """
    Bulk-update all external absolute paths when drive mount changes.

    Args:
        old_mount: Old mount point (e.g., "/media/usb")
        new_mount: New mount point (e.g., "/media/usb2")

    Returns:
        {
            'success': bool,
            'paths_updated': int,
            'links_recreated': int
        }
    """
    try:
        # Update registry paths
        paths_updated = link_registry.bulk_update_external_paths(old_mount, new_mount)

        # Recreate broken symlinks with new paths
        links_recreated = 0
        broken_links = link_registry.find_broken_links()

        for link in broken_links:
            target = Path(link['target_app_path'])
            new_source = Path(str(link['source_model_path']).replace(old_mount, new_mount, 1))

            if new_source.exists():
                # Recreate link
                if target.is_symlink():
                    target.unlink()
                target.symlink_to(new_source)
                links_recreated += 1

        return {
            'success': True,
            'paths_updated': paths_updated,
            'links_recreated': links_recreated
        }
    except Exception as e:
        logger.error(f"Drive relocation failed: {e}")
        return {'success': False, 'error': str(e)}
```

**UI Integration**: Add to Settings

```tsx
<Section title="External Drive Management">
  <p className="text-sm text-gray-600">
    If your external drive's mount point changed (e.g., from /media/usb to /media/usb2),
    use this tool to update all links.
  </p>

  <div className="flex gap-3 mt-3">
    <input
      type="text"
      placeholder="Old mount (e.g., /media/usb)"
      value={oldMount}
      onChange={e => setOldMount(e.target.value)}
      className="flex-1 px-3 py-2 border rounded"
    />
    <input
      type="text"
      placeholder="New mount (e.g., /media/usb2)"
      value={newMount}
      onChange={e => setNewMount(e.target.value)}
      className="flex-1 px-3 py-2 border rounded"
    />
    <button onClick={handleRelocate} className="px-4 py-2 bg-blue-600 text-white rounded">
      Relocate
    </button>
  </div>
</Section>
```

---

## Network & API Resilience

### 1. Offline-First Import Strategy

**Problem**: HuggingFace API lookups can timeout, rate-limit, or fail entirely offline.

**Solution**: Never block import on network operations. Use aggressive timeouts and queue for later.

**Implementation**:

```python
def lookup_model_metadata_by_filename(
    self,
    filename: str,
    file_path: Optional[Path] = None,
    timeout: float = 5.0  # Strict 5-second timeout
) -> Optional[dict]:
    """
    Look up HuggingFace metadata with strict timeout.

    If lookup fails (offline, timeout, rate-limit), returns None
    and the import proceeds with filename-only metadata.

    The model will be marked for "Pending Online Lookup" which
    can be retried later when connectivity returns.
    """
    try:
        # Set socket timeout globally for this call
        import socket
        original_timeout = socket.getdefaulttimeout()
        socket.setdefaulttimeout(timeout)

        try:
            # Existing lookup logic with hash verification
            result = self._perform_lookup(filename, file_path)
            return result
        finally:
            socket.setdefaulttimeout(original_timeout)

    except (TimeoutError, ConnectionError, requests.exceptions.Timeout) as e:
        logger.warning(
            f"HuggingFace lookup timed out for {filename}: {e}. "
            f"Proceeding with offline import."
        )
        return None

    except requests.exceptions.HTTPError as e:
        if e.response.status_code == 429:  # Rate limit
            logger.warning(f"HF rate limit hit. Proceeding with offline import.")
        elif e.response.status_code >= 500:  # Server error
            logger.warning(f"HF server error: {e}. Proceeding with offline import.")
        else:
            logger.error(f"HF HTTP error: {e}")

        return None

    except Exception as e:
        logger.error(f"Unexpected error during HF lookup: {e}", exc_info=True)
        return None
```

**Retry Service**:

```python
def retry_pending_lookups(self) -> dict:
    """
    Retry HF metadata lookup for models marked 'pending_online_lookup'.

    Runs in background thread, can be triggered manually or on app startup.
    """
    models = self.list_models(filter={'pending_online_lookup': True})

    success_count = 0
    failed_count = 0

    for model_metadata in models:
        library_path = self.library_root / model_metadata['library_path']
        metadata_path = library_path / "metadata.json"

        # Find the actual model file
        model_files = [
            f for f in library_path.iterdir()
            if f.suffix in ('.safetensors', '.ckpt', '.gguf', '.pt', '.bin')
        ]

        if not model_files:
            continue

        model_file = model_files[0]

        # Attempt lookup
        hf_metadata = self.downloader.lookup_model_metadata_by_filename(
            model_file.name,
            model_file,
            timeout=10.0  # Longer timeout for retry
        )

        if hf_metadata:
            # Update metadata.json with HF data
            with open(metadata_path, 'r') as f:
                current_metadata = json.load(f)

            # Merge HF metadata
            current_metadata.update({
                'official_name': hf_metadata.get('official_name', current_metadata['official_name']),
                'tags': hf_metadata.get('tags', []),
                'download_url': hf_metadata.get('download_url'),
                'pending_online_lookup': False,
                'last_lookup_attempt': get_iso_timestamp()
            })

            with open(metadata_path, 'w') as f:
                json.dump(current_metadata, f, indent=2, ensure_ascii=False)

            success_count += 1
        else:
            # Increment attempts
            with open(metadata_path, 'r') as f:
                current_metadata = json.load(f)

            current_metadata['lookup_attempts'] = current_metadata.get('lookup_attempts', 0) + 1

            with open(metadata_path, 'w') as f:
                json.dump(current_metadata, f, indent=2, ensure_ascii=False)

            failed_count += 1

    return {
        'success': True,
        'retried': len(models),
        'success_count': success_count,
        'failed_count': failed_count
    }
```

---

### 2. Aggressive Metadata Caching

**Problem**: Importing multiple files from the same HF repo causes redundant API calls.

**Solution**: LRU cache with 24-hour TTL for repo file lists.

**Implementation**:

```python
from datetime import datetime, timedelta

# Cache repo file lists for 24 hours
_repo_cache: dict[str, tuple[list, datetime]] = {}
_repo_cache_ttl = timedelta(hours=24)

def _get_lfs_files_cached(self, repo_id: str) -> list:
    """
    Get LFS files for a repo with 24-hour caching.

    This dramatically reduces API calls when importing multiple
    files from the same repository.
    """
    now = datetime.now()

    # Check cache
    if repo_id in _repo_cache:
        cached_files, cached_time = _repo_cache[repo_id]
        if now - cached_time < _repo_cache_ttl:
            logger.debug(f"Cache hit for {repo_id} (age: {now - cached_time})")
            return cached_files

    # Cache miss or expired - fetch from API
    try:
        api = self._get_api()
        lfs_files = list(api.list_lfs_files(repo_id))

        # Store in cache
        _repo_cache[repo_id] = (lfs_files, now)
        logger.debug(f"Cached LFS files for {repo_id} ({len(lfs_files)} files)")

        return lfs_files

    except Exception as e:
        logger.warning(f"Failed to fetch LFS files for {repo_id}: {e}")

        # Return stale cache if available
        if repo_id in _repo_cache:
            logger.info(f"Returning stale cache for {repo_id}")
            return _repo_cache[repo_id][0]

        raise
```

---

## Filesystem Validation

### Filesystem Pre-Flight Validation

**Purpose**: Validate filesystem health and capabilities before import/mapping operations to prevent silent failures.

**Implementation**: New module `backend/model_library/fs_validator.py`

```python
import os
import subprocess
from pathlib import Path
from typing import Literal, Optional

class FilesystemValidator:
    """Validates filesystem health and capabilities."""

    def validate_import_source(self, source_path: Path) -> dict:
        """
        Validate source filesystem before import.

        Returns:
            {
                'valid': bool,
                'filesystem_type': str,
                'warnings': list[str],
                'errors': list[str],
                'readonly': bool
            }
        """
        result = {
            'valid': True,
            'filesystem_type': 'unknown',
            'warnings': [],
            'errors': [],
            'readonly': False
        }

        try:
            # Detect filesystem type
            fs_type = self._detect_filesystem_type(source_path)
            result['filesystem_type'] = fs_type

            # Check if mounted read-only
            if self._is_readonly_mount(source_path):
                result['readonly'] = True
                result['errors'].append(
                    f"Source path {source_path} is mounted read-only."
                )
                result['valid'] = False
                return result

            # NTFS-specific checks
            if fs_type == 'ntfs':
                dirty_check = self._check_ntfs_dirty_bit(source_path)

                if dirty_check['is_dirty']:
                    result['errors'].append(
                        f"NTFS filesystem has 'dirty bit' set (improper Windows shutdown). "
                        f"Boot into Windows to fix or run: sudo ntfsfix {dirty_check['device']}"
                    )
                    result['valid'] = False

                if dirty_check['lowntfs_detected']:
                    result['warnings'].append(
                        f"NTFS mounted with lowntfs-3g driver. "
                        f"Symlink support may be unreliable."
                    )

                # NTFS Canary Link Test - verify link support before proceeding
                canary_result = self._test_canary_links(source_path)
                if not canary_result['symlink_works'] and not canary_result['hardlink_works']:
                    result['errors'].append(
                        "NTFS link test failed: Neither symlinks nor hardlinks work on this mount. "
                        "Check mount options (try: mount -o remount,permissions)."
                    )
                    result['valid'] = False
                elif not canary_result['symlink_works']:
                    result['warnings'].append(
                        "NTFS symlinks failed canary test. Using hardlinks only."
                    )
                    result['force_hardlinks'] = True

        except Exception as e:
            logger.error(f"Filesystem validation failed: {e}", exc_info=True)
            result['errors'].append(f"Validation error: {e}")
            result['valid'] = False

        return result

    def validate_mapping_target(
        self,
        library_path: Path,
        app_path: Path,
        link_type: Literal['symlink', 'hardlink'] = 'symlink'
    ) -> dict:
        """
        Validate mapping target filesystem and link capability.

        Returns:
            {
                'valid': bool,
                'same_filesystem': bool,
                'link_type_supported': bool,
                'recommended_link_type': 'symlink' | 'hardlink' | 'absolute_symlink',
                'warnings': list[str],
                'errors': list[str]
            }
        """
        result = {
            'valid': True,
            'same_filesystem': False,
            'link_type_supported': False,
            'recommended_link_type': 'symlink',
            'warnings': [],
            'errors': []
        }

        try:
            # Check write permission for target app directory
            if not os.access(app_path, os.W_OK):
                result['errors'].append(
                    f"No write permission for {app_path}. "
                    f"Run: sudo chmod -R u+w {app_path}"
                )
                result['valid'] = False
                return result

            # Check if same filesystem
            same_fs = self._same_filesystem(library_path, app_path)
            result['same_filesystem'] = same_fs

            # Detect filesystem types
            lib_fs = self._detect_filesystem_type(library_path)
            app_fs = self._detect_filesystem_type(app_path)

            # Determine recommended link type
            if same_fs:
                if lib_fs in ('ext4', 'btrfs', 'xfs'):
                    result['recommended_link_type'] = 'symlink'
                elif lib_fs == 'ntfs':
                    result['recommended_link_type'] = 'hardlink'
                    result['warnings'].append(
                        "NTFS detected. Hard links recommended over symlinks for reliability."
                    )
            else:
                # Cross-filesystem: Only absolute symlinks possible
                result['recommended_link_type'] = 'absolute_symlink'
                result['warnings'].append(
                    "Library and app are on different filesystems. "
                    "Using absolute symlinks. Unplugging external drives will break links."
                )

            result['link_type_supported'] = True

        except Exception as e:
            logger.error(f"Mapping validation failed: {e}", exc_info=True)
            result['errors'].append(f"Validation error: {e}")
            result['valid'] = False

        return result

    def _detect_filesystem_type(self, path: Path) -> str:
        """Detect filesystem type (ext4, btrfs, xfs, ntfs, etc.)."""
        # Implementation details...
        pass

    def _same_filesystem(self, path1: Path, path2: Path) -> bool:
        """Check if two paths are on the same filesystem."""
        try:
            return path1.stat().st_dev == path2.stat().st_dev
        except OSError:
            return False

    def _test_canary_links(self, target_dir: Path) -> dict:
        """
        Test if symlinks and hardlinks actually work on this filesystem.

        Creates tiny test files and attempts to link them. This catches
        NTFS mount issues where links appear to succeed but don't work.

        Args:
            target_dir: Directory to test link creation in

        Returns:
            {
                'symlink_works': bool,
                'hardlink_works': bool,
                'symlink_error': str | None,
                'hardlink_error': str | None
            }
        """
        import tempfile
        import uuid

        result = {
            'symlink_works': False,
            'hardlink_works': False,
            'symlink_error': None,
            'hardlink_error': None
        }

        # Create unique test filenames to avoid collisions
        test_id = uuid.uuid4().hex[:8]
        canary_file = target_dir / f".pumas_canary_{test_id}.tmp"
        symlink_test = target_dir / f".pumas_symlink_test_{test_id}.tmp"
        hardlink_test = target_dir / f".pumas_hardlink_test_{test_id}.tmp"

        try:
            # Create canary file
            canary_file.write_text("canary")

            # Test symlink
            try:
                symlink_test.symlink_to(canary_file)
                # Verify it actually works by reading through the link
                if symlink_test.read_text() == "canary":
                    result['symlink_works'] = True
                else:
                    result['symlink_error'] = "Symlink created but read failed"
            except OSError as e:
                result['symlink_error'] = str(e)
            finally:
                if symlink_test.is_symlink() or symlink_test.exists():
                    symlink_test.unlink(missing_ok=True)

            # Test hardlink
            try:
                hardlink_test.hardlink_to(canary_file)
                # Verify it actually works
                if hardlink_test.read_text() == "canary":
                    result['hardlink_works'] = True
                else:
                    result['hardlink_error'] = "Hardlink created but read failed"
            except OSError as e:
                result['hardlink_error'] = str(e)
            finally:
                if hardlink_test.exists():
                    hardlink_test.unlink(missing_ok=True)

        except Exception as e:
            logger.error(f"Canary link test failed: {e}")
        finally:
            # Cleanup canary file
            if canary_file.exists():
                canary_file.unlink(missing_ok=True)

        logger.info(
            f"Canary link test: symlink={result['symlink_works']}, "
            f"hardlink={result['hardlink_works']}"
        )

        return result

# Global instance
fs_validator = FilesystemValidator()
```

---

### NTFS-Compatible Filename Normalization

**Problem**: Models imported on ext4/btrfs may contain characters forbidden by NTFS.

**Solution**: Normalize all filenames to be NTFS-compatible at import time with collision detection.

**Implementation**: Add to `backend/model_library/naming.py`

```python
import re
import hashlib
from pathlib import Path

# NTFS forbidden characters
NTFS_FORBIDDEN_CHARS = r'[<>:"|?*]'
NTFS_RESERVED_NAMES = {
    'CON', 'PRN', 'AUX', 'NUL',
    'COM1', 'COM2', 'COM3', 'COM4', 'COM5', 'COM6', 'COM7', 'COM8', 'COM9',
    'LPT1', 'LPT2', 'LPT3', 'LPT4', 'LPT5', 'LPT6', 'LPT7', 'LPT8', 'LPT9'
}

def sanitize_filename_for_ntfs(filename: str) -> str:
    """
    Sanitize filename to be NTFS-compatible.

    Replaces forbidden characters with underscores:
    - < > : " | ? *
    - Control characters (0x00-0x1F)
    - Trailing dots and spaces
    """
    # Preserve extension
    stem, ext = Path(filename).stem, Path(filename).suffix

    # Replace forbidden characters with underscore
    stem = re.sub(NTFS_FORBIDDEN_CHARS, '_', stem)

    # Replace control characters
    stem = re.sub(r'[\x00-\x1f]', '_', stem)

    # Replace backslash
    stem = stem.replace('\\', '_')

    # Remove trailing dots and spaces
    stem = stem.rstrip('. ')

    # Check for reserved names
    if stem.upper() in NTFS_RESERVED_NAMES:
        stem = f"{stem}_file"

    return f"{stem}{ext}"


def resolve_ntfs_collision(dest_path: Path, file_hash: str) -> Path:
    """
    Resolve NTFS filename collision by checking hashes.

    If destination exists and has different hash, append 4-char hash suffix.

    Args:
        dest_path: Intended destination path
        file_hash: BLAKE3 hash of source file

    Returns:
        Collision-free path (may have hash suffix appended)

    Example:
        diffusion/family/sd-v1-5 → diffusion/family/sd-v1-5-8f2a
    """
    if not dest_path.exists():
        return dest_path

    # Check if existing file has same hash (same model)
    existing_metadata = dest_path / "metadata.json"
    if existing_metadata.exists():
        try:
            import json
            with open(existing_metadata, 'r') as f:
                metadata = json.load(f)
                existing_hash = metadata.get('hashes', {}).get('blake3')

                if existing_hash == file_hash:
                    # Same model, no collision
                    return dest_path
        except Exception:
            pass

    # Collision detected: Append 4-char hash suffix
    hash_suffix = file_hash[:4]
    new_name = f"{dest_path.name}-{hash_suffix}"
    return dest_path.parent / new_name
```

---

### Cross-Filesystem Detection

**Problem**: Relative symlinks don't work across filesystems on Linux.

**Solution**: Detect cross-filesystem scenarios and use absolute symlinks with warnings.

**Implementation**:

```python
def _determine_link_type(
    self,
    source: Path,
    target_dir: Path,
    validation_result: dict
) -> Literal['relative_symlink', 'absolute_symlink', 'hardlink']:
    """
    Determine optimal link type based on filesystem validation.

    Priority:
    1. Same filesystem + ext4/btrfs/xfs → relative_symlink (portable)
    2. Same filesystem + NTFS → hardlink (NTFS symlinks unreliable)
    3. Different filesystems → absolute_symlink (with warning)
    """
    recommended = validation_result.get('recommended_link_type')
    same_fs = validation_result.get('same_filesystem', False)

    if recommended == 'relative_symlink':
        return 'relative_symlink'
    elif recommended == 'hardlink':
        return 'hardlink'
    elif recommended == 'absolute_symlink':
        # Warn user about external drive dependency
        logger.warning(
            f"Cross-filesystem mapping detected. "
            f"Using absolute symlinks. App will break if library drive is unmounted."
        )
        return 'absolute_symlink'
    else:
        # Fallback
        return 'relative_symlink' if same_fs else 'absolute_symlink'


def make_relative_symlink(source: Path, target: Path) -> bool:
    """
    Create a relative symlink correctly.

    Critical: The source path must be relative to the link's directory,
    NOT the current working directory.

    Args:
        source: Absolute path to source file/directory
        target: Absolute path where symlink will be created

    Returns:
        True if symlink created, False if already exists
    """
    import os

    # Calculate relative path from link location to source
    # Example: source=/library/models/file.safetensors, target=/app/models/checkpoints/file.safetensors
    # Result: ../../../library/models/file.safetensors
    rel_path = os.path.relpath(str(source), start=str(target.parent))

    # Create symlink with relative path
    if target.exists() or target.is_symlink():
        return False

    target.symlink_to(rel_path)
    return True
```

---

### NVMe Device Handling

**Problem**: NVMe devices show up as `nvme0n1` not `sda`, breaking device name parsing.

**Solution**: Robust device name extraction for all Linux block devices.

**Implementation**: Already included in IOManager `_extract_base_device()` method above.

---

## Platform Compatibility

### 1. Sandbox/Flatpak Detection

**Problem**: ComfyUI often runs in Flatpak/Docker, where relative symlinks may fail.

**Solution**: Detect sandbox environment and warn user to grant permissions.

**Implementation**:

```python
def detect_sandbox_environment() -> dict:
    """
    Detect if running in a sandboxed environment.

    Returns:
        {
            'sandboxed': bool,
            'type': 'flatpak' | 'snap' | 'docker' | None,
            'permissions_needed': list[str]
        }
    """
    sandboxed = False
    sandbox_type = None
    permissions = []

    # Check for Flatpak
    if Path("/.flatpak-info").exists():
        sandboxed = True
        sandbox_type = "flatpak"
        permissions = [
            "Filesystem access to library directory",
            "Filesystem access to ComfyUI directory"
        ]

    # Check for Snap
    elif "SNAP" in os.environ:
        sandboxed = True
        sandbox_type = "snap"
        permissions = ["Connect 'removable-media' interface"]

    # Check for Docker
    elif Path("/.dockerenv").exists():
        sandboxed = True
        sandbox_type = "docker"
        permissions = ["Mount library directory as volume"]

    return {
        'sandboxed': sandboxed,
        'type': sandbox_type,
        'permissions_needed': permissions
    }
```

---

### 2. Dynamic Directory Scanning

**Problem**: Hardcoding 22 ComfyUI subdirectories breaks when custom nodes add new directories.

**Solution**: Scan the actual ComfyUI `models/` folder at runtime.

**Implementation**:

```python
def _discover_model_directories(self, models_root: Path) -> list[str]:
    """
    Scan ComfyUI models/ directory for subdirectories.

    Returns:
        List of subdirectory names (e.g., ['checkpoints', 'loras', 'ipadapter'])
    """
    if not models_root.is_dir():
        return []

    subdirs = []
    for item in models_root.iterdir():
        if item.is_dir() and not item.name.startswith('.'):
            subdirs.append(item.name)

    return sorted(subdirs)
```

---

## Import Modes

### Fast Import (Move)

**When**: Source and destination are on same filesystem

**How**: Use `os.rename()` for instant operation, compute hashes after move

**Implementation**:

```python
def import_model(self, source_path: Path, ..., use_move: bool = False) -> dict:
    """Import model with automatic same-filesystem detection for move operations."""

    # Critical: Check if source and destination are on same filesystem
    source_stat = source_path.stat()
    dest_parent_stat = dest_dir.parent.stat()
    same_fs = source_stat.st_dev == dest_parent_stat.st_dev

    if use_move and not same_fs:
        # Cross-filesystem "move" is actually Copy + Delete
        # Auto-switch to safe copy and warn user
        logger.warning(
            f"Fast import (move) requested but source and destination are on "
            f"different filesystems (dev {source_stat.st_dev} → {dest_parent_stat.st_dev}). "
            f"Switching to safe copy mode."
        )
        use_move = False

    if use_move and same_fs:
        # Fast move (instant rename)
        logger.info(f"Fast import (move): {source_path.name}")

        # Atomic move with hash computation AFTER move
        temp_dest = dest_path.with_suffix(dest_path.suffix + '.tmp')
        source_path.rename(temp_dest)

        # Compute hashes in place
        blake3_hash, sha256_hash = self._compute_hashes_in_place(temp_dest)

        # Rename to final name
        temp_dest.rename(dest_path)
    else:
        # Safe copy with stream hashing
        ...
```

**Performance**: Instant (no file copy) when on same filesystem

---

### Safe Import (Copy)

**When**: Cross-filesystem import OR user prefers to keep originals

**How**: Use `copy_and_hash()` for stream hashing, atomic rename

**Implementation**: Already covered in "Stream Hashing" section above.

**Performance**: ~2-3 minutes for 20GB on SSD, ~5-10 minutes on HDD

---

### Space Management (Delete Originals)

**Purpose**: Allow users to optionally delete source files after successful import to save disk space.

**Safety Checks**:
- All imports must have succeeded
- All files must have hash verification
- Source filesystem must be writable
- Source must not be in library directory

**Implementation**:

```python
def delete_source_files_safely(
    self,
    import_results: List[dict],
    source_files: List[Path]
) -> dict:
    """
    Safely delete source files after successful import.

    Returns:
        {
            'success': bool,
            'deleted_count': int,
            'errors': List[str]
        }
    """
    deleted = []
    errors = []

    # Safety check: All imports successful
    if not all(r.get('success') for r in import_results):
        return {
            'success': False,
            'deleted_count': 0,
            'errors': ['Cannot delete: Some imports failed']
        }

    # Safety check: All imports have hash verification
    if not all(r.get('hash_verified') for r in import_results):
        return {
            'success': False,
            'deleted_count': 0,
            'errors': ['Cannot delete: Some files lack hash verification']
        }

    for source_path in source_files:
        try:
            source = Path(source_path)

            # Safety checks
            if not source.exists():
                errors.append(f"File not found: {source}")
                continue

            if source.is_relative_to(self.library_dir):
                errors.append(f"Refusing to delete library file: {source}")
                continue

            if not os.access(source.parent, os.W_OK):
                errors.append(f"Source directory is read-only: {source.parent}")
                continue

            # Delete the file
            source.unlink()
            deleted.append(str(source))
            logger.info(f"Deleted source file: {source}")

        except Exception as e:
            errors.append(f"Failed to delete {source}: {e}")
            logger.error(f"Error deleting {source}: {e}")

    return {
        'success': len(errors) == 0,
        'deleted_count': len(deleted),
        'errors': errors
    }
```

---

## Performance Summary

| Optimization | Before | After | Improvement |
|--------------|--------|-------|-------------|
| **Import Time** | Copy + hash separately | Stream hashing | 40% faster |
| **Disk I/O** | Parallel (thrashing on HDD) | Drive-aware queue | No thrashing |
| **Sync Time** | Full tree scan (33k checks) | Incremental (15 checks) | 2200× faster |
| **API Calls** | Up to 350/batch | Max 200 (throttled) | Rate-limit safe |
| **Database** | Blocking reads/writes | WAL mode | Concurrent access |
| **Recovery** | No rebuild capability | Deep scan from metadata | Full recovery |

---

**End of Performance & Data Integrity Document**
