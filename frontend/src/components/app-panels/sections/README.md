# frontend app-panels sections

## Purpose
Composable section components used by app panels to render status, selectors, dependency info, and runtime controls.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `ModelSelectorSection.tsx` | Model selection section UI. |
| `DependencyStatusSection.tsx` | Dependency health and missing-packages display. |
| `StatsSection.tsx` | Runtime/resource/status metrics block. |
| `OllamaRegisteredModels.tsx` | Presentational registered Ollama model list with load, unload, delete, loaded-state, and VRAM details. |
| `OllamaRegisteredModels.test.tsx` | Rendering and interaction coverage for registered Ollama model state, actions, disabled controls, and size formatting. |
| `OllamaModelSection.tsx` | Ollama library/registered model controls; refreshes from running-state changes, local operations, and runtime-profile update events rather than owning a polling interval. |
| `LlamaCppModelLibrarySection.tsx` | Focused llama.cpp local model library panel that lists compatible GGUF models without entering the generic remote-download model manager state machine. |
| `RuntimeProfileSettingsSection.tsx` | Backend-confirmed runtime profile settings section for Ollama and llama.cpp profile lifecycle. |
| `RuntimeProfileSettingsEditor.tsx` | Runtime profile editor shell that composes field and action subcomponents. |
| `RuntimeProfileSettingsFields.tsx` | Runtime profile identity, endpoint, mode, and device setting fields. |
| `RuntimeProfileSettingsActions.tsx` | Runtime profile save, delete, start, and stop controls. |
| `RuntimeProfileSettingsDraft.ts` | Draft/profile conversion helpers for runtime profile settings. |
| `RuntimeProfileSettingsList.tsx` | Runtime profile list and status display. |
| `RuntimeProfileSettingsShared.ts` | Runtime profile labels, mode/device option helpers, and shared draft types. |
| `ollamaModelFormatting.ts` | Shared display formatter for Ollama model and VRAM sizes. |
| `TorchModelSlotsSection.tsx` | Torch model slot management section. |
| `TorchActiveSlots.tsx` | Presentational active Torch slot list with unload controls, state badges, and device memory summaries. |
| `TorchActiveSlots.test.tsx` | Rendering and interaction coverage for active Torch slot badges, memory summaries, unload controls, and size formatting. |
| `torchModelSlotFormatting.ts` | Shared display formatter for Torch model and device memory sizes. |
| `llamaCppLibraryViewModels.ts` | Pure llama.cpp library compatibility, served-instance identity, route, status, and placement-label derivation helpers. |
| `TorchServerConfigSection.tsx` | Torch server configuration controls. |
| `index.ts` | Section exports for panel composition. |

## Design Decisions
- Keep sections focused and composable to reduce duplicated panel markup.
- Shared section API surface is centralized via `index.ts` exports.
- Section-level polling is allowed only for backend state that is not available
  through a shared hook or event stream. Runtime/profile views use the
  backend-pushed runtime profile event path.

## Timer Ownership
| Section | Current Reason | Required Guardrail |
| ------- | -------------- | ------------------ |
| `ModelSelectorSection.tsx` | Loaded model options are read from app-specific backend state. | Clear the interval on unmount and avoid polling when the app is not running. |
| `StatsSection.tsx` | Runtime stats are sampled while the app is running. | Make interval configurable and clear it on dependency changes/unmount. |
| `TorchModelSlotsSection.tsx` | Torch slot state is backend-owned and currently sampled. | Poll only while the app is running and clear on unmount. |

Event-driven replacement trigger: when app panels receive a shared runtime-state
subscription, section intervals should collapse into that owner or move into
dedicated hooks.

## Dependencies
**Internal:** shared types/hooks/components.
**External:** React.

## Usage Examples
```tsx
<StatsSection stats={stats} />
```
