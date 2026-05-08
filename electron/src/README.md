# electron src

## Purpose
Own the Electron main-process and preload bridge source for the desktop shell
that hosts the frontend and routes RPC calls to backend services.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `main.ts` | Main process startup, window lifecycle, and IPC wiring. |
| `preload.ts` | Secure renderer bridge exposing the canonical desktop API. |
| `python-bridge.ts` | Backend process bridge, lifecycle helpers, model-library update stream client, and deterministic timer ownership. |

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
- Backend lifecycle timers are owned by `python-bridge.ts` and must be
  injectable for deterministic tests instead of relying on wall-clock sleeps.
- The model-library update stream is owned by Electron main, not renderer code.
  Renderer consumers receive only validated preload notifications.

## Decision
- Keep window lifecycle and backend process ownership in `main.ts`.
- Keep renderer API exposure constrained to `preload.ts`.
- Keep backend health-check and restart backoff scheduling in `python-bridge.ts`
  behind an injectable timer controller.
- Keep the backend model-library SSE connection in `python-bridge.ts` and
  forward validated notifications through the preload bridge.

## Alternatives Rejected
- Expose Node/Electron primitives directly to the renderer: rejected because it
  weakens process-boundary safety.
- Maintain multiple renderer bridge names: rejected because one canonical bridge
  is easier to verify.

## Invariants
- The renderer reaches backend methods through preload, not direct Node access.
- `window.electronAPI` remains the canonical bridge contract.
- Backend health-check and restart timers are cleared during bridge stop before
  process shutdown checks continue.
- Model-library update subscriptions are additive, cancellable, and never give
  renderer code direct access to backend ports or Node stream primitives.

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
  frontend. Runtime-profile and serving methods are additive bridge contracts;
  serving update feeds are exposed as RPC calls rather than Electron-owned
  persistent state.
- `python-bridge.ts` produces the backend lifecycle scheduling contract consumed
  by `main.ts`, including restartable model-library update stream ownership,
  and is verified by package-local tests.
- `main.ts` produces IPC channels and window lifecycle behavior, but not
  persisted machine-consumed artifacts.
- Revisit trigger: bridge schema/codegen or persisted Electron metadata becomes
  part of this directory's output contract.
