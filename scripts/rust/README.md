# scripts/rust

## Purpose
Rust workspace verification entrypoints for local development and CI.

## Contents
| File | Description |
| ---- | ----------- |
| `check.sh` | Runs standards-aligned Cargo format, check, clippy, test, doc-test, no-default-feature checks, focused isolation checks, and blocking-work audits. |

## Problem
The Rust workspace needs one documented verification contract so local runs and
CI cannot drift into different Cargo command sets.

## Constraints
- `pumas_rustler` is excluded from the default script because it links through
  the BEAM runtime and needs a dedicated Erlang/OTP environment.
- Commands must resolve paths relative to the repository root regardless of the
  caller's working directory.
- The script must fail fast and return Cargo's non-zero status directly.

## Usage
```bash
./scripts/rust/check.sh
./scripts/rust/check.sh clippy
./scripts/rust/check.sh no-default
./scripts/rust/check.sh blocking-audit
PUMAS_RUST_TEST_ISOLATION_REPEATS=3 ./scripts/rust/check.sh test-isolation
```

## Blocking Audit
`blocking-audit` prints candidate synchronous filesystem, process, thread, and
wait calls from Rust production source roots. It is intentionally informational:
use the output to classify each hit as an async request path, sync service path,
explicit background worker, or test fixture before making R05 refactors.

## Test Isolation
`test-isolation` repeatedly runs the `pumas-library` crate's guarded in-crate
API tests and `api_tests` integration binary with multiple test threads. Use it
after changing registry overrides, IPC startup ownership, or process-global
environment guards.

## Revisit Triggers
- A dedicated BEAM-aware Rustler CI job is added.
- Workspace lint policy changes the required Cargo flags.
- New feature combinations become part of the public compatibility contract.
