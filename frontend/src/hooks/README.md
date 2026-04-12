# frontend hooks

## Purpose
Custom React hooks for backend polling, process status, version/model workflows, and local UI state orchestration.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `useModels.ts` | Model list/search data lifecycle. |
| `useModelDownloads.ts` | Download state and operation controls. |
| `useStatus.ts` | Launcher/app status polling and refresh behavior. |
| `useVersions.ts` | Version list and version operations state flow. |
| `useAppImportDialog.ts` | App-level drag-and-drop import dialog state and completion handlers. |
| `useAppImportDialog.test.ts` | Hook coverage for app-level import dialog open/close and completion behavior. |
| `useAppProcessActions.ts` | Shared launch/stop/log handlers for app process controls at the root shell level. |
| `useAppProcessActions.test.ts` | Hook coverage for app-process launch/stop routing and delayed refresh behavior. |
| `useAppWindowActions.ts` | Root-shell helpers for window controls and shared filesystem open actions. |
| `usePhysicsDrag.ts` | Physics-based drag behavior for interactive UI elements. |
| `physicsDragUtils.ts` | Shared constants, types, and pure drag math used by `usePhysicsDrag.ts`. |

## Design Decisions
- Hooks encapsulate async side effects and state transitions outside UI components.
- Domain hooks consume typed API wrappers and return UI-friendly state.

## Dependencies
**Internal:** `api/`, `types/`, `utils/`, and component state needs.
**External:** React hooks.

## Usage Examples
```tsx
const { models, isLoading } = useModels();
```
