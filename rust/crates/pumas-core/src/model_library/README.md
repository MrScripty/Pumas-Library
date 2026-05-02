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
| `importer/` | Recovery-oriented `ModelImporter` helpers for orphan adoption, interrupted-download discovery, and shard recovery |
| `directory_import.rs` | Side-effect-free import-path classification for files, bundle roots, single model directories, and multi-model containers |
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

## Problem

Provide a single backend-owned model registry that can import, classify, validate, index, and map models while preserving SQLite as the canonical persisted state for consumers and recovery flows.

## Constraints
- SQLite is the single source of truth for queryable model state.
- `metadata.json` is a derived on-disk projection and must not become a competing authority.
- Startup, watcher, and reconcile flows must avoid steady-state churn on unchanged libraries.
- Download recovery, orphan adoption, and external-reference assets must reuse the same model-state pipeline.

## Decision

- **Phased Mutation** for library merge: Gather (read-only) -> Validate -> Move -> Index -> Cleanup.
  This ensures no data loss if any phase fails.
- **Dual hashing** (SHA256 + BLAKE3): SHA256 for HuggingFace LFS compatibility, BLAKE3 for fast
  local deduplication.
- **Symlink-first mapping**: Prefer symlinks over hardlinks for cross-filesystem support; fall back
  to hardlinks when symlinks are unavailable (Windows without developer mode).
- **In-place import**: Models already on disk (post-download or orphan recovery) skip the copy step,
  importing metadata directly.
- **Persisted HF evidence**: Normalized Hugging Face provenance is captured before download,
  enriched during file selection, and persisted into `metadata.json`/the SQLite index so later
  local evaluation does not depend on transient API responses.
- **Single resolver, staged evidence**: Model typing runs through one resolver for remote-only,
  partial-download, and fully imported models. The evidence set grows by stage; the resolver does
  not change by phase.
- **Repair-before-report migration flow**: Migration dry-run and execution must
  operate on reconciled library state so duplicate cleanup and path/family
  normalization are reflected in generated reports.
- **External-reference assets**: Directory-root bundles must extend the existing metadata/index
  system instead of introducing a second registry or runtime-routing contract.
- **Backend-owned path classification**: Drag/drop and picker intake must classify raw paths
  through the model library before import so bundle/container decisions stay deterministic.
- **Package facts as read-only projection**: `resolve_model_package_facts`
  exposes bounded package evidence from existing metadata and package files
  without adding runtime selection policy or a new persisted source of truth.
- **Recovery helper split**: Filesystem repair and recovery scans stay in `importer/` child
  modules so the main importer keeps the copy/hash/metadata pipeline readable without widening
  `ModelImporter` visibility.

## Alternatives Rejected
- File-first source of truth with SQLite as a cache: rejected because recovery, partial-download handling, and query consistency would depend on repeated filesystem projection.
- Separate registries for downloaded, external, and imported models: rejected because classification and reconcile behavior would drift across model lifecycles.

## Invariants
- SQLite remains canonical for persisted model state and query behavior.
- `metadata.json` is a derived projection that should change only when derived model content changes.
- `dependency_bindings` in `metadata.json` are projected from active SQLite binding rows and must never be used as dependency-resolution authority.
- Library-owned diffusers bundles must project `source_path` and `entry_path` back to the canonical library model directory.
- Watcher-triggered reconcile must not loop on Pumas-owned derived writes.
- Duplicate cleanup, reclassification, and index rebuild must be idempotent on unchanged libraries.
- Saved Hugging Face evidence must remain available for future backfill and
  reclassification passes even when the original remote lookup is not repeated.
- Package-fact resolution must stay read-only until lazy package-fact
  persistence is explicitly introduced. It may parse bounded package metadata
  files, but it must not execute Python or load Transformers classes.

## Revisit Triggers
- A second persisted source of truth is introduced for model-state queries.
- External tools need to author canonical state outside SQLite.
- Idempotence or startup-latency requirements cannot be met without schema or architecture changes.

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

## Related ADRs
- None identified as of 2026-03-11.
- Reason: the SQLite-canonical / derived-metadata contract is currently recorded in implementation plans and module documentation rather than a standalone ADR.
- Revisit trigger: another subsystem or repository starts depending on these persistence semantics as a formal cross-team contract.

## API Consumer Contract

- Metadata payloads may include `recommended_backend` as an optional runtime hint.
- `recommended_backend` is deterministic-only and remains `null` when signals are ambiguous.
- Canonical values are lowercase backend tokens (`llama.cpp`, `onnx-runtime`, etc.).
- Consumers must treat missing/`null` as "fallback heuristics required."
- Metadata payloads may include `huggingface_evidence` and should treat it as audit/provenance
  data owned by the backend classifier, not as a UI-authored override.
- External directory-root assets must be consumed through a dedicated execution descriptor rather
  than `primary_file`-style path resolution.

## Structured Producer Contract

- SQLite/indexed metadata is the canonical persisted model state and query surface.
- `metadata.json` is a derived on-disk projection of the same model record and must not become a competing source of truth.
- Dependency resolution and runtime autobind repair must read authoritative binding state from SQLite plus canonical bundle filesystem facts, not from projected `metadata.json` fields.
- External-reference assets extend persisted metadata with `source_path`, `entry_path`,
  `storage_kind`, `bundle_format`, `pipeline_class`, `import_state`, and asset validation fields.
- Execution descriptors for `storage_kind=library_owned` diffusers bundles must resolve to the canonical library model directory even when projected path fields are stale.
- Download flows may create a preliminary metadata record with `match_source = download_partial`
  before weight files complete so recovery/reclassification can reuse persisted HF evidence.
- `huggingface_evidence` stores normalized remote facts and selected-file context. Resolved
  `model_type` stays separate so future resolver improvements do not destroy source evidence.
- Bulk repair or backfill flows must use stored evidence to reproject task and
  model typing without requiring model-weight re-download.
- These fields describe asset ownership and current executable health; they must not create a
  second source-of-truth outside the model-library metadata/index flow.
- Fields intentionally volatile in derived artifacts include timestamps and validation state that
  are refreshed only when underlying derived content changes.
- Regeneration rule for `dependency_bindings`: after index/rebuild/runtime-autobind changes active
  SQLite bindings, rewrite projected binding refs from SQLite before treating the projection as current.
- Regeneration rule for library-owned diffusers paths: after index/rebuild, rewrite `source_path`
  and `entry_path` to the canonical library model directory when the projected values drift.
- Regeneration rule: when SQLite-backed model state changes, regenerate `metadata.json` only if the
  projected content differs.
- Compatibility expectation for milestone one is append-only: new optional fields may be added,
  but existing file-based model records must remain readable without migration-only consumers.
