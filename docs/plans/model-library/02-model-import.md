# Model Import System

**Version**: 3.1
---

## Table of Contents

- [Overview](#overview)
- [Visual Design](#visual-design)
- [Component Architecture](#component-architecture)
- [HuggingFace Metadata Lookup](#huggingface-metadata-lookup)
- [Backend Implementation](#backend-implementation)
- [Frontend Implementation](#frontend-implementation)
- [Import Flow](#import-flow)
- [Testing Strategy](#testing-strategy)

---

## Overview

Enable users to drag model files (or folders) onto the GUI to import them into the library with automatic HuggingFace metadata enrichment.

### Key Features

- Window-level drag-and-drop target with animated overlay
- Multi-file and folder support
- **Sharded set grouping** - Automatically detect and group multi-file models (e.g., model-00001-of-00005.safetensors)
- Hybrid HuggingFace lookup: hash verification + fuzzy filename fallback
- Trust badges indicating match confidence
- Related file detection with download option
- Progressive disclosure of technical details
- Granular progress states (Copying → Hashing → Indexing)
- Optional "delete originals" for space management
- Atomic operations with crash recovery
- Incremental sync to installed apps
- Offline-first design (imports work without network)

---

## Visual Design

### Drop Interaction States

#### State 1: Normal Operation
```
┌─────────────────────────────────────────┐
│ [Model Manager UI - Normal]            │
│ • Search bar visible                    │
│ • Model list visible                    │
│ • Full opacity                          │
└─────────────────────────────────────────┘
```

#### State 2: Drop Zone Active (file dragged over window)
```
┌─────────────────────────────────────────┐
│ [Blurred Background - blur(8px)]       │
│                                         │
│   ╔═══════════════════════════════╗   │
│   ║   📦 Drop Model Files Here    ║   │
│   ║                               ║   │
│   ║   Supported formats:          ║   │
│   ║   .safetensors, .ckpt, .gguf  ║   │
│   ║   .pt, .bin                   ║   │
│   ║                               ║   │
│   ║   [Dashed border animation]   ║   │
│   ╚═══════════════════════════════╝   │
│                                         │
└─────────────────────────────────────────┘
```

#### State 3: Import Dialog (after drop)
```
┌─────────────────────────────────────────┐
│ [Blurred Background - blur(4px)]       │
│                                         │
│   ┌─────────────────────────────────┐  │
│   │ Import Model                    │  │
│   ├─────────────────────────────────┤  │
│   │ [Multi-step import wizard]      │  │
│   │ [See detailed mockup below]     │  │
│   └─────────────────────────────────┘  │
│                                         │
└─────────────────────────────────────────┘
```

---

## Component Architecture

### 1. ModelImportDropZone Component

**File**: `frontend/src/components/ModelImportDropZone.tsx`

**Purpose**: Global drop target overlay with window-level event listeners

**Features**:
- Window-level drag event listeners
- File type validation (.safetensors, .ckpt, .gguf, .pt, .bin)
- Multi-file and folder support
- Backdrop blur animation
- Pulsing border animation
- **PyWebView URI handling** (GTK/WebKit compatibility)

**Key State**:
```typescript
const [isDragActive, setIsDragActive] = useState(false);
const [dragCounter, setDragCounter] = useState(0); // Handle nested elements
```

**PyWebView GTK/WebKit Compatibility** (CRITICAL):

PyWebView on Linux (GTK/WebKit) may return URI strings (`file:///path/to/model`) instead of standard File objects when dragging from file managers like Nautilus or Dolphin. The frontend must handle both formats.

```typescript
/**
 * Convert file URI to standard path.
 * Handles PyWebView GTK/WebKit quirks where dataTransfer may contain URIs.
 *
 * @param uri - Either a file:// URI or standard path
 * @returns Decoded file path
 */
function fileUriToPath(uri: string): string {
  // Check if it's a file:// URI
  if (uri.startsWith('file://')) {
    // Remove file:// prefix and decode URI encoding
    const path = decodeURIComponent(uri.slice(7));
    // Handle Windows-style file:///C:/path (remove leading slash before drive letter)
    if (/^\/[A-Za-z]:/.test(path)) {
      return path.slice(1);
    }
    return path;
  }
  // Already a standard path
  return uri;
}

/**
 * Extract file paths from drag event, handling both File API and URI list.
 * PyWebView GTK/WebKit may return text/uri-list instead of File objects.
 */
function extractFilePaths(dataTransfer: DataTransfer): string[] {
  const paths: string[] = [];

  // Try standard File API first
  if (dataTransfer.files && dataTransfer.files.length > 0) {
    // PyWebView provides file paths via webkitRelativePath or we use the API
    for (const file of Array.from(dataTransfer.files)) {
      // In PyWebView, file.path may be available (Electron-style)
      const filePath = (file as any).path || file.name;
      if (filePath && filePath !== file.name) {
        paths.push(filePath);
      }
    }

    // If we got paths from File API, return them
    if (paths.length > 0) {
      return paths;
    }
  }

  // Fallback: Try text/uri-list (common in GTK file managers)
  const uriList = dataTransfer.getData('text/uri-list');
  if (uriList) {
    const uris = uriList.split('\n')
      .map(line => line.trim())
      .filter(line => line && !line.startsWith('#')); // Filter comments

    for (const uri of uris) {
      paths.push(fileUriToPath(uri));
    }

    if (paths.length > 0) {
      return paths;
    }
  }

  // Last resort: Try text/plain (some file managers use this)
  const plainText = dataTransfer.getData('text/plain');
  if (plainText) {
    const lines = plainText.split('\n')
      .map(line => line.trim())
      .filter(line => line);

    for (const line of lines) {
      paths.push(fileUriToPath(line));
    }
  }

  return paths;
}
```

**Event Handling** (Critical for preventing browser default behavior):
```typescript
useEffect(() => {
  const handleDragEnter = (e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragCounter(prev => prev + 1);
    setIsDragActive(true);
  };

  const handleDragOver = (e: DragEvent) => {
    e.preventDefault(); // CRITICAL: Prevents browser from opening/downloading file
    e.stopPropagation();
  };

  const handleDragLeave = (e: DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    setDragCounter(prev => {
      const newCount = prev - 1;
      if (newCount === 0) {
        setIsDragActive(false);
      }
      return newCount;
    });
  };

  const handleDrop = (e: DragEvent) => {
    e.preventDefault(); // CRITICAL: Prevents browser from opening/downloading file
    e.stopPropagation();

    setIsDragActive(false);
    setDragCounter(0);

    // Extract paths using PyWebView-compatible method
    const filePaths = extractFilePaths(e.dataTransfer!);

    // Filter for supported file types
    const validPaths = filePaths.filter(path => {
      const ext = path.toLowerCase().match(/\.(safetensors|ckpt|gguf|pt|bin)$/);
      return ext !== null;
    });

    if (validPaths.length > 0) {
      onFileDrop(validPaths);
    }
  };

  // Attach to window for global coverage
  window.addEventListener('dragenter', handleDragEnter);
  window.addEventListener('dragover', handleDragOver);
  window.addEventListener('dragleave', handleDragLeave);
  window.addEventListener('drop', handleDrop);

  return () => {
    window.removeEventListener('dragenter', handleDragEnter);
    window.removeEventListener('dragover', handleDragOver);
    window.removeEventListener('dragleave', handleDragLeave);
    window.removeEventListener('drop', handleDrop);
  };
}, [onFileDrop]);
```

**Visual Styling**:
```tsx
<AnimatePresence>
  {isDragActive && (
    <motion.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      className="fixed inset-0 z-50"
    >
      <div className="absolute inset-0 backdrop-blur-[8px] bg-[hsl(var(--launcher-bg-primary)/0.6)]" />

      <div className="absolute inset-0 flex items-center justify-center p-8 pointer-events-auto">
        <div className="border-4 border-dashed border-[hsl(var(--launcher-accent-primary))] rounded-2xl w-full max-w-2xl h-64 flex flex-col items-center justify-center gap-4 bg-[hsl(var(--surface-mid)/0.8)] animate-pulse-border">
          <Upload className="w-16 h-16 text-[hsl(var(--launcher-accent-primary))]" />
          <p className="text-xl font-semibold">Drop Model Files Here</p>
          <p className="text-sm text-[hsl(var(--text-secondary))]">
            Supported: .safetensors, .ckpt, .gguf, .pt, .bin
          </p>
        </div>
      </div>
    </motion.div>
  )}
</AnimatePresence>
```

**CSS Animation** (add to `frontend/src/index.css`):
```css
@keyframes pulse-border {
  0%, 100% { border-color: hsl(var(--launcher-accent-primary)); }
  50% { border-color: hsl(var(--launcher-accent-info)); }
}

.animate-pulse-border {
  animation: pulse-border 2s ease-in-out infinite;
}
```

---

### 2. ModelImportDialog Component

**File**: `frontend/src/components/ModelImportDialog.tsx`

**Purpose**: Multi-step import wizard with metadata enrichment

**Props**:
```typescript
interface ModelImportDialogProps {
  isOpen: boolean;
  onClose: () => void;
  files: File[] | FileList;
  onImportComplete?: () => void;
}
```

**Progress Updates**:

The import dialog uses **callback-based progress updates**, not polling:
- Backend calls `progress_callback()` during import operations
- Real-time updates for copying, hashing, indexing, syncing stages
- No 10-second delay between status changes

**Note**: The existing `useModels.ts` 10-second polling is ONLY for detecting external changes (models added outside the app), NOT for tracking active import operations.

---

## HuggingFace Metadata Lookup

### Strategy

Hybrid approach combining hash verification with fuzzy filename fallback:

1. **Filename-based search** to find top 3-5 candidate repos (fast, no downloads)
2. **Fast hash filter** (first 8MB + last 8MB) to quickly eliminate non-matching candidates
3. **Full hash verification** during copy stream for final 100% verification
4. **Exact hash match**: Return with 100% confidence
5. **No hash match**: Fall back to filename similarity scoring with confidence warning

### Fast Hash Optimization (Large File Handling)

**Problem**: Computing full SHA256 of a 20GB model file takes minutes on HDD, causing UI hangs during "Searching..." phase.

**Solution**: Use a "Fast Hash" (first 8MB + last 8MB) as a candidate filter before committing to full verification during the copy stream.

```python
def compute_fast_hash(file_path: Path) -> str:
    """
    Compute a fast hash using first and last 8MB of file.

    This provides a quick candidate filter without reading the entire file.
    For a 20GB file on HDD, this reads ~16MB instead of 20GB.

    Args:
        file_path: Path to the model file

    Returns:
        SHA256 hash of (first_8MB + last_8MB + file_size)
    """
    CHUNK_SIZE = 8 * 1024 * 1024  # 8MB

    file_size = file_path.stat().st_size
    hasher = hashlib.sha256()

    with open(file_path, 'rb') as f:
        # Read first 8MB
        first_chunk = f.read(CHUNK_SIZE)
        hasher.update(first_chunk)

        # Read last 8MB (if file is larger than 16MB)
        if file_size > CHUNK_SIZE * 2:
            f.seek(-CHUNK_SIZE, 2)  # Seek to last 8MB
            last_chunk = f.read(CHUNK_SIZE)
            hasher.update(last_chunk)
        elif file_size > CHUNK_SIZE:
            # File is between 8-16MB, read remaining
            remaining = f.read()
            hasher.update(remaining)

        # Include file size to differentiate files with same head/tail
        hasher.update(str(file_size).encode())

    return hasher.hexdigest()
```

**Integration with Lookup Flow**:

```python
def lookup_model_metadata_by_filename(
    self,
    filename: str,
    file_path: Optional[Path] = None,
    timeout: float = 5.0
) -> Optional[dict]:
    """
    Lookup HuggingFace metadata using fast hash + stream verification.

    Strategy:
    1. Filename-based search to find top candidates (3-5 repos)
    2. Compute FAST HASH (first 8MB + last 8MB) - takes ~2 seconds on HDD
    3. Filter candidates by comparing fast hash patterns
    4. Return top match with confidence score
    5. Full hash verification happens DURING COPY (stream hashing)
    """
    # Step 1: Get candidate repos via filename search
    candidates = self._get_candidate_repos(filename, limit=5)

    if not candidates:
        return None

    # Step 2: Compute fast hash for candidate filtering (quick)
    fast_hash = None
    if file_path and file_path.exists():
        fast_hash = compute_fast_hash(file_path)
        logger.debug(f"Fast hash computed: {fast_hash[:16]}...")

    # Step 3: Filter candidates using fast hash
    # Note: HuggingFace doesn't store fast hashes, so we use this for
    # local deduplication and size-based filtering
    candidates_sorted = sorted(
        candidates,
        key=lambda r: getattr(r, 'downloads', 0),
        reverse=True
    )

    # Step 4: Return best filename match
    # Full hash verification happens during copy stream (Option B)
    best_match = self._find_best_filename_match(filename, candidates_sorted)

    if best_match:
        # Mark for full verification during copy
        best_match['pending_full_verification'] = True
        best_match['fast_hash'] = fast_hash
        return best_match

    return None
```

**Full Verification During Copy** (Option B - Stream Hashing):

The full SHA256 hash is computed during the copy operation via `copy_and_hash()`. This means:
- UI shows metadata immediately after fast hash (~2 seconds)
- Full verification happens in background during file copy
- No double-read of the file (copy + hash in single pass)
- Final hash comparison occurs after copy completes

```python
def import_model(self, source_path: Path, hf_metadata: dict, ...) -> dict:
    """Import with deferred full hash verification."""

    # Copy with stream hashing (computes full SHA256)
    blake3_hash, sha256_hash = copy_and_hash(source_path, temp_path)

    # Verify against HF metadata if available
    if hf_metadata and hf_metadata.get('pending_full_verification'):
        expected_hash = hf_metadata.get('expected_sha256')
        if expected_hash and sha256_hash != expected_hash:
            # Hash mismatch - update metadata to reflect modified version
            hf_metadata['hash_mismatch'] = True
            hf_metadata['match_confidence'] = 0.7  # Downgrade confidence
            logger.warning(
                f"Hash mismatch for {source_path.name}: "
                f"expected {expected_hash[:16]}..., got {sha256_hash[:16]}..."
            )

    # Continue with import...
```

### Match Methods

- **`hash`**: SHA256 hash matched exactly (100% confidence, verified)
- **`filename_exact`**: Exact filename match (>90% confidence)
- **`filename_fuzzy`**: Fuzzy search match (<90% confidence, requires confirmation)
- **`hash_mismatch`**: Filename matched but hash differs (pruned/quantized/modified version)

### Trust Badges

Visual indicators of match quality:

```tsx
{hfData.match_method === 'hash' ? (
  <div className="flex items-center gap-1.5 px-2 py-1 bg-[hsl(var(--accent-success)/0.1)] border border-[hsl(var(--accent-success))] rounded text-xs">
    <BookCheck className="w-4 h-4 text-[hsl(var(--accent-success))]" />
    <span className="font-medium text-[hsl(var(--accent-success))]">Verified</span>
  </div>
) : hfData.match_method === 'filename_exact' && hfData.match_confidence > 0.9 ? (
  <div className="flex items-center gap-1.5 px-2 py-1 bg-[hsl(var(--accent-info)/0.1)] border border-[hsl(var(--accent-info))] rounded text-xs">
    <Book className="w-4 h-4 text-[hsl(var(--accent-info))]" />
    <span className="font-medium text-[hsl(var(--accent-info))]">High Confidence</span>
  </div>
) : hfData.match_method === 'filename_fuzzy' ? (
  <div className="flex items-center gap-1.5 px-2 py-1 bg-[hsl(var(--accent-warning)/0.1)] border border-[hsl(var(--accent-warning))] rounded text-xs">
    <BookAlert className="w-4 h-4 text-[hsl(var(--accent-warning))]" />
    <span className="font-medium text-[hsl(var(--accent-warning))]">Low Confidence</span>
  </div>
) : hfData.hash_mismatch ? (
  <div className="flex items-center gap-1.5 px-2 py-1 bg-[hsl(var(--accent-warning)/0.1)] border border-[hsl(var(--accent-warning))] rounded text-xs">
    <BookAlert className="w-4 h-4 text-[hsl(var(--accent-warning))]" />
    <span className="font-medium text-[hsl(var(--accent-warning))]">Modified Version</span>
  </div>
) : (
  <div className="flex items-center gap-1.5 px-2 py-1 bg-[hsl(var(--surface-low))] border border-[hsl(var(--border-default))] rounded text-xs">
    <BookX className="w-4 h-4 text-[hsl(var(--text-tertiary))]" />
    <span className="font-medium text-[hsl(var(--text-tertiary))]">No Match</span>
  </div>
)}
```

---

## Backend Implementation

### Hybrid Lookup with Hash Verification

**File**: `backend/model_library/downloader.py`

```python
import hashlib
from pathlib import Path
from typing import Optional
from functools import lru_cache
from datetime import datetime, timedelta

# Cache repo file lists for 24 hours
_repo_cache: dict[str, tuple[list, datetime]] = {}
_repo_cache_ttl = timedelta(hours=24)

def lookup_model_metadata_by_filename(
    self,
    filename: str,
    file_path: Optional[Path] = None,
    timeout: float = 5.0
) -> Optional[dict]:
    """
    Lookup HuggingFace metadata using hybrid filename + hash verification.

    Strategy:
    1. Filename-based search to find top candidates (3-5 repos)
    2. Compute SHA256 of local file if file_path provided
    3. Fetch LFS file hashes for each candidate repo (top 2 only)
    4. Compare hashes - if exact match, return with 100% confidence
    5. Otherwise fall back to filename similarity with confidence score

    Args:
        filename: Name of the model file
        file_path: Optional path to local file for hash verification
        timeout: Timeout for API calls (default: 5 seconds)

    Returns:
        dict with keys:
        - repo_id, official_name, family, tags, etc.
        - match_confidence: float (0.0-1.0)
        - match_method: "hash" | "filename_exact" | "filename_fuzzy"
        - requires_confirmation: bool
        - hash_mismatch: bool (if filename matched but hash didn't)
    """
    logger.info(f"Looking up metadata for: {filename}")

    # Step 1: Get candidate repos via filename search
    candidates = self._get_candidate_repos(filename, limit=5)

    if not candidates:
        logger.info("No candidates found")
        return None

    # Step 2: If we have the file, try hash verification (top 2 only)
    if file_path and file_path.exists():
        # Sort by popularity (download count)
        candidates_sorted = sorted(
            candidates,
            key=lambda r: getattr(r, 'downloads', 0),
            reverse=True
        )

        for candidate in candidates_sorted[:2]:  # TOP 2 ONLY
            hash_match = self._verify_hash_single_candidate(
                file_path,
                candidate,
                filename
            )

            if hash_match:
                logger.info(f"Hash match found: {candidate.id}")
                return hash_match

    # Step 3: Fall back to filename matching with confidence
    best_match = self._find_best_filename_match(filename, candidates)

    if best_match:
        logger.info(
            f"Filename match: {best_match['repo_id']} "
            f"(confidence: {best_match['match_confidence']:.2f})"
        )
        return best_match

    return None


def _verify_hash_single_candidate(
    self,
    file_path: Path,
    candidate_repo,
    filename: str
) -> Optional[dict]:
    """
    Verify local file hash against single candidate repo.

    Returns metadata dict with match_confidence=1.0 if found, else None.
    """
    # Compute local file hash (SHA256 for LFS compatibility)
    local_hash = self._compute_sha256(file_path)
    logger.debug(f"Local file SHA256: {local_hash[:16]}...")

    repo_id = candidate_repo.id

    try:
        # Fetch LFS files with hashes (single throttled API call)
        lfs_files = self._api_call_with_throttle(
            self._get_lfs_files_cached,
            repo_id
        )

        # Look for hash match
        for lfs_file in lfs_files:
            file_oid = getattr(lfs_file, 'oid', None)
            if not file_oid:
                continue

            # SHA256 OID is prefixed with "sha256:" in some cases
            file_hash = file_oid.replace("sha256:", "").lower()

            if file_hash == local_hash.lower():
                logger.info(
                    f"Hash match! {repo_id} / {lfs_file.filename} "
                    f"(SHA256: {file_hash[:16]}...)"
                )

                # Extract full metadata with 100% confidence
                metadata = self._extract_metadata_from_repo(candidate_repo)
                metadata['match_confidence'] = 1.0
                metadata['match_method'] = 'hash'
                metadata['matched_filename'] = lfs_file.filename
                metadata['requires_confirmation'] = False
                metadata['hash_mismatch'] = False

                return metadata

    except Exception as e:
        logger.warning(f"Could not fetch LFS files for {repo_id}: {e}")

    return None


def _compute_sha256(self, file_path: Path) -> str:
    """Compute SHA256 hash of file (matches HF LFS hash format)."""
    sha256_hash = hashlib.sha256()

    with open(file_path, "rb") as f:
        for byte_block in iter(lambda: f.read(8192), b""):
            sha256_hash.update(byte_block)

    return sha256_hash.hexdigest()


def _find_best_filename_match(self, filename: str, candidates: list) -> Optional[dict]:
    """
    Find best candidate based on filename similarity.

    Returns metadata with match_confidence < 1.0 and requires_confirmation flag.
    """
    from difflib import SequenceMatcher

    base_name = self._extract_base_name(filename).lower()
    best_match = None
    best_score = 0.0

    for repo in candidates:
        repo_name = repo.id.lower()
        score = SequenceMatcher(None, base_name, repo_name).ratio()

        if score > best_score:
            best_score = score
            best_match = repo

    if not best_match:
        return None

    # Extract metadata
    metadata = self._extract_metadata_from_repo(best_match)

    # Determine match method
    if best_score > 0.9:
        match_method = 'filename_exact'
    else:
        match_method = 'filename_fuzzy'

    metadata['match_confidence'] = best_score
    metadata['match_method'] = match_method
    metadata['requires_confirmation'] = best_score < 0.6
    metadata['hash_mismatch'] = False

    return metadata


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
    api = self._get_api()
    lfs_files = list(api.list_lfs_files(repo_id))

    # Store in cache
    _repo_cache[repo_id] = (lfs_files, now)
    logger.debug(f"Cached LFS files for {repo_id} ({len(lfs_files)} files)")

    return lfs_files
```

### Model Variant & Precision Detection

Add variant and precision inference to metadata:

```python
def _infer_variant_and_precision(self, filename: str) -> tuple[str, str]:
    """
    Infer model variant and precision from filename.

    Returns:
        (variant, precision)
    """
    filename_lower = filename.lower()

    # Variant detection
    if 'ema' in filename_lower:
        variant = 'ema'
    elif 'pruned' in filename_lower:
        variant = 'pruned'
    elif 'full' in filename_lower:
        variant = 'full'
    elif filename.endswith('.safetensors'):
        variant = 'safetensors'
    else:
        variant = 'standard'

    # Precision detection
    if 'fp16' in filename_lower or 'half' in filename_lower:
        precision = 'fp16'
    elif 'bf16' in filename_lower:
        precision = 'bf16'
    elif 'fp32' in filename_lower or 'float32' in filename_lower:
        precision = 'fp32'
    elif 'int8' in filename_lower or '8bit' in filename_lower:
        precision = 'int8'
    elif 'int4' in filename_lower or '4bit' in filename_lower:
        precision = 'int4'
    else:
        precision = 'unknown'

    return variant, precision
```

### Sharded Set Detection & Grouping

**Purpose**: Automatically detect and group multi-file models (sharded sets) during import.

**Common Patterns**:
- `model-00001-of-00005.safetensors`, `model-00002-of-00005.safetensors`, ...
- `pytorch_model-00001-of-00003.bin`, `pytorch_model-00002-of-00003.bin`, ...
- `model.safetensors.part1`, `model.safetensors.part2`, ...

**Implementation**: Add to `backend/model_library/importer.py`

```python
import re
from pathlib import Path
from typing import List, Dict, Optional

def detect_sharded_sets(files: List[Path]) -> Dict[str, List[Path]]:
    """
    Detect and group sharded model files.

    Args:
        files: List of file paths to analyze

    Returns:
        Dict mapping base name to list of shard files
        Example: {'model': [Path('model-00001-of-00005.safetensors'), ...]}
    """
    # Pattern 1: model-00001-of-00005.safetensors
    pattern1 = re.compile(r'^(.+)-(\d+)-of-(\d+)(\.[^.]+)$')

    # Pattern 2: model.safetensors.part1
    pattern2 = re.compile(r'^(.+\.[^.]+)\.part(\d+)$')

    # Pattern 3: model_00001.safetensors (no total count)
    pattern3 = re.compile(r'^(.+)_(\d{5})(\.[^.]+)$')

    sharded_groups: Dict[str, List[Path]] = {}
    standalone_files: List[Path] = []

    for file_path in files:
        filename = file_path.name

        # Try pattern 1: model-00001-of-00005.ext
        match1 = pattern1.match(filename)
        if match1:
            base_name = match1.group(1)
            current_idx = int(match1.group(2))
            total_count = int(match1.group(3))
            ext = match1.group(4)

            group_key = f"{base_name}{ext}"

            if group_key not in sharded_groups:
                sharded_groups[group_key] = []

            sharded_groups[group_key].append(file_path)
            continue

        # Try pattern 2: model.ext.part1
        match2 = pattern2.match(filename)
        if match2:
            base_name = match2.group(1)
            part_num = int(match2.group(2))

            if base_name not in sharded_groups:
                sharded_groups[base_name] = []

            sharded_groups[base_name].append(file_path)
            continue

        # Try pattern 3: model_00001.ext
        match3 = pattern3.match(filename)
        if match3:
            base_name = match3.group(1)
            shard_num = int(match3.group(2))
            ext = match3.group(3)

            group_key = f"{base_name}{ext}"

            # Only group if we see multiple files with this pattern
            if group_key not in sharded_groups:
                sharded_groups[group_key] = []

            sharded_groups[group_key].append(file_path)
            continue

        # No pattern matched - standalone file
        standalone_files.append(file_path)

    # Filter out groups with only one file (false positives)
    filtered_groups = {
        key: sorted(files_list, key=lambda p: p.name)
        for key, files_list in sharded_groups.items()
        if len(files_list) > 1
    }

    # Add standalone files back as single-item groups
    for file_path in standalone_files:
        filtered_groups[file_path.name] = [file_path]

    # Re-add single-file groups that were filtered out
    for key, files_list in sharded_groups.items():
        if len(files_list) == 1 and key not in filtered_groups:
            filtered_groups[key] = files_list

    return filtered_groups


def validate_shard_completeness(
    shard_files: List[Path],
    expected_pattern: str = "sequential"
) -> Dict[str, any]:
    """
    Validate that a sharded set is complete.

    Args:
        shard_files: List of shard files in the group
        expected_pattern: "sequential" or "indexed"

    Returns:
        {
            'complete': bool,
            'missing_shards': List[int],
            'total_expected': int,
            'total_found': int
        }
    """
    if not shard_files:
        return {
            'complete': False,
            'missing_shards': [],
            'total_expected': 0,
            'total_found': 0
        }

    # Extract shard indices from filenames
    pattern = re.compile(r'-(\d+)-of-(\d+)\.')

    indices = []
    expected_total = None

    for file_path in shard_files:
        match = pattern.search(file_path.name)
        if match:
            current_idx = int(match.group(1))
            total_count = int(match.group(2))

            indices.append(current_idx)

            if expected_total is None:
                expected_total = total_count
            elif expected_total != total_count:
                # Inconsistent total counts
                return {
                    'complete': False,
                    'missing_shards': [],
                    'total_expected': expected_total,
                    'total_found': len(indices),
                    'error': 'Inconsistent shard counts in filenames'
                }

    if expected_total is None:
        # Could not determine expected total
        return {
            'complete': True,
            'missing_shards': [],
            'total_expected': len(shard_files),
            'total_found': len(shard_files)
        }

    # Check for missing shards
    expected_indices = set(range(1, expected_total + 1))
    found_indices = set(indices)
    missing_indices = sorted(expected_indices - found_indices)

    return {
        'complete': len(missing_indices) == 0,
        'missing_shards': missing_indices,
        'total_expected': expected_total,
        'total_found': len(found_indices)
    }
```

**UI Integration**: Display sharded sets as grouped items in import dialog

```tsx
interface ShardedGroup {
  baseName: string;
  files: File[];
  validation: {
    complete: boolean;
    missing_shards: number[];
    total_expected: number;
    total_found: number;
  };
}

function ImportDialog({ files }: { files: File[] }) {
  const [shardedGroups, setShardedGroups] = useState<ShardedGroup[]>([]);

  useEffect(() => {
    // Detect sharded sets on mount
    api.detectShardedSets(files).then(setShardedGroups);
  }, [files]);

  return (
    <div className="space-y-4">
      {shardedGroups.map(group => (
        <div key={group.baseName} className="border rounded-lg p-4">
          <div className="flex items-center justify-between">
            <div>
              <h4 className="font-semibold">{group.baseName}</h4>
              <p className="text-sm text-gray-600">
                Sharded set: {group.validation.total_found} of {group.validation.total_expected} files
              </p>
            </div>

            {group.validation.complete ? (
              <Badge variant="green" icon={<CheckCircle />}>
                Complete
              </Badge>
            ) : (
              <Badge variant="yellow" icon={<AlertTriangle />}>
                Incomplete ({group.validation.missing_shards.length} missing)
              </Badge>
            )}
          </div>

          {!group.validation.complete && (
            <div className="mt-2 p-2 bg-yellow-50 rounded text-sm">
              <p className="text-yellow-800">
                Missing shards: {group.validation.missing_shards.join(', ')}
              </p>
              <button className="mt-2 text-blue-600 hover:underline">
                Download missing shards from HuggingFace
              </button>
            </div>
          )}

          <details className="mt-3">
            <summary className="cursor-pointer text-sm text-gray-600">
              View {group.files.length} files
            </summary>
            <ul className="mt-2 space-y-1 pl-4">
              {group.files.map(file => (
                <li key={file.name} className="text-sm font-mono">
                  {file.name}
                </li>
              ))}
            </ul>
          </details>
        </div>
      ))}
    </div>
  );
}
```

**Storage**: Sharded sets are stored in a single model directory with all files

```
shared-resources/models/
└── llm/llama/llama-3-70b/
    ├── metadata.json                      # References all shard files
    ├── model-00001-of-00005.safetensors
    ├── model-00002-of-00005.safetensors
    ├── model-00003-of-00005.safetensors
    ├── model-00004-of-00005.safetensors
    └── model-00005-of-00005.safetensors
```

**metadata.json for sharded sets**:

```json
{
  "model_id": "llm/llama/llama-3-70b",
  "official_name": "Llama 3 70B",
  "family": "llama",
  "model_type": "llm",
  "is_sharded_set": true,
  "files": [
    {
      "name": "model-00001-of-00005.safetensors",
      "size": 9663676416,
      "blake3_hash": "abc123...",
      "sha256_hash": "def456...",
      "shard_index": 1,
      "shard_total": 5
    },
    {
      "name": "model-00002-of-00005.safetensors",
      "size": 9663676416,
      "blake3_hash": "ghi789...",
      "sha256_hash": "jkl012...",
      "shard_index": 2,
      "shard_total": 5
    }
  ]
}
```

**Mapping Behavior**: When mapping sharded sets, all files are symlinked to the target directory

```python
def map_sharded_set(
    self,
    model_metadata: dict,
    target_dir: Path
) -> List[Path]:
    """
    Map all files in a sharded set to target directory.

    Returns:
        List of created link paths
    """
    created_links = []

    for file_entry in model_metadata['files']:
        source_path = self.library_root / model_metadata['library_path'] / file_entry['name']
        target_path = target_dir / file_entry['name']

        # Create symlink
        make_relative_symlink(source_path, target_path)

        # Register in link registry
        link_registry.register_link(
            model_id=model_metadata['model_id'],
            target_path=target_path,
            source_path=source_path,
            is_external=False,
            link_type='symlink'
        )

        created_links.append(target_path)

    return created_links
```

### Manual Match Protection & Offline Fallback

Protect user-corrected metadata from auto-overwrite during Deep Scan or retry lookups.

**Implementation**: Add to metadata schema

```python
# New metadata fields
metadata = {
    # ... existing fields ...
    'match_source': 'auto',  # 'auto' or 'manual'
    'match_method': 'hash',  # 'hash', 'filename_exact', 'filename_fuzzy'
    'match_confidence': 1.0,  # 0.0-1.0
    'pending_online_lookup': False,  # True if imported offline
    'lookup_attempts': 0,  # Number of retry attempts
    'last_lookup_attempt': None,  # ISO timestamp
}


def rebuild_index_from_metadata(self, ...) -> dict:
    """
    Deep Scan respects manual match_source.

    Models with match_source="manual" retain their metadata.
    Only models with match_source="auto" can be enriched.
    """
    for metadata_path in self.library_root.rglob("metadata.json"):
        with open(metadata_path, 'r') as f:
            metadata = json.load(f)

        # Skip HF enrichment if manually corrected
        if metadata.get('match_source') == 'manual':
            logger.debug(f"Preserving manual metadata for {metadata['model_id']}")
            # Insert as-is without HF lookup
            self._index_metadata(metadata)
        else:
            # Allow auto-enrichment for auto-matched models
            self._index_metadata(metadata)


def retry_pending_lookups(self) -> dict:
    """
    Retry HF lookup for models marked 'pending_online_lookup'.

    Respects match_source="manual" protection.
    Adds .pending_lookup hidden marker file for UI prompts.
    """
    models = self.list_models(filter={'pending_online_lookup': True})

    for metadata in models:
        model_dir = self.library_root / metadata['library_path']

        # Skip if manually corrected
        if metadata.get('match_source') == 'manual':
            logger.debug(f"Skipping manual metadata: {metadata['model_id']}")
            continue

        # Find model file
        model_files = [
            f for f in model_dir.iterdir()
            if f.suffix in ('.safetensors', '.ckpt', '.gguf', '.pt', '.bin')
        ]
        if not model_files:
            continue

        # Attempt HF lookup
        hf_metadata = self.downloader.lookup_model_metadata_by_filename(
            model_files[0].name,
            model_files[0],
            timeout=10.0
        )

        metadata_path = model_dir / "metadata.json"

        if hf_metadata:
            # Success: Merge HF data and clear pending flag
            with open(metadata_path, 'r') as f:
                current = json.load(f)

            current.update({
                'official_name': hf_metadata.get('official_name', current['official_name']),
                'tags': hf_metadata.get('tags', []),
                'download_url': hf_metadata.get('download_url'),
                'pending_online_lookup': False,
                'match_source': 'auto',  # Mark as auto-matched
                'match_method': hf_metadata.get('match_method'),
                'match_confidence': hf_metadata.get('match_confidence', 1.0),
                'last_lookup_attempt': get_iso_timestamp()
            })

            with open(metadata_path, 'w') as f:
                json.dump(current, f, indent=2, ensure_ascii=False)

            logger.info(f"Enriched metadata for {metadata['model_id']}")
        else:
            # Failed: Increment attempt counter
            with open(metadata_path, 'r') as f:
                current = json.load(f)

            current['lookup_attempts'] = current.get('lookup_attempts', 0) + 1
            current['last_lookup_attempt'] = get_iso_timestamp()

            with open(metadata_path, 'w') as f:
                json.dump(current, f, indent=2, ensure_ascii=False)

            logger.warning(f"Retry failed for {metadata['model_id']} (attempt {current['lookup_attempts']})")


def mark_metadata_as_manual(self, model_id: str) -> bool:
    """
    Mark model metadata as manually corrected.

    Prevents future Deep Scans and retries from overwriting user changes.

    Args:
        model_id: Model ID to protect

    Returns:
        True if successful
    """
    metadata = self.get_model_by_id(model_id)
    if not metadata:
        return False

    metadata_path = self.library_root / metadata['library_path'] / "metadata.json"

    with open(metadata_path, 'r') as f:
        current = json.load(f)

    current['match_source'] = 'manual'
    current['updated_date'] = get_iso_timestamp()

    with open(metadata_path, 'w') as f:
        json.dump(current, f, indent=2, ensure_ascii=False)

    logger.info(f"Marked metadata as manual for {model_id}")
    return True
```

### File Type Validation (Magic Bytes)

Validate file types using magic bytes instead of relying solely on extensions.

**Implementation**: Add to `backend/model_library/importer.py`

```python
def validate_file_type(file_path: Path) -> dict:
    """
    Validate file type using magic bytes.

    Prevents importing .txt/.html files masquerading as models.

    Returns:
        {
            'valid': bool,
            'detected_type': str,  # 'safetensors', 'gguf', 'pickle', 'unknown'
            'error': Optional[str]
        }
    """
    magic_signatures = {
        'safetensors': [b'{"'],  # Safetensors starts with JSON header
        'gguf': [b'GGUF'],  # GGUF magic number
        'pickle': [b'\x80\x02', b'\x80\x03', b'\x80\x04', b'\x80\x05'],  # Pickle protocols 2-5
    }

    try:
        with open(file_path, 'rb') as f:
            header = f.read(16)

        # Check for safetensors JSON header
        if header.startswith(b'{"'):
            return {'valid': True, 'detected_type': 'safetensors', 'error': None}

        # Check for GGUF
        if header.startswith(b'GGUF'):
            return {'valid': True, 'detected_type': 'gguf', 'error': None}

        # Check for PyTorch pickle
        for sig in magic_signatures['pickle']:
            if header.startswith(sig):
                return {'valid': True, 'detected_type': 'pickle', 'error': None}

        # Unknown/invalid
        return {
            'valid': False,
            'detected_type': 'unknown',
            'error': f"Unrecognized file format. Header: {header[:8].hex()}"
        }

    except Exception as e:
        return {'valid': False, 'detected_type': 'error', 'error': str(e)}
```

### Backend API

**File**: `backend/api/core.py`

```python
def lookup_hf_metadata_for_file(self, filename: str, file_path: Optional[str] = None) -> dict:
    """
    Look up HuggingFace metadata for a given filename.

    Args:
        filename: Name of the model file
        file_path: Optional path to local file for hash verification

    Returns:
        dict with success, found, metadata, or error
    """
    try:
        path = Path(file_path) if file_path else None

        metadata = self.downloader.lookup_model_metadata_by_filename(
            filename,
            path,
            timeout=5.0
        )

        if metadata:
            return {
                'success': True,
                'found': True,
                'metadata': metadata,
            }
        else:
            return {
                'success': True,
                'found': False,
                'metadata': None,
            }
    except Exception as e:
        logger.error(f"Error looking up metadata: {e}")
        return {
            'success': False,
            'found': False,
            'error': str(e),
        }


def import_model_batch(
    self,
    files: List[dict],
    progress_callback: Optional[Callable] = None
) -> List[dict]:
    """
    Import multiple models in a batch with deferred sync.

    Args:
        files: List of {file_path, hf_metadata, user_overrides}
        progress_callback: Optional progress callback

    Returns:
        List of import results
    """
    results = []

    with ImportBatchContext(self):
        for i, file_data in enumerate(files):
            if progress_callback:
                progress_callback({
                    'stage': 'importing',
                    'current': i + 1,
                    'total': len(files),
                    'file': Path(file_data['file_path']).name
                })

            # Import without auto-sync
            result = self._import_single_model(
                file_path=file_data['file_path'],
                hf_metadata=file_data.get('hf_metadata'),
                user_overrides=file_data.get('user_overrides', {}),
                skip_sync=True  # Deferred to end of batch
            )
            results.append(result)

        # Sync once after all imports complete
        if progress_callback:
            progress_callback({
                'stage': 'syncing',
                'current': len(files),
                'total': len(files),
                'message': 'Syncing library with installed apps...'
            })

    return results


def _import_single_model(
    self,
    file_path: str,
    hf_metadata: Optional[dict],
    user_overrides: dict,
    skip_sync: bool = False
) -> dict:
    """
    Import a single model file.

    Args:
        file_path: Absolute path to the model file
        hf_metadata: Optional HF metadata from lookup
        user_overrides: User-specified values
        skip_sync: If True, skip auto-sync (for batching)

    Returns:
        dict with success, model_path, model_id, or error
    """
    try:
        # Merge metadata with user overrides
        family = user_overrides.get('family') or (hf_metadata.get('family') if hf_metadata else 'unknown')
        name = user_overrides.get('name') or (hf_metadata.get('official_name') if hf_metadata else Path(file_path).stem)
        repo_id = hf_metadata.get('repo_id') if hf_metadata else None

        # Import the file
        result = self.importer.import_model(
            source_path=file_path,
            family=family,
            name=name,
            repo_id=repo_id,
            model_type=user_overrides.get('model_type'),
            subtype=user_overrides.get('subtype'),
            tags=user_overrides.get('tags', []),
            notes=user_overrides.get('notes', ''),
            use_move=user_overrides.get('use_move', False),
        )

        # Refresh model index
        self.library.refresh_index()

        # Incremental sync if not in batch mode
        if not skip_sync:
            self._auto_sync_all_apps_incremental(
                model_ids=[result['model_id']]
            )

        return {
            'success': True,
            'model_path': result['model_path'],
            'model_id': result['model_id'],
            'hash_verified': True,
        }
    except Exception as e:
        logger.error(f"Error importing model: {e}")
        return {
            'success': False,
            'error': str(e),
            'hash_verified': False,
        }


def check_files_writable(self, file_paths: List[str]) -> dict:
    """
    Check if files can be safely deleted.

    Returns:
        {
            'all_writable': bool,
            'details': List[dict]  # Per-file writability status
        }
    """
    details = []
    all_writable = True

    for file_path in file_paths:
        try:
            path = Path(file_path)
            writable = path.exists() and os.access(path.parent, os.W_OK)

            details.append({
                'path': str(path),
                'writable': writable,
                'reason': None if writable else 'Read-only filesystem or no write permission'
            })

            if not writable:
                all_writable = False

        except Exception as e:
            details.append({
                'path': file_path,
                'writable': False,
                'reason': str(e)
            })
            all_writable = False

    return {
        'all_writable': all_writable,
        'details': details
    }


def delete_source_files_safely(
    self,
    import_results: List[dict],
    source_files: List[str]
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
    return self.importer.delete_source_files_safely(import_results, source_files)


def get_library_status(self) -> dict:
    """
    Get current library status, including indexing state.

    Returns:
        {
            'success': bool,
            'indexing': bool,
            'deep_scan_progress': {
                'current': int,
                'total': int,
                'stage': str
            } | None
        }
    """
    try:
        # Check if Deep Scan is in progress
        is_indexing = self.library.is_deep_scan_in_progress()
        progress = None

        if is_indexing:
            progress = self.library.get_deep_scan_progress()

        return {
            'success': True,
            'indexing': is_indexing,
            'deep_scan_progress': progress
        }
    except Exception as e:
        logger.error(f"Error getting library status: {e}")
        return {
            'success': False,
            'error': str(e),
            'indexing': False
        }


def mark_metadata_as_manual(self, model_id: str) -> dict:
    """
    Mark model metadata as manually corrected to protect from auto-updates.

    Args:
        model_id: Model ID to protect

    Returns:
        {
            'success': bool,
            'error': str | None
        }
    """
    try:
        success = self.library.mark_metadata_as_manual(model_id)

        if success:
            return {'success': True}
        else:
            return {'success': False, 'error': 'Model not found'}
    except Exception as e:
        logger.error(f"Error marking metadata as manual: {e}")
        return {'success': False, 'error': str(e)}
```

---

## Frontend Implementation

### Import Dialog Steps

#### Step 1: HuggingFace Metadata Lookup

```tsx
<div className="space-y-4">
  <h3>Looking up models on HuggingFace...</h3>

  {files.map((file, idx) => (
    <div key={idx} className="flex items-center gap-3 p-3 bg-[hsl(var(--surface-mid))] rounded">
      <FileIcon className="w-5 h-5" />
      <span className="flex-1 text-sm">{file.name}</span>

      {lookupStatus[file.name] === 'searching' && (
        <Loader2 className="w-4 h-4 animate-spin text-[hsl(var(--launcher-accent-info))]" />
      )}

      {lookupStatus[file.name] === 'found' && (
        <Check className="w-4 h-4 text-[hsl(var(--accent-success))]" />
      )}

      {lookupStatus[file.name] === 'not-found' && (
        <AlertCircle className="w-4 h-4 text-[hsl(var(--accent-warning))]" />
      )}
    </div>
  ))}
</div>
```

#### Step 2: Metadata Review & Import Options

See the extensive UI mockup in the original document's "Step 2: Metadata Review & File Matching" section, which includes:

- Trust badges (Verified, High Confidence, Low Confidence, Modified Version, No Match)
- Match details (hash match, filename match, confidence percentage)
- Related files from HuggingFace with download options
- Editable metadata fields (family, type, tags, notes)
- Progressive disclosure of technical details (destination path, hashes, repo ID)
- Import mode selection (Fast Move vs Safe Copy)
- Optional "Delete originals after import" checkbox with safety validation

**"Mark as Manual" Protection**:

When user edits metadata fields (especially for low-confidence matches), show a button to protect their changes:

```tsx
{(hfData.match_method === 'filename_fuzzy' || userHasEditedFields) && (
  <div className="mt-3 p-3 bg-[hsl(var(--surface-low))] border border-[hsl(var(--border-default))] rounded">
    <div className="flex items-start gap-2">
      <Shield className="w-4 h-4 text-[hsl(var(--launcher-accent-info))] mt-0.5" />
      <div className="flex-1">
        <p className="text-xs text-[hsl(var(--text-secondary))]">
          Protect your edits from future auto-updates by marking this as manually verified.
        </p>
        <button
          onClick={() => markAsManual(file.name)}
          className="mt-2 text-xs px-2 py-1 border border-[hsl(var(--launcher-accent-info))] rounded hover:bg-[hsl(var(--surface-interactive-hover))]"
        >
          Mark as Manual (Protect from Auto-Update)
        </button>
      </div>
    </div>
  </div>
)}
```

This sets `match_source: 'manual'` in metadata, preventing Deep Scan and retry lookups from overwriting user corrections.

#### Step 3: Importing Progress

```tsx
<div className="space-y-4">
  <div className="flex items-center gap-3">
    <Loader2 className="w-6 h-6 animate-spin" />
    <h3 className="text-lg font-semibold">Importing models...</h3>
  </div>

  {importProgress.map((fileProgress) => (
    <div key={fileProgress.filename} className="space-y-2">
      <div className="flex items-center justify-between text-sm">
        <span className="flex-1 truncate">{fileProgress.filename}</span>
        <span className="flex-shrink-0 ml-3 text-[hsl(var(--text-secondary))]">
          {/* Granular status display */}
          {fileProgress.stage === 'copying' && `Copying... (${fileProgress.progress}%)`}
          {fileProgress.stage === 'hashing' && 'Computing hashes...'}
          {fileProgress.stage === 'writing_metadata' && 'Writing metadata...'}
          {fileProgress.stage === 'indexing' && 'Indexing...'}
          {fileProgress.stage === 'syncing' && 'Syncing to apps...'}
          {fileProgress.stage === 'complete' && '✓ Complete'}
          {fileProgress.stage === 'error' && `✗ ${fileProgress.error}`}
        </span>
      </div>
      <div className="h-2 bg-[hsl(var(--surface-mid))] rounded-full overflow-hidden">
        <motion.div
          initial={{ width: 0 }}
          animate={{ width: `${fileProgress.progress}%` }}
          transition={{ duration: 0.3, ease: "easeOut" }}
          className={`h-full ${
            fileProgress.stage === 'error'
              ? 'bg-[hsl(var(--accent-error))]'
              : fileProgress.stage === 'complete'
                ? 'bg-[hsl(var(--accent-success))]'
                : 'bg-[hsl(var(--launcher-accent-primary))]'
          }`}
        />
      </div>
    </div>
  ))}
</div>
```

#### Step 4: Complete

```tsx
<div className="flex flex-col items-center justify-center py-8">
  <motion.div
    initial={{ scale: 0 }}
    animate={{ scale: 1 }}
    transition={{ type: "spring", stiffness: 200 }}
  >
    <Check className="w-16 h-16 text-[hsl(var(--accent-success))]" />
  </motion.div>

  <h3 className="text-lg font-semibold mt-4">Import Complete!</h3>
  <p className="text-sm text-[hsl(var(--text-secondary))] mt-2">
    {successCount} {successCount === 1 ? 'model' : 'models'} imported successfully
  </p>

  {failedCount > 0 && (
    <div className="mt-4 p-3 bg-[hsl(var(--accent-error)/0.1)] border border-[hsl(var(--accent-error))] rounded">
      <p className="text-sm text-[hsl(var(--accent-error))]">
        {failedCount} {failedCount === 1 ? 'model' : 'models'} failed to import
      </p>
    </div>
  )}

  <button onClick={onClose} className="mt-6 px-4 py-2 rounded bg-[hsl(var(--launcher-accent-primary))] text-black font-semibold">
    Done
  </button>
</div>
```

### ImportAPI Class (New File)

**File**: `frontend/src/api/import.ts`

Create a separate API class for import operations to keep concerns separated from `ModelsAPI.ts`:

```typescript
import { APIError } from '../errors';
import type {
  HFMetadataLookupResponse,
  ModelImportRequest,
  ModelImportResponse,
  LibraryStatusResponse
} from '../types/pywebview';

class ImportAPI {
  private getAPI() {
    if (!window.pywebview?.api) {
      throw new APIError('PyWebView API not available');
    }
    return window.pywebview.api;
  }

  async lookupHFMetadata(filename: string, filePath?: string): Promise<HFMetadataLookupResponse> {
    const api = this.getAPI();
    return await api.lookup_hf_metadata_for_file(filename, filePath);
  }

  async importModelBatch(
    files: ModelImportRequest[],
    progressCallback?: (progress: any) => void
  ): Promise<ModelImportResponse[]> {
    const api = this.getAPI();
    return await api.import_model_batch(files, progressCallback);
  }

  async checkFilesWritable(filePaths: string[]): Promise<{
    all_writable: boolean;
    details: Array<{ path: string; writable: boolean; reason?: string }>;
  }> {
    const api = this.getAPI();
    return await api.check_files_writable(filePaths);
  }

  async deleteSourceFiles(
    importResults: ModelImportResponse[],
    sourceFiles: string[]
  ): Promise<{ success: boolean; deleted_count: number; errors: string[] }> {
    const api = this.getAPI();
    return await api.delete_source_files_safely(importResults, sourceFiles);
  }

  async getFileLinkCount(filePath: string): Promise<number> {
    const api = this.getAPI();
    return await api.get_file_link_count(filePath);
  }

  async getLibraryStatus(): Promise<LibraryStatusResponse> {
    const api = this.getAPI();
    return await api.get_library_status();
  }
}

export const importAPI = new ImportAPI();
```

**Rationale**: Separates import workflow from model management. `ModelsAPI.ts` remains focused on model listing, searching HuggingFace, and downloads. `ImportAPI.ts` handles the drag-and-drop import workflow.

---

### TypeScript Types

**File**: `frontend/src/types/pywebview.d.ts`

```typescript
// ============================================================================
// Library Status Types (NEW)
// ============================================================================

export interface LibraryStatusResponse extends BaseResponse {
  indexing: boolean;
  deep_scan_progress?: {
    current: number;
    total: number;
    stage: 'scanning' | 'indexing' | 'complete';
  };
}

// ============================================================================
// Model Import Types
// ============================================================================

export interface HFMetadataLookupResponse extends BaseResponse {
  found: boolean;
  metadata?: {
    repo_id: string;
    official_name: string;
    family: string;
    model_type: string;
    subtype: string;
    variant: string;
    precision: string;
    tags: string[];
    base_model?: string;
    download_url: string;
    files: Array<{
      filename: string;
      size: number | null;
    }>;
    preview_image?: string;
    model_card: Record<string, unknown>;
    cleaned_name: string;
    match_confidence: number;
    match_method: 'hash' | 'filename_exact' | 'filename_fuzzy';
    requires_confirmation: boolean;
    hash_mismatch: boolean;
    matched_filename?: string;
  };
}

export interface ModelImportRequest {
  file_path: string;
  hf_metadata: HFMetadataLookupResponse['metadata'] | null;
  user_overrides: {
    family?: string;
    name?: string;
    model_type?: string;
    subtype?: string;
    tags?: string[];
    notes?: string;
    use_move?: boolean;
  };
}

export interface ModelImportResponse extends BaseResponse {
  model_path?: string;
  model_id?: string;
  hash_verified?: boolean;
}

// Update PyWebViewAPI interface
export interface PyWebViewAPI {
  // ... existing methods

  lookup_hf_metadata_for_file(
    filename: string,
    file_path?: string
  ): Promise<HFMetadataLookupResponse>;

  import_model_batch(
    files: ModelImportRequest[],
    progress_callback?: (progress: {
      stage: 'importing' | 'syncing';
      current: number;
      total: number;
      file?: string;
      message?: string;
    }) => void
  ): Promise<ModelImportResponse[]>;

  check_files_writable(
    file_paths: string[]
  ): Promise<{
    all_writable: boolean;
    details: Array<{
      path: string;
      writable: boolean;
      reason?: string;
    }>;
  }>;

  delete_source_files_safely(
    import_results: ModelImportResponse[],
    source_files: string[]
  ): Promise<{
    success: boolean;
    deleted_count: number;
    errors: string[];
  }>;

  get_file_link_count(file_path: string): Promise<number>;

  get_library_status(): Promise<LibraryStatusResponse>;

  mark_metadata_as_manual(model_id: string): Promise<BaseResponse>;
}
```

### Hard Link Deletion Warning (NTFS)

For NTFS filesystems using hard links, warn users when deleting models that space won't be freed if other links exist.

**Implementation**: Add to delete confirmation dialog

```tsx
// In delete model dialog
const [linkCount, setLinkCount] = useState<number | null>(null);

useEffect(() => {
  // Get link count from file stat
  if (modelToDelete) {
    window.pywebview.api.get_file_link_count(modelToDelete.file_path).then(count => {
      setLinkCount(count);
    });
  }
}, [modelToDelete]);

// In dialog body
{linkCount && linkCount > 1 && (
  <div className="bg-[hsl(var(--accent-warning)/0.1)] border-l-4 border-[hsl(var(--accent-warning))] p-3 mb-4">
    <div className="flex items-start gap-2">
      <AlertTriangle className="w-4 h-4 text-[hsl(var(--accent-warning))] mt-0.5" />
      <div className="flex-1 text-xs">
        <p className="font-semibold mb-1">File Has Multiple Hard Links</p>
        <p className="text-[hsl(var(--text-secondary))]">
          This file has {linkCount} hard links. Deleting it from the library will not free up disk space
          until all {linkCount - 1} other link(s) are also removed.
        </p>
        <p className="text-[hsl(var(--text-secondary))] mt-2">
          Linked locations: ComfyUI model folders (NTFS hard links)
        </p>
      </div>
    </div>
  </div>
)}
```

**Backend API**:

```python
def get_file_link_count(self, file_path: str) -> int:
    """
    Get number of hard links for a file.

    Returns:
        st_nlink count (1 = no other links, >1 = has hard links)
    """
    try:
        path = Path(file_path)
        return path.stat().st_nlink
    except Exception as e:
        logger.error(f"Failed to get link count: {e}")
        return 1  # Safe default
```

### Integration

**Update**: `frontend/src/components/ModelManager.tsx`

```tsx
export const ModelManager: React.FC<ModelManagerProps> = ({
  // ... existing props
}) => {
  const [showImportDialog, setShowImportDialog] = useState(false);
  const [droppedFiles, setDroppedFiles] = useState<File[]>([]);

  const handleFileDrop = (files: File[]) => {
    setDroppedFiles(files);
    setShowImportDialog(true);
  };

  const handleImportComplete = () => {
    setShowImportDialog(false);
    setDroppedFiles([]);
    onAddModels?.();
  };

  return (
    <div className="flex-1 bg-[hsl(var(--launcher-bg-tertiary)/0.2)] overflow-hidden flex flex-col relative">
      <ModelImportDropZone
        onFileDrop={handleFileDrop}
        isActive={!showImportDialog}
      />

      <ModelSearchBar ... />

      <div className="flex-1 overflow-y-auto">
        {/* ... existing content */}
      </div>

      <AnimatePresence>
        {showImportDialog && (
          <ModelImportDialog
            isOpen={showImportDialog}
            onClose={() => setShowImportDialog(false)}
            files={droppedFiles}
            onImportComplete={handleImportComplete}
          />
        )}
      </AnimatePresence>
    </div>
  );
};
```

---

## Import Flow

### Complete Flow Diagram

```
User drops files
      ↓
ModelImportDropZone captures files
      ↓
ModelImportDialog opens
      ↓
Step 1: HF Metadata Lookup (with throttling)
  ├─→ For each file:
  │   ├─→ Filename search (1-2 API calls)
  │   ├─→ Hash verification (top 2 candidates, 2 API calls max)
  │   └─→ Fuzzy fallback if no hash match
      ↓
Step 2: Metadata Review
  ├─→ User sees trust badges
  ├─→ User reviews/edits metadata
  ├─→ User selects import mode (move vs copy)
  └─→ User optionally enables "delete originals"
      ↓
Step 3: Batch Import (atomic, with progress)
  ├─→ For each file:
  │   ├─→ Copy/Move to .tmp location
  │   ├─→ Stream hashing (BLAKE3 + SHA256)
  │   ├─→ Write metadata.json
  │   ├─→ Atomic rename .tmp → final
  │   └─→ Update SQLite index
  ├─→ WAL checkpoint
  └─→ Incremental sync to installed apps
      ↓
Step 4: Optional Delete Originals
  ├─→ Safety checks (all succeeded, hashes verified, writable)
  └─→ Delete source files
      ↓
Step 5: Complete
  └─→ Show success/failure summary
```

---

## Testing Strategy

### Unit Tests

- [ ] HuggingFace exact match search
- [ ] HuggingFace fuzzy fallback search
- [ ] Hash verification against candidate repos
- [ ] Filename cleaning (remove quants, versions)
- [ ] Metadata extraction from repo
- [ ] File type detection
- [ ] Variant and precision inference
- [ ] Sharded set detection: Pattern 1 (model-00001-of-00005.safetensors)
- [ ] Sharded set detection: Pattern 2 (model.safetensors.part1)
- [ ] Sharded set detection: Pattern 3 (model_00001.safetensors)
- [ ] Shard completeness validation
- [ ] Shard completeness: Detect missing shards
- [ ] Shard completeness: Detect inconsistent totals

### Integration Tests

- [ ] Drop single .safetensors file
- [ ] Drop multiple files at once
- [ ] Drop folder containing model files
- [ ] Drop unsupported file type (should reject)
- [ ] HF metadata found (hash match)
- [ ] HF metadata found (filename match)
- [ ] HF metadata not found (manual entry)
- [ ] Name collision handling
- [ ] Hash computation accuracy
- [ ] Multi-file model import
- [ ] Sharded set import: Drop all 5 shards → Group as one model
- [ ] Sharded set import: Drop incomplete set → Warning shown, allow partial import
- [ ] Sharded set import: Missing shard download from HuggingFace
- [ ] Sharded set mapping: All shards symlinked to target directory
- [ ] Sharded set deletion: Cascade delete all shards and links
- [ ] Atomic import: Crash during copy leaves no partial files
- [ ] Stream hashing: Hash matches separate hash computation
- [ ] Offline import: Works without network connection
- [ ] Incremental sync after import: Models appear in installed apps
- [ ] SQLite WAL mode: Concurrent read/write operations
- [ ] Files copied (not moved) by default
- [ ] Fast import (move) works on same filesystem
- [ ] Delete originals: Safety checks prevent deletion on errors

### UI Tests

- [ ] Drop zone appears on drag
- [ ] Drop zone animates correctly
- [ ] Import dialog shows metadata
- [ ] Related files displayed correctly
- [ ] Download missing files works
- [ ] Edit metadata fields
- [ ] Import progress tracking (granular stages)
- [ ] Error handling (disk full, permission denied)
- [ ] Trust badges display correctly (Verified, High Confidence, Low Confidence, Modified, No Match)
- [ ] Progressive disclosure: Technical details expand/collapse
- [ ] Granular progress states: Copying → Hashing → Indexing → Syncing → Complete
- [ ] Offline mode: Import proceeds without HF lookup
- [ ] Import mode selection: Move vs Copy
- [ ] Delete originals checkbox: Disabled when files not writable
- [ ] Sharded set grouping: Multiple files grouped as one model in UI
- [ ] Sharded set badge: "Complete" shown for full sets
- [ ] Sharded set badge: "Incomplete (X missing)" shown for partial sets
- [ ] Sharded set details: Expandable list of shard files
- [ ] Sharded set download: "Download missing shards" button appears for incomplete sets

---

**End of Model Import Document**
