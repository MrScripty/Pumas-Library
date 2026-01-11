# Model Library Implementation Progress

**Last Updated:** 2026-01-10
**Current Phase:** Phase 1A - Core Infrastructure
**Status:** Planning Complete - Ready to Begin Implementation

---

## Phase 1A: Core Infrastructure

### Prerequisite: Refactor downloader.py
**Goal:** Split 996-line downloader.py into focused <300 line modules

- [ ] Create directory structure (hf/, io/, network/, search/)
- [ ] Extract hf/client.py (HTTP/2 client wrapper)
- [ ] Extract hf/throttle.py (rate limiting)
- [ ] Extract hf/metadata_lookup.py (~250 lines)
- [ ] Extract hf/file_download.py (~200 lines)
- [ ] Extract hf/cache.py (LRU cache)
- [ ] Slim downloader.py to coordinator (~150 lines)

### Part 1: I/O Infrastructure
**Goal:** Drive-aware file operations with stream hashing

- [ ] io/manager.py - Drive-aware I/O queue
- [ ] io/validator.py - Filesystem validation
- [ ] io/hashing.py - Stream hashing utilities
- [ ] io/platform.py - Platform abstraction for links

### Part 2: Networking Infrastructure
**Goal:** Robust HTTP/2 networking with circuit breaker

- [ ] network/circuit_breaker.py - Circuit breaker state machine
- [ ] network/retry.py - Exponential backoff retry
- [ ] network/manager.py - NetworkManager coordinator

### Part 3: Search Infrastructure
**Goal:** FTS5 full-text search for fast queries

- [ ] search/fts5.py - FTS5 virtual table setup
- [ ] search/query.py - Search query builder

### Part 4: Update Existing Files
**Goal:** Integrate new modules into existing codebase

- [ ] importer.py - Use io/* modules for stream hashing
- [ ] library.py - Add FTS5 support
- [ ] mapper.py - Use io/platform for link creation
- [ ] naming.py - Add NTFS sanitization functions

### Part 5: API Layer
**Goal:** Expose new functionality via PyWebView API

- [ ] api/core.py - Add import batch endpoints
- [ ] api/core.py - Add FTS5 search endpoint
- [ ] api/core.py - Add network status endpoint

### Part 6: Frontend (TypeScript FIRST!)
**Goal:** User interface for model import

- [ ] **CRITICAL:** frontend/src/types/pywebview.d.ts - TypeScript types
- [ ] frontend/src/api/import.ts - ImportAPI class
- [ ] frontend/src/components/ModelImportDropZone.tsx
- [ ] frontend/src/components/ModelImportDialog.tsx
- [ ] frontend/src/hooks/useModels.ts - Add debouncing + SWR
- [ ] frontend/src/components/ModelManager.tsx - Integrate UI

---

## Phase 1B: Link Registry Database

**Status:** Not Started (depends on Phase 1A)

- [ ] backend/model_library/link_registry.py
- [ ] Create registry.db schema
- [ ] Add cascade delete support
- [ ] Add health check methods
- [ ] Add broken link detection
- [ ] Add orphaned link detection
- [ ] Update API endpoints
- [ ] Add frontend health status UI

---

## Phase 1C: Basic Link Mapping System

**Status:** Not Started (depends on Phase 1A + 1B)

- [ ] Create default ComfyUI mapping config
- [ ] Add dynamic directory discovery
- [ ] Add version constraint validation
- [ ] Add incremental sync
- [ ] Add sandbox detection
- [ ] Add cross-filesystem warnings
- [ ] Update version_manager.py integration
- [ ] Add mapping preview UI

---

## Completed Items

(None yet - implementation starting)

---

## Blockers / Issues

(None currently)

---

## Notes

- Pre-commit hooks enforce: Black, isort, no print(), specific exceptions, pytest, mypy, coverage
- All new files must have ≥80% test coverage
- Target: Files < 700 lines (ideally < 300)
- Atomic commits: one feat/fix/chore at a time

---

**Next Session Focus:** Refactor downloader.py into hf/* modules
