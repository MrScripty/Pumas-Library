# Model Library

## Purpose

Core model storage, metadata management, and HuggingFace integration. The model library is the
central registry for managing canonical AI model storage with content-based type detection,
SHA256/BLAKE3 hash verification, symlink/hardlink mapping to application directories, and
full-text search via SQLite FTS5.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, re-exports public API |
| `library.rs` | `ModelLibrary` - Central registry managing directory structure, metadata, and FTS5 index |
| `types.rs` | Data structures: `ModelType`, `ModelMetadata`, `ModelOverrides`, re-exports from `models` |
| `importer.rs` | `ModelImporter` - Import local files with hash verification, in-place import, orphan recovery |
| `mapper.rs` | `ModelMapper` - Link models to application directories via symlinks/hardlinks |
| `hf_client.rs` | `HuggingFaceClient` - HF Hub API integration: search, download, metadata lookup |
| `hf_cache.rs` | `HfSearchCache` - Cached HuggingFace search results and repo details |
| `identifier.rs` | GGUF metadata extraction and model type identification |
| `naming.rs` | Model name normalization and base name extraction |
| `hashing.rs` | Dual-hash computation (SHA256 + BLAKE3) and fast-hash for dedup |
| `link_registry.rs` | `LinkRegistry` - Tracks created symlinks/hardlinks for cascade delete |
| `watcher.rs` | `ModelLibraryWatcher` - Filesystem watcher triggering index rebuilds on changes |
| `download_store.rs` | `DownloadPersistence` - Crash-recovery persistence for paused/errored downloads |
| `merge.rs` | `LibraryMerger` - Consolidate duplicate libraries with hash-based dedup (Phased Mutation) |
| `sharding.rs` | Sharded model detection and completeness validation |
| `hf/` | HuggingFace helper submodule |
| `CACHING.md` | Documentation for the caching strategy |

## Design Decisions

- **Phased Mutation** for library merge: Gather (read-only) -> Validate -> Move -> Index -> Cleanup.
  This ensures no data loss if any phase fails.
- **Dual hashing** (SHA256 + BLAKE3): SHA256 for HuggingFace LFS compatibility, BLAKE3 for fast
  local deduplication.
- **Symlink-first mapping**: Prefer symlinks over hardlinks for cross-filesystem support; fall back
  to hardlinks when symlinks are unavailable (Windows without developer mode).
- **In-place import**: Models already on disk (post-download or orphan recovery) skip the copy step,
  importing metadata directly.

## Dependencies

### Internal
- `crate::index` - FTS5 full-text search index
- `crate::metadata` - Atomic JSON persistence
- `crate::network` - Web source traits for HuggingFace client registration
- `crate::models` - Shared data types (DTOs)

### External
- `rusqlite` - SQLite database access
- `reqwest` - HTTP client for HuggingFace API
- `notify` / `notify-debouncer-mini` - Filesystem watching
- `walkdir` - Recursive directory traversal
- `blake3` / `sha2` - Hash computation
- `regex` - Shard pattern detection
