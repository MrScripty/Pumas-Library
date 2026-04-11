# Model Library Migration

## Purpose
This directory contains the migration-report and migration-execution helpers for `ModelLibrary`. It isolates dry-run generation, report artifact persistence, report retention, and execution-report rewrites from the rest of the library so migration-specific lifecycle code does not sprawl across normal import/index/update paths.

## Contents
| File/Folder | Description |
|-------------|-------------|
| `migration.rs` | Dry-run generation, report artifact writing, report retention, and execution-report rewrite helpers for model-library migrations. |

## Problem
Model type migrations and library repair passes need explainable dry runs, persisted report artifacts, and safe execution reporting. Those workflows must use current library/index state and share existing metadata logic, but they should not be mixed into ordinary model CRUD paths where the migration lifecycle is irrelevant.

## Constraints
- Dry-run evaluation must not mutate model files or index rows.
- Report artifacts must stay inside the library-owned migration report area.
- Partial downloads and missing source paths must be reported explicitly instead of being silently moved.
- Execution-report rewrites must preserve referential integrity with the recorded artifact paths.

## Decision
- Keep migration/report behavior in a dedicated `library/` submodule so migration lifecycle code is separate from day-to-day model-library operations.
- Generate both machine-readable JSON and human-readable Markdown artifacts for each persisted report.
- Maintain an index of generated reports so the UI can list, delete, and prune reports deterministically.

## Alternatives Rejected
- Generate migration reports only through external scripts: rejected because the frontend and RPC layers need first-class report access through the core library.
- Write standalone report files without an index: rejected because pruning and UI listing would become filesystem-scanning logic with weaker lifecycle guarantees.

## Invariants
- `generate_migration_dry_run_report()` is non-mutating with respect to model files and index state.
- Persisted report artifact paths remain constrained to the library-owned report directory.
- Report index entries remain sufficient to list, delete, and prune report pairs without rescanning arbitrary filesystem paths.
- Execution-report rewrites update the recorded artifact pair rather than creating orphaned duplicates.

## Revisit Triggers
- A second migration family requires a distinct report schema or retention policy.
- Migration execution starts depending on background jobs or checkpoint state that should live outside `ModelLibrary`.
- Report artifact consumers need versioned schemas or stronger backward-compatibility guarantees than the current internal format.

## Dependencies
**Internal:** `ModelLibrary`, model index reads, metadata loading, type resolution helpers, atomic JSON read/write helpers, and migration report path utilities in the parent module.
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

## API Consumer Contract
- Dry-run and execution reports are surfaced through higher-level API/RPC methods as additive diagnostics, not as the primary model-library query surface.
- Consumers should treat report entries as snapshots of library state at generation time, not live views.
- Delete and prune operations are best-effort maintenance commands over persisted report artifacts and index rows.

## Structured Producer Contract
- Persisted migration reports are written as a JSON/Markdown pair plus an index entry describing `generated_at`, `report_kind`, and both artifact paths.
- `report_kind` is currently expected to be `dry_run` or `execution`.
- Artifact paths recorded in the report index must remain library-owned paths, never arbitrary caller-provided paths.
- Compatibility rule: report payloads may gain new optional fields, but existing persisted reports should remain listable and deletable without migration-only tooling.

## Regeneration Rules
- When execution reconciliation changes recorded action rows, rewrite the existing execution artifact pair instead of emitting a second canonical copy.
- When pruning history, remove artifact files and update `migration-reports/index.json` together so the retained index never points at deleted files.
