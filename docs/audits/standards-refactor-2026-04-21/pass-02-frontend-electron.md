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

- `frontend/src/App.tsx` at 411 lines after launcher update, model preference, and dependency install state extraction;
- `frontend/src/components/ModelManager.tsx` at 361 lines after filter, import-picker, existing-library chooser, HF auth prompt, and download-refresh state extraction;
- `frontend/src/components/LocalModelsList.tsx` at 307 lines and 299 effective lines after related-panel, empty-state, metadata-summary, name-button, and group-header extraction;
- `frontend/src/components/model-import/useModelImportWorkflow.ts` at 295 lines and 259 effective lines after embedded-metadata toggle, sharded-set detection, and metadata lookup extraction;
- `frontend/src/components/InstallDialog.tsx` at 432 lines after release-size calculation extraction;
- `frontend/src/components/ConflictResolutionDialog.tsx` at 454 lines.

The standards require one owner for state machines and backend-owned data to be confirmed by the backend, not speculatively owned by the frontend.

Rectification:
- Completed: move launcher update availability, update-check action state, URL opening, and stale-update clearing from `App.tsx` into `frontend/src/hooks/useLauncherUpdates.ts` with hook tests.
- Completed: clear the delayed background launcher update check on `App.tsx` unmount now that the timer belongs to the extracted startup wiring.
- Completed: move model starring and backend-owned link-exclusion preferences from `App.tsx` into `frontend/src/hooks/useModelPreferences.ts` with load, optimistic update, rollback, and API-unavailable tests.
- Completed: prevent in-flight link-exclusion loads from overwriting newer local optimistic changes by guarding backend load application with an exclusion revision.
- Completed: move setup dependency installation action state and error classification from `App.tsx` into `frontend/src/hooks/useDependencyInstaller.ts` with success, pending-state, failure, and API-unavailable tests.
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
- Completed: move install-dialog background release-size calculation from `InstallDialog.tsx` into `frontend/src/hooks/useReleaseSizeCalculation.ts` with size calculation, skipped-state, session guard, and close/reopen reset tests.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/components/InstallDialog.tsx` from 430 to 390 effective lines after release-size calculation extraction.
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
- `InstallDialog`, `ModelMetadataModal`, `ConflictResolutionDialog`, and `MappingPreviewDialog` use named dialogs, native backdrop buttons, Escape handling, and focus-return tests.
- Install cancellation and migration report destructive actions use app-owned confirmation dialogs instead of `window.confirm`.

### F07 - Frontend Lint Config Has Legacy Waivers That Hide Refactor Pressure
Status: partially remediated

`frontend/eslint.config.js` disables:

- `@typescript-eslint/no-unnecessary-condition`
- `@typescript-eslint/no-non-null-assertion`
- `max-lines`
- `max-lines-per-function`
- `complexity`

The file comments say this is due to legacy noise. That is an acceptable temporary explanation, but it should become a tracked ratchet with explicit thresholds and target files.

Rectification:
- Completed: `frontend/scripts/check-file-size.js` now enforces a committed baseline ratchet so existing oversized files cannot grow and new files still fail above 300 source lines.
- Completed: `frontend/scripts/file-size-baseline.json` records the current oversized file baseline for decomposition tracking.
- Completed: `frontend/scripts/file-size-baseline.json` ratchets `src/App.tsx` from 498 to 376 effective lines after the F04 state-owner extractions.
- Completed: broad lint waiver comments now point at audit F07 instead of an untracked “for now” note.
- Remaining: convert `@typescript-eslint/no-unnecessary-condition`, `@typescript-eslint/no-non-null-assertion`, `max-lines-per-function`, and `complexity` to scoped overrides or enforceable warnings after the first decomposition wave.

## Pass 02 Refactor Inputs
- Desktop bridge executable contract.
- Electron IPC validation module.
- Frontend state ownership review.
- Timer/polling ownership review.
- Accessibility remediation queue.
- Frontend lint ratchet.
