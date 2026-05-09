# frontend components app-panels

## Purpose
App-specific panel containers and renderers for ComfyUI, Ollama, llama.cpp, and Torch sections in the main UI.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `AppPanelRenderer.tsx` | Chooses panel implementation by app/runtime key. |
| `ComfyUIPanel.tsx` | ComfyUI panel composition. |
| `LlamaCppPanel.tsx` | llama.cpp panel composition, including the focused compatible-model serving workspace. |
| `OllamaPanel.tsx` | Ollama panel composition. |
| `TorchPanel.tsx` | Torch panel composition. |
| `VersionManagementPanel.tsx` | Version management panel composition. |
| `sections/` | Reusable panel sections shared across panel variants. |

## Design Decisions
- Keep app-level composition here and reusable building blocks in `sections/`.
- Maintain explicit panel boundaries to avoid cross-app coupling.
- Keep provider-specific serving workflow in the owning app panel. The
  llama.cpp panel uses a dedicated compatible-model section instead of adding
  llama.cpp route/profile controls to the generic model manager used by other
  apps.

## Dependencies
**Internal:** panel section components, app config, hooks.
**External:** React.

## Usage Examples
```tsx
<AppPanelRenderer appId="comfyui" />
```
