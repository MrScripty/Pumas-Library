# pumas-library Tests

## Purpose
This directory contains integration tests for the `pumas-library` crate.

## Producer Contract
Tests should verify public crate behavior and durable data contracts rather than private implementation details. Fixtures belong under `fixtures/` and should be small enough to review.

## Consumer Contract
Tests may create temporary roots, model-library fixtures, and isolated database files. They must not require the developer's real `launcher-data` or shared model cache.

## Isolation Requirements
Process-global environment or path overrides must use a serialized guard and document why parallel execution is unsafe.

Registry override tests are intentionally guarded with a process-wide mutex
because Rust 2024 treats environment mutation as unsafe. After changing these
guards, run:

```bash
PUMAS_RUST_TEST_ISOLATION_REPEATS=3 ./scripts/rust/check.sh test-isolation
```

This command repeats the guarded in-crate API tests and `api_tests` integration
binary with multiple test threads so accidental unguarded process-global state
is more likely to surface before CI.

## Non-Goals
RPC transport and Electron IPC behavior are out of scope. Reason: those belong to `pumas-rpc` and Electron tests. Revisit trigger: add cross-layer contract tests that intentionally span crates.
