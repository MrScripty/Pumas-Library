# Pass 02 - Frontend and Electron

## Standards Consulted
- `FRONTEND-STANDARDS.md`
- `ACCESSIBILITY-STANDARDS.md`
- `SECURITY-STANDARDS.md`
- `CONCURRENCY-STANDARDS.md`
- `INTEROP-STANDARDS.md`
- `TESTING-STANDARDS.md`
- `TOOLING-STANDARDS.md`

## Positive Baseline
- `frontend/tsconfig.json` enables strict TypeScript checks, `noUncheckedIndexedAccess`, `noImplicitReturns`, and related safety options.
- `frontend/eslint.config.js` uses ESLint 9 flat config and includes `eslint-plugin-jsx-a11y`.
- React 19 rules for automatic JSX runtime are handled.
- Many frontend hooks/components have colocated Vitest tests.
- Electron window settings use `contextIsolation: true`, `nodeIntegration: false`, `sandbox: true`, and `webSecurity: true`.
- `shell:openExternal` restricts external URLs to `http:` and `https:`.

## Findings

### F01 - Desktop Bridge Contract Is Hand-Maintained in Multiple Places
Status: partially remediated; schema generation remains open

The renderer contract is represented by `frontend/src/types/api.ts` at 2,176 lines, Electron preload exposes a large manual `electronAPI` object in `electron/src/preload.ts`, and Rust JSON-RPC dispatch separately maps string method names in `rust/crates/pumas-rpc/src/handlers/mod.rs`.

This creates a high-risk drift path:

- TypeScript types can compile while runtime payload shape diverges.
- Preload can expose methods that Rust does not validate as typed requests.
- Rust handlers use `serde_json::Value` and local helper extraction in many handlers instead of shared executable schemas.

Rectification:
- Completed: define `electron/src/rpc-method-registry.ts` as the executable desktop RPC method registry with method name, ownership, stability tier, request-schema, response-schema, and params-validation policy metadata.
- Completed: make `electron/src/ipc-validation.ts` consume the registry instead of owning a separate inline method list.
- Completed: add an Electron package test that rejects duplicate registry entries and verifies representative allowed methods still pass runtime validation.
- Remaining: add per-method request and response schemas.
- Generate or validate:
  - `frontend/src/types/api.ts`
  - `electron/src/preload.ts` bridge signatures
  - Rust JSON-RPC route/dispatch metadata
  - contract tests for representative methods.
- Keep hand-written behavior in implementation modules only.

### F02 - Electron IPC Payloads Are Not Validated at the Main-Process Boundary
Status: remediated

`electron/src/main.ts` uses handlers such as:

- `ipcMain.handle('api:call', async (_event, method: string, params: Record<string, unknown>) => ...)`
- `ipcMain.handle('dialog:openFile', async (_event, options: Electron.OpenDialogOptions) => ...)`
- `ipcMain.handle('shell:openExternal', async (_event, url: string) => ...)`

Type annotations in Electron main do not validate untrusted renderer input. The standards explicitly state preload and renderer typing are not a security boundary.

Rectification:
- Introduce `electron/src/ipc-validation.ts`.
- Validate `api:call` method against an allowlist and validate params shape before forwarding.
- Validate dialog options and restrict allowed properties from renderer callers.
- Change `shell:openExternal` raw input type to `unknown`, validate string shape, parse URL, and restrict schemes.

Implementation notes:
- `electron/src/ipc-validation.ts` validates RPC method names, params records, dialog options, and external URL schemes before main-process handlers use renderer-supplied values.
- `electron/tests/ipc-validation.test.mjs` covers malformed methods, non-record params, unsupported dialog fields, malformed filters, and non-http URL schemes through the package-local Electron test script.

### F03 - Frontend Uses Local Casts Around `window.electronAPI`
Status: remediated for local casts; executable contract consolidation remains in F01

Examples:

- `frontend/src/api/adapter.ts`
- `frontend/src/components/ModelImportDropZone.tsx`
- `frontend/src/config/theme.ts`

These use `window as unknown as { electronAPI: ... }` casts. This is tolerable in a thin adapter if the global type is declared once, but current casts repeat local shape fragments.

Rectification:
- Completed: `frontend/src/types/api.ts` declares the global `Window.electronAPI` augmentation.
- Completed: direct bridge reads are centralized through `frontend/src/api/adapter.ts` and `getElectronAPI()`.
- Completed: component-level bridge casts were removed from the audited source paths.
- Completed: adapter tests cover Electron detection and browser fallback behavior.

### F04 - UI State and Backend-Owned Data Boundaries Need Review
Status: partially remediated; large workflow owners remain

Long UI modules combine display, backend calls, local derived state, and workflow transitions:

- `frontend/src/App.tsx` at 302 lines and 288 effective lines after launcher update, model preference, dependency install, app shell, startup-check, selected-version, panel-prop, and shell-state extraction;
- `frontend/src/components/ModelManager.tsx` at 307 lines and 282 effective lines after filter, import-picker, existing-library chooser, HF auth prompt, download-refresh, and remote-download starter extraction;
- `frontend/src/components/LocalModelsList.tsx` at 307 lines and 299 effective lines after related-panel, empty-state, metadata-summary, name-button, and group-header extraction;
- `frontend/src/components/model-import/useModelImportWorkflow.ts` at 295 lines and 259 effective lines after embedded-metadata toggle, sharded-set detection, and metadata lookup extraction;
- `frontend/src/components/model-import/ImportLookupCard.tsx` at 193 lines and 182 effective lines after expanded metadata details extraction;
- `frontend/src/components/InstallDialog.tsx` at 307 lines and 275 effective lines after release-size calculation, link-opening, and frame extraction;
- `frontend/src/components/ConflictResolutionDialog.tsx` at 235 lines and 220 effective lines after conflict-resolution state and row extraction.
- `frontend/src/components/ModelMetadataModal.tsx` at 302 lines and 275 effective lines after metadata modal frame extraction.
- `frontend/src/components/RemoteModelListItem.tsx` at 233 lines and 225 effective lines after remote model summary extraction.

The standards require one owner for state machines and backend-owned data to be confirmed by the backend, not speculatively owned by the frontend.

Rectification:
- Completed: move launcher update availability, update-check action state, URL opening, and stale-update clearing from `App.tsx` into `frontend/src/hooks/useLauncherUpdates.ts` with hook tests.
- Completed: clear the delayed background launcher update check on `App.tsx` unmount now that the timer belongs to the extracted startup wiring.
- Completed: move model starring and backend-owned link-exclusion preferences from `App.tsx` into `frontend/src/hooks/useModelPreferences.ts` with load, optimistic update, rollback, and API-unavailable tests.
- Completed: prevent in-flight link-exclusion loads from overwriting newer local optimistic changes by guarding backend load application with an exclusion revision.
- Completed: move setup dependency installation action state and error classification from `App.tsx` into `frontend/src/hooks/useDependencyInstaller.ts` with success, pending-state, failure, and API-unavailable tests.
- Completed: move app shell rendering from `App.tsx` into `frontend/src/components/AppShell.tsx`.
- Completed: move app-panel prop graph construction from `App.tsx` into `frontend/src/components/AppShellPanels.ts` with shared/app-specific panel prop tests.
- Completed: move App running flags, selected-app display state, setup display state, managed-app inputs, model-manager props, and header/sidebar prop construction from `App.tsx` into `frontend/src/components/AppShellState.ts` with projection and prop-builder tests.
- Completed: move startup API readiness wait, delayed launcher update check, disk-space fetch, and active-version status refresh from `App.tsx` into `frontend/src/hooks/useAppStartupChecks.ts` with readiness, cleanup, and refresh tests.
- Completed: move selected-app version hook routing from `App.tsx` into `frontend/src/hooks/useSelectedAppVersions.ts` with selected-app and unsupported-state tests.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/App.tsx` from the oversized-file baseline after it reached 299 effective lines.
- Completed: move model manager search, category, remote-kind, and download-mode filter state into `frontend/src/hooks/useModelManagerFilters.ts` with local/remote/developer-search tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/ModelManager.tsx` from 401 to 388 effective lines after filter-state extraction.
- Completed: move model manager file-picker import dialog state into `frontend/src/hooks/useModelImportPicker.ts` with picker success, close/reset, completion callback, and API-unavailable tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/ModelManager.tsx` from 388 to 368 effective lines after import-picker extraction.
- Completed: move existing-library chooser pending state and duplicate-invocation guard into `frontend/src/hooks/useExistingLibraryChooser.ts` with success, pending, duplicate, and missing-callback tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/ModelManager.tsx` from 368 to 361 effective lines after existing-library chooser extraction.
- Completed: move Hugging Face auth prompt visibility and new-auth-error detection into `frontend/src/hooks/useHfAuthPrompt.ts` with new-error, repeated-error, non-auth-error, and explicit open/close tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/ModelManager.tsx` from 361 to 355 effective lines after HF auth prompt extraction.
- Completed: move delayed model-list refresh scheduling after completed or disappeared downloads into `frontend/src/hooks/useDownloadCompletionRefresh.ts`, with completion, disappeared-download, initial completed no-op, and unmount cleanup tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/ModelManager.tsx` from 355 to 333 effective lines after download-refresh extraction.
- Completed: move remote Hugging Face download payload shaping, backend result handling, error updates, and auth escalation from `ModelManager.tsx` into `frontend/src/components/ModelManagerRemoteDownload.ts` with successful start, backend failure, and auth-required error tests.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/components/ModelManager.tsx` from the oversized-file baseline after it reached 282 effective lines.
- Completed: move related-model expansion rendering from `LocalModelsList.tsx` into `frontend/src/components/RelatedModelsPanel.tsx` with loading, error, empty, and URL-opening tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/LocalModelsList.tsx` from 455 to 402 effective lines after related-panel extraction.
- Completed: move empty-library and no-match filter rendering from `LocalModelsList.tsx` into `frontend/src/components/LocalModelsEmptyState.tsx` with picker, disabled, default empty, and clear-filter tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/LocalModelsList.tsx` from 402 to 374 effective lines after empty-state extraction.
- Completed: move local model format, quant, size, dependency, and partial-error rendering from `LocalModelsList.tsx` into `frontend/src/components/LocalModelMetadataSummary.tsx` with fallback, dependency, and partial-error tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/LocalModelsList.tsx` from 374 to 348 effective lines after metadata-summary extraction.
- Completed: move local model name metadata access and status badge rendering from `LocalModelsList.tsx` into `frontend/src/components/LocalModelNameButton.tsx` with modified-click and badge tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/LocalModelsList.tsx` from 348 to 304 effective lines after name-button extraction.
- Completed: move local model group header rendering from `LocalModelsList.tsx` into `frontend/src/components/LocalModelGroupHeader.tsx` with category/count tests.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/components/LocalModelsList.tsx` from the oversized-file baseline after it reached 299 effective lines.
- Completed: move import embedded-metadata disclosure and on-demand metadata loading from `useModelImportWorkflow.ts` into `frontend/src/components/model-import/useEmbeddedMetadataToggles.ts` with loading, cached, unsupported, and all-field toggle tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/model-import/useModelImportWorkflow.ts` from 451 to 391 effective lines after embedded-metadata toggle extraction.
- Completed: move import sharded-file detection and expansion state from `useModelImportWorkflow.ts` into `frontend/src/components/model-import/useShardedSetDetection.ts` with detection, entry annotation, expansion, and clear-state tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/model-import/useModelImportWorkflow.ts` from 391 to 354 effective lines after sharded-set detection extraction.
- Completed: move import metadata lookup execution from `useModelImportWorkflow.ts` into `frontend/src/components/model-import/modelImportMetadataLookup.ts` with embedded match, bundle lookup, invalid file, and progress tests.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/components/model-import/useModelImportWorkflow.ts` from the oversized-file baseline after it reached 259 effective lines.
- Completed: move expanded metadata details from `ImportLookupCard.tsx` into `frontend/src/components/model-import/ImportMetadataDetails.tsx` with Hugging Face field, source-switching, linked embedded metadata, and hidden-field disclosure tests.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/components/model-import/ImportLookupCard.tsx` from the oversized-file baseline after it reached 182 effective lines.
- Completed: move install-dialog background release-size calculation from `InstallDialog.tsx` into `frontend/src/hooks/useReleaseSizeCalculation.ts` with size calculation, skipped-state, session guard, and close/reopen reset tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/InstallDialog.tsx` from 430 to 390 effective lines after release-size calculation extraction.
- Completed: move install-dialog log-path and release-link opening from `InstallDialog.tsx` into `frontend/src/hooks/useInstallDialogLinks.ts` with backend bridge, unavailable bridge, and browser fallback tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/InstallDialog.tsx` from 390 to 356 effective lines after link-opening extraction.
- Completed: move install-dialog modal/page frame rendering, backdrop dismissal, Escape handling, and focus restoration from `InstallDialog.tsx` into `frontend/src/components/InstallDialogFrame.tsx` with modal and page-frame tests.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/components/InstallDialog.tsx` from the oversized-file baseline after it reached 275 effective lines.
- Completed: move conflict-resolution default decisions, counts, bulk updates, expanded row state, and async apply handling from `ConflictResolutionDialog.tsx` into `frontend/src/hooks/useConflictResolutions.ts` with default, individual, bulk, expansion, and apply tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/ConflictResolutionDialog.tsx` from 421 to 379 effective lines after conflict-resolution state extraction.
- Completed: move conflict row summary, selector, expanded path details, and per-action descriptions from `ConflictResolutionDialog.tsx` into `frontend/src/components/ConflictResolutionItem.tsx` with summary, selector, and expanded-detail tests.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/components/ConflictResolutionDialog.tsx` from the oversized-file baseline after it reached 220 effective lines.
- Completed: move metadata modal frame rendering, header actions, backdrop dismissal, Escape handling, and focus restoration from `ModelMetadataModal.tsx` into `frontend/src/components/ModelMetadataModalFrame.tsx` with modal shell, dismissal, and disabled-refetch tests.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/components/ModelMetadataModal.tsx` from the oversized-file baseline after it reached 275 effective lines.
- Completed: move remote model metadata summary rendering from `RemoteModelListItem.tsx` into `frontend/src/components/RemoteModelSummary.tsx` with metadata, developer-search, auth, and retry tests.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/components/RemoteModelListItem.tsx` from the oversized-file baseline after it reached 225 effective lines.
- Completed: move physics drag pointer lifecycle, delete completion, undo restoration, and settle animation from `usePhysicsDrag.ts` into `frontend/src/hooks/usePhysicsDragPointerEvents.ts`, `frontend/src/hooks/usePhysicsDragDelete.ts`, `frontend/src/hooks/usePhysicsDragUndo.ts`, and `frontend/src/hooks/usePhysicsDragSettle.ts`, preserving drag/reorder/delete tests.
- Completed: split `frontend/src/types/api.ts` into domain response modules, bridge-domain interfaces, and Electron global augmentation while keeping `api.ts` as the public re-export barrel.
- Remaining: classify each remaining local state variable as transient UI, form input, derived view state, or backend-owned.
- Move durable workflow state to backend or a single owning hook.
- Extract presentational subcomponents only after state ownership is settled.

### F05 - Polling Is Widespread and Needs Justification or Event-Driven Replacement
Status: partially remediated

Polling/timer locations include:

- `frontend/src/hooks/useNetworkStatus.ts`
- `frontend/src/hooks/usePlugins.ts`
- `frontend/src/hooks/useActiveModelDownload.ts`
- `frontend/src/hooks/useInstallationManager.ts`
- `frontend/src/hooks/useStatus.ts`
- `frontend/src/hooks/useInstallationProgress.ts`
- `frontend/src/hooks/useAvailableVersionState.ts`
- `frontend/src/hooks/useModelDownloads.ts`
- `frontend/src/components/app-panels/sections/ModelSelectorSection.tsx`
- `frontend/src/components/app-panels/sections/StatsSection.tsx`
- `frontend/src/components/app-panels/sections/OllamaModelSection.tsx`
- `frontend/src/components/app-panels/sections/TorchModelSlotsSection.tsx`
- `electron/src/python-bridge.ts`

Several hooks use refs and cleanup tests, which is good. The gap is architectural: the standards prefer event-driven synchronization and require documentation when polling is unavoidable.

Rectification:
- Completed: `frontend/src/hooks/README.md` now records hook-level polling ownership, current justification, guardrails, and event-stream replacement trigger.
- Completed: `frontend/src/components/app-panels/sections/README.md` now records section-level polling ownership and the shared-hook/event-stream replacement trigger.
- Completed: existing hook tests cover cleanup or polling behavior for active downloads, model downloads, network status, installation manager/progress, available versions, and status.
- Completed: `electron/src/python-bridge.ts` accepts an injectable timer controller, and `electron/tests/python-bridge.test.mjs` covers health-check rescheduling, restart backoff replacement, and stop-time timer cleanup without spawning a backend.
- Completed: `frontend/src/hooks/useDownloadCompletionRefresh.ts` owns the model-manager delayed refresh timer and clears pending refreshes on unmount.
- Completed: move active Torch slot row, unload control, badge, and device memory rendering from `TorchModelSlotsSection.tsx` into `frontend/src/components/app-panels/sections/TorchActiveSlots.tsx` with slot badge, device memory, unload control, and size-formatting tests.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/components/app-panels/sections/TorchModelSlotsSection.tsx` from the oversized-file baseline after it reached 249 effective lines.
- Completed: move registered Ollama model row, load/unload, delete, loaded-state, and VRAM rendering from `OllamaModelSection.tsx` into `frontend/src/components/app-panels/sections/OllamaRegisteredModels.tsx` with action, disabled-control, loaded-state, and size-formatting tests.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/components/app-panels/sections/OllamaModelSection.tsx` from the oversized-file baseline after it reached 246 effective lines.
- Remaining: consolidate install/download/status polling behind a backend event stream or single scheduler when the backend exposes one.

### F06 - Accessibility Is Enforced but Still Has Component-Level Risks
Status: remediated for audited controls

Potential risks found by search:

- `frontend/src/components/AppIndicator.tsx` uses `role="button"` with `onClick` and a keyboard handler.
- `frontend/src/components/VersionSelectorDropdown.tsx` uses `role="button"`.
- dialog backdrops in `ModelMetadataModal.tsx`, `ConflictResolutionDialog.tsx`, and `MappingPreviewDialog.tsx` use clickable `div` patterns.
- `window.confirm` is used in `InstallDialog.tsx` and `MigrationReportsPanel.tsx`, bypassing application dialog focus management.

Some generic interactive wrappers may be legitimate, but the accessibility standards require semantic elements wherever possible and full dialog focus management.

Rectification:
- Replace generic role-button wrappers with `<button type="button">` where layout allows.
- For modal backdrops, document exception and ensure Escape close, focus trap, focus return, and accessible naming.
- Replace `window.confirm` with app-owned confirmation dialogs that manage focus.
- Add keyboard interaction tests for any custom interactive generic element retained.

Implementation notes:
- App launcher indicator, version selector controls, and import lookup metadata expansion now use native buttons instead of custom role-button wrappers.
- Sidebar background deselection now uses pointer handling plus a window Escape listener instead of casting a keyboard event through a generic JSX handler.
- Sidebar app icon motion rendering now lives in `frontend/src/components/SidebarAppIcon.tsx`, keeping `AppSidebar.tsx` below the 300 effective-line standard while preserving the existing selection, background deselection, Escape, and add-button tests.
- `InstallDialog`, `ModelMetadataModal`, `ConflictResolutionDialog`, and `MappingPreviewDialog` use named dialogs, native backdrop buttons, Escape handling, and focus-return tests.
- Install cancellation and migration report destructive actions use app-owned confirmation dialogs instead of `window.confirm`.

### F07 - Frontend Lint Config Has Legacy Waivers That Hide Refactor Pressure
Status: partially remediated

`frontend/eslint.config.js` still disables:

- `@typescript-eslint/no-unnecessary-condition`

The file comments say this is due to legacy noise. That is an acceptable temporary explanation, but it should become a tracked ratchet with explicit thresholds and target files.

Rectification:
- Completed: `frontend/scripts/check-file-size.js` now enforces a committed baseline ratchet so existing oversized files cannot grow and new files still fail above 300 source lines.
- Completed: `frontend/scripts/file-size-baseline.json` records the current oversized file baseline for decomposition tracking.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/App.tsx` from 498 to 376 effective lines after the F04 state-owner extractions.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/App.tsx` from the oversized-file baseline after shell, startup-check, selected-version, and panel-prop extraction reached 299 effective lines.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/components/AppSidebar.tsx` from the oversized-file baseline after it reached 256 effective lines.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/hooks/usePhysicsDrag.ts` from the oversized-file baseline after drag lifecycle helper extraction reached 298 effective lines.
- Completed: `frontend/scripts/file-size-baseline.json` removes `src/types/api.ts` from the oversized-file baseline after API contract decomposition left every `api-*` module below 300 effective lines.
- Completed: split `modelImportWorkflowHelpers.test.ts` and `useRemoteModelSearch.test.ts` by test responsibility, then re-enabled ESLint `max-lines` at 300 effective lines for frontend source files.
- Completed: reduce `frontend/src/hooks/modelDownloadState.ts` selector complexity by extracting download payload normalization from repo-priority selection.
- Completed: reduce `frontend/src/hooks/installationProgressTracking.ts` normalizer complexity by extracting tracker tag/stage synchronization from progress projection.
- Completed: reduce `frontend/src/hooks/useStatus.ts` fetch complexity by moving stale system-resource, network-status, and library-status refreshes behind focused callbacks.
- Completed: reduce `frontend/src/components/ModelImportDropZone.tsx` dropped-path extraction complexity by splitting Electron, URI-list, plain-text, and File API fallback extraction paths.
- Completed: reduce `frontend/src/components/app-panels/sections/StatsSection.tsx` render complexity by extracting stat mapping, count card, memory card, and loaded-model badge rendering.
- Completed: reduce `frontend/src/components/SidebarAppIcon.tsx` render complexity by extracting icon-face selection and delete-zone overlay rendering.
- Completed: reduce `frontend/src/components/InstallDialog.tsx` orchestration complexity by extracting version filtering, sticky failure derivation, and installation/cancellation error reporting helpers.
- Completed: reduce `frontend/src/components/model-import/modelImportWorkflowHelpers.ts` import-entry complexity by separating single-result and multi-model-container entry construction.
- Completed: reduce `frontend/src/components/model-import/ImportMetadataDetails.tsx` render complexity by moving metadata state projection and detail-row rendering into focused model-import detail modules.
- Completed: reduce `frontend/src/components/model-import/ImportLookupCard.tsx` render complexity by extracting repository links, trust/status badges, metadata expand controls, status icons, and non-file card rendering.
- Completed: reduce `frontend/src/components/ModelManagerRemoteDownload.ts` remote-download complexity by extracting request shaping, stale-error clearing, error recording, and exception reporting helpers.
- Completed: reduce `frontend/src/components/VersionSelector.tsx` state/handler complexity by moving display-state derivation and version action error reporters into `frontend/src/components/VersionSelectorState.ts`.
- Completed: reduce `frontend/src/components/VersionSelectorDropdown.tsx` item complexity by moving default-version hover state, icon selection, and set/unset error reporting into `frontend/src/components/VersionSelectorDefaultButton.tsx`.
- Completed: reduce `frontend/src/components/VersionSelectorTrigger.tsx` render complexity by moving first-install, default, active-folder, and version-manager status controls into `frontend/src/components/VersionSelectorTriggerControls.tsx`.
- Completed: reduce `frontend/src/components/ModelNotesMarkdownPreview.tsx` render complexity by splitting markdown block collection for code fences, headings, lists, quotes, and paragraphs into focused helpers with renderer coverage.
- Completed: reduce `frontend/src/components/MappingPreviewDetails.tsx` render complexity by extracting cross-filesystem, summary, warning, action-list, apply-result, and control sections into focused mapping preview detail modules with interaction coverage.
- Completed: reduce `frontend/src/components/MappingPreview.tsx` render complexity by moving header rendering, unavailable-state rendering, and derived preview count/status calculation into focused modules with workflow coverage.
- Completed: reduce `frontend/src/components/RemoteModelListItem.tsx` render complexity by extracting remote download flags/options and row action controls into focused modules with action coverage.
- Completed: reduce `frontend/src/components/Header.tsx` render/status complexity by extracting status projection, update/window controls, status badge, and resource strip rendering into focused header modules.
- Completed: reduce `frontend/src/components/VersionListItem.tsx` render complexity by extracting install-version display-state projection, info rendering, and action-button state rendering into focused modules while preserving install-dialog row coverage.
- Completed: reduce `frontend/src/components/LocalModelsList.tsx` render complexity by moving local model row state, metadata, download actions, installed actions, and related-model disclosure into focused row modules while preserving local-model list coverage.
- Completed: reduce `frontend/src/App.tsx` orchestration complexity by moving shell state projection and prop construction into `frontend/src/components/AppShellState.ts` while preserving App shell behavior with focused helper coverage.
- Completed: `frontend/eslint.config.js` enforces ESLint `complexity` at a maximum of 20 after the function-level decomposition wave cleared the full frontend source tree.
- Completed: `frontend/eslint.config.js` enforces a coarse `max-lines-per-function` ratchet at 300 effective lines; the failed 80-line trial inventory is recorded in `pass-02-frontend-function-length-inventory.md`.
- Completed: replace frontend non-null assertions in production helpers and tests with explicit guards, then enforce `@typescript-eslint/no-non-null-assertion`.
- Completed: clear the first low-risk `@typescript-eslint/no-unnecessary-condition` batch in resource, mapping, migration-summary, and library-model helpers; the remaining queue is recorded in `pass-02-frontend-unnecessary-condition-inventory.md`.
- Completed: clear a second low-risk `@typescript-eslint/no-unnecessary-condition` batch in link-health, mapping-preview, migration-report, and import-metadata helpers, reducing the remaining inventory from 78 to 66 findings.
- Completed: clear a third `@typescript-eslint/no-unnecessary-condition` batch in version display-state and default-version controls, reducing the remaining inventory from 66 to 58 findings.
- Completed: broad lint waiver comments now point at audit F07 instead of an untracked “for now” note.
- Remaining: lower `max-lines-per-function` toward the 80-line decomposition target after the inventory queue is reduced, and convert `@typescript-eslint/no-unnecessary-condition` to scoped overrides or enforceable warnings.

## Pass 02 Refactor Inputs
- Desktop bridge executable contract.
- Electron IPC validation module.
- Frontend state ownership review.
- Timer/polling ownership review.
- Accessibility remediation queue.
- Frontend lint ratchet.
