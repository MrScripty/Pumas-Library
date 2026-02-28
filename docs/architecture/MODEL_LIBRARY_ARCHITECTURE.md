# Model Library Architecture

## Purpose

Document the current model library architecture implemented in `pumas-core`.

## Canonical Storage

Model assets live under:

- `shared-resources/models/`

Core persisted artifacts:

- `models.db` (SQLite index for fast lookup/search/filter)
- per-model metadata files (including canonical metadata + overlays/overrides where applicable)

## Core Subsystems

### 1. Indexing and Search

- SQLite-backed index with FTS support for model discovery.
- Supports list/search/filter operations used by both library APIs and RPC handlers.
- Search cache for HuggingFace queries is separate (`shared-resources/cache/search.sqlite`).

### 2. Import Pipeline

- Imports local model files/directories into canonical library layout.
- Performs normalization, model-type/family inference, and hash recording.
- Preserves metadata fidelity while enforcing naming and consistency rules.

### 3. Download Pipeline (HuggingFace)

- `HuggingFaceClient` handles search + download flows.
- Download persistence is stored under launcher data cache.
- Auxiliary-file completion callback writes early metadata so in-progress downloads become visible to index consumers.

### 4. Mapping Pipeline

- `ModelMapper` maps canonical library records into app-specific target locations.
- Mapping configs live under `launcher-data/mapping-configs/`.
- Link strategy and path resolution are app/config dependent.

## Dependency Requirements Contract (0.2.x)

The old multi-step dependency endpoints were replaced by a resolve-only contract.

Current core surface:

- `resolve_model_dependency_requirements(model_id, platform_context, backend_key)`

Current RPC method:

- `resolve_model_dependency_requirements`

Response includes:

- `dependency_contract_version`
- per-binding requirement groups
- validation state and structured validation errors

Consumers are expected to:

1. check contract version compatibility
2. treat non-resolved states (`unknown_profile`, `invalid_profile`, `profile_conflict`) as non-ready
3. execute environment installation/check flows outside core library contract calls

## Metadata Projection

Library list/search responses may project active dependency binding references into model metadata views so consumers can reason about current dependency linkage without separate calls.

## App-Level Interaction

`pumas-rpc` and `pumas-app-manager` consume model library outputs for:

- model linking and app compatibility workflows
- dependency requirement UI/reporting
- import/download operations in desktop workflows

## Operational Guarantees

- API startup favors resilience (best-effort behavior around external systems where possible).
- Database/index consistency is maintained through explicit index update paths in import/download/mapping operations.
- Contracted payloads (especially dependency requirements) are versioned for consumer safety.

## Related Code

- `rust/crates/pumas-core/src/model_library/`
- `rust/crates/pumas-core/src/index/`
- `rust/crates/pumas-rpc/src/handlers/models/`
- `rust/crates/pumas-rpc/src/handlers/links.rs`
