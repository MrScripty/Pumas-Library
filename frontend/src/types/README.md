# frontend types

## Purpose
Define the stable TypeScript contracts that the frontend uses to consume backend
responses, preload bridge methods, app metadata, and plugin/version models.
This directory exists so renderer components, hooks, and API helpers all share
one authoritative view of the desktop bridge boundary.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `api.ts` | Canonical desktop bridge request/response types and renderer globals. |
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
- Legacy `PyWebViewAPI` naming may remain only as a deprecated compatibility
  alias while older callers migrate.
- Response fields consumed by hooks and components must stay additive where
  possible to avoid cross-layer breakage.

## Decision
- Keep bridge, payload, and global-window contracts centralized in `api.ts`.
- Use `DesktopBridgeAPI` as the primary bridge interface name.
- Retain `PyWebViewAPI` only as a deprecated alias so existing callers can
  migrate without a runtime fork.

## Alternatives Rejected
- Duplicate bridge types in each API module: rejected because drift would be
  inevitable and type review would become scattered.
- Remove the legacy alias in the same pass: rejected because compatibility is
  still easier to preserve than to coordinate as a breaking change.

## Invariants
- `window.electronAPI` remains the canonical renderer bridge.
- Shared response types describe backend-owned data and are not redefined in
  hooks or components.
- Legacy alias names may exist temporarily, but they must resolve to the same
  underlying contract as the canonical bridge.

## Revisit Triggers
- The preload bridge stops being the primary renderer/backend boundary.
- A generated schema or executable contract replaces these hand-written types.
- The deprecated PyWebView alias is ready for removal.

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
- Deprecated `PyWebViewAPI` usage is compatibility-only and must not be used as
  the primary naming in new code.
- Compatibility expectation: additive field and method growth is preferred;
  breaking bridge removals require coordinated migration work.

## Structured Producer Contract
- `api.ts` defines stable field names and method signatures expected by the
  renderer and preload bridge.
- Optional fields remain optional unless a coordinated migration changes them.
- Global-window declarations document which runtime globals are canonical versus
  deprecated compatibility aliases.
- Revisit trigger: a generated schema or codegen pipeline becomes the producer
  of these contracts.
