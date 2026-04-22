# scripts/rust

## Purpose
Rust workspace verification entrypoints for local development and CI.

## Contents
| File | Description |
| ---- | ----------- |
| `check.sh` | Runs standards-aligned Cargo format, check, clippy, test, doc-test, and no-default-feature checks. |

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
```

## Revisit Triggers
- A dedicated BEAM-aware Rustler CI job is added.
- Workspace lint policy changes the required Cargo flags.
- New feature combinations become part of the public compatibility contract.
