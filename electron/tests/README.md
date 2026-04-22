# Electron Tests

Package-local Node tests for Electron main-process boundary helpers.

## Purpose
This directory verifies Electron source behavior that can be tested without launching the desktop shell. It currently covers IPC validation helpers compiled from `electron/src`.

## Contents
| File | Description |
| ---- | ----------- |
| `ipc-validation.test.mjs` | Exercises compiled IPC validation helpers for RPC method allowlisting, dialog option sanitization, and external URL scheme validation. |

## Problem
Renderer input is not trusted just because TypeScript types exist in preload or frontend code. The Electron package needs fast tests that prove main-process boundary validators reject malformed payloads before IPC handlers forward work.

## Constraints
- Tests run with Node's built-in `node:test` runner to avoid adding another Electron test framework.
- Tests import compiled files from `electron/dist`, so `pnpm run test` builds the package first.
- Tests must not launch Electron windows, spawn backend processes, or depend on desktop display services.

## Decision
Keep Electron boundary tests in this package and execute them through the package-local `test` script. Runtime integration and GUI smoke coverage remain owned by the launcher and CI release-smoke flows.

## Alternatives Rejected
- **Vitest for Electron tests:** Rejected for this slice because Node's built-in runner is sufficient for pure validation helpers.
- **Tests beside source files:** Rejected because compiling source-adjacent tests would place test artifacts under `dist`, which is packaged by Electron Builder.

## Invariants
- Tests do not import Electron runtime modules directly.
- Tests cover rejected payloads, not only successful payloads.
- New IPC validation helpers get a package-local test before being wired into main-process handlers.

## Revisit Triggers
- Electron tests need BrowserWindow, preload, or renderer integration coverage.
- Electron Builder packaging rules change so source-adjacent tests can be excluded reliably.
- A shared contract generator replaces hand-written IPC validators.

## Dependencies
### Internal
- `../src/ipc-validation.ts` - Source validators compiled before the tests run.
- `../dist/ipc-validation.js` - Runtime module imported by the Node tests.

### External
- `node:test` - Built-in Node test runner.
- `node:assert/strict` - Assertion API used by the tests.

## Related ADRs
- `None identified as of 2026-04-22.`
- `Reason: This is a package-local verification boundary for an existing Electron IPC contract.`
- `Revisit trigger: Add an ADR if Electron IPC validation moves to generated schemas shared with Rust and frontend code.`

## Usage Examples
```bash
corepack pnpm --filter ./electron test
```

## API Consumer Contract
- `None identified as of 2026-04-22.`
- `Reason: These tests are not consumed as a runtime API.`
- `Revisit trigger: Add this section if external tooling begins invoking individual test files directly.`

## Structured Producer Contract
- `None identified as of 2026-04-22.`
- `Reason: Test output is ephemeral command output and is not a persisted structured artifact.`
- `Revisit trigger: Add this section if CI starts publishing Electron test reports from this directory.`
