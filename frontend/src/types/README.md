# frontend types

## Purpose
Define the stable TypeScript contracts that the frontend uses to consume backend
responses, preload bridge methods, app metadata, and plugin/version models.
This directory exists so renderer components, hooks, and API helpers all share
one authoritative view of the desktop bridge boundary.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `api.ts` | Compatibility barrel that re-exports the split API contract modules. |
| `api-common.ts` | Shared response, async status, pagination, validation, and result helpers. |
| `api-system.ts` | System status, disk-space, and resource response contracts. |
| `api-models.ts` | Model records, Hugging Face search, inference settings, and download response contracts. |
| `api-import.ts` | Model-library import, metadata lookup, storage, and validation contracts. |
| `api-links.ts` | Link registry health and cleanup contracts. |
| `api-mapping.ts` | Mapping preview, synchronization, sandbox, exclusion, and migration report contracts. |
| `api-conversion.ts` | Model conversion state, progress, and environment contracts. |
| `api-versions.ts` | Version, installation-progress, cache, and background-fetch contracts. |
| `api-processes.ts` | Process, Ollama, Torch, shortcut, and launcher-update contracts. |
| `api-window.ts` | Renderer utility response contracts for paths, URLs, and window actions. |
| `api-bridge*.ts` | Domain bridge method interfaces composed into `DesktopBridgeAPI`. |
| `api-plugins.ts` | Plugin response contracts. |
| `api-electron.ts` | Electron window API, `ElectronAPI`, and global `window.electronAPI` augmentation. |
| `apps.ts` | App-level view and capability models consumed by the renderer. |
| `plugins.ts` | Plugin manifest and capability contracts surfaced to the UI. |
| `versions.ts` | Version-management and launcher-facing models. |

## Problem
The frontend crosses a preload/IPC boundary to reach backend services. Without
one shared contract surface, hooks and components would drift on field names,
lifecycle assumptions, and compatibility aliases.

## Constraints
- Renderer callers must treat the desktop bridge as a typed process boundary,
  not as an unstructured bag of methods.
- `window.electronAPI` is the canonical runtime facade.
- Response fields consumed by hooks and components must stay additive where
  possible to avoid cross-layer breakage.

## Decision
- Keep `api.ts` as the public import barrel while splitting bridge, payload,
  and global-window contracts into domain modules.
- Use `DesktopBridgeAPI` as the primary bridge interface name.

## Alternatives Rejected
- Duplicate bridge types in each API module: rejected because drift would be
  inevitable and type review would become scattered.
- Preserve multiple bridge names indefinitely: rejected because they hide the
  real canonical API and complicate future contract changes.

## Invariants
- `window.electronAPI` remains the canonical renderer bridge.
- Shared response types describe backend-owned data and are not redefined in
  hooks or components.

## Revisit Triggers
- The preload bridge stops being the primary renderer/backend boundary.
- A generated schema or executable contract replaces these hand-written types.

## Dependencies
**Internal:** `frontend/src/api/`, hooks, components, preload contract usage.
**External:** TypeScript only.

## Related ADRs
- None identified as of 2026-04-12.
- Reason: bridge naming and type-shape decisions are currently tracked in
  implementation plans and module READMEs.
- Revisit trigger: a durable contract/versioning policy needs an ADR.

## Usage Examples
```ts
import type { DesktopBridgeAPI, LibraryStatusResponse } from './api';
```

## API Consumer Contract
- Consumers import bridge and payload types from this directory rather than
  recreating local structural types.
- Renderer code should call backend methods through the canonical
  `DesktopBridgeAPI`/`window.electronAPI` contract.
- Compatibility expectation: additive field and method growth is preferred;
  breaking bridge removals require coordinated migration work.

## Structured Producer Contract
- The split `api-*` modules define stable field names and method signatures
  expected by the renderer and preload bridge.
- Optional fields remain optional unless a coordinated migration changes them.
- Global-window declarations remain isolated in `api-electron.ts`.
- Revisit trigger: a generated schema or codegen pipeline becomes the producer
  of these contracts.
