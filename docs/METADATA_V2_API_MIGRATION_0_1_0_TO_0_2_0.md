# API Migration Guide: 0.1.0 -> 0.2.0

## Purpose
This guide maps removed legacy metadata/dependency APIs from release `0.1.0` to the
Metadata v2 API surface introduced in release `0.2.0`.

Use this when migrating local app/binding consumers from `0.1.0` contracts to `0.2.0`
`pumas-core` / `pumas-rpc` APIs.

## Last Updated
2026-02-28

## Release Scope
- Source release: `0.1.0`
- Target release: `0.2.0`

## Cutover Summary
- `0.2.0` introduces a breaking metadata/dependency API cutover.
- Legacy `0.1.0` metadata/dependency methods are removed at `0.2.0` cutover.
- There is no compatibility window.

Reference plan: `docs/plans/PUMAS_LIBRARY_METADATA_V2_CONSUMER_MIGRATION.md`.

## Legacy -> Replacement Mapping

| Legacy endpoint/method | Status | Replacement |
| --- | --- | --- |
| `mark_metadata_as_manual(model_id)` | Removed | `submit_model_review(model_id, patch, reviewer, reason?)` |
| `get_model_overrides(rel_path)` | Removed | `get_effective_model_metadata(model_id)` and/or `list_models_needing_review(filter?)` |
| `update_model_overrides(rel_path, overrides)` | Removed | `submit_model_review(model_id, patch, reviewer, reason?)` |
| Reset manual/override edits (legacy ad-hoc) | Removed | `reset_model_review(model_id, reviewer, reason?)` |
| Legacy dependency checks/install calls | Removed | `resolve_model_dependency_requirements` |

## Migration Patterns

### 1) Manual metadata edits
Old:
- call `mark_metadata_as_manual`
- mutate metadata overrides separately

New:
1. call `submit_model_review(model_id, patch, reviewer, reason?)`
2. read current effective metadata via `get_library_model_metadata` (`effective_metadata`) or `get_effective_model_metadata`
3. if needed, rollback with `reset_model_review`

### 2) Dependency requirements flow before inference
New required order:
1. `resolve_model_dependency_requirements`
2. consumer validates `dependency_contract_version == 1`
3. consumer treats `unknown_profile`, `invalid_profile`, and `profile_conflict` as non-ready
4. consumer performs environment check/install outside Pumas

### 3) Review queue operations
Use:
- `list_models_needing_review(filter?)`
- `submit_model_review(...)`
- `reset_model_review(...)`

## JSON-RPC Examples

### Submit a review patch
```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "submit_model_review",
  "params": {
    "model_id": "llm/llama/example-model",
    "reviewer": "alice",
    "reason": "manual-correction",
    "patch": {
      "task_type_primary": "text-generation",
      "task_classification_source": "task-signature-mapping",
      "task_classification_confidence": 1.0
    }
  }
}
```

### Reset review edits to baseline
```json
{
  "jsonrpc": "2.0",
  "id": 2,
  "method": "reset_model_review",
  "params": {
    "model_id": "llm/llama/example-model",
    "reviewer": "alice",
    "reason": "revert-to-baseline"
  }
}
```

### Dependency requirements resolution
```json
{
  "jsonrpc": "2.0",
  "id": 3,
  "method": "resolve_model_dependency_requirements",
  "params": {
    "model_id": "llm/llama/example-model",
    "platform_context": "linux-x86_64",
    "backend_key": "transformers"
  }
}
```

## Migration Runner APIs

Use these v2 migration/report endpoints:
- `generate_model_migration_dry_run_report`
- `execute_model_migration`
- `list_model_migration_reports`
- `delete_model_migration_report`
- `prune_model_migration_reports`

Execution reports now include:
- `reindexed_model_count`
- `index_model_count`
- `referential_integrity_ok`
- `referential_integrity_errors[]`

## Consumer Cutover Checklist
1. Stop calling removed endpoints listed above.
2. Switch metadata edits to review-overlay APIs.
3. Switch dependency flow to `resolve_model_dependency_requirements`.
4. Ensure contract tests cover `dependency_contract_version`, per-binding requirements, and non-ready states (`unknown_profile`, `invalid_profile`, `profile_conflict`).
5. Validate with migration dry-run/execution reports before production release.
