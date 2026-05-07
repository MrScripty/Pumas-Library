# frontend hooks

## Purpose
Custom React hooks for backend polling, process status, version/model workflows, and local UI state orchestration.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `useModels.ts` | Model list fetching, backend-pushed model-library refreshes, shared-storage rescans, and stale-while-revalidate FTS search state. |
| `useModels.test.ts` | Hook coverage for initial fetch, rescan refresh, backend-pushed refreshes, cached FTS revalidation, stale response suppression, and new-results notifications. |
| `useModelLibraryUpdateSubscription.ts` | Electron preload subscription adapter for debounced backend-owned model-library update notifications. |
| `useRuntimeProfiles.ts` | Runtime profile snapshot hook and backend-pushed runtime/profile update subscription adapter. |
| `useDownloadCompletionRefresh.ts` | Delayed model-list refresh scheduling when tracked downloads complete or leave active state. |
| `useDownloadCompletionRefresh.test.ts` | Hook coverage for completion refreshes, disappeared-download refreshes, initial completed no-op behavior, and timer cleanup on unmount. |
| `useExistingLibraryChooser.ts` | Existing-library chooser pending state and duplicate-invocation guard. |
| `useExistingLibraryChooser.test.ts` | Hook coverage for chooser success, pending state, duplicate request suppression, and missing-callback no-ops. |
| `useHfAuthPrompt.ts` | Hugging Face auth prompt visibility and auto-open behavior for new auth-required download errors. |
| `useHfAuthPrompt.test.ts` | Hook coverage for new auth errors, repeated existing errors, non-auth errors, and explicit open/close actions. |
| `useModelDownloads.ts` | Download state and operation controls. |
| `useModelDownloads.test.ts` | Hook coverage for startup download recovery, pushed backend updates, duplicate-start protection, and pause/cancel/resume transitions. |
| `useActiveModelDownload.ts` | Top-level selector for the most relevant active model download and active download count from backend-owned download updates. |
| `useActiveModelDownload.test.ts` | Hook coverage for active download prioritization, pushed refreshes, subscription cleanup, and empty-download clearing. |
| `useModelManagerFilters.ts` | Model manager local/remote search, category, kind, and download-mode filter state. |
| `useModelManagerFilters.test.ts` | Hook coverage for local filters, remote kind filters, mode switching, and developer search. |
| `useModelImportPicker.ts` | Model manager file-picker import dialog state and selected import paths. |
| `useModelImportPicker.test.ts` | Hook coverage for import picker success, close/reset behavior, completion callback, and API-unavailable no-op behavior. |
| `useDependencyInstaller.ts` | Root setup dependency installation action state and post-install status refresh. |
| `useDependencyInstaller.test.ts` | Hook coverage for dependency install success, pending state, failure reset, and API-unavailable no-ops. |
| `useRemoteModelSearch.ts` | Debounced Hugging Face search, kind derivation, and follow-up download-detail hydration for remote model discovery. |
| `useRemoteModelSearch.test.ts` | Hook coverage for debounced search flow, blank-query resets, API-unavailable errors, hydration dedupe, and stale-generation protection. |
| `useModelLibraryActions.ts` | Related-model expansion, partial-download recovery, delete orchestration, and remote URL handling for the library UI. |
| `useModelLibraryActions.test.ts` | Hook coverage for related-model fetch caching, offline errors, partial-download recovery flows, delete-side cancellation, and remote URL opening. |
| `useModelPreferences.ts` | Local model starring and backend-owned link-exclusion preference state. |
| `useModelPreferences.test.ts` | Hook coverage for backend exclusion loading, local starring, optimistic link persistence, rollback, and API-unavailable no-ops. |
| `useDiskSpace.ts` | Disk-space lookup state for storage-health indicators in version and app controls. |
| `useDiskSpace.test.ts` | Hook coverage for successful disk-space updates, unavailable-API no-ops, and failure swallowing. |
| `statusTelemetryStore.ts` | Process-wide status telemetry snapshot cache, refresh coalescing, and Electron subscription ownership shared by status hooks. |
| `useNetworkStatus.ts` | Selector over shared status telemetry for offline, rate-limit, and circuit-breaker status indicators. |
| `useNetworkStatus.test.ts` | Hook coverage for initial status derivation, zero-request defaults, pushed telemetry updates, manual refresh, and error propagation. |
| `useStatus.ts` | Launcher/app status selector and refresh behavior backed by shared status telemetry. |
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
| `useAppStartupChecks.ts` | Root startup API readiness wait, background launcher update check, disk-space fetch, and active-version status refresh. |
| `useAppStartupChecks.test.ts` | Hook coverage for API readiness waits, update timer cleanup, and active-version status refresh. |
| `useAppWindowActions.ts` | Root-shell helpers for window controls and shared filesystem open actions. |
| `useAppWindowActions.test.ts` | Hook coverage for backend/window routing of models-root and window control actions. |
| `useLauncherUpdates.ts` | Launcher update availability, update-check action state, and update URL opening. |
| `useLauncherUpdates.test.ts` | Hook coverage for update metadata storage, URL preference, stale-state clearing, and API-unavailable no-ops. |
| `useSelectedAppVersions.ts` | Selected-app version hook routing for ComfyUI, Ollama, Torch, and unsupported app fallback state. |
| `useSelectedAppVersions.test.ts` | Hook coverage for selected-app version tracking and unsupported version state. |
| `usePhysicsDrag.ts` | Physics-based drag state, frame updates, delete/reorder commits, and public hook output. |
| `usePhysicsDragDelete.ts` | Drag delete completion, fallback reset, and post-delete cleanup lifecycle. |
| `usePhysicsDragPointerEvents.ts` | Pointer pending, drag, release, cancel, and blur lifecycle for physics drag. |
| `usePhysicsDragSettle.ts` | Spring settle animation from current drag position to the resolved anchor. |
| `usePhysicsDragUndo.ts` | Keyboard undo listener and selection restoration for the last drag reorder/delete snapshot. |
| `physicsDragUtils.ts` | Shared constants, types, and pure drag math used by `usePhysicsDrag.ts`. |
| `physicsDragUtils.test.ts` | Unit coverage for drag selection fallback, anchor hysteresis, reorder helpers, and delete-zone math. |
| `useInstallationProgress.ts` | Installation-progress polling, cancellation notices, and failed-install tracking for the install dialog. |
| `useInstallationProgress.test.ts` | Hook coverage for external progress sync, local polling, cancellation notices, and completion-stop behavior. |
| `useReleaseSizeCalculation.ts` | Install-dialog background release-size calculation with one-run-per-open-session guard and version refresh. |
| `useReleaseSizeCalculation.test.ts` | Hook coverage for size calculation, skipped states, open-session guard, and close/reopen reset behavior. |
| `useInstallDialogLinks.ts` | Install-dialog log-path and release-link opening through the backend bridge with browser fallback for release URLs. |
| `useInstallDialogLinks.test.ts` | Hook coverage for log opening, unavailable bridge handling, release URL opening, and browser fallback. |
| `useConflictResolutions.ts` | Conflict-resolution dialog state for defaults, counts, bulk changes, expansion, and async apply handling. |
| `useConflictResolutions.test.ts` | Hook coverage for default resolutions, individual and bulk changes, expanded rows, and apply state. |
| `useInstallationAccess.ts` | Filesystem-open and version-info helpers for installed version management flows. |
| `useInstallationAccess.test.ts` | Hook coverage for API gating, active-install access, path opening, version-info lookups, and backend failure surfacing. |
| `useInstallationManager.ts` | Version install, switch, remove, progress polling, and install-access orchestration. |
| `useInstallationManager.test.ts` | Hook coverage for install progress normalization, completion reset, install failure cleanup, and polling startup. |
| `useInstallationState.ts` | Install-dialog UI state for filters, hover state, completed-item disclosure, and list/details routing. |
| `useInstallationState.test.ts` | Hook coverage for dialog-open resets and detail-view fallback when progress state disappears. |

## Design Decisions
- Hooks encapsulate async side effects and state transitions outside UI components.
- Domain hooks consume typed API wrappers and return UI-friendly state.
- Backend-pushed model-library notifications refresh canonical model data
  through `useModels`; integrity labels remain derived from backend projections,
  not directly mutated in display components.
- Backend-pushed runtime profile notifications refresh canonical runtime
  profile snapshots through `useRuntimeProfiles`; profile settings and routes
  remain backend-confirmed rather than optimistic component state.
- Status telemetry is backend-owned and shared through `statusTelemetryStore`.
  Status consumers must select from that store instead of opening duplicate
  snapshot requests or Electron subscriptions.
- Polling remains hook-owned for resources that still lack a durable event
  stream. Polling hooks must own overlap prevention, cleanup on unmount, and
  API-unavailable fallback behavior in the hook instead of pushing timers into
  components.

## Timer Ownership
| Polling Owner | Current Reason | Required Guardrail |
| ------------- | -------------- | ------------------ |
| `statusTelemetryStore.ts` | Launcher, resource, network, and model-library status telemetry is backend-owned and pushed through Electron. | Keep one initial snapshot request and one Electron subscription active across mounted status consumers. |
| `useActiveModelDownload.ts` and `useModelDownloads.ts` | Download progress is backend-owned and exposed through model-download update subscriptions. | Load one startup snapshot, subscribe through Electron, and unsubscribe on unmount. |
| `useInstallationManager.ts` and `useInstallationProgress.ts` | Installation progress can outlive a single dialog render and still lacks a push channel. | Stop polling on completion/cancel and clear completion-delay timers. |
| `useAvailableVersionState.ts` | Background fetch status is backend-owned cache state without push notifications. | Clear wait, interval, and follow-up refresh timers together. |

Event-driven replacement trigger: when the RPC backend exposes durable app
event streams for installs, downloads, and cache updates, those polling owners
should move behind subscription adapters and the per-hook intervals should be
removed.

Runtime profile status already has a backend event stream. New runtime/profile
views should use `useRuntimeProfiles` or
`useRuntimeProfileUpdateSubscription` instead of adding component-owned
intervals.

## Download Progress Keys
- Download state maps are keyed by selected artifact, not by repository, when
  the backend provides that identity. The key preference is
  `selectedArtifactId`, then `artifactId`, then legacy `repoId`.
- `repoId` remains display and provenance data. It must not be treated as a
  unique active-download key because one Hugging Face repository can expose
  multiple artifact selections.
- `artifactId` is a compatibility alias for selected-artifact identity in JSON
  responses. Hooks should preserve it for older callers but prefer
  `selectedArtifactId` in new code.
- Repo-keyed names such as `downloadStatusByRepo` are compatibility naming
  debt. New hook code should use artifact-key semantics even when an older prop
  or return field still carries the repo wording.
- Same-repository downloads may coexist when their selected-artifact keys or
  destination paths differ. Duplicate-start guards should block only the same
  selected artifact or the same destination path.

## Dependencies
**Internal:** `api/`, `types/`, `utils/`, and component state needs.
**External:** React hooks.

## Usage Examples
```tsx
const { models, isLoading } = useModels();
```
