# scripts

## Purpose
Top-level developer and release scripts for binding generation, contract checks,
local environment setup, and system validation.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `generate-bindings.sh` | Generates host-language bindings and the Rustler NIF integration guidance from the compiled Pumas native surface. |
| `check-uniffi-surface.sh` | Builds `pumas-uniffi`, dumps UniFFI metadata, and asserts that core exported binding items are still present. |
| `check-uniffi-csharp-smoke.sh` | Generates Pumas C# bindings into build output, compiles the checked-in smoke harness, and runs it against the native library. |
| `contract-tests.ts` | Runs repo-specific contract checks for JavaScript/TypeScript-facing integration paths. |
| `system-check.sh` | Verifies required local tools and runtime prerequisites for common development workflows. |
| `dev/` | Developer setup, build, and local-run helpers that support day-to-day iteration. |

## Problem
This repo has multiple developer workflows that cross Rust, frontend, and
binding boundaries. Without stable entrypoint scripts, verification becomes
tribal knowledge and binding regressions are easier to miss.

## Constraints
- Scripts must stay runnable from the repo root with predictable paths.
- Generated artifacts should go under ignored build directories when possible.
- Fast verification scripts should be usable locally before heavier release
  workflows run.
- Binding verification must work without hand-editing generated host-language
  code.

## Decision
Keep small, explicit shell entrypoints under `scripts/` for repeatable
developer and release tasks. Binding generation and metadata verification are
separate commands so developers can choose between producing artifacts and
checking that the exported surface still matches expectations.

## Alternatives Rejected
- Rely on ad hoc Cargo commands only: rejected because multi-step binding and
  packaging workflows would remain hard to discover and easy to run incorrectly.
- Check generated binding files into source control as the primary verification
  method: rejected because generated artifacts drift easily and should not be
  hand-maintained.

## Invariants
- Scripts run relative to the repository root, not the caller's current shell
  location.
- Metadata and generated artifact checks write into transient build output, not
  committed source-of-truth files.
- Binding verification scripts fail fast when expected exported items disappear.

## Revisit Triggers
- The repo adds more binding targets that need their own smoke or packaging
  flows.
- CI standardizes all verification behind a different task runner.
- Top-level scripts become numerous enough that they need per-domain
  subdirectories and separate READMEs.

## Dependencies
**Internal:** `rust/`, `bindings/`, and `docs/` artifacts referenced by
specific scripts.
**External:** `cargo`, shell utilities, and optional host-language toolchains
such as `.NET` or bindgen CLIs depending on the script.

## Related ADRs
- None identified as of 2026-04-12.
- Reason: The current script layout is a repo-local workflow choice rather than
  a recorded architecture decision.
- Revisit trigger: Multiple repos need a shared binding/release automation
  pattern or CI contract.

## Usage Examples
```bash
./scripts/check-uniffi-surface.sh
./scripts/check-uniffi-csharp-smoke.sh
./scripts/generate-bindings.sh csharp
```

## API Consumer Contract
- Primary consumers are repository developers and CI jobs, not end users.
- Scripts expect to be run from a checked-out repo with the required toolchains
  installed.
- Scripts return non-zero on validation or prerequisite failure and print a
  direct error message to stderr.
- Script names are stable entrypoints; internal command details may evolve as
  long as the documented workflow remains valid.

## Structured Producer Contract
- Verification scripts may emit transient metadata dumps and generated outputs
  under ignored build directories.
- Packaging or checksum-producing scripts must document output paths and
  compatibility expectations for produced artifacts.
- Generated binding outputs are derived artifacts and must be regenerated when
  the native binding surface changes.
- If a script's produced file layout changes, the corresponding docs and any CI
  callers must be updated in the same slice.
