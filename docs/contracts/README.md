# Contracts

## Purpose
This directory owns executable and documented contracts for data crossing process, runtime, or language boundaries.

## Contents
| File | Description |
| --- | --- |
| `desktop-rpc-methods.md` | Current desktop bridge and Rust JSON-RPC method registry contract. |

## Problem
The Electron preload bridge, Electron main process, Rust JSON-RPC server, frontend TypeScript API types, and binding layers all need to agree on method names and payload shapes.

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

## Revisit Triggers
- Adding generated schema validation.
- Adding a second renderer runtime.
- Exposing a method to external host-language bindings.

## Dependencies
**Internal:** `electron/src/ipc-validation.ts`, `electron/src/main.ts`, `electron/src/preload.ts`, `rust/crates/pumas-rpc/src/handlers/mod.rs`, `frontend/src/types/api.ts`.

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
- `electron/src/ipc-validation.ts` is the current executable allowlist.
- Field additions must be append-only unless a migration note is recorded.
