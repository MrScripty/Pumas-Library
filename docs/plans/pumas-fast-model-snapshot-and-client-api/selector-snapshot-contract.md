# Selector Snapshot Contract

## Purpose

Pantograph and other Rust consumers can use the selector snapshot to populate
model pickers and graph-facing model references without hydrating every model.

## Consumer Rules

- Use `row.model_ref` as the stable Pumas reference.
- Treat `indexed_path` as display/debug data only.
- Treat `entry_path` as executable only when `entry_path_state == "ready"` and
  `artifact_state == "ready"`.
- Missing or invalid `package_facts_summary` means the row is still visible,
  but the consumer should hydrate package facts only if the user selects that
  model.
- `model_ref.model_ref_contract_version` is the model-reference contract
  version. It is not the upstream model revision; use `model_ref.revision` for
  that.

## Lazy Selection Flow

1. Open `PumasReadOnlyLibrary` when the process only needs indexed snapshots,
   or use the direct owner API when it owns the Pumas instance.
2. Request `model_library_selector_snapshot` with the desired page/filter.
3. Build Pantograph graph-facing references from `row.model_ref`.
4. Hydrate selected models only when detail state or user action requires it.
5. Subscribe from the returned cursor after the subscription milestone lands.

## Fixture

See `fixtures/selector-snapshot-row.json` for the current row shape.
