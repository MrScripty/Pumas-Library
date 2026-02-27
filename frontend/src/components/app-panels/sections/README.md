# frontend app-panels sections

## Purpose
Composable section components used by app panels to render status, selectors, dependency info, and runtime controls.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `ModelSelectorSection.tsx` | Model selection section UI. |
| `DependencyStatusSection.tsx` | Dependency health and missing-packages display. |
| `StatsSection.tsx` | Runtime/resource/status metrics block. |
| `TorchModelSlotsSection.tsx` | Torch model slot management section. |
| `TorchServerConfigSection.tsx` | Torch server configuration controls. |
| `index.ts` | Section exports for panel composition. |

## Design Decisions
- Keep sections focused and composable to reduce duplicated panel markup.
- Shared section API surface is centralized via `index.ts` exports.

## Dependencies
**Internal:** shared types/hooks/components.
**External:** React.

## Usage Examples
```tsx
<StatsSection stats={stats} />
```
