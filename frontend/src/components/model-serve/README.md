# frontend components model-serve

## Purpose
Small components and hooks that compose the model serving page/dialog without coupling runtime
profile selection, placement controls, and serving actions into one oversized component.

## Contents
| File | Description |
| ---- | ----------- |
| `ModelServeDialogContent.tsx` | Container that assembles the serving header, form, status, and actions. |
| `ModelServeHeader.tsx` | Title, model identity, back button, and dialog close control. |
| `ModelServeForm.tsx` | Runtime profile selector, readiness summary, placement controls, and keep-loaded toggle. |
| `ModelServeActions.tsx` | Start serving, unload, and dialog cancel buttons. |
| `ModelServeStatusMessage.tsx` | Shared status/error message presentation. |
| `modelServeHelpers.ts` | Pure helper functions for serving validation state and request config construction. |
| `useDialogFocusTrap.ts` | Dialog-only focus trapping and Escape close behavior. |
| `useModelServingActions.ts` | Serving status lookup plus serve/unserve request actions. |

## Design Decisions
- Keep `ModelServeDialog.tsx` as the public API boundary for existing callers.
- Keep backend request construction in pure helpers so UI controls stay presentational.
- Keep Electron API access in `useModelServingActions.ts`; presentational components receive callbacks only.
