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
| `external_assets.rs` | External diffusers bundle validation, metadata construction, and execution-contract constants |
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
- **External-reference assets**: Directory-root bundles must extend the existing metadata/index
  system instead of introducing a second registry or runtime-routing contract.

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

## API Consumer Contract

- Metadata payloads may include `recommended_backend` as an optional runtime hint.
- `recommended_backend` is deterministic-only and remains `null` when signals are ambiguous.
- Canonical values are lowercase backend tokens (`llama.cpp`, `onnx-runtime`, etc.).
- Consumers must treat missing/`null` as "fallback heuristics required."
- External directory-root assets must be consumed through a dedicated execution descriptor rather
  than `primary_file`-style path resolution.

## Structured Producer Contract

- `metadata.json` under the library root remains the canonical persisted model-record artifact.
- External-reference assets extend persisted metadata with `source_path`, `entry_path`,
  `storage_kind`, `bundle_format`, `pipeline_class`, `import_state`, and asset validation fields.
- These fields describe asset ownership and current executable health; they must not create a
  second source-of-truth outside the model-library metadata/index flow.
- Compatibility expectation for milestone one is append-only: new optional fields may be added,
  but existing file-based model records must remain readable without migration-only consumers.
