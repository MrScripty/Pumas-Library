# electron src

## Purpose
Own the Electron main-process and preload bridge source for the desktop shell
that hosts the frontend and routes RPC calls to backend services.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `main.ts` | Main process startup, window lifecycle, and IPC wiring. |
| `preload.ts` | Secure renderer bridge exposing the canonical desktop API. |
| `python-bridge.ts` | Python backend process bridge and lifecycle helpers. |

## Problem
The desktop shell needs a single place to own window lifecycle, backend process
startup, and secure renderer access without letting frontend code reach Node or
OS APIs directly.

## Constraints
- Main and preload remain transport/orchestration layers, not business-logic
  owners.
- `window.electronAPI` is the canonical renderer facade.
- Cross-platform shell behavior must stay isolated to Electron and thin
  platform-specific paths rather than leaking into renderer features.

## Decision
- Keep window lifecycle and backend process ownership in `main.ts`.
- Keep renderer API exposure constrained to `preload.ts`.

## Alternatives Rejected
- Expose Node/Electron primitives directly to the renderer: rejected because it
  weakens process-boundary safety.
- Maintain multiple renderer bridge names: rejected because one canonical bridge
  is easier to verify.

## Invariants
- The renderer reaches backend methods through preload, not direct Node access.
- `window.electronAPI` remains the canonical bridge contract.

## Revisit Triggers
- A non-Electron desktop shell becomes a first-class runtime.
- Backend process management moves out of the Electron main process.

## Dependencies
**Internal:** frontend renderer contract and backend RPC methods.
**External:** Electron runtime APIs and Node process/fs modules.

## Related ADRs
- None identified as of 2026-04-12.
- Reason: the desktop shell boundary is currently documented in module READMEs
  and execution plans rather than ADRs.
- Revisit trigger: a lasting desktop-runtime contract change spans multiple
  teams or repos.

## Usage Examples
```ts
contextBridge.exposeInMainWorld('electronAPI', apiMethods);
```

## API Consumer Contract
- The renderer consumes methods exposed from `preload.ts`, with
  `window.electronAPI` as the canonical global.
- Main-process IPC handlers own error catching and boundary validation before
  work crosses into backend services.
- Compatibility expectation: bridge growth should be additive and preserve the
  existing `window.electronAPI` method contracts.

## Structured Producer Contract
- `preload.ts` produces the renderer-visible global bridge shape consumed by the
  frontend.
- `main.ts` produces IPC channels and window lifecycle behavior, but not
  persisted machine-consumed artifacts.
- Revisit trigger: bridge schema/codegen or persisted Electron metadata becomes
  part of this directory's output contract.
