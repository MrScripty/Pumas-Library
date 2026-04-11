# Model Index Submodules

## Purpose
This directory splits specialized `ModelIndex` table logic out of the main `model_index.rs` file. It isolates dependency-profile/binding persistence and metadata-overlay/history persistence so those SQLite-backed concerns can evolve without burying them deeper inside the already large primary index module.

## Contents
| File/Folder | Description |
|-------------|-------------|
| `governance.rs` | Metadata-v2 schema/bootstrap helpers plus link-exclusion, task-signature mapping, and model-type rule queries. |
| `dependency_profiles.rs` | `ModelIndex` methods for dependency profile versioning and model-to-profile binding rows. |
| `metadata_overlays.rs` | `ModelIndex` methods for metadata overlays, baselines, effective metadata reads, and append-only history records. |

## Problem
The model index owns more than search rows: it also persists governance tables for task-signature mappings and model-type rules, dependency profile contracts, and metadata override history. Those concerns need transactional SQLite access through the same `ModelIndex` connection, but leaving all of that logic in `model_index.rs` makes the index boundary harder to review and maintain.

## Constraints
- SQLite remains the canonical store for index-backed model state.
- Governance seed rows for task signatures and model-type rules must remain idempotent across reopen/migration paths.
- Dependency profile versions must be immutable once published for a given `(profile_id, profile_version)`.
- Effective metadata reads must merge a baseline row plus the latest active overlay deterministically.
- History rows must remain ordered and append-only enough to support audits and repair flows.

## Decision
- Keep these methods as `impl ModelIndex` blocks in separate files so they still share the same connection, transactions, and error types.
- Separate governance/schema helpers from dependency-profile and metadata-overlay helpers because they own different tables, migration behavior, and query surfaces.
- Separate dependency-profile logic from metadata-overlay logic because they mutate different tables and have different correctness rules.
- Canonicalize dependency profile specs before persistence so hash comparisons and immutability checks are stable.

## Alternatives Rejected
- Keep dependency and overlay persistence inside `model_index.rs`: rejected because the main file is already too broad and obscures table-specific invariants.
- Keep governance seed/migration logic inline with search/index CRUD code: rejected because schema bootstrap and table-family queries are a separate responsibility cluster.
- Create separate repository/service objects with their own SQLite handles: rejected because these operations need to stay inside the same index transaction and locking model.

## Invariants
- Task-signature seed rows and model-type rule repairs must be safe to run repeatedly on startup.
- A `(profile_id, profile_version)` pair must not change content after publication.
- Effective metadata is always computed as `baseline + latest active overlay`.
- Metadata history rows are returned in deterministic `(created_at, event_id)` order.
- Overlay writes and supersession happen transactionally so readers never observe a half-applied transition.

## Revisit Triggers
- New index-owned table families are added and start competing for ownership inside `ModelIndex`.
- Governance policy becomes externally managed and needs a separate source of truth from SQLite bootstrap rows.
- Overlay logic grows into workflow orchestration that should live above the raw SQLite layer.
- Dependency profile resolution needs cross-table operations that no longer fit cleanly as low-level index methods.

## Dependencies
**Internal:** `crate::index::ModelIndex`, `crate::model_library::dependency_pins`, shared index row structs, and `crate::error`/`Result`.
**External:** `rusqlite` for transactional persistence and `serde_json` for overlay JSON handling.

## Related ADRs
- None identified as of 2026-04-11.
- Reason: the split between search rows, dependency rows, and metadata overlays is documented in module boundaries rather than a formal ADR.
- Revisit trigger: a second storage backend or replicated index implementation is introduced.

## Usage Examples
```rust
let changed = index.upsert_dependency_profile(&profile_record)?;
let effective = index.get_effective_metadata_json(model_id)?;
```

```rust
let mapping = index.get_active_task_signature_mapping("text->image")?;
let model_type = index.resolve_model_type_hint("image-text-to-text")?;
```

## API Consumer Contract
- None identified as of 2026-04-11.
- Reason: this directory is an internal persistence layer behind `ModelIndex`, not a direct host-facing API.
- Revisit trigger: these submodules are exposed outside the crate or mirrored in bindings.

## Structured Producer Contract
- `dependency_profiles` rows store canonicalized `spec_json` plus a stable `profile_hash`; callers must not rely on input-order preservation from the original JSON.
- `task_signature_mappings` and model-type rule tables are SQLite-governed policy rows; callers must treat active rows as backend-owned configuration rather than user-authored state.
- `model_dependency_bindings` rows reference immutable profile versions and are the authoritative active binding source for downstream projection.
- `model_metadata_overlays` store merge-patch JSON fragments, not full metadata snapshots.
- `model_metadata_history` is append-oriented audit data and must preserve deterministic ordering for consumers that diff or replay changes.
- Compatibility rule: new optional fields may be added to these row projections, but existing rows must remain readable by current index code.

## Regeneration Rules
- Recompute `profile_hash` from canonicalized JSON before comparing or writing dependency profile rows.
- Rebuild effective metadata by reapplying the active overlay to the stored baseline rather than mutating the baseline in place.
