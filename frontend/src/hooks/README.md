# frontend hooks

## Purpose
Custom React hooks for backend polling, process status, version/model workflows, and local UI state orchestration.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `useModels.ts` | Model list fetching, shared-storage rescans, and stale-while-revalidate FTS search state. |
| `useModels.test.ts` | Hook coverage for initial fetch, rescan refresh, cached FTS revalidation, stale search suppression, and new-results notifications. |
| `useModelDownloads.ts` | Download state and operation controls. |
| `useModelDownloads.test.ts` | Hook coverage for startup download recovery, active polling updates, duplicate-start protection, and pause/cancel/resume transitions. |
| `useActiveModelDownload.ts` | Top-level polling for the most relevant active model download and active download count. |
| `useActiveModelDownload.test.ts` | Hook coverage for active download prioritization, polling refreshes, API-unavailable resets, and empty-download clearing. |
| `useRemoteModelSearch.ts` | Debounced Hugging Face search, kind derivation, and follow-up download-detail hydration for remote model discovery. |
| `useRemoteModelSearch.test.ts` | Hook coverage for debounced search flow, blank-query resets, API-unavailable errors, hydration dedupe, and stale-generation protection. |
| `useModelLibraryActions.ts` | Related-model expansion, partial-download recovery, delete orchestration, and remote URL handling for the library UI. |
| `useModelLibraryActions.test.ts` | Hook coverage for related-model fetch caching, offline errors, partial-download recovery flows, delete-side cancellation, and remote URL opening. |
| `useNetworkStatus.ts` | Polling and derivation of offline, rate-limit, and circuit-breaker status for network health indicators. |
| `useNetworkStatus.test.ts` | Hook coverage for initial status derivation, zero-request defaults, interval polling, overlap protection, and error propagation. |
| `useStatus.ts` | Launcher/app status polling and refresh behavior. |
| `useVersions.ts` | Version list and version operations state flow. |
| `useVersions.test.ts` | Hook coverage for API-gated refresh startup, installing-tag merge behavior, and install-refresh wiring. |
| `useVersionShortcutState.ts` | Shortcut-toggle state for installed versions, backend refresh, and active-version shortcut sync. |
| `useVersionShortcutState.test.ts` | Hook coverage for shortcut-state refresh, support resets, active-version sync, optimistic toggles, and rollback on toggle failure. |
| `useAvailableVersionState.ts` | Available-version caching, background fetch polling, rate-limit state, and installing-tag discovery. |
| `useAvailableVersionState.test.ts` | Hook coverage for version mapping, follow-up refresh scheduling, rate-limit handling, and background-fetch refresh. |
| `useVersionFetching.ts` | Fetches installed, active, default, status, and available version state for one app. |
| `useVersionFetching.test.ts` | Hook coverage for refresh orchestration, default-version updates, and available-version fetch errors. |
| `useAppImportDialog.ts` | App-level drag-and-drop import dialog state and completion handlers. |
| `useAppImportDialog.test.ts` | Hook coverage for app-level import dialog open/close and completion behavior. |
| `useAppProcessActions.ts` | Shared launch/stop/log handlers for app process controls at the root shell level. |
| `useAppProcessActions.test.ts` | Hook coverage for app-process launch/stop routing and delayed refresh behavior. |
| `useAppWindowActions.ts` | Root-shell helpers for window controls and shared filesystem open actions. |
| `useAppWindowActions.test.ts` | Hook coverage for backend/window routing of models-root and window control actions. |
| `usePhysicsDrag.ts` | Physics-based drag behavior for interactive UI elements. |
| `physicsDragUtils.ts` | Shared constants, types, and pure drag math used by `usePhysicsDrag.ts`. |
| `physicsDragUtils.test.ts` | Unit coverage for drag selection fallback, anchor hysteresis, reorder helpers, and delete-zone math. |
| `useInstallationProgress.ts` | Installation-progress polling, cancellation notices, and failed-install tracking for the install dialog. |
| `useInstallationProgress.test.ts` | Hook coverage for external progress sync, local polling, cancellation notices, and completion-stop behavior. |
| `useInstallationAccess.ts` | Filesystem-open and version-info helpers for installed version management flows. |
| `useInstallationAccess.test.ts` | Hook coverage for API gating, active-install access, path opening, version-info lookups, and backend failure surfacing. |
| `useInstallationManager.ts` | Version install, switch, remove, progress polling, and install-access orchestration. |
| `useInstallationManager.test.ts` | Hook coverage for install progress normalization, completion reset, install failure cleanup, and polling startup. |
| `useInstallationState.ts` | Install-dialog UI state for filters, hover state, completed-item disclosure, and list/details routing. |
| `useInstallationState.test.ts` | Hook coverage for dialog-open resets and detail-view fallback when progress state disappears. |

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
