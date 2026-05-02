# Model Library Migration

## Purpose
This directory contains focused `ModelLibrary` submodules for migration/report lifecycle logic and index/display projection helpers. It isolates dry-run generation, report artifact persistence, report retention, execution-report rewrites, and metadata-to-index projection rules from the rest of the library so those specialized concerns do not sprawl across normal import/index/update paths.

## Contents
| File/Folder | Description |
|-------------|-------------|
| `migration.rs` | Dry-run generation, report artifact writing, report retention, and execution-report rewrite helpers for model-library migrations. |
| `projection.rs` | Metadata-to-index record projection, derived format/quantization fields, cleanup dry-run reporting, freshness timestamps, and canonical display-path helpers. |

## Problem
Model type migrations, library repair passes, and derived index/display projections need explainable dry runs, persisted report artifacts, and deterministic metadata shaping. Those workflows must use current library/index state and share existing metadata logic, but they should not be mixed into ordinary model CRUD paths where the migration lifecycle or projection rules are irrelevant.

## Constraints
- Dry-run evaluation must not mutate model files or index rows.
- Report artifacts must stay inside the library-owned migration report area.
- Partial downloads and missing source paths must be reported explicitly instead of being silently moved.
- Execution-report rewrites must preserve referential integrity with the recorded artifact paths.
- Derived projection fields such as `primary_format`, `quantization`, and `entry_path` display strings must remain deterministic for legacy index rows and UI consumers.
- Metadata projection cleanup reports must be non-mutating and must preserve user/provenance exceptions such as license, model card, notes, and preview image fields.
- Write-mode metadata projection cleanup must be idempotent and limited to SQLite projection rows.

## Decision
- Keep migration/report behavior and projection helpers in dedicated `library/` submodules so those lifecycle and shaping concerns stay separate from day-to-day model-library operations.
- Generate both machine-readable JSON and human-readable Markdown artifacts for each persisted report.
- Maintain an index of generated reports so the UI can list, delete, and prune reports deterministically.
- Keep metadata-to-record projection and canonical display-path normalization together so index rows and execution descriptors reuse one set of derived-field rules.
- Keep projection cleanup dry-run analysis next to the projection cleanup rules so reports and future write-mode cleanup cannot drift.

## Alternatives Rejected
- Generate migration reports only through external scripts: rejected because the frontend and RPC layers need first-class report access through the core library.
- Write standalone report files without an index: rejected because pruning and UI listing would become filesystem-scanning logic with weaker lifecycle guarantees.
- Leave record projection and path canonicalization inline inside `library.rs`: rejected because those helpers form a separate responsibility cluster and materially contribute to the main file exceeding the decomposition-review threshold.

## Invariants
- `generate_migration_dry_run_report()` is non-mutating with respect to model files and index state.
- Persisted report artifact paths remain constrained to the library-owned report directory.
- Report index entries remain sufficient to list, delete, and prune report pairs without rescanning arbitrary filesystem paths.
- Execution-report rewrites update the recorded artifact pair rather than creating orphaned duplicates.
- Metadata-derived projection fields are recomputed from canonical metadata/filesystem inputs rather than stored as independent mutable state.
- Metadata projection cleanup reports compare existing SQLite payloads against cleanup rules on cloned values and must not mutate index rows.
- Display-path normalization must use `platform::platform_display_path` so Windows verbatim-prefix
  handling and long-path FFI remain inside the platform boundary.

## Revisit Triggers
- A second migration family requires a distinct report schema or retention policy.
- Migration execution starts depending on background jobs or checkpoint state that should live outside `ModelLibrary`.
- Report artifact consumers need versioned schemas or stronger backward-compatibility guarantees than the current internal format.
- Projection rules expand into multiple independently versioned consumers that need a stronger contract than an internal helper module.

## Dependencies
**Internal:** `ModelLibrary`, model index reads, metadata loading, type resolution helpers, atomic JSON read/write helpers, download projection helpers, and migration report path utilities in the parent module.
**External:** `chrono` for timestamps and standard filesystem APIs for report artifact persistence.

## Related ADRs
- None identified as of 2026-04-11.
- Reason: migration reporting behavior is currently documented in implementation plans and tests rather than a standalone architecture record.
- Revisit trigger: report formats become a cross-repo or externally consumed contract.

## Usage Examples
```rust
let dry_run = library.generate_migration_dry_run_report_with_artifacts()?;
let reports = library.list_migration_reports()?;
```

```rust
let cleanup = library.generate_metadata_projection_cleanup_dry_run_report()?;
let execution = library.execute_metadata_projection_cleanup()?;
```

```rust
let record = library.get_model("llm/llama/example")?.unwrap();
let primary_format = record.metadata.get("primary_format");
```

## API Consumer Contract
- Dry-run and execution reports are surfaced through higher-level API/RPC methods as additive diagnostics, not as the primary model-library query surface.
- Consumers should treat report entries as snapshots of library state at generation time, not live views.
- Delete and prune operations are best-effort maintenance commands over persisted report artifacts and index rows.
- Index/API consumers should treat `primary_format` and `quantization` as derived convenience fields that may be recomputed from canonical metadata and filesystem facts.
- Metadata projection cleanup dry-run reports are diagnostics over SQLite cache rows; consumers must not treat them as source metadata deletion plans.
- Metadata projection cleanup execution applies only the reviewed projection-row cleanup; source `metadata.json` files remain untouched.

## Structured Producer Contract
- Persisted migration reports are written as a JSON/Markdown pair plus an index entry describing `generated_at`, `report_kind`, and both artifact paths.
- `report_kind` is currently expected to be `dry_run` or `execution`.
- Artifact paths recorded in the report index must remain library-owned paths, never arbitrary caller-provided paths.
- Projected model records may add derived metadata fields such as `primary_format` and `quantization`, but those fields must remain consistent with canonical metadata and on-disk payload evidence.
- Metadata projection cleanup reports include affected row counts, removed field names, preserved exception fields, and before/after JSON byte counts for review before any write-mode cleanup.
- Metadata projection cleanup execution reports include the dry-run plan and the number of rows actually updated.
- Compatibility rule: report payloads may gain new optional fields, but existing persisted reports should remain listable and deletable without migration-only tooling.

## Regeneration Rules
- When execution reconciliation changes recorded action rows, rewrite the existing execution artifact pair instead of emitting a second canonical copy.
- When pruning history, remove artifact files and update `migration-reports/index.json` together so the retained index never points at deleted files.
- When metadata or payload facts change, recompute derived format/quantization fields from source metadata and payload evidence instead of patching derived fields independently.
