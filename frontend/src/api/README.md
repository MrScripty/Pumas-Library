# frontend api

## Purpose
Typed frontend API wrappers that call methods exposed through the Electron preload bridge and normalize adapter behavior for UI consumers.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `adapter.ts` | API availability checks and low-level bridge access. |
| `import.ts` | Model import and file-validation calls. |
| `models.ts` | Model-management and metadata API wrappers. |
| `versions.ts` | Version-management API wrappers. |

## Design Decisions
- Keep API-call concerns out of components by exposing focused helper classes/functions.
- Preserve backend response structure and rely on shared type contracts.

## Dependencies
**Internal:** `frontend/src/types/api.ts`, preload API contract.
**External:** none beyond TypeScript/runtime platform APIs.

## Usage Examples
```ts
const res = await modelsAPI.getModels();
if (!res.success) throw new Error(res.error || 'Failed');
```
