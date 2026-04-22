# Contracts

## Purpose
This directory owns executable and documented contracts for data crossing process, runtime, or language boundaries.

## Contents
| File | Description |
| --- | --- |
| `desktop-rpc-methods.md` | Current desktop bridge and Rust JSON-RPC method registry contract. |
| `native-bindings-surface.md` | UniFFI export support tiers and host-input validation contract. |
| `release-artifacts.md` | Release artifact naming, checksum, SBOM, and native binding compatibility contract. |

## Problem
The Electron preload bridge, Electron main process, Rust JSON-RPC server, frontend TypeScript API types, binding layers, and release scripts all need to agree on cross-boundary names, payloads, artifacts, support tiers, and compatibility rules.

## Constraints
- Renderer typing is not a security boundary.
- Rust remains the backend source of truth for method behavior.
- Contract changes must be additive unless a migration note and changelog entry accompany the change.

## Decision
Track method ownership in this directory and enforce the current method allowlist in Electron main-process IPC validation. Later refactor passes should replace the hand-maintained list with generated TypeScript and Rust artifacts from a single registry.

## Alternatives Rejected
- Keep method names only in `preload.ts` and Rust match arms: rejected because that already permits drift.
- Validate only in the renderer: rejected because compromised renderer input reaches `ipcMain.handle`.

## Invariants
- Every renderer-visible backend call must pass through `api:call` validation before it reaches the backend bridge.
- New desktop/RPC methods must be added to this contract and to the Electron allowlist in the same change.
- New native binding exports must be classified by support tier and validate host-facing path/string inputs before reaching core services.
- Release artifacts published under the same version must be built from the same commit and covered by checksums.

## Revisit Triggers
- Adding generated schema validation.
- Adding a second renderer runtime.
- Exposing a method to external host-language bindings.
- Promoting or adding UniFFI exports.
- Adding or renaming release artifacts.

## Dependencies
**Internal:** `electron/src/ipc-validation.ts`, `electron/src/main.ts`, `electron/src/preload.ts`, `rust/crates/pumas-rpc/src/handlers/mod.rs`, `rust/crates/pumas-uniffi/src/bindings.rs`, `frontend/src/types/api.ts`, `RELEASING.md`, `scripts/package-uniffi-csharp-artifacts.sh`, `scripts/dev/generate-sbom.sh`.

**External:** Electron IPC and JSON-RPC 2.0 conventions.

## Related ADRs
- None identified as of 2026-04-21.
- Reason: this is the first explicit cross-process contract registry.
- Revisit trigger: schema generation or multi-client compatibility requires a persistent design decision.

## Usage Examples
```ts
const request = validateApiCallPayload(method, params);
await pythonBridge.call(request.method, request.params);
```

## API Consumer Contract
- Consumers send a method name and object-shaped params through `api:call`.
- Unknown method names are rejected in the Electron main process.
- Param values remain method-specific until the next pass adds per-method schemas.

## Structured Producer Contract
- `desktop-rpc-methods.md` is the human-readable registry.
- `native-bindings-surface.md` is the human-readable native binding support-tier registry.
- `release-artifacts.md` is the human-readable release artifact registry.
- `electron/src/ipc-validation.ts` is the current executable allowlist.
- Field additions must be append-only unless a migration note is recorded.
