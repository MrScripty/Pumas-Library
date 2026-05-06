# Selector Snapshot Performance

## Scope

Milestone 2 Slice 2.5 records a local debug-test timing for the first selector
projection. The timing covers 100 warm rows from SQLite with missing package
summaries. It is a smoke signal, not a release benchmark.

## Command

```bash
cargo test --manifest-path rust/Cargo.toml -p pumas-library selector_snapshot_reports_warm_100_row_timing -- --nocapture
```

## Result

- Direct `ModelIndex` warm snapshot, 100 rows: `0.878ms`
- `PumasReadOnlyLibrary` warm snapshot, 100 rows: `0.694ms`

## Notes

- Correctness tests are gating.
- The plan target remains `<= 5ms` for warm direct/read-only common pages.
- If the selector projection misses the target on release/profiled builds,
  materialized selector columns remain the next optimization.
