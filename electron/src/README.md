# electron src

## Purpose
Electron main-process and preload bridge source for the desktop shell that hosts the frontend and routes RPC calls to backend services.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `main.ts` | Main process startup, window lifecycle, and IPC wiring. |
| `preload.ts` | Secure renderer API bridge exposing typed backend methods. |
| `python-bridge.ts` | Python backend process bridge and lifecycle helpers. |

## Design Decisions
- Keep renderer access constrained through preload-exposed methods.
- Main/preload layers act as orchestration and transport glue, not business-logic owners.

## Dependencies
**Internal:** frontend renderer contract and backend RPC methods.
**External:** Electron runtime APIs and Node process/fs modules.

## Usage Examples
```ts
contextBridge.exposeInMainWorld('api', apiMethods);
```
