# Model Library System Documentation

**Version**: 3.2

---

## Overview

This directory contains the complete implementation plan for the Pumas Library Model Management System. The documentation has been split into focused, self-contained documents for easier navigation and reference.

---

## Documentation Structure

### [00-overview.md](00-overview.md)
**High-level architecture and goals**

- System goals and key requirements
- Technology stack and platform requirements
- Directory structure and library layout
- Metadata schema (Single Source of Truth)
- ComfyUI integration overview
- Core design principles
- Success criteria

**Read this first** to understand the overall system architecture.

---

### [01-performance-and-integrity.md](01-performance-and-integrity.md)
**Performance optimizations and data integrity strategies**

- Smart I/O Queue Manager (drive-aware concurrency)
- Stream Hashing (hash-while-copy)
- Incremental Sync Strategy (~2200× speedup)
- HuggingFace API Throttling
- Atomic Operations with `.tmp` files
- SQLite Write-Ahead Logging (WAL)
- **Link Registry Database** (registry.db for tracking all symlinks)
- **Deep Scan File Verification** (prevent phantom models)
- **Ghost State Health Checks** (detect broken/orphaned links)
- **Drive/Mount-Point Relocation** (bulk update external paths)
- Offline-First Import Strategy
- Filesystem Pre-Flight Validation
- NTFS Filename Normalization
- Cross-Filesystem Detection
- NVMe Device Handling
- Sandbox Detection (Flatpak/Snap/Docker)

**Read this** to understand how the system achieves high performance and reliability.

---

### [02-model-import.md](02-model-import.md)
**Drag-and-drop model import system**

- Visual design and interaction states
- Component architecture (DropZone, ImportDialog)
- **Sharded Set Grouping** (auto-detect multi-file models)
- HuggingFace metadata lookup (hybrid hash + filename)
- Trust badges (Verified, High Confidence, Low Confidence)
- Progressive disclosure of technical details
- Import modes (Fast Move vs Safe Copy)
- Space management (delete originals)
- Backend API implementation
- Frontend TypeScript types
- Complete import flow diagram
- Testing strategy

**Read this** to implement the drag-and-drop import feature.

---

### [03-mapping-system.md](03-mapping-system.md)
**Configuration-based model mapping**

- Configuration schema (JSON format)
- File naming convention and precedence rules
- Default ComfyUI configuration
- Dynamic directory discovery
- **Mapping Preview (Dry Run)** - preview all changes with conflict detection
- **Conflict Resolution** - skip+warn strategy with detailed reporting
- Mapping engine internals
- Sync strategies (auto, incremental, manual)
- Version constraints (PEP 440)
- Link types (relative symlink, absolute symlink, hard link)
- Backend implementation details
- Frontend integration (warnings, sync button)
- Testing strategy

**Read this** to implement the model mapping system.

---

### [04-implementation-phases.md](04-implementation-phases.md)
**Concrete implementation steps and file checklist**

- **Phase 1: Core Infrastructure** (import, registry, mapping)
- **Phase 2: Data Integrity & Reliability** (file verification, health checks)
- **Phase 3: User Experience** (sharded sets, preview, relocation)
- **Phase 4: Standardization** (industry terminology)
- **Phase 5: Mapping UI** (future, visual editor)
- Complete file checklist (create/modify)
- Completion criteria for each phase
- Testing requirements
- Implementation priority rationale

**Read this** to execute the implementation.

---

### [05-networking-and-caching.md](05-networking-and-caching.md)
**Unified network stack and advanced caching strategies**

- **HTTP/2 Multiplexing** (concurrent requests over single TCP connection)
- **Circuit Breaker Pattern** (fail-fast on network failures, 60s blackout)
- **FTS5 Full-Text Search** (sub-millisecond prefix matching with unicode61 tokenizer)
- **Dual-Path Strategy (Stale-While-Revalidate)** (instant UI with background refresh)
- **Search Coordination** (300ms debouncing, sequence guard for race conditions)
- **Rate Limit Detection** (proactive throttling via X-RateLimit headers)
- Integration with existing downloader.py and models.db
- NetworkManager implementation
- Migration strategy for FTS5
- Testing strategy

**Read this** to implement advanced networking and search features.

---

### [06-implementation-integration-guide.md](06-implementation-integration-guide.md)
**Integration guide for merging networking features into the existing plan**

- **Compatibility Matrix** (how networking integrates with existing components)
- **Integration Points** (where to add networking features in each phase)
- **Updated Implementation Timeline** (Phase 1 with networking tasks)
- **File Dependencies** (what files to create/modify and in what order)
- **Testing Integration** (unified test strategy)
- **Migration Path** (how to add networking to existing implementations)
- Step-by-step integration strategy
- Critical path and dependencies
- Risk assessment for each integration step

**Read this** when integrating networking features into an existing or in-progress implementation.

---

### [07-v3.2-changelog.md](07-v3.2-changelog.md)
**v3.2 changelog and design rationale (reference only)**

- What changed in v3.2 and why
- Critique that prompted changes
- Design decision rationale
- Migration notes from v3.1

**You do NOT need to read this for implementation.** All changes are integrated into the main documents. This file explains the reasoning behind v3.2 decisions for historical reference.

---

## Quick Start Guide

### For Developers

1. **Start here**: Read [00-overview.md](00-overview.md) to understand the system architecture
2. **Understand optimizations**: Read [01-performance-and-integrity.md](01-performance-and-integrity.md)
3. **Pick a phase**:
   - Implementing import? → [02-model-import.md](02-model-import.md) + [04-implementation-phases.md](04-implementation-phases.md#phase-1-core-infrastructure)
   - Implementing mapping? → [03-mapping-system.md](03-mapping-system.md) + [04-implementation-phases.md](04-implementation-phases.md#phase-1-core-infrastructure)
   - Adding networking/search? → [05-networking-and-caching.md](05-networking-and-caching.md) + [04-implementation-phases.md](04-implementation-phases.md#phase-4-performance--scale)
   - Adding reliability features? → [01-performance-and-integrity.md](01-performance-and-integrity.md) + [04-implementation-phases.md](04-implementation-phases.md#phase-2-reliability--self-healing)
4. **Follow the checklist**: Use [04-implementation-phases.md](04-implementation-phases.md#file-checklist) to track progress

### For Reviewers

1. Read [00-overview.md](00-overview.md) for context
2. Review architecture decisions in [01-performance-and-integrity.md](01-performance-and-integrity.md)
3. Review networking strategy in [05-networking-and-caching.md](05-networking-and-caching.md)
4. Verify UI/UX design in [02-model-import.md](02-model-import.md#visual-design) and [03-mapping-system.md](03-mapping-system.md#frontend-integration)
5. Check implementation plan in [04-implementation-phases.md](04-implementation-phases.md)
6. (Optional) Review v3.2 rationale in [07-v3.2-changelog.md](07-v3.2-changelog.md)

---

## Key Features

### Model Import
- ✅ Drag-and-drop onto GUI
- ✅ Automatic HuggingFace metadata lookup
- ✅ Hash verification for 100% accuracy
- ✅ Fuzzy filename fallback
- ✅ Trust badges (Verified, High Confidence, Low Confidence)
- ✅ **Sharded set grouping** (auto-detect multi-file models)
- ✅ Stream hashing (40% faster)
- ✅ Atomic operations (crash-safe)
- ✅ Offline-first design
- ✅ Fast import mode (instant for same filesystem)
- ✅ Optional delete originals

### Model Mapping
- ✅ JSON-based configuration
- ✅ Wildcard version support
- ✅ Dynamic directory discovery
- ✅ **Mapping preview (dry run)** with conflict detection
- ✅ **Conflict resolution** with skip+warn strategy
- ✅ Incremental sync (~2200× faster)
- ✅ Relative symlinks (portable)
- ✅ Cross-filesystem support (absolute symlinks with warnings)
- ✅ Version constraints (PEP 440)
- ✅ Auto-sync on import and install
- ✅ Manual sync API
- ✅ Clean uninstall

### Performance & Reliability
- ✅ Drive-aware I/O (no HDD thrashing)
- ✅ Stream hashing (no double-read)
- ✅ SQLite WAL mode (concurrent access)
- ✅ **Link registry database** (registry.db) for cascade operations
- ✅ **Deep Scan file verification** (prevent phantom models)
- ✅ **Ghost state health checks** (detect broken/orphaned links)
- ✅ **Drive relocation helper** (bulk update external paths)
- ✅ **Cascade delete** (clean model removal with all links)
- ✅ Deep Scan rebuild (from metadata.json)
- ✅ API throttling (rate-limit safe)
- ✅ Offline imports (retry later)
- ✅ NTFS-compatible filenames
- ✅ Sandbox detection
- ✅ NVMe device support

### Networking & Search
- ✅ **Circuit breaker pattern** (fail-fast on network failures, 60s blackout) - Phase 1
- ✅ **Async httpx** (7s timeout, offline-first) - Phase 1
- ✅ Offline indicator ("Using Cached Data") - Phase 1
- ⏸️ **HTTP/2 multiplexing** (concurrent requests over single TCP connection) - Phase 4
- ⏸️ **FTS5 full-text search** (sub-millisecond prefix matching) - Phase 4
- ⏸️ **Dual-path strategy (Stale-While-Revalidate)** (instant UI with background refresh) - Phase 4
- ⏸️ **Search coordination** (300ms debouncing, sequence guard for race conditions) - Phase 4
- ⏸️ **Rate limit detection** (proactive throttling via X-RateLimit headers) - Phase 4

### Industry Standardization
- ✅ **Standard terminology** (Single-File Model, Diffusion Folder, Sharded Set)
- ✅ Consistent naming conventions
- ✅ Aligned with industry best practices

---

## Implementation Status

| Phase | Status | Timeline | Completion |
|-------|--------|----------|-----------|
| Phase 1: Core Infrastructure + Circuit Breaker | 📋 Planning Complete (v3.2) | 2-3 weeks | 0% |
| Phase 2: Reliability & Self-Healing | 📋 Planning Complete (v3.2) | 1-2 weeks | 0% |
| Phase 3: UX Polish & Conflict Resolution | 📋 Planning Complete (v3.2) | 1-2 weeks | 0% |
| Phase 4: Performance & Scale (HTTP/2, FTS5) | 📋 Planning Complete (v3.2) | 1 week | 0% |
| Phase 5: Mapping UI | 🔮 Future | 2 weeks | 0% |

---

## Architecture Decisions

### Why these choices?

1. **Metadata as SSoT**: `metadata.json` files are the authoritative source, SQLite is a disposable cache
2. **Link Registry**: `registry.db` tracks all symlinks for cascade deletion, health validation, and relocation
3. **Atomic Operations**: `.tmp` files prevent partial imports from being indexed
4. **Offline-First**: Network failures never block local operations
5. **Incremental Sync**: Only process what changed (~2200× speedup)
6. **Hybrid HF Lookup**: Hash verification (100% accurate) with filename fallback (graceful degradation)
7. **Dynamic Discovery**: Scan actual ComfyUI installation instead of hardcoding directories
8. **Relative Symlinks**: Portable within same filesystem, absolute with warnings for cross-filesystem
9. **Mapping Preview**: Dry-run before execution prevents surprises and shows conflicts
10. **Industry Terminology**: Standard names (Single-File Model, Diffusion Folder, Sharded Set) align with conventions
11. **Circuit Breaker**: Fail-fast on network failures (7s timeout, 60s blackout) prevents UI ghost hangs (Phase 1)
12. **Platform Abstraction**: `platform_utils.py` enables future Windows support without refactoring (v3.2)
13. **Link Self-Healing**: Auto-repair broken symlinks by searching for models by hash (Phase 2)
14. **Pickle Security**: Warning banner for unsafe formats educates users without blocking (Phase 1)
15. **Interactive Conflicts**: Ask users how to resolve conflicts instead of silent skip (Phase 3)
16. **HTTP/2 Multiplexing**: Single TCP connection for concurrent requests reduces TLS handshake overhead (Phase 4)
17. **FTS5 Full-Text Search**: Sub-millisecond prefix matching with unicode61 tokenizer for instant UI updates (Phase 4)
18. **Stale-While-Revalidate**: Instant UI response (<20ms) with background refresh for up-to-date data (Phase 4)
19. **Sequence Guard**: Prevents race conditions where out-of-order API responses overwrite newer searches (Phase 4)

---

## Testing Strategy

### Test Coverage

- **Unit Tests**: Individual functions and methods
- **Integration Tests**: Multi-component workflows
- **System Tests**: End-to-end user scenarios
- **UI Tests**: Component rendering and interactions

### Critical Test Scenarios

1. **The Move Test**: Move entire Pumas-Library folder → relative symlinks remain valid
2. **Deep Scan**: Delete SQLite DB → Rebuild from metadata.json → All models restored (file verification prevents phantoms)
3. **Offline Import**: Import without network → Connect later → Retry enriches metadata
4. **Crash Recovery**: Kill process mid-import → No partial files indexed
5. **Cross-Filesystem**: Library on HDD, ComfyUI on SSD → Absolute symlinks with warnings
6. **Cascade Delete**: Delete model with 50 links → All links removed, registry purged, files deleted
7. **Health Check**: Manually delete source files → Health check detects broken links → Cleanup button removes them
8. **Circuit Breaker Test**: Disconnect network → 3 API failures → Circuit opens → "Using Cached Data" shown → Reconnect → Circuit closes after 60s
9. **Pickle Warning**: Import .ckpt file → Red banner shown → Checkbox required → Import proceeds only after acknowledgment
10. **Link Self-Healing**: Delete model source → Health check detects → Auto-repair searches by hash → Link recreated with new source
11. **Conflict Resolution**: Sync with conflicts → Dialog shows all conflicts → User picks overwrite/rename/skip per model → Sync executes
12. **Sharded Set Import**: Drop 5-part model → Groups as one, all shards mapped together
13. **Mapping Preview**: Sync with conflicts → Preview shows warnings → User decides whether to proceed
14. **Drive Relocation**: External drive mount changes → Bulk update all absolute paths
15. **Search Performance** (Phase 4): 1000+ models → Search query → Results appear in <20ms using FTS5
16. **Race Condition Test** (Phase 4): Type "LLAMA" quickly → 5 searches triggered → Only final results rendered (sequence guard)

---

## Migration Notes

### From Original Plan

The original `model-import-and-mapping-system.md` (5965 lines) has been refactored into:
- 4 focused documents (~1000-1500 lines each)
- 1 README (this file)

**Changes**:
- ✅ Removed revision history and addendums
- ✅ Unified all implementation details into coherent sections
- ✅ No "cobbled together" references to past versions
- ✅ Each document is self-contained and reads as a single unified plan

**v3.2 Amendments** (2026-01-10):
- ✅ Added platform abstraction layer for future Windows support
- ✅ Added pickle security warnings (Phase 1)
- ✅ Added link self-healing (Phase 2)
- ✅ Moved HTTP/2 and FTS5 to Phase 4 (MVP simplification)
- ✅ Added interactive conflict resolution (Phase 3)

**Deprecated File**:
- `docs/plans/model-import-and-mapping-system.md` (can be deleted after verification)

---

## Contributing

When updating these documents:

1. **Maintain self-containment**: Each document should stand alone
2. **Update version numbers**: Bump version at top of changed documents
3. **Update "Last Updated" date**: Keep dates current
4. **Update this README**: If you add/remove documents, update the structure section
5. **Cross-reference when needed**: Link to other documents, but include enough context

---

## Known Limitations

### Platform Support
- ✅ **Linux** (v1.0) - Full support for ext4, Btrfs, NTFS
- 🔮 **Windows** (v2.0) - Designed for, not yet implemented
- ❌ **macOS** - Not planned

### Symlink Behavior
- ✅ Same filesystem - Portable relative links
- ⚠️ Cross-filesystem - Absolute links with warnings
- ⚠️ Sandboxed environments - Requires permissions (Flatpak/Snap/Docker)
- ✅ Drive unmounting - Self-healing in Phase 2

### Security
- ✅ Pickle warnings (.ckpt, .pt, .bin) - Phase 1
- ❌ Deep scanning - Planned for Phase 3+
- ✅ Safetensors recommended

### Search Performance
- ✅ <100ms for libraries up to 1000 models (LIKE query, Phase 1)
- ⏸️ <20ms for 5000+ models (FTS5, Phase 4)

---

## Questions?

- Architecture questions? → [00-overview.md](00-overview.md)
- v3.2 rationale questions? → [07-v3.2-changelog.md](07-v3.2-changelog.md)
- Performance questions? → [01-performance-and-integrity.md](01-performance-and-integrity.md)
- Import UI questions? → [02-model-import.md](02-model-import.md)
- Mapping config questions? → [03-mapping-system.md](03-mapping-system.md)
- Implementation questions? → [04-implementation-phases.md](04-implementation-phases.md)
- Networking questions? → [05-networking-and-caching.md](05-networking-and-caching.md)

---

**End of README**
