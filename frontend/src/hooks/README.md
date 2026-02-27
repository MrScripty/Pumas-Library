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
| `usePhysicsDrag.ts` | Physics-based drag behavior for interactive UI elements. |

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
