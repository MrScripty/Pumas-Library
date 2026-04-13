# frontend api

## Purpose
Provide typed frontend API wrappers that call the canonical Electron desktop
bridge and normalize low-level adapter behavior for hooks and components.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `adapter.ts` | Canonical desktop bridge access, availability checks, and safe wrappers. |
| `import.ts` | Model import and file-validation calls. |
| `models.ts` | Model-management and metadata API wrappers. |
| `versions.ts` | Version-management API wrappers. |

## Problem
Renderer components need a narrow API layer that understands preload-bridge
availability and shared response types without duplicating transport concerns in
every hook or component.

## Constraints
- Components should not talk directly to preload globals.
- The adapter must preserve backend-owned response semantics instead of
  inventing local business state.
- `window.electronAPI` is the canonical bridge.

## Decision
- Keep bridge access centralized in `adapter.ts`.
- Re-export shared bridge types from this directory so callers stay on one
  contract surface.
- Keep feature wrappers (`import.ts`, `models.ts`, `versions.ts`) focused on
  typed method grouping rather than runtime environment detection.

## Alternatives Rejected
- Call preload globals directly from components: rejected because that spreads
  transport coupling across presentation code.
- Rebuild response shapes per feature module: rejected because it weakens the
  shared contract boundary.

## Invariants
- API wrappers call the desktop bridge, not infrastructure directly.
- Availability failures surface as explicit `APIError` values at the boundary.
- Shared response and method names stay aligned with `frontend/src/types/api.ts`.

## Revisit Triggers
- The renderer bridge stops being Electron/preload-based.
- A generated client replaces these hand-written wrappers.
- API wrappers start owning business workflow state instead of transport glue.

## Dependencies
**Internal:** `frontend/src/types/api.ts`, frontend error helpers, preload API contract.
**External:** none beyond TypeScript/runtime platform APIs.

## Related ADRs
- None identified as of 2026-04-12.
- Reason: the current bridge naming and adapter shape are documented at the
  module level and in implementation plans.
- Revisit trigger: a long-lived client/runtime contract change needs an ADR.

## Usage Examples
```ts
const res = await modelsAPI.getModels();
if (!res.success) throw new Error(res.error || 'Failed');
```

## API Consumer Contract
- UI callers import wrappers from this directory instead of touching preload
  globals directly.
- `adapter.ts` returns the canonical desktop bridge contract and treats missing
  bridge access as a boundary error.
- Compatibility expectation: wrappers may grow additively, but existing method
  semantics should remain aligned with the shared type contracts.

## Structured Producer Contract
- This directory does not persist machine-consumed artifacts directly.
- It produces typed wrapper surfaces over the desktop bridge; those wrappers
  must preserve field names, nullability, and error semantics from the shared
  contracts.
- Revisit trigger: wrapper metadata or generated clients become persisted
  artifacts.
