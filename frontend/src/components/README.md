# frontend components

## Purpose
UI components for dashboards, dialogs, status displays, app panels, and model-management workflows.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `ModelManager.tsx` | Main model management screen and interactions. |
| `VersionSelector.tsx` | Version install/switch/update UI. |
| `ModelImportDialog.tsx` | Import flow for local and remote model files. |
| `MappingPreview.tsx` | Mapping preview and conflict-resolution workflow. |
| `ui/` | Small reusable primitives (buttons, tooltips, list items). |
| `app-panels/` | App-specific panel renderers and sections. |

## Design Decisions
- Keep feature-level composition in high-level components; primitives stay in `ui/`.
- Move long-running async interactions into hooks/APIs, not direct component internals.

## Dependencies
**Internal:** hooks, shared types, API wrappers, config.
**External:** React and styling utilities.

## Usage Examples
```tsx
<ModelManager />
```
