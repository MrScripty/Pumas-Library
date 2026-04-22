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
Status: non-compliant with executable boundary contract expectations

The renderer contract is represented by `frontend/src/types/api.ts` at 2,176 lines, Electron preload exposes a large manual `electronAPI` object in `electron/src/preload.ts`, and Rust JSON-RPC dispatch separately maps string method names in `rust/crates/pumas-rpc/src/handlers/mod.rs`.

This creates a high-risk drift path:

- TypeScript types can compile while runtime payload shape diverges.
- Preload can expose methods that Rust does not validate as typed requests.
- Rust handlers use `serde_json::Value` and local helper extraction in many handlers instead of shared executable schemas.

Rectification:
- Define a single method registry artifact with method name, request schema, response schema, ownership, stability tier, and validation policy.
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
Status: needs decomposition review

Long UI modules combine display, backend calls, local derived state, and workflow transitions:

- `frontend/src/App.tsx` at 563 lines;
- `frontend/src/components/ModelManager.tsx` at 453 lines;
- `frontend/src/components/LocalModelsList.tsx` at 467 lines;
- `frontend/src/components/model-import/useModelImportWorkflow.ts` at 507 lines;
- `frontend/src/components/InstallDialog.tsx` at 449 lines;
- `frontend/src/components/ConflictResolutionDialog.tsx` at 419 lines.

The standards require one owner for state machines and backend-owned data to be confirmed by the backend, not speculatively owned by the frontend.

Rectification:
- Classify each local state variable as transient UI, form input, derived view state, or backend-owned.
- Move durable workflow state to backend or a single owning hook.
- Extract presentational subcomponents only after state ownership is settled.

### F05 - Polling Is Widespread and Needs Justification or Event-Driven Replacement
Status: partially compliant

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
- Add timer ownership comments only where event-driven alternatives are impractical.
- Consolidate install/download/status polling behind a backend event stream or single scheduler if feasible.
- Add deterministic cleanup tests for hooks and Electron bridge timers that lack them.

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
- Completed: broad lint waiver comments now point at audit F07 instead of an untracked “for now” note.
- Remaining: convert `@typescript-eslint/no-unnecessary-condition`, `@typescript-eslint/no-non-null-assertion`, `max-lines-per-function`, and `complexity` to scoped overrides or enforceable warnings after the first decomposition wave.

## Pass 02 Refactor Inputs
- Desktop bridge executable contract.
- Electron IPC validation module.
- Frontend state ownership review.
- Timer/polling ownership review.
- Accessibility remediation queue.
- Frontend lint ratchet.
