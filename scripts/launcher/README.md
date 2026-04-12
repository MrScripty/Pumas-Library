# scripts/launcher

## Purpose
This directory contains the shared cross-platform launcher core for local
desktop workflows. It exists so Bash and PowerShell entry points can expose the
same lifecycle contract without duplicating parsing, dependency checks, build
logic, or runtime orchestration.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `cli.mjs` | Node entrypoint that parses launcher args, builds context, and dispatches actions. |
| `actions.mjs` | Action orchestration for install, build, run, run-release, test, and release-smoke flows. |
| `dependencies.mjs` | Dependency checks, per-dependency install behavior, and runtime prerequisite enforcement. |
| `parse-args.mjs` | Canonical CLI parsing and validation for launcher flags and forwarded args. |
| `platform-*.mjs` | Platform-specific command-name and artifact conventions selected by the factory. |
| `*.test.mjs` | Node built-in test runner coverage for launcher contract and wrapper behavior. |

## Problem
The repo needs a real cross-platform desktop entry workflow, but shell scripts
alone are a poor place to duplicate orchestration logic across Unix and
Windows. Without a shared core, the README, wrappers, and actual build/run
behavior drift apart.

## Constraints
- `launcher.sh` must remain the root Unix entry point.
- Windows must get the same CLI contract without re-implementing launcher logic.
- Platform detection must stay in one boundary layer.
- Paths with spaces must be supported end to end.
- The launcher contract must stay explicit and testable.

## Decision
Implement the launcher core in Node because the repo already depends on Node
for frontend and Electron workflows. Keep platform wrappers thin and move all
behavioral logic into small modules under this directory.

## Alternatives Rejected
- Keep all logic in `launcher.sh`: rejected because Windows would require a
  second, drift-prone implementation.
- Replace wrappers with raw npm commands in the README: rejected because that
  would weaken the launcher contract instead of preserving it.

## Invariants
- The shared core is the single owner of launcher action semantics.
- Wrappers may set environment and locate the core, but must not duplicate
  orchestration logic.
- Platform-specific differences are selected via the platform factory, not
  scattered through action handlers.

## Revisit Triggers
- The launcher contract gains enough complexity that a dedicated package or
  typed build step becomes justified.
- More than one platform needs materially different runtime-launch behavior.
- CI requires launcher behavior to be imported as a reusable library rather
  than executed as a CLI.

## Dependencies
**Internal:** Root `package.json`, `electron/package.json`, `launcher.sh`, and
the Rust/frontend/Electron build outputs.
**External:** Node runtime, npm, and shell hosts that invoke the wrappers.

## Related ADRs
- None identified as of 2026-04-12.
- Reason: The launcher refactor is currently tracked through an implementation
  plan rather than a durable ADR.
- Revisit trigger: Another app adopts the same shared launcher architecture or
  the launcher becomes a reusable framework.

## Usage Examples
```bash
node scripts/launcher/cli.mjs --help
node scripts/launcher/cli.mjs --build-release
node scripts/launcher/cli.mjs --run -- --devtools
node scripts/launcher/cli.mjs --release-smoke
node --test scripts/launcher/*.test.mjs
```

## API Consumer Contract
- Consumers are the root platform wrappers and developers invoking the launcher
  core directly for debugging.
- Supported actions are `--help`, `--install`, `--build`, `--build-release`,
  `--run`, `--run-release`, `--test`, and `--release-smoke`.
- Only `--run` and `--run-release` accept forwarded args after `--`.
- Errors return stable exit codes for usage, missing dependencies, missing
  release artifacts, and general operation failure.
- The CLI contract is preserved even if internal modules are reorganized.

## Structured Producer Contract
- `buildUsage()` defines the canonical launcher help text and examples.
- The platform factory publishes stable command-name and artifact conventions
  consumed by action handlers.
- Dependency install output keeps the `[ok]`, `[install]`, `[done]`, and
  `[error]` message prefixes expected by callers and docs.
- `--release-smoke` remains a bounded startup check rather than an open-ended
  runtime launcher.
- If action names, exit codes, or help text examples change, wrappers, tests,
  and README documentation must be updated in the same slice.
