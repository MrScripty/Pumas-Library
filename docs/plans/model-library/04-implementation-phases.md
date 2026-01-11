# Implementation Phases

**Version**: 3.2

---

## Table of Contents

- [Overview](#overview)
- [Phase 1: Core Infrastructure](#phase-1-core-infrastructure)
  - [Part A: Model Import System](#part-a-model-import-system)
  - [Part B: Link Registry Database](#part-b-link-registry-database)
  - [Part C: Basic Link Mapping System](#part-c-basic-link-mapping-system)
  - [Part D: Platform Abstraction Layer](#part-d-platform-abstraction-layer) *(v3.2)*
  - [Part E: Pickle Security Warnings](#part-e-pickle-security-warnings) *(v3.2)*
  - [Part F: Circuit Breaker Networking](#part-f-circuit-breaker-networking) *(v3.2)*
- [Phase 2: Reliability & Self-Healing](#phase-2-reliability--self-healing)
  - [Part A: Deep Scan File Verification](#part-a-deep-scan-file-verification)
  - [Part B: Link Self-Healing](#part-b-link-self-healing) *(v3.2)*
- [Phase 3: UX Polish & Conflict Resolution](#phase-3-ux-polish--conflict-resolution)
  - [Part A: Sharded Set Grouping](#part-a-sharded-set-grouping)
  - [Part B: Interactive Conflict Resolution](#part-b-interactive-conflict-resolution) *(v3.2)*
  - [Part C: Drive/Mount-Point Relocation Helper](#part-c-drivemount-point-relocation-helper)
- [Phase 4: Performance & Scale](#phase-4-performance--scale) *(v3.2 - formerly Standardization)*
  - [Part A: FTS5 Full-Text Search](#part-a-fts5-full-text-search)
  - [Part B: HTTP/2 Multiplexing](#part-b-http2-multiplexing)
  - [Part C: Stale-While-Revalidate](#part-c-stale-while-revalidate)
  - [Part D: Standardization](#part-d-standardization)
- [Phase 5: Mapping UI (Future)](#phase-5-mapping-ui-future)
- [File Checklist](#file-checklist)

---

## Overview

This document outlines the concrete implementation steps for the Model Library System. Phases are organized by priority based on user value and system stability requirements.

### Development Approach

**Phase 1: Core Infrastructure** - Foundation features that enable basic functionality
- Model Import System (drag-and-drop with metadata)
- Link Registry Database (registry.db)
- Link Mapping System (basic symlink creation)
- Platform Abstraction Layer (Windows-ready design) *(v3.2)*
- Pickle Security Warnings (safetensors first) *(v3.2)*
- Circuit Breaker Networking (async httpx, 7s timeout, fail-fast) *(v3.2)*

**Phase 2: Reliability & Self-Healing** - Features that ensure system correctness
- Deep Scan File Verification (prevent phantom models)
- Link Self-Healing (auto-repair broken symlinks by hash) *(v3.2)*

**Phase 3: UX Polish & Conflict Resolution** - Features that improve usability and workflow
- Sharded Set Grouping (UI + importer)
- Interactive Conflict Resolution (overwrite/rename/skip dialog) *(v3.2)*
- Drive/Mount-Point Relocation Helper

**Phase 4: Performance & Scale** - Optimizations for large libraries *(v3.2 - moved from Phase 1)*
- FTS5 Full-Text Search (sub-20ms queries for 1000+ models)
- HTTP/2 Multiplexing (concurrent requests over single TCP)
- Stale-While-Revalidate (instant UI with background refresh)
- Industry Terminology Standardization

### Implementation Priority Rationale

1. **Phase 1 Core** must come first - nothing works without it
2. **Phase 2 Reliability** prevents data loss and auto-repairs issues - critical for trust
3. **Phase 3 UX** improves workflow with interactive conflict handling
4. **Phase 4 Performance** scales for power users with 1000+ models (deferred from MVP)

### Phase 1 Internal Ordering (Recommended)

Based on feedback, Phase 1 parts should be implemented in this order:

1. **Part A: Model Import System** - Core I/O and file handling (FIRST)
2. **Part B: Link Registry Database** - Tracking infrastructure (can parallel with A)
3. **Part C: Basic Link Mapping System** - Depends on A and B
4. **Part D: Platform Abstraction Layer** - Can be done early, low effort
5. **Part E: Pickle Security Warnings** - Depends on A, lower priority
6. **Part F: Circuit Breaker Networking** - Depends on A, lower priority

**Rationale**: The core I/O and linking logic (Parts A-D) must work flawlessly before adding network resilience (Part F) and security warnings (Part E). If local linking fails, network circuit breakers don't matter.

---

## Phase 1: Core Infrastructure

### Part A: Model Import System

**Goal**: Enable drag-and-drop model import with HuggingFace metadata lookup

**Status**: Ready to implement
**Estimated Complexity**: Medium-High
**Dependencies**: None (can start immediately)

### Tasks

#### Backend Implementation

1. **Create I/O Manager** (`backend/model_library/io_manager.py`)
   - [ ] Implement `IOManager` class with drive detection
   - [ ] Add NVMe device name extraction (`_extract_base_device()`)
   - [ ] Add LVM/LUKS device detection with slave resolution
   - [ ] Add drive type detection (SSD vs HDD via `/sys/block/`)
   - [ ] Create drive-aware semaphores (2 for SSD, 1 for HDD)
   - [ ] Add global `io_manager` instance
   - [ ] Test on SSD, HDD, NVMe, and LVM/LUKS setups

2. **Create Filesystem Validator** (`backend/model_library/fs_validator.py`)
   - [ ] Implement `FilesystemValidator` class
   - [ ] Add `validate_import_source()` method
   - [ ] Add `validate_mapping_target()` method
   - [ ] Add NTFS dirty bit detection
   - [ ] Add read-only mount detection
   - [ ] Add cross-filesystem detection
   - [ ] Add sandbox detection (Flatpak/Snap/Docker)
   - [ ] Create global `fs_validator` instance

3. **Update NTFS Filename Normalization** (`backend/model_library/naming.py`)
   - [ ] Add `sanitize_filename_for_ntfs()` function
   - [ ] Add `normalize_model_directory_name()` function
   - [ ] Add `resolve_ntfs_collision()` function with hash-based deduplication
   - [ ] Add `get_safe_import_path()` function
   - [ ] Test with forbidden characters (`:`, `*`, `?`, `"`, `<`, `>`, `|`)
   - [ ] Test with reserved names (CON, PRN, AUX, etc.)
   - [ ] Test with trailing dots and spaces
   - [ ] Test collision detection (e.g., `Model:A` and `Model*A` → `Model_A` and `Model_A-8f2a`)

4. **Update Importer** (`backend/model_library/importer.py`)
   - [ ] Add `copy_and_hash()` function for stream hashing
   - [ ] Add `_compute_hashes_in_place()` for move operations
   - [ ] Add `_compute_directory_hashes()` for Diffusers folders
   - [ ] Add `validate_file_type()` using magic bytes (safetensors, GGUF, pickle)
   - [ ] Update `import_model()` to use atomic `.tmp` operations
   - [ ] Add support for both files and directories
   - [ ] Add fast import (move) with automatic `st_dev` check
   - [ ] Auto-switch to copy mode if cross-filesystem move attempted
   - [ ] Add `delete_source_files_safely()` method
   - [ ] Integrate I/O manager for drive-aware queueing
   - [ ] Integrate filesystem validator
   - [ ] Integrate NTFS filename normalization and collision resolution
   - [ ] Add relative symlink helper: `make_relative_symlink()`

5. **Update Library Manager** (`backend/model_library/library.py`)
   - [ ] Enable SQLite WAL mode in `_initialize_database()`
   - [ ] Add `checkpoint_wal()` method
   - [ ] Add aggressive checkpointing every 5 models during batch imports
   - [ ] Add `rebuild_index_from_metadata()` (Deep Scan) with `match_source` protection
   - [ ] Add `retry_pending_lookups()` for offline imports with `match_source` protection
   - [ ] Add `mark_metadata_as_manual()` to protect user-corrected metadata
   - [ ] Add `is_deep_scan_in_progress()` method for status checks
   - [ ] Add `get_deep_scan_progress()` method returning current/total/stage
   - [ ] Add periodic WAL checkpointing (daily at 3:00 AM)
   - [ ] Respect `match_source="manual"` in Deep Scan (no auto-enrichment)

6. **Update Downloader** (`backend/model_library/downloader.py`)
   - [ ] Create `HFAPIThrottle` class for rate limiting
   - [ ] Add `compute_fast_hash()` function (first 8MB + last 8MB)
   - [ ] Add `lookup_model_metadata_by_filename()` with fast hash + stream verification
   - [ ] Add `_verify_hash_single_candidate()` for hash verification
   - [ ] Add `_compute_sha256()` method
   - [ ] Add `_find_best_filename_match()` with confidence scoring
   - [ ] Add `_get_lfs_files_cached()` with 24-hour cache
   - [ ] Add `_infer_variant_and_precision()` for metadata enrichment
   - [ ] Add 5-second timeout for API calls
   - [ ] Integrate global `hf_throttle` instance
   - [ ] Mark matches as `pending_full_verification` for stream hash verification

7. **Update API Core** (`backend/api/core.py`)
   - [ ] Add `lookup_hf_metadata_for_file()` method
   - [ ] Add `import_model_batch()` method with deferred sync and periodic WAL checkpoints
   - [ ] Add `_import_single_model()` method
   - [ ] Add `check_files_writable()` method
   - [ ] Add `delete_source_files_safely()` method
   - [ ] Add `get_file_link_count()` method for hard link warnings
   - [ ] Add `get_library_status()` method for indexing state
   - [ ] Add `mark_metadata_as_manual()` method
   - [ ] Add `_auto_sync_all_apps_incremental()` method
   - [ ] Create `ImportBatchContext` class for batch operations

#### Frontend Implementation

8. **Update TypeScript Types FIRST** (`frontend/src/types/pywebview.d.ts`) ⭐ **PRIORITY #1**
   - [ ] Add `LibraryStatusResponse` interface for indexing state
   - [ ] Add `HFMetadataLookupResponse` interface
   - [ ] Add `ModelImportRequest` interface
   - [ ] Add `ModelImportResponse` interface
   - [ ] Add match method types ('hash', 'filename_exact', 'filename_fuzzy')
   - [ ] Add import stage types ('copying', 'hashing', 'writing_metadata', 'indexing', 'syncing')
   - [ ] Update `PyWebViewAPI` interface with new methods:
     - `lookup_hf_metadata_for_file()`
     - `import_model_batch()`
     - `check_files_writable()`
     - `delete_source_files_safely()`
     - `get_file_link_count()`
     - `get_library_status()`
     - `mark_metadata_as_manual()`
   - [ ] Document in PR: "Types-first approach for type safety"

9. **Create ImportAPI Class** (`frontend/src/api/import.ts`)
   - [ ] Create separate API class for import operations
   - [ ] Add `lookupHFMetadata()` method
   - [ ] Add `importModelBatch()` method
   - [ ] Add `checkFilesWritable()` method
   - [ ] Add `deleteSourceFiles()` method
   - [ ] Add `getFileLinkCount()` method
   - [ ] Add `getLibraryStatus()` method
   - [ ] Export singleton instance
   - [ ] Keep `ModelsAPI.ts` focused on model listing/downloads

10. **Create Drop Zone Component** (`frontend/src/components/ModelImportDropZone.tsx`)
    - [ ] Implement window-level drag event listeners with `preventDefault()`
    - [ ] Add `fileUriToPath()` utility for PyWebView GTK/WebKit URI conversion
    - [ ] Add `extractFilePaths()` to handle both File API and `text/uri-list`
    - [ ] Add file type validation (.safetensors, .ckpt, .gguf, .pt, .bin)
    - [ ] Add backdrop blur animation
    - [ ] Add pulsing border animation (CSS)
    - [ ] Handle multi-file and folder drops
    - [ ] Use drag counter for nested element handling
    - [ ] CRITICAL: Use `e.preventDefault()` on both `dragOver` and `drop` events
    - [ ] Test with Nautilus, Dolphin, and Thunar file managers

11. **Create Import Dialog Component** (`frontend/src/components/ModelImportDialog.tsx`)
    - [ ] Create multi-step wizard structure
    - [ ] Implement Step 1: HF Metadata Lookup with progress indicators
    - [ ] Implement Step 2: Metadata Review with trust badges
    - [ ] Add "Mark as Manual" protection UI for user-edited metadata
    - [ ] Add progressive disclosure for technical details
    - [ ] Add related files display with download option
    - [ ] Add editable metadata fields (family, type, tags, notes)
    - [ ] Add import mode selection (Fast Move vs Safe Copy)
    - [ ] Add "Delete originals" checkbox with safety validation
    - [ ] Add hard link count warning for NTFS (if `st_nlink > 1`)
    - [ ] Implement Step 3: Importing Progress with granular stages (callback-based, not polling)
    - [ ] Implement Step 4: Complete with success/failure summary

12. **Update CSS Animations** (`frontend/src/index.css`)
    - [ ] Add `@keyframes pulse-border` animation
    - [ ] Add `.animate-pulse-border` class

13. **Update useModels Hook** (`frontend/src/hooks/useModels.ts`)
    - [ ] Check library status before rendering model list
    - [ ] Show "Indexing..." overlay when `status.indexing === true`
    - [ ] Display Deep Scan progress if available
    - [ ] Clarify that 10-second polling is for external changes only

14. **Update Model Manager** (`frontend/src/components/ModelManager.tsx`)
    - [ ] Import and integrate `ModelImportDropZone`
    - [ ] Import and integrate `ModelImportDialog`
    - [ ] Add state for dropped files
    - [ ] Add handlers for file drop and import complete
    - [ ] Wrap components with `AnimatePresence`

15. **Add List Virtualization** (`frontend/src/components/ModelManager.tsx`)
    - [ ] Install `react-window` or `@tanstack/react-virtual`
    - [ ] Wrap model list in VirtualList component
    - [ ] Optimize ModelCard component (memoization, lightweight rendering)
    - [ ] Test with 500+ models to verify FPS
    - [ ] Add to dependencies: `npm install react-window` or `@tanstack/react-virtual`

16. **Update Settings Component** (`frontend/src/components/Settings.tsx`)
    - [ ] Add "Rebuild Index" button for Deep Scan with warning dialog
    - [ ] Add restricted environment warning UI (when drive detection fails)
    - [ ] Add drive type override selector (Auto / Force SSD / Force HDD)
    - [ ] Show helpful text: "Drive type detection unavailable in sandboxed environments"

#### Testing

13. **Unit Tests**
    - [ ] Test drive type detection (SSD, HDD, NVMe)
    - [ ] Test NTFS filename normalization
    - [ ] Test stream hashing accuracy
    - [ ] Test HF exact match search
    - [ ] Test HF fuzzy fallback search
    - [ ] Test hash verification against candidates
    - [ ] Test variant and precision inference

14. **Integration Tests**
    - [ ] Test single file import with hash match
    - [ ] Test batch file import with deferred sync
    - [ ] Test directory import (Diffusers format)
    - [ ] Test atomic operations (crash recovery)
    - [ ] Test offline import (no network)
    - [ ] Test fast import (move) on same filesystem
    - [ ] Test fast import auto-switches to copy on cross-filesystem
    - [ ] Test safe import (copy) across filesystems
    - [ ] Test delete originals with safety checks
    - [ ] Test incremental sync after import
    - [ ] Test WAL mode concurrent access
    - [ ] Test WAL checkpointing every 5 models
    - [ ] Test Deep Scan rebuild with `match_source="manual"` protection
    - [ ] Test retry pending lookups respects manual metadata
    - [ ] Test NTFS collision detection (different hashes → hash suffix)
    - [ ] Test magic byte validation (reject .txt files)
    - [ ] Test relative symlink path calculation

15. **UI Tests**
    - [ ] Test drop zone appearance and animation
    - [ ] Test trust badges display correctly
    - [ ] Test progressive disclosure expand/collapse
    - [ ] Test granular progress states
    - [ ] Test import mode selection
    - [ ] Test delete originals checkbox validation
    - [ ] Test hard link count warning displays for NTFS
    - [ ] Test "Mark as manual" button protects metadata
    - [ ] Test error handling (disk full, permission denied)
    - [ ] Test magic byte validation rejects invalid files

### Completion Criteria (Part A)

- [ ] Users can drag model files onto GUI
- [ ] Files are looked up on HuggingFace (hash verification + fuzzy fallback)
- [ ] Metadata is displayed with trust badges indicating match quality
- [ ] Related files are shown with download option
- [ ] Files are copied into library with atomic operations
- [ ] Stream hashing computes BLAKE3/SHA256 during copy (no double-read)
- [ ] SQLite index is updated with WAL mode
- [ ] Incremental sync applies new models to all installed apps
- [ ] All test scenarios pass

---

### Part B: Link Registry Database

**Goal**: Track all symlinks/hardlinks for clean operations and health validation

**Status**: Ready to implement
**Estimated Complexity**: Low-Medium
**Dependencies**: None (can be done in parallel with Part A)

#### Backend Implementation

1. **Create Link Registry** (`backend/model_library/link_registry.py`)
   - [ ] Create `LinkRegistry` class
   - [ ] Implement `_initialize_database()` with WAL mode
   - [ ] Add `register_link()` method
   - [ ] Add `get_links_for_model()` method
   - [ ] Add `delete_links_for_model()` method (cascade cleanup)
   - [ ] Add `find_broken_links()` method
   - [ ] Add `find_orphaned_links()` method
   - [ ] Add `bulk_update_external_paths()` method for drive relocation
   - [ ] Implement hybrid path storage (relative for internal, absolute for external)
   - [ ] Add indexes on `model_id`, `target_app_path`, `is_external`
   - [ ] Create global `link_registry` instance
   - [ ] Test link registration on creation
   - [ ] Test cascade delete removes all links
   - [ ] Test broken link detection
   - [ ] Test orphaned link detection
   - [ ] Test path relocation updates registry

2. **Update Mapper** (`backend/model_library/mapper.py`)
   - [ ] Add `_create_link_with_registry()` method
   - [ ] Update all link creation to use registry
   - [ ] Add `delete_model_with_cascade()` method
   - [ ] Query registry before deleting models
   - [ ] Unlink all symlinks before file deletion
   - [ ] Purge registry entries after successful deletion
   - [ ] Test cascade delete workflow
   - [ ] Test registry tracks all created links
   - [ ] Test sharded set links are all registered

3. **Add Health Checks** (`backend/model_library/library.py`)
   - [ ] Add `perform_health_check()` method
   - [ ] Detect broken links (source missing)
   - [ ] Detect orphaned links (not in registry)
   - [ ] Return health status: 'healthy', 'warnings', 'errors'
   - [ ] Add `clean_broken_links()` method
   - [ ] Add `remove_orphaned_links()` method
   - [ ] Test health check detects all issues
   - [ ] Test cleanup methods work correctly

4. **Update API Core** (`backend/api/core.py`)
   - [ ] Add `get_library_health()` method
   - [ ] Add `clean_broken_links()` method
   - [ ] Add `remove_orphaned_links()` method
   - [ ] Add `relocate_external_drive()` method
   - [ ] Test health check API returns correct data
   - [ ] Test cleanup APIs work correctly
   - [ ] Test drive relocation updates paths

#### Frontend Implementation

5. **Update TypeScript Types** (`frontend/src/types/pywebview.d.ts`)
   - [ ] Add `LibraryHealthResponse` interface
   - [ ] Add `BrokenLinkInfo` interface
   - [ ] Add health status types ('healthy', 'warnings', 'errors')
   - [ ] Update `PyWebViewAPI` interface with health methods

6. **Add Health Status UI** (Settings page)
   - [ ] Add health status badge (green/yellow/red)
   - [ ] Add "View Details" button for warnings/errors
   - [ ] Create Health Details Dialog component
   - [ ] Show broken links with cleanup button
   - [ ] Show orphaned links with remove button
   - [ ] Add drive relocation tool (input old/new mount)
   - [ ] Test UI displays health status correctly
   - [ ] Test cleanup buttons work
   - [ ] Test drive relocation UI

#### Testing

7. **Registry Tests**
   - [ ] Link registration on creation
   - [ ] Cascade delete removes all links and files
   - [ ] Broken link detection after manual file deletion
   - [ ] Orphaned link detection
   - [ ] Hybrid path storage (relative vs absolute)
   - [ ] Drive relocation bulk update

8. **Health Check Tests**
   - [ ] Health check runs on startup
   - [ ] Broken links detected correctly
   - [ ] Orphaned links detected correctly
   - [ ] Cleanup operations work
   - [ ] UI displays health status

### Completion Criteria (Part B)

- [ ] Link registry database created and initialized
- [ ] All symlink/hardlink creation is tracked
- [ ] Cascade delete cleanly removes models and links
- [ ] Health checks detect broken and orphaned links
- [ ] UI displays health status with badges
- [ ] Cleanup operations work correctly
- [ ] Drive relocation helper updates paths
- [ ] All test scenarios pass

---

### Part C: Basic Link Mapping System

**Goal**: Get ComfyUI working with library models using default configs

**Status**: Ready to implement after Part A
**Estimated Complexity**: Medium
**Dependencies**: Part A (import system), Part B (link registry), existing mapper.py

### Tasks

#### Backend Implementation

1. **Update Mapper** (`backend/model_library/mapper.py`)
   - [ ] Add `create_default_comfyui_config()` method
   - [ ] Add `_discover_model_directories()` for dynamic scanning
   - [ ] Add `_load_and_merge_configs()` method
   - [ ] Add `_calculate_specificity()` for config precedence
   - [ ] Add `_determine_link_type()` based on filesystem validation
   - [ ] Add `_create_link_with_type()` supporting relative/absolute/hard links
   - [ ] Add `make_relative_symlink()` with correct `os.path.relpath()` usage
   - [ ] Add `sync_models_incremental()` method
   - [ ] Add `detect_sandbox_environment()` function
   - [ ] Update `apply_for_app()` to use filesystem validation
   - [ ] Update `_iter_matching_files()` to support file vs directory link types
   - [ ] Update `_version_allowed()` to validate overrides.json

2. **Update Version Manager** (`backend/version_manager.py`)
   - [ ] Add `_setup_model_mappings()` method
   - [ ] Add `_check_mapping_config_exists()` method
   - [ ] Update `_finalize_installation()` to auto-apply mappings
   - [ ] Add `_clean_model_symlinks()` method
   - [ ] Update `delete_version()` to clean up symlinks

3. **Update API Core** (`backend/api/core.py`)
   - [ ] Add `get_mapping_config()` method
   - [ ] Add `save_mapping_config()` method
   - [ ] Add `apply_mapping_config()` method (manual sync)
   - [ ] Add `preview_mapping_config()` method
   - [ ] Add `get_available_mapping_targets()` method
   - [ ] Add `sync_app_models()` method for manual user-triggered sync
   - [ ] Add `garbage_collect_orphaned_links()` method for filter mismatch cleanup
   - [ ] Add `_clean_broken_symlinks()` helper method
   - [ ] Update `_auto_sync_all_apps_incremental()` (from Phase 1) to use incremental mapper

4. **Create Default ComfyUI Config** (`launcher-data/config/model-library-translation/comfyui_*_default.json`)
   - [ ] Define baseline mappings for core directories (checkpoints, loras, vae, etc.)
   - [ ] Add support for auto-discovered directories
   - [ ] Set appropriate priorities for each mapping
   - [ ] Add file vs directory link type specifications

#### Frontend Implementation

5. **Update Settings Component** (`frontend/src/components/Settings.tsx`)
   - [ ] Add sandbox warning UI
   - [ ] Add cross-filesystem warning UI
   - [ ] Add "Sync Library Models" button
   - [ ] Add "Rebuild Index" button for Deep Scan
   - [ ] Add handler for manual sync
   - [ ] Add handler for Deep Scan

#### Testing

6. **Unit Tests**
   - [ ] Test config loading (version-specific and wildcard)
   - [ ] Test config precedence calculation
   - [ ] Test config merging
   - [ ] Test filter matching (model_type, subtype, tags, family)
   - [ ] Test tag filter logic (AND/OR, exclusion wins)
   - [ ] Test version constraint validation (PEP 440)
   - [ ] Test pattern matching (globs)
   - [ ] Test link type determination

7. **Integration Tests**
   - [ ] Test default mapping auto-applies on install
   - [ ] Test manual "Sync Models" button
   - [ ] Test relative symlinks created on same filesystem using `os.path.relpath()`
   - [ ] Test absolute symlinks created for cross-filesystem
   - [ ] Test hard links created on NTFS
   - [ ] Test broken symlink cleanup
   - [ ] Test orphaned link garbage collection (filter rule changes)
   - [ ] Test multiple ComfyUI versions don't conflict
   - [ ] Test incremental sync only processes new models
   - [ ] Test version constraints filter models correctly
   - [ ] Test dynamic directory scanning detects custom nodes
   - [ ] Test sandbox detection warnings
   - [ ] Test clean uninstall removes all symlinks

8. **System Tests**
   - [ ] Test end-to-end: Import model → Auto-maps to ComfyUI → Appears in app
   - [ ] Test multiple variants can coexist
   - [ ] Test config changes apply immediately
   - [ ] Test **The "Move Test"**: Move Pumas-Library folder → relative symlinks remain valid
   - [ ] Test Deep Scan rebuild from metadata.json

### Completion Criteria (Part C)

- [ ] Default mapping config is created on ComfyUI install with dynamic directory discovery
- [ ] Mappings are auto-applied (symlinks created)
- [ ] All ComfyUI model directories have correct mappings
- [ ] Symlinks are relative when possible, absolute with warnings when necessary
- [ ] Version constraints work (overrides.json)
- [ ] Manual sync API works
- [ ] Clean uninstall removes symlinks
- [ ] Sandbox detection warns users (Flatpak/Snap/Docker)
- [ ] All test scenarios pass

---

### Part D: Platform Abstraction Layer *(v3.2)*

**Goal**: Design for Windows v2.0 without implementing it in v1.0

**Status**: Ready to implement
**Estimated Complexity**: Low
**Dependencies**: None (can be done early in Phase 1)

#### Backend Implementation

1. **Create Platform Utils** (`backend/model_library/platform_utils.py`)
   - [ ] Create `PlatformUtils` class with OS detection
   - [ ] Add `LinkStrategy` type: `Literal["symlink", "hardlink", "copy"]`
   - [ ] Add `create_link()` method with Linux implementation
   - [ ] Add `supports_relative_links()` method (same filesystem check)
   - [ ] Add `NotImplementedError` stub for Windows (v2.0 hook)
   - [ ] Create global `platform_utils` singleton
   - [ ] Test link creation on ext4, Btrfs, NTFS

```python
# Key implementation:
class PlatformUtils:
    def __init__(self):
        self.is_windows = sys.platform == "win32"
        self.is_linux = sys.platform.startswith("linux")

    def create_link(
        self,
        source: Path,
        target: Path,
        strategy: LinkStrategy = "symlink",
        relative: bool = True
    ) -> bool:
        """
        v1.0 (Linux): Uses symlink or hardlink
        v2.0 (Windows): Will check Developer Mode, use junctions/hardlinks
        """
        if self.is_windows:
            raise NotImplementedError("Windows support planned for v2.0")
        return self._create_link_linux(source, target, strategy, relative)

platform_utils = PlatformUtils()
```

2. **Update All Link Creation** (multiple files)
   - [ ] Update `mapper.py` to use `platform_utils.create_link()`
   - [ ] Update `link_registry.py` to use `platform_utils.create_link()`
   - [ ] Remove direct `Path.symlink_to()` calls outside platform_utils
   - [ ] Store link type in registry: `link_type TEXT` column

#### Frontend Implementation

3. **Add Link Strategy to Settings** (`frontend/src/components/Settings.tsx`)
   - [ ] Add fallback policy selector per mapping (if needed)
   - [ ] Add link type display in model details tooltip

#### Testing

4. **Platform Abstraction Tests**
   - [ ] All link creation goes through platform_utils
   - [ ] Link registry stores correct link type
   - [ ] Same-filesystem detection works
   - [ ] Cross-filesystem falls back to absolute paths

### Completion Criteria (Part D)

- [ ] All link creation uses `platform_utils.create_link()`
- [ ] No direct `Path.symlink_to()` calls outside platform_utils
- [ ] Link registry stores link type for future migration
- [ ] Windows stub raises NotImplementedError with clear message
- [ ] All test scenarios pass

---

### Part E: Pickle Security Warnings *(v3.2)*

**Goal**: Warn users about potentially unsafe pickle-based model formats

**Status**: Ready to implement
**Estimated Complexity**: Low
**Dependencies**: Part A (import system)
**Priority**: Lower priority within Phase 1 (implement after Parts A-D)

> **Note**: This is primarily a UI warning system. We use the existing `picklescan` library
> for basic scanning rather than implementing custom detection. The main goal is informing
> users about risk, not blocking imports.

#### Backend Implementation

1. **Add Security Tiering** (`backend/model_library/library.py`)
   - [ ] Add `SecurityTier` enum: `GREEN`, `YELLOW`, `RED`
   - [ ] Add `assess_security_tier()` function based on file extension
   - [ ] Store `security_tier` in models table
   - [ ] Update `add_model_to_library()` to include security assessment

```python
# Key implementation:
class SecurityTier(Enum):
    GREEN = "safe"       # Safetensors, GGUF, ONNX
    YELLOW = "unknown"   # Untested formats
    RED = "pickle"       # PyTorch pickle (.ckpt, .pt, .bin)

def assess_security_tier(file_path: Path) -> SecurityTier:
    suffix = file_path.suffix.lower()
    if suffix in ['.safetensors', '.gguf', '.onnx']:
        return SecurityTier.GREEN
    if suffix in ['.ckpt', '.pt', '.bin', '.pth']:
        return SecurityTier.RED
    return SecurityTier.YELLOW
```

2. **Optional: Integrate picklescan** (`backend/model_library/security.py`)
   - [ ] Add `picklescan` to dependencies (optional, can skip if not needed)
   - [ ] Create wrapper function `scan_pickle_file()` using picklescan library
   - [ ] **Run scan as low-priority background thread AFTER import completes**
   - [ ] Update `security_tier` in SQLite once scan finishes
   - [ ] Return list of detected dangerous globals (informational only)
   - [ ] Do NOT block imports based on scan results

> **Performance Note**: `picklescan` can be slow on large `.ckpt` files as it scans the
> entire data stream. To avoid blocking imports, run the scan in a background thread
> after the file is already on disk. The security tier starts as "RED" (pickle warning)
> and gets updated to "GREEN" or stays "RED" based on scan results.

```python
# Optional picklescan integration (use existing library):
import threading
from queue import Queue

# Background scan queue
_scan_queue: Queue = Queue()
_scan_thread: Optional[threading.Thread] = None

def _background_scanner():
    """Background thread that processes pickle scans."""
    while True:
        item = _scan_queue.get()
        if item is None:  # Shutdown signal
            break

        file_path, model_id, callback = item
        try:
            result = _do_pickle_scan(file_path)
            if callback:
                callback(model_id, result)
        except Exception as e:
            logger.error(f"Background pickle scan failed: {e}")
        finally:
            _scan_queue.task_done()

def queue_pickle_scan(file_path: Path, model_id: str, callback=None):
    """
    Queue a pickle file for background security scanning.

    The scan runs AFTER import completes, in a low-priority background thread.
    Results update the security_tier column in SQLite.
    """
    global _scan_thread

    # Start background thread if not running
    if _scan_thread is None or not _scan_thread.is_alive():
        _scan_thread = threading.Thread(
            target=_background_scanner,
            daemon=True,
            name="PickleScanThread"
        )
        _scan_thread.start()

    _scan_queue.put((file_path, model_id, callback))
    logger.debug(f"Queued pickle scan for {file_path.name}")

try:
    from picklescan.scanner import scan_file_path

    def _do_pickle_scan(file_path: Path) -> dict:
        """
        Scan pickle file for dangerous globals using picklescan.

        This is INFORMATIONAL ONLY - we do not block imports.
        Users should be informed but allowed to proceed.
        """
        try:
            result = scan_file_path(str(file_path))
            return {
                'scanned': True,
                'dangerous_globals': result.globals,
                'issues_found': len(result.globals) > 0
            }
        except Exception as e:
            return {'scanned': False, 'error': str(e)}

except ImportError:
    # picklescan not installed - just use extension-based warnings
    def _do_pickle_scan(file_path: Path) -> dict:
        return {'scanned': False, 'reason': 'picklescan not installed'}
```

2. **Update Database Schema** (`backend/model_library/library.py`)
   - [ ] Add `security_tier TEXT` column to models table
   - [ ] Add migration for existing databases

#### Frontend Implementation

3. **Add Security Warning UI** (`frontend/src/components/ModelImportDialog.tsx`)
   - [ ] Create `SecurityWarning` component
   - [ ] Show green badge for safetensors (or no badge)
   - [ ] Show yellow info banner for unknown formats
   - [ ] Show red warning banner for pickle formats
   - [ ] Add required checkbox for pickle imports: "I understand the risks"
   - [ ] Block import until checkbox is checked for RED tier

```tsx
// Key implementation:
function SecurityWarning({ tier, filename }: Props) {
  if (tier === 'safe') return null;

  if (tier === 'pickle') {
    return (
      <div className="border-l-4 border-red-500 bg-red-50 p-4">
        <p className="text-sm text-gray-700">
          This model uses PyTorch pickle format, which can execute
          arbitrary code. Only import from trusted sources.
        </p>
        <label className="flex items-center mt-3">
          <input type="checkbox" required className="mr-2" />
          <span className="text-sm">I understand the risks</span>
        </label>
      </div>
    );
  }
  // ... yellow for unknown
}
```

4. **Update TypeScript Types** (`frontend/src/types/pywebview.d.ts`)
   - [ ] Add `SecurityTier` type: `'safe' | 'unknown' | 'pickle'`
   - [ ] Add `security_tier` to model interfaces

#### Testing

5. **Security Warning Tests**
   - [ ] Import .safetensors → No warning shown (GREEN)
   - [ ] Import .gguf → No warning shown (GREEN)
   - [ ] Import .ckpt → Red warning with checkbox (RED)
   - [ ] Import .pt → Red warning with checkbox (RED)
   - [ ] Import unknown extension → Yellow info banner (YELLOW)
   - [ ] Cannot proceed without checkbox for RED tier

### Completion Criteria (Part E)

- [ ] Security tier assessed for all imports
- [ ] Pickle formats show clear warning banner
- [ ] Required acknowledgment checkbox for unsafe formats
- [ ] Security tier stored in database
- [ ] All test scenarios pass

---

### Part F: Circuit Breaker Networking *(v3.2)*

**Goal**: Prevent UI freezes on network failures with async httpx and circuit breaker

**Status**: Ready to implement
**Estimated Complexity**: Medium
**Dependencies**: Part A (downloader)

> **Note**: This implements the circuit breaker from [05-networking-and-caching.md](05-networking-and-caching.md).
> HTTP/2 and FTS5 are deferred to Phase 4. This phase uses HTTP/1.1 only.

#### Backend Implementation

1. **Create Network Manager** (`backend/model_library/network_manager.py`)
   - [ ] Create `CircuitBreakerState` dataclass
   - [ ] Create `CircuitBreakerOpen` exception
   - [ ] Create `NetworkManager` class
   - [ ] Add async httpx client with 7s timeout (HTTP/1.1, not HTTP/2 yet)
   - [ ] Add circuit breaker logic (3 failures → 60s blackout)
   - [ ] Add `_record_failure()` and `_record_success()` methods
   - [ ] Add `_is_circuit_open()` check
   - [ ] Create global `network_manager` singleton

```python
# Key implementation (HTTP/1.1 for Phase 1):
class NetworkManager:
    def __init__(self):
        self._client: Optional[httpx.AsyncClient] = None
        self._circuit_breaker: dict[str, CircuitBreakerState] = {}
        self._failure_threshold = 3
        self._cooldown_seconds = 60

    async def get_client(self) -> httpx.AsyncClient:
        if self._client is None:
            self._client = httpx.AsyncClient(timeout=7.0)
            # Phase 4: Add http2=True here
        return self._client

    async def request(self, method: str, url: str, **kwargs) -> httpx.Response:
        domain = self._extract_domain(url)
        if self._is_circuit_open(domain):
            raise CircuitBreakerOpen(f"Circuit breaker OPEN for {domain}")
        try:
            client = await self.get_client()
            response = await client.request(method, url, **kwargs)
            self._record_success(domain)
            return response
        except (httpx.TimeoutException, httpx.ConnectError) as e:
            self._record_failure(domain)
            raise

network_manager = NetworkManager()
```

2. **Update Downloader** (`backend/model_library/downloader.py`)
   - [ ] Import `network_manager` and `CircuitBreakerOpen`
   - [ ] Add `async` keyword to all lookup methods
   - [ ] Replace `requests.get()` with `await network_manager.request()`
   - [ ] Add `CircuitBreakerOpen` exception handling → return cached data
   - [ ] Add `httpx.TimeoutException` handling → return cached data
   - [ ] Update all callers to use `await`

3. **Add Simple Search** (`backend/model_library/library.py`)
   - [ ] Add `search_models()` method using SQLite LIKE query
   - [ ] Phase 4 will upgrade this to FTS5

```python
# Phase 1: Simple LIKE query (good enough for <1000 models)
def search_models(self, query: str, limit: int = 100) -> list[dict]:
    cursor = self.conn.execute("""
        SELECT * FROM models
        WHERE repo_id LIKE ? OR official_name LIKE ?
           OR family LIKE ? OR tags LIKE ?
        ORDER BY last_used DESC LIMIT ?
    """, (f'%{query}%',) * 4 + (limit,))
    return [dict(row) for row in cursor.fetchall()]
```

#### Frontend Implementation

4. **Add Offline Indicator** (`frontend/src/components/ModelManager.tsx`)
   - [ ] Add `isOffline` state from API
   - [ ] Show "Using Cached Data" banner when circuit breaker is open
   - [ ] Style with yellow background and clear messaging

```tsx
{isOffline && (
  <div className="bg-yellow-50 border-l-4 border-yellow-400 p-3">
    <p className="text-sm text-yellow-700">
      Using Cached Data (offline)
    </p>
  </div>
)}
```

5. **Update API to Expose Circuit State** (`backend/api/core.py`)
   - [ ] Add `get_network_status()` method
   - [ ] Return which domains have open circuits
   - [ ] Return last failure time

#### Testing

6. **Circuit Breaker Tests**
   - [ ] Circuit opens after 3 consecutive failures
   - [ ] Circuit stays open for 60 seconds
   - [ ] Circuit closes after cooldown expires
   - [ ] UI shows "Using Cached Data" when circuit open
   - [ ] Offline mode returns cached data within 7s (no UI freeze)
   - [ ] Success resets failure counter

### Completion Criteria (Part F)

- [ ] Network requests use async httpx with 7s timeout
- [ ] Circuit breaker prevents repeated failures
- [ ] UI shows offline indicator when circuit open
- [ ] No UI freezes on network failures (max 7s delay)
- [ ] Cached data returned when offline
- [ ] All test scenarios pass

---

## Phase 2: Reliability & Self-Healing

**Goal**: Ensure system correctness, prevent data loss, and auto-repair issues

**Status**: Ready to implement after Phase 1
**Estimated Complexity**: Medium
**Dependencies**: Phase 1 complete (core infrastructure in place)

### Part A: Deep Scan File Verification

**Goal**: Prevent phantom models where metadata exists but files were manually deleted

**Status**: Ready to implement
**Estimated Complexity**: Low
**Dependencies**: Phase 1 Part A (models.db and metadata.json)

#### Backend Implementation

1. **Update Deep Scan** (`backend/model_library/library.py`)
   - [ ] Update `rebuild_index_from_metadata()` to verify file existence
   - [ ] Skip models where metadata.json exists but weight files are missing
   - [ ] Return `models_skipped_missing_files` count
   - [ ] Log warnings for skipped models
   - [ ] Test Deep Scan skips phantom models
   - [ ] Test Deep Scan indexes valid models

#### Testing

2. **File Verification Tests**
   - [ ] Create metadata.json with no weight files → Deep Scan skips it
   - [ ] Delete weight files after import → Deep Scan detects and skips
   - [ ] Partial deletion (1 of 5 shards) → Deep Scan skips entire model
   - [ ] Deep Scan respects `match_source="manual"` protection

### Completion Criteria (Part A)

- [ ] Deep Scan verifies physical file existence
- [ ] Phantom models (metadata-only) are skipped with warnings
- [ ] Skipped model count is reported to user
- [ ] All test scenarios pass

---

### Part B: Link Self-Healing *(v3.2)*

**Goal**: Automatically repair broken symlinks by searching for models by hash

**Status**: Ready to implement
**Estimated Complexity**: Medium
**Dependencies**: Phase 1 Part B (Link Registry)

**Automatic Trigger**: Self-healing runs automatically on startup if:
1. App detects its own root path has changed (stored path vs current path)
2. Broken links are detected during health check
3. Library location has changed since last run

This handles the "Library Relocation Edge Case" where moving the App but not the Library breaks relative symlinks.

#### Backend Implementation

1. **Add Path Change Detection** (`backend/model_library/link_registry.py`)
   - [ ] Create `settings` table in registry.db for storing app state
   - [ ] Store `last_known_app_path` on startup
   - [ ] Store `last_known_library_root` on startup
   - [ ] Add `check_app_relocation()` method to detect app moves
   - [ ] Add `check_library_relocation()` method to detect library moves
   - [ ] On startup, compare current paths to stored paths
   - [ ] If either path differs, automatically trigger self-healing
   - [ ] Update stored paths after successful heal

```python
def check_app_relocation(self) -> dict:
    """
    Detect if app has been moved since last run.
    Called automatically on startup.
    """
    current_path = str(self.app_root.resolve())

    # Get stored path
    conn = sqlite3.connect(str(self.db_path))
    cursor = conn.cursor()
    cursor.execute("""
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT
        )
    """)
    cursor.execute("SELECT value FROM settings WHERE key = 'last_known_app_path'")
    row = cursor.fetchone()
    stored_path = row[0] if row else None
    conn.close()

    if stored_path is None:
        # First run - store current path
        self._save_setting('last_known_app_path', current_path)
        return {'relocated': False, 'needs_healing': False}

    if stored_path != current_path:
        logger.warning(f"App relocation detected: {stored_path} → {current_path}")
        return {
            'relocated': True,
            'old_path': stored_path,
            'new_path': current_path,
            'needs_healing': True
        }

    return {'relocated': False, 'needs_healing': False}


def check_library_relocation(self, current_library_root: Path) -> dict:
    """
    Detect if library has been moved since last run.
    Called automatically on startup.

    The library_root is stored in registry.db to detect when the
    library folder changes location (e.g., moved to different drive).
    """
    current_path = str(current_library_root.resolve())

    # Get stored path
    conn = sqlite3.connect(str(self.db_path))
    cursor = conn.cursor()
    cursor.execute("SELECT value FROM settings WHERE key = 'last_known_library_root'")
    row = cursor.fetchone()
    stored_path = row[0] if row else None
    conn.close()

    if stored_path is None:
        # First run - store current path
        self._save_setting('last_known_library_root', current_path)
        return {'relocated': False, 'needs_healing': False}

    if stored_path != current_path:
        logger.warning(f"Library relocation detected: {stored_path} → {current_path}")
        return {
            'relocated': True,
            'old_path': stored_path,
            'new_path': current_path,
            'needs_healing': True
        }

    return {'relocated': False, 'needs_healing': False}


def check_relocations_on_startup(self, current_library_root: Path) -> dict:
    """
    Check for both app and library relocations on startup.

    This is the main entry point called during app initialization.
    If either location changed, triggers automatic self-healing.

    Returns:
        {
            'app_relocated': bool,
            'library_relocated': bool,
            'needs_healing': bool,
            'auto_healed': bool,
            'heal_results': dict | None
        }
    """
    app_check = self.check_app_relocation()
    lib_check = self.check_library_relocation(current_library_root)

    needs_healing = app_check.get('needs_healing') or lib_check.get('needs_healing')

    result = {
        'app_relocated': app_check.get('relocated', False),
        'library_relocated': lib_check.get('relocated', False),
        'needs_healing': needs_healing,
        'auto_healed': False,
        'heal_results': None
    }

    if needs_healing:
        logger.info("Relocation detected - triggering automatic self-healing...")
        heal_results = self.auto_repair_links()
        result['auto_healed'] = heal_results.get('repaired', 0) > 0
        result['heal_results'] = heal_results

        # Update stored paths after successful heal
        if result['auto_healed']:
            self._save_setting('last_known_app_path', str(self.app_root.resolve()))
            self._save_setting('last_known_library_root', str(current_library_root.resolve()))
            logger.info("Stored paths updated after successful heal")

    return result
```

2. **Add Self-Healing to Link Registry** (`backend/model_library/link_registry.py`)
   - [ ] Add `find_broken_links()` method (already exists from Phase 1)
   - [ ] Add `auto_repair_links()` method
   - [ ] Search for models by hash across all indexed locations
   - [ ] Recreate links with new source paths
   - [ ] Update registry with new source paths
   - [ ] Return repair results: `{repaired: int, failed: list, details: list}`

```python
# Key implementation:
def auto_repair_links(self) -> dict:
    """
    Attempt to repair broken links by searching for models by hash.
    """
    broken = self.find_broken_links()
    repaired = 0
    failed = []
    details = []

    for link_info in broken:
        # Search all indexed models for matching hash
        new_source = self.library.find_file_by_hash(link_info['model_hash'])

        if new_source and new_source.exists():
            success = self._recreate_link(
                source=new_source,
                target=link_info['target_path'],
                link_id=link_info['link_id']
            )
            if success:
                repaired += 1
                details.append({
                    'target': str(link_info['target_path']),
                    'old_source': str(link_info['source_path']),
                    'new_source': str(new_source),
                    'status': 'repaired'
                })
            else:
                failed.append(link_info['target_path'])
        else:
            failed.append(link_info['target_path'])

    return {'repaired': repaired, 'failed': failed, 'details': details}
```

2. **Add Hash-Based Model Lookup** (`backend/model_library/library.py`)
   - [ ] Add `find_file_by_hash()` method
   - [ ] Search models table for matching BLAKE3/SHA256 hash
   - [ ] Return file path if found, None otherwise

3. **Update API Core** (`backend/api/core.py`)
   - [ ] Add `auto_repair_links()` endpoint
   - [ ] Return repair results to frontend
   - [ ] Add optional startup repair check

#### Frontend Implementation

4. **Add Repair UI** (`frontend/src/components/Settings.tsx`)
   - [ ] Add "Repair Broken Links" button in Health Check section
   - [ ] Show repair results dialog
   - [ ] Display repaired count and failed list
   - [ ] Add "Auto-repair on startup" toggle (optional)

```tsx
// Health Check Panel enhancement:
function HealthCheckPanel() {
  const [repairResults, setRepairResults] = useState(null);

  const runAutoRepair = async () => {
    const results = await api.autoRepairLinks();
    setRepairResults(results);

    if (results.repaired > 0) {
      toast.success(`Repaired ${results.repaired} broken links`);
    }
    if (results.failed.length > 0) {
      toast.warning(`Could not repair ${results.failed.length} links`);
    }
  };

  return (
    <div>
      <Button onClick={runAutoRepair}>
        🔧 Auto-Repair Broken Links
      </Button>
      {repairResults && <RepairResultsDialog results={repairResults} />}
    </div>
  );
}
```

5. **Update TypeScript Types** (`frontend/src/types/pywebview.d.ts`)
   - [ ] Add `RepairResult` interface
   - [ ] Add `auto_repair_links()` to PyWebViewAPI

#### Testing

6. **Self-Healing Tests**
   - [ ] Delete model source → Health check detects broken link
   - [ ] Same model exists elsewhere (by hash) → Auto-repair finds it
   - [ ] Link recreated with new source path → Works correctly
   - [ ] Registry updated with new source path
   - [ ] Model not found by hash → Repair fails gracefully
   - [ ] Multiple broken links → All repaired in one operation
   - [ ] UI shows accurate repair results

### Completion Criteria (Part B)

- [ ] Broken links detected automatically
- [ ] Models found by hash across all indexed locations
- [ ] Links recreated with correct new source paths
- [ ] Registry updated to reflect repairs
- [ ] UI provides clear repair feedback
- [ ] All test scenarios pass

---

## Phase 3: UX Polish & Conflict Resolution

**Goal**: Improve workflow and usability with interactive conflict handling

**Status**: Ready to implement after Phase 2
**Estimated Complexity**: Medium
**Dependencies**: Phase 1 and Phase 2 complete

### Part A: Sharded Set Grouping

**Goal**: Automatically detect and group multi-file models

**Status**: Ready to implement
**Estimated Complexity**: Medium
**Dependencies**: Phase 1 (import system)

#### Backend Implementation

1. **Add Shard Detection** (`backend/model_library/importer.py`)
   - [ ] Add `detect_sharded_sets()` function with 3 pattern matchers
   - [ ] Add `validate_shard_completeness()` function
   - [ ] Update import flow to group shards before HF lookup
   - [ ] Store sharded sets in single directory with `is_sharded_set: true`
   - [ ] Update metadata.json schema with `files` array for shards
   - [ ] Test pattern 1: model-00001-of-00005.safetensors
   - [ ] Test pattern 2: model.safetensors.part1
   - [ ] Test pattern 3: model_00001.safetensors
   - [ ] Test completeness validation
   - [ ] Test incomplete set warnings

2. **Update Mapper** (`backend/model_library/mapper.py`)
   - [ ] Add `map_sharded_set()` method
   - [ ] Symlink all shard files to target directory
   - [ ] Register all shard links in registry
   - [ ] Test sharded set mapping
   - [ ] Test cascade delete removes all shards

#### Frontend Implementation

3. **Update Import Dialog** (`frontend/src/components/ModelImportDialog.tsx`)
   - [ ] Add sharded set grouping UI
   - [ ] Show "Complete" badge for full sets
   - [ ] Show "Incomplete (X missing)" badge for partial sets
   - [ ] Add expandable shard file list
   - [ ] Add "Download missing shards" button
   - [ ] Test UI displays grouped shards
   - [ ] Test badges show correct status

#### Testing

4. **Sharded Set Tests**
   - [ ] Import 5-shard set → Groups as one model
   - [ ] Import incomplete set → Warning shown, allows import
   - [ ] Map sharded set → All files symlinked
   - [ ] Delete sharded set → All shards and links removed
   - [ ] HF lookup for sharded set

### Completion Criteria (Part A)

- [ ] Sharded sets automatically detected and grouped
- [ ] UI shows grouped shards with completion badges
- [ ] All shards mapped to target directory
- [ ] Cascade delete removes all shards
- [ ] All test scenarios pass

---

### Part B: Interactive Conflict Resolution *(v3.2)*

**Goal**: Replace passive "skip + warn" with interactive user choice dialog

**Status**: Ready to implement
**Estimated Complexity**: Medium
**Dependencies**: Phase 1 Part C (mapping system)

> **Note**: v3.2 upgrade from passive skip+warn to active user choice.
> Users now choose how to handle each conflict instead of silent skipping.

#### Backend Implementation

1. **Add Conflict Detection** (`backend/model_library/mapper.py`)
   - [ ] Add `ConflictAction` enum: `ASK`, `SKIP`, `OVERWRITE`, `RENAME`
   - [ ] Add `conflict_strategy` to mapping config schema
   - [ ] Add `MappingAction` dataclass with conflict reason
   - [ ] Add `MappingPreview` dataclass
   - [ ] Add `preview_mapping()` method (dry run)
   - [ ] Add `_preview_single_link()` helper
   - [ ] Detect new links to create
   - [ ] Detect already-correct links
   - [ ] Detect conflicts with reason: `already_linked`, `linked_to_different_source`, `file_exists`
   - [ ] Detect broken links to remove

```python
# Key implementation:
class ConflictAction(Enum):
    ASK = "ask"
    SKIP = "skip"
    OVERWRITE = "overwrite"
    RENAME = "rename"

def _detect_conflict_reason(self, target: Path, model: dict) -> str:
    if target.is_symlink():
        existing_source = target.readlink()
        if existing_source == Path(model['file_path']):
            return "already_linked"
        else:
            return "linked_to_different_source"
    else:
        return "file_exists"
```

2. **Add Resolution Execution** (`backend/model_library/mapper.py`)
   - [ ] Add `sync_with_resolutions()` method
   - [ ] Accept resolution dict: `{model_id: 'skip' | 'overwrite' | 'rename'}`
   - [ ] Execute overwrite: delete existing, create link
   - [ ] Execute rename: rename existing to `.old`, create link
   - [ ] Execute skip: no action
   - [ ] Return execution results

```python
def sync_with_resolutions(self, mapping_name: str, resolutions: dict) -> dict:
    """Execute sync with user-provided conflict resolutions."""
    for model_id, action in resolutions.items():
        model = self.library.get_model(model_id)
        target = self._resolve_target_path(model, mapping)

        if action == 'overwrite':
            if target.exists():
                target.unlink()
            self.create_link(model['file_path'], target)
        elif action == 'rename':
            if target.exists():
                target.rename(target.with_suffix(target.suffix + '.old'))
            self.create_link(model['file_path'], target)
        elif action == 'skip':
            continue
```

3. **Update API Core** (`backend/api/core.py`)
   - [ ] Add `preview_model_mapping()` method
   - [ ] Add `sync_with_resolutions()` method
   - [ ] Return conflict details for frontend display

#### Frontend Implementation

4. **Create Conflict Resolution Dialog** (`frontend/src/components/ConflictResolutionDialog.tsx`)
   - [ ] Create dialog component showing all conflicts
   - [ ] Display conflict reason for each model
   - [ ] Add per-model resolution selector: Overwrite / Rename / Skip
   - [ ] Add "Apply to all" shortcuts for batch operations
   - [ ] Add Apply button to execute resolutions
   - [ ] Add Cancel button to abort

```tsx
// Key implementation:
function ConflictResolutionDialog({ conflicts }: Props) {
  const [resolutions, setResolutions] = useState<Record<string, string>>({});

  return (
    <Dialog>
      <h2>Resolve Conflicts ({conflicts.length})</h2>
      <table>
        <thead>
          <tr><th>Model</th><th>Issue</th><th>Action</th></tr>
        </thead>
        <tbody>
          {conflicts.map(conflict => (
            <tr key={conflict.model_id}>
              <td>{conflict.filename}</td>
              <td><ConflictReason reason={conflict.conflict_reason} /></td>
              <td>
                <select
                  value={resolutions[conflict.model_id] || 'skip'}
                  onChange={e => handleResolve(conflict.model_id, e.target.value)}
                >
                  <option value="skip">Skip (keep existing)</option>
                  <option value="overwrite">Overwrite</option>
                  <option value="rename">Rename existing to .old</option>
                </select>
              </td>
            </tr>
          ))}
        </tbody>
      </table>
      <Button onClick={applyResolutions}>Apply</Button>
    </Dialog>
  );
}
```

5. **Create Preview Dialog** (`frontend/src/components/MappingPreviewDialog.tsx`)
   - [ ] Add summary cards (New/Exists/Conflicts/Broken)
   - [ ] Add errors and warnings sections
   - [ ] Add broken links expandable list
   - [ ] Add new links expandable list (limit to 10, show "... and X more")
   - [ ] Add conflicts section with "Resolve" button → opens ConflictResolutionDialog
   - [ ] Add "No changes needed" state
   - [ ] Add Cancel/Apply buttons

6. **Update Settings** (`frontend/src/components/Settings.tsx`)
   - [ ] Update "Sync Library Models" to show preview first
   - [ ] Wire up preview → conflict resolution → sync workflow
   - [ ] Test workflow from button click to completion

7. **Update TypeScript Types** (`frontend/src/types/pywebview.d.ts`)
   - [ ] Add `ConflictInfo` interface with reason
   - [ ] Add `MappingPreview` interface
   - [ ] Add `sync_with_resolutions()` to PyWebViewAPI

#### Testing

8. **Conflict Resolution Tests**
   - [ ] Empty library → "No changes needed"
   - [ ] New models → Shows create count, no conflicts
   - [ ] Already synced → Shows "already correct" count
   - [ ] Conflict exists → Shows conflict with reason
   - [ ] User selects "Overwrite" → Existing deleted, link created
   - [ ] User selects "Rename" → Existing renamed to .old, link created
   - [ ] User selects "Skip" → No action taken
   - [ ] User cancels → No changes made
   - [ ] Multiple conflicts → All resolutions applied

### Completion Criteria (Part B)

- [ ] Mapping preview shows accurate summary with conflict reasons
- [ ] Conflict resolution dialog allows per-model choices
- [ ] Overwrite/Rename/Skip all work correctly
- [ ] Preview → Resolve → Apply workflow works end-to-end
- [ ] UI clearly communicates what will happen
- [ ] All test scenarios pass

---

### Part C: Drive/Mount-Point Relocation Helper

**Goal**: Bulk-update paths when external drive mount points change

**Status**: Already implemented in Phase 1 Part B
**Note**: This is covered by `bulk_update_external_paths()` in Link Registry (Phase 1 Part B)

---

## Phase 4: Performance & Scale *(v3.2)*

**Goal**: Optimize for large libraries (1000+ models) with advanced networking and search

**Status**: Ready to implement after Phase 3
**Estimated Complexity**: Medium
**Dependencies**: Phases 1-3 complete

> **Note**: These features were deferred from Phase 1 MVP to reduce initial complexity.
> Phase 1 uses simple LIKE queries and HTTP/1.1. Phase 4 upgrades to FTS5 and HTTP/2.

---

### Part A: FTS5 Full-Text Search

**Goal**: Sub-20ms search queries for libraries with 1000+ models

**Status**: Ready to implement
**Estimated Complexity**: Medium
**Dependencies**: Phase 1 Part F (basic search working)

#### Backend Implementation

1. **Add FTS5 Virtual Table** (`backend/model_library/library.py`)
   - [ ] Add FTS5 virtual table in `_initialize_database()`
   - [ ] Configure tokenizer: `unicode61 remove_diacritics 1 tokenchars "-_"`
   - [ ] Add AFTER INSERT trigger to sync with models table
   - [ ] Add AFTER UPDATE trigger (delete old, insert new)
   - [ ] Add AFTER DELETE trigger
   - [ ] Add `_migrate_to_fts5()` for existing databases
   - [ ] Call migration on startup if FTS5 table doesn't exist

```python
# FTS5 virtual table:
cursor.execute("""
    CREATE VIRTUAL TABLE IF NOT EXISTS model_search USING fts5(
        model_id UNINDEXED,
        repo_id, official_name, family, tags, description,
        tokenize='unicode61 remove_diacritics 1 tokenchars "-_"'
    )
""")

# Trigger to keep in sync:
cursor.execute("""
    CREATE TRIGGER IF NOT EXISTS models_ai AFTER INSERT ON models BEGIN
        INSERT INTO model_search(model_id, repo_id, official_name, family, tags, description)
        VALUES (NEW.model_id, NEW.repo_id, NEW.official_name, NEW.family, NEW.tags, NEW.description);
    END
""")
```

2. **Add FTS5 Search Method** (`backend/model_library/library.py`)
   - [ ] Add `search_models_fts()` method
   - [ ] Build FTS5 query with prefix matching (`term*`)
   - [ ] Join `model_search` with `models` table
   - [ ] Return query time for monitoring
   - [ ] Add fallback to LIKE query if FTS5 fails

```python
def search_models_fts(self, query: str, limit: int = 100) -> dict:
    """Search using FTS5 for sub-millisecond results."""
    start = time.time()
    fts_query = ' OR '.join(f'{term}*' for term in query.split())

    cursor.execute("""
        SELECT m.* FROM model_search ms
        JOIN models m ON ms.model_id = m.model_id
        WHERE model_search MATCH ?
        ORDER BY rank LIMIT ?
    """, (fts_query, limit))

    models = [dict(row) for row in cursor.fetchall()]
    query_time_ms = (time.time() - start) * 1000
    return {'models': models, 'query_time_ms': query_time_ms}
```

3. **Update API Core** (`backend/api/core.py`)
   - [ ] Update search endpoint to use FTS5
   - [ ] Return query time in response

#### Testing

4. **FTS5 Tests**
   - [ ] Insert model → FTS5 entry created
   - [ ] Update model → FTS5 entry updated
   - [ ] Delete model → FTS5 entry removed
   - [ ] Search "Llama" finds "Llama-3-7b" in <20ms
   - [ ] Search with 1000+ models → Results in <20ms
   - [ ] Migration works for existing databases

### Completion Criteria (Part A)

- [ ] FTS5 virtual table created and synced
- [ ] Search queries complete in <20ms for 1000+ models
- [ ] Migration works for existing databases
- [ ] All test scenarios pass

---

### Part B: HTTP/2 Multiplexing

**Goal**: Reduce network overhead with concurrent requests over single TCP connection

**Status**: Ready to implement
**Estimated Complexity**: Low (upgrade from Phase 1 HTTP/1.1)
**Dependencies**: Phase 1 Part F (NetworkManager exists)

#### Backend Implementation

1. **Upgrade NetworkManager** (`backend/model_library/network_manager.py`)
   - [ ] Enable HTTP/2: `httpx.AsyncClient(http2=True, timeout=7.0)`
   - [ ] Add rate limit header detection (X-RateLimit-Remaining)
   - [ ] Add proactive throttling when rate limit < 10%
   - [ ] Test HTTP/2 multiplexing with concurrent requests

```python
# Upgrade from Phase 1:
async def get_client(self) -> httpx.AsyncClient:
    if self._client is None:
        self._client = httpx.AsyncClient(
            http2=True,  # Enable HTTP/2
            timeout=7.0
        )
    return self._client

def _check_rate_limit(self, response: httpx.Response):
    remaining = response.headers.get('X-RateLimit-Remaining')
    limit = response.headers.get('X-RateLimit-Limit')
    if remaining and limit:
        if int(remaining) / int(limit) < 0.1:
            self._rate_limit_warning = True
            logger.warning("Rate limit low, throttling requests")
```

#### Testing

2. **HTTP/2 Tests**
   - [ ] Verify HTTP/2 connection established
   - [ ] Multiple concurrent requests use single TCP connection
   - [ ] Rate limit detection triggers throttling
   - [ ] Performance improvement measured vs HTTP/1.1

### Completion Criteria (Part B)

- [ ] HTTP/2 enabled for all API requests
- [ ] Rate limit headers detected and logged
- [ ] Concurrent requests multiplexed
- [ ] All test scenarios pass

---

### Part C: Stale-While-Revalidate (SWR)

**Goal**: Instant UI response with background refresh for up-to-date data

**Status**: Ready to implement
**Estimated Complexity**: Medium
**Dependencies**: Phase 1 Part F (basic search), Part A (FTS5)

#### Frontend Implementation

1. **Add SWR Pattern** (`frontend/src/hooks/useModels.ts`)
   - [ ] Add `sequenceId` state and `lastRenderedId` ref
   - [ ] Add 300ms debouncing (lodash or custom)
   - [ ] Add sequence guard: discard stale responses
   - [ ] Add stale-while-revalidate: show cached, refresh in background
   - [ ] Add "New results available" toast notification

```typescript
export function useModels() {
  const [sequenceId, setSequenceId] = useState(0);
  const lastRenderedId = useRef(0);

  const debouncedSearch = useRef(
    debounce(async (query: string, seqId: number) => {
      const results = await api.searchModelsFTS(query);

      // Sequence guard: discard stale responses
      if (seqId > lastRenderedId.current) {
        lastRenderedId.current = seqId;
        setModels(results.models);
      }
    }, 300)
  ).current;

  const handleSearch = (query: string) => {
    const newSeqId = sequenceId + 1;
    setSequenceId(newSeqId);
    debouncedSearch(query, newSeqId);
  };
}
```

2. **Add Background Refresh Indicator** (`frontend/src/components/ModelManager.tsx`)
   - [ ] Add "Refreshing..." spinner during background refresh
   - [ ] Add "New results available" toast (optional)
   - [ ] Prevent UI jitter with append-only updates

#### Testing

3. **SWR Tests**
   - [ ] Type "LLAMA" → only 1 API call made (debouncing)
   - [ ] Out-of-order responses discarded (sequence guard)
   - [ ] Cached results shown immediately
   - [ ] Background refresh updates without jitter

### Completion Criteria (Part C)

- [ ] 300ms debouncing reduces API calls
- [ ] Sequence guard prevents race conditions
- [ ] Instant UI response with background refresh
- [ ] All test scenarios pass

---

### Part D: Standardization

**Goal**: Align with industry conventions and improve documentation

**Status**: Ready to implement
**Estimated Complexity**: Low

#### Tasks

1. **Update Documentation**
   - [ ] Ensure all docs use standard terminology
   - [ ] Single-File Model (not "standalone model")
   - [ ] Diffusion Folder (not "diffusers repo")
   - [ ] Sharded Set (not "multi-part model")
   - [ ] Update comments in code
   - [ ] Update UI labels

2. **Update Metadata Schema**
   - [ ] Add `model_format` field: "single-file", "diffusion-folder", "sharded-set"
   - [ ] Deprecate old format indicators
   - [ ] Migration script for existing metadata

### Completion Criteria (Part D)

- [ ] All documentation uses standard terminology
- [ ] Code comments updated
- [ ] UI uses correct industry terms
- [ ] Metadata schema aligned

---

## Phase 5: Mapping UI (Future)

**Goal**: Visual interface for customizing model mappings

**Status**: Planning phase
**Estimated Complexity**: High
**Dependencies**: Phases 1-4 must be complete

### Planned Features

- Two-panel layout: Library models (left) ↔ App directories (right)
- Drag models from library to app folders
- Auto-generate filter rules based on selections
- Preview symlinks before applying
- Edit mapping rules visually
- Enable/disable individual mappings
- Create custom variants
- **Interactive conflict resolution dialog**:
  - Show conflicting file details (size, date, symlink target)
  - Options: [Rename Existing] [Overwrite with Link] [Skip] [Skip All]
  - Remember choice for batch operations
  - Display conflicts in dedicated dialog before sync

### Mockup

```
┌─────────────────────────────────────────────────────┐
│ Model Mapping: ComfyUI v0.6.0                      │
├──────────────────────┬──────────────────────────────┤
│ LIBRARY MODELS       │ COMFYUI FOLDERS             │
├──────────────────────┼──────────────────────────────┤
│ 📦 Diffusion (12)    │ 📁 checkpoints (3 mapped)   │
│   └ Checkpoints (8)  │    ├ sd-v1-5 ──────────┐    │
│      ├ sd-v1-5 ──────────────────────────────→│    │
│      ├ sdxl-base ────────────────────────────→│    │
│      └ ...           │    └ sdxl-base          │    │
│   └ LoRAs (4)        │                              │
│      ├ detail-lora   │ 📁 loras (1 mapped)         │
│      └ ...           │    └ detail-lora ──────┐    │
│                      │                         │    │
│ 🧠 LLM (5)          │ 📁 vae (0 mapped)           │
│   └ ...              │                              │
├──────────────────────┼──────────────────────────────┤
│ [Filter: All ▼]     │ [Preview Links] [Apply Now] │
└──────────────────────┴──────────────────────────────┘
```

### Tasks (Deferred)

1. Design drag-and-drop mapping interface
2. Create two-panel layout component
3. Implement drag-and-drop between panels
4. Add visual connection indicators
5. Implement filter rule generation
6. Add preview functionality
7. Integrate with Settings page

### Files to Create (Deferred)

- `frontend/src/components/MappingManager.tsx`
- `frontend/src/components/MappingRuleEditor.tsx`

---

## File Checklist

### Files to Create

**Phase 1**:
- [ ] `backend/model_library/io_manager.py` - Smart I/O queue manager with restricted environment detection
- [ ] `backend/model_library/fs_validator.py` - Filesystem validation with write permission checks
- [ ] `backend/model_library/platform_utils.py` - Platform abstraction for link creation *(v3.2)*
- [ ] `backend/model_library/network_manager.py` - Async httpx with circuit breaker *(v3.2)*
- [ ] `frontend/src/api/import.ts` - Separate ImportAPI class
- [ ] `frontend/src/components/ModelImportDropZone.tsx` - Drop zone overlay with preventDefault()
- [ ] `frontend/src/components/ModelImportDialog.tsx` - Import wizard with security warnings *(v3.2)*

**Phase 1 Part C**:
- [ ] `launcher-data/config/model-library-translation/comfyui_*_default.json` - Default config

**Phase 3**:
- [ ] `frontend/src/components/ConflictResolutionDialog.tsx` - Interactive conflict resolution *(v3.2)*
- [ ] `frontend/src/components/MappingPreviewDialog.tsx` - Mapping preview with conflicts

**Phase 5 (Future)**:
- [ ] `frontend/src/components/MappingManager.tsx` - Visual mapping editor
- [ ] `frontend/src/components/MappingRuleEditor.tsx` - Rule editor

### Files to Modify

**Phase 1**:
- [ ] `backend/model_library/naming.py` - Add NTFS sanitization functions
- [ ] `backend/model_library/importer.py` - Add stream hashing, atomic imports, move option
- [ ] `backend/model_library/library.py` - Add WAL mode, Deep Scan, retry lookups, status, security tier *(v3.2)*
- [ ] `backend/model_library/downloader.py` - Add async httpx, circuit breaker handling *(v3.2)*
- [ ] `backend/model_library/mapper.py` - Use platform_utils for link creation *(v3.2)*
- [ ] `backend/model_library/link_registry.py` - Use platform_utils, store link type *(v3.2)*
- [ ] `backend/api/core.py` - Add import batch API, network status, security tier *(v3.2)*
- [ ] `frontend/src/types/pywebview.d.ts` - Add import types, SecurityTier, NetworkStatus *(v3.2)*
- [ ] `frontend/src/hooks/useModels.ts` - Add library status check, indexing overlay
- [ ] `frontend/src/components/ModelManager.tsx` - Integrate drop zone, dialog, offline indicator *(v3.2)*
- [ ] `frontend/src/components/Settings.tsx` - Add Rebuild Index, drive override selector
- [ ] `frontend/src/index.css` - Add drop zone animations

**Phase 2**:
- [ ] `backend/model_library/link_registry.py` - Add auto_repair_links() *(v3.2)*
- [ ] `backend/model_library/library.py` - Add find_file_by_hash() *(v3.2)*
- [ ] `backend/api/core.py` - Add auto_repair_links() endpoint *(v3.2)*
- [ ] `frontend/src/components/Settings.tsx` - Add repair UI, health check enhancements *(v3.2)*

**Phase 3**:
- [ ] `backend/model_library/mapper.py` - Add conflict detection, sync_with_resolutions() *(v3.2)*
- [ ] `backend/version_manager.py` - Add auto-mapping setup, clean uninstall
- [ ] `backend/api/core.py` - Add preview_mapping(), sync_with_resolutions() *(v3.2)*
- [ ] `frontend/src/components/Settings.tsx` - Add preview → resolve → sync workflow *(v3.2)*

**Phase 4**:
- [ ] `backend/model_library/library.py` - Add FTS5 virtual table and triggers *(v3.2)*
- [ ] `backend/model_library/network_manager.py` - Enable HTTP/2 *(v3.2)*
- [ ] `frontend/src/hooks/useModels.ts` - Add SWR pattern, debouncing *(v3.2)*

**Phase 5 (Future)**:
- [ ] `frontend/src/components/Settings.tsx` - Add mapping tab

### Existing Files (No Changes Needed)

- `backend/model_library/mapper.py` - `apply_for_app()` already works (will be enhanced in Phase 2)
- `backend/models.py` - Metadata schemas already defined
- `shared-resources/models/` - Directory structure already correct

---

## Summary

**Total Implementation Effort** *(v3.2 adjusted)*:
- Phase 1: ~2-3 weeks (core + platform abstraction + security + circuit breaker)
- Phase 2: ~1-2 weeks (reliability + self-healing)
- Phase 3: ~1-2 weeks (UX polish + interactive conflict resolution)
- Phase 4: ~1 week (performance: FTS5, HTTP/2, SWR)
- Phase 5: ~2 weeks (mapping UI, future)

**Critical Path**:
1. Phase 1 → Phase 2 → Phase 3 → Phase 4 (sequential)
2. Testing throughout (unit → integration → system)
3. Phases 3 and 4 can be deferred; Phases 1 and 2 provide full functionality
4. Phase 4 only needed for large libraries (1000+ models)

**Key Milestones** *(v3.2)*:
- [ ] Phase 1 Complete: Import with security warnings, offline-safe with circuit breaker
- [ ] Phase 2 Complete: Self-healing links, robust reliability
- [ ] Phase 3 Complete: Interactive conflict resolution, improved UX
- [ ] Phase 4 Complete: Sub-20ms search for 1000+ models

**v3.2 Changes Summary**:
- Platform abstraction layer for future Windows support
- Pickle security warnings in import flow
- Circuit breaker networking (HTTP/1.1 in Phase 1, HTTP/2 in Phase 4)
- Link self-healing (auto-repair by hash)
- Interactive conflict resolution (overwrite/rename/skip)
- FTS5 and SWR deferred to Phase 4

---

**End of Implementation Phases Document**
