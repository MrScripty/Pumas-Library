# Plan: llama.cpp Compatible Library Profile Serving

## Objective

Make the llama.cpp app page's bottom model library panel a focused serving workspace:
only llama.cpp-compatible local models are listed, each model can be assigned a
llama.cpp runtime profile from the row, and served models clearly show the
hardware placement actually used: CPU, GPU, iGPU, or Hybrid.

The workflow must support serving CPU and GPU models at the same time through
the shared Pumas `/v1` gateway by routing each served model alias to the
selected llama.cpp profile.

## Scope

### In Scope

- Filter the llama.cpp page model library to local models that llama.cpp can
  serve.
- Add a compact per-model llama.cpp profile selector to the llama.cpp page
  library rows.
- Persist selected model-to-profile routes through the existing backend-owned
  runtime route contract.
- Display hardware placement tags for selected and served model rows.
- Show the real served placement from backend `ServedModelStatus` when loaded.
- Replace renderer serving-status polling with backend-pushed serving-status
  updates before building row-level served-state UI.
- Keep existing model modal/row serving actions working.
- Require unique gateway aliases when the same model is served on multiple
  profiles.
- Add tests for compatibility filtering, profile selection, hardware tags,
  route persistence, and serving status rendering.

### Out of Scope

- Automatic memory scheduling or automatic CPU/GPU placement.
- Evicting, moving, or reconfiguring already-loaded models to make new models
  fit.
- Replacing the existing Pumas gateway.
- Changing llama.cpp runtime installation semantics.
- Exposing raw provider endpoints as the primary user workflow.
- Adding new accelerator runtimes beyond the already-modeled CPU/GPU/iGPU/Hybrid
  placement labels.

## Inputs

### Problem

The llama.cpp app page currently embeds the general `ModelManager`, so the
bottom panel can show models that llama.cpp cannot serve. Serving a model also
requires opening a general serve dialog, selecting a profile, and inferring
hardware placement from profile settings. This makes CPU/GPU simultaneous
serving possible but not obvious.

The desired page behavior is model-centered: the llama.cpp page should list only
servable llama.cpp models, let users choose the runtime profile directly from
the model row, and tag loaded models with the actual hardware placement used.

### Constraints

- Backend remains the source of truth for runtime profiles, model routes,
  served-model state, endpoint status, and load errors.
- Frontend may own only row-local drafts and transient in-flight UI state before
  persisting a selected route.
- Pumas must not choose hardware placement automatically beyond safe defaults in
  existing profile creation/editing.
- CPU and GPU simultaneous serving must use separate profiles/processes behind
  the shared Pumas gateway.
- llama.cpp compatibility must be model-general and based on artifacts/metadata,
  not special-cased repository names.
- UI changes must stay consistent with existing app-panel styling and component
  size/complexity standards.
- Serving state must be pushed from the backend-owned source of truth to the
  renderer. Do not add or rely on renderer polling for serving-status updates.

### Assumptions

- Local GGUF models are the first compatibility boundary for llama.cpp serving.
- Existing model metadata fields are sufficient for the first filter:
  `primaryFormat === "gguf"` or equivalent `format === "gguf"`.
- Model category/type can label rows as Chat, Embedding, or Reranker where
  existing metadata provides that signal, but missing type metadata should not
  block GGUF compatibility.
- Existing `ModelRuntimeRoute` can persist one default profile per model.
- Serving the same underlying model on CPU and GPU at the same time requires
  separate aliases and separate `ServedModelStatus` records.
- Frontend served-state display and actions must identify loaded instances by
  `model_id + profile_id + model_alias`, not `model_id` alone.
- iGPU labeling can be represented as a placement label derived from profile
  device settings/runtime capabilities before any deeper hardware telemetry
  exists.

### Dependencies

- `frontend/src/components/app-panels/LlamaCppPanel.tsx`
- `frontend/src/components/ModelManager.tsx`
- `frontend/src/components/LocalModelsList.tsx`
- `frontend/src/components/LocalModelRow.tsx`
- `frontend/src/components/LocalModelInstalledActions.tsx`
- `frontend/src/components/LocalModelRowActions.tsx`
- `frontend/src/components/ModelServeDialog.tsx`
- `frontend/src/components/model-serve/`
- `frontend/src/hooks/useRuntimeProfiles.ts`
- `frontend/src/hooks/useServingStatus.ts`
- `frontend/src/types/api-runtime-profiles.ts`
- `frontend/src/types/api-serving.ts`
- `frontend/src/types/apps.ts`
- `frontend/src/types/api-electron.ts`
- `electron/src/main.ts`
- `electron/src/preload.ts`
- `electron/src/python-bridge.ts`
- `rust/crates/pumas-core/src/runtime_profiles.rs`
- `rust/crates/pumas-core/src/serving/`
- `rust/crates/pumas-rpc/src/handlers/mod.rs`
- `rust/crates/pumas-rpc/src/handlers/runtime_profiles.rs`
- `rust/crates/pumas-rpc/src/handlers/serving.rs`
- `docs/contracts/desktop-rpc-methods.md`
- Standards reviewed:
  - `PLAN-STANDARDS.md`
  - `FRONTEND-STANDARDS.md`
  - `ARCHITECTURE-PATTERNS.md`
  - `TESTING-STANDARDS.md`
  - `DOCUMENTATION-STANDARDS.md`
  - `COMMIT-STANDARDS.md`

### Affected Structured Contracts

- `RuntimeProfilesSnapshot.routes` remains the durable model-to-profile route
  source.
- `ServedModelStatus` is the source of truth for loaded state and actual
  placement display.
- `ModelServingConfig.model_alias` must support unique gateway aliases when the
  same model is served through more than one profile.
- Serving validation and status recording must derive one effective gateway
  alias for every served instance before load. `None` may remain a wire input,
  but `ServedModelStatus` used by the gateway should not leave alias semantics
  ambiguous once duplicate serving is enabled.
- Gateway alias uniqueness is a backend-owned serving contract. The frontend may
  preflight conflicts for usability, but `/v1` routing must not depend on
  frontend-only alias checks.
- `ServingStatusUpdateFeed` must be delivered to the renderer through a pushed
  subscription, matching runtime-profile updates. The interactive UI must not
  use polling or fallback update paths.
- Runtime-profile lifecycle changes that invalidate served models must update
  `ServingStatusSnapshot`; runtime-profile status alone is not enough for
  llama.cpp row state.
- Alias validation must have access to the current served-model snapshot and
  should use explicit error codes for duplicate aliases and ambiguous gateway
  routing.
- Frontend view models may add derived display-only types for compatibility and
  hardware labels; they must not become persisted API contracts unless backend
  support is added.
- If iGPU needs a durable backend distinction, add an explicit typed profile
  capability/placement field instead of encoding it only in display text.

### Affected Persisted Artifacts

- `launcher-data/metadata/runtime-profiles.json` routes may be updated when a
  user selects a profile for a model row.
- Existing runtime-profile config remains compatible.
- No model-library metadata migration is planned for the first slice.
- No serving status durability change is planned; runtime served status remains
  backend-owned as currently implemented.

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| Frontend-only compatibility filtering drifts from backend serve validation. | Medium | Put filter logic in a small pure helper with tests, and treat backend validation as authoritative when serving. |
| Hardware tag overstates what happened if llama.cpp falls back to CPU. | High | Show requested placement before load, but show backend-confirmed `ServedModelStatus` and load errors after serve attempts. Do not mark served on GPU unless the selected profile/load succeeded. |
| Same model served on CPU and GPU overwrites one `servedModelById` entry. | High | Add canonical served-instance identity before row rendering: `model_id + profile_id + model_alias`. |
| Pumas `/v1` gateway routes to the wrong instance when duplicate model ids or aliases exist. | High | Validate alias uniqueness in backend serving validation and make duplicate base-model routing explicit instead of relying on frontend checks. |
| Adding selectors to every row makes the generic model list complex. | Medium | Implement a llama.cpp-specific row/view-model wrapper rather than forcing profile controls into all app panels. |
| New UI duplicates backend-owned route state. | Medium | Row selector drafts are transient; saved selection calls existing route mutation RPC and refreshes from backend snapshot. |
| Existing renderer serving-status hook polls the update endpoint. | High | Add a backend-pushed serving-status update subscription and refactor `useServingStatus` to subscribe instead of polling. |
| Adding another Electron SSE stream copies existing bridge duplication. | Medium | Extract a small named-SSE stream owner/helper before or while adding serving-status push delivery. |
| Runtime profile stop/crash leaves stale served-model rows. | High | Clear or mark served instances when their owning profile is stopped, fails, or becomes unreachable, and publish a serving-status update. |
| iGPU semantics vary by Vulkan/SYCL/OpenCL/runtime asset. | Medium | Treat iGPU as a profile/display classification derived from typed runtime capability or explicit device id; keep validation conservative. |

## Definition of Done

- The llama.cpp page bottom model library shows only llama.cpp-compatible local
  models.
- Each listed model row can select and save a llama.cpp profile route.
- Row serve action uses the selected profile without requiring the user to find
  the profile again.
- Loaded rows show a backend-confirmed placement tag: CPU, GPU, iGPU, or Hybrid.
- CPU and GPU profiles can load different models simultaneously and both appear
  through the shared Pumas `/v1/models` gateway.
- Same underlying model can be served on multiple profiles only with distinct
  aliases.
- Same-model CPU/GPU served instances remain individually visible and unloadable
  from the llama.cpp page.
- Serving-status UI updates are driven by backend-pushed events, not renderer
  polling.
- Existing generic model manager behavior for other app pages is unchanged.
- Tests and release validation pass for changed frontend/backend slices.

## Milestones

### Milestone 1: llama.cpp Compatibility View Model

**Goal:** Isolate llama.cpp page filtering and display derivation from generic
model manager rendering.

**Tasks:**
- [x] Add a pure helper for llama.cpp local-model compatibility.
- [x] Start with GGUF primary-format detection and preserve a clear extension
      point for embedding/reranker/chat labels.
- [x] Add a derived row view model for display labels: model type, selected
      route profile, saved route state, served state, and hardware tag.
- [x] Keep backend validation authoritative; the filter only improves UI focus.

**Verification:**
- Unit tests for compatible/incompatible model groups:
  - GGUF is included.
  - safetensors-only/diffusion/audio/image rows are excluded.
  - empty filtered groups are removed.
- Typecheck for frontend view-model helpers.

**Status:** Completed.

**Progress:**
- 2026-05-09: Added pure frontend llama.cpp compatibility and row view-model
  helpers with provider-filtered served-state grouping and missing-profile
  route handling. Verified with
  `npm run -w frontend test:run -- llamaCppLibraryViewModels.test.ts` and
  `npm run -w frontend check:types`.

### Milestone 2: Push-Based Serving Status Updates

**Goal:** Remove renderer polling from serving state and consume
backend-owned serving-status updates through a pushed subscription.

**Tasks:**
- [x] Add an RPC/SSE serving-status update stream backed by
      `PumasApi::subscribe_serving_status_updates`, following the existing
      runtime-profile update stream shape.
- [x] Extract or introduce a small reusable named-SSE stream helper in the
      Electron bridge before adding another copy of the stream/open/close/error
      handling pattern.
- [x] Add Electron main/preload forwarding for a new
      `onServingStatusUpdate` bridge subscription.
- [x] Add renderer bridge types for `onServingStatusUpdate`.
- [x] Refactor `useServingStatus` to load an initial `get_serving_status`
      snapshot, then refresh only when pushed serving-status updates require it.
- [x] Remove the `setInterval`/polling path from `useServingStatus`.
- [x] Remove interactive renderer use of cursor-based
      `list_serving_status_updates_since`; pushed subscription delivery is the
      only accepted UI update path.
- [x] Ensure subscription cleanup happens on unmount and Electron unsubscribe.
- [x] Treat subscription setup or delivery failure as a surfaced design/runtime
      error, not as a reason to start polling.

**Verification:**
- Frontend hook tests prove no interval is registered for serving status.
- Frontend hook tests prove pushed `snapshot_required` or event notifications
  refresh the backend snapshot.
- Frontend hook tests prove subscription setup failure surfaces an error and
  does not create a polling timer.
- Electron/preload tests cover subscribe/unsubscribe bridge behavior.
- Rust/RPC tests cover serving-status SSE event delivery and initial
  snapshot-required behavior for new subscribers.

**Status:** Completed.

**Progress:**
- 2026-05-09: Added `/events/serving-status-updates` in `pumas-rpc`, backed by
  `PumasApi::subscribe_serving_status_updates`, with initial
  `snapshot_required` delivery. Verified with
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml test_serving_status_update_event_stream_emits_initial_snapshot_required`.
- 2026-05-09: Extracted the Electron named-SSE stream owner, migrated existing
  backend update streams to it, and added serving-status subscribe/unsubscribe
  forwarding through main/preload with renderer types. Verified with
  `npm run -w electron validate`, `npm run -w electron test`, and
  `npm run -w frontend check:types`.

- 2026-05-09: Replaced `useServingStatus` polling with the pushed
  `onServingStatusUpdate` subscription. Added renderer-visible subscription and
  stream error delivery for serving status, with early subscriptions queued
  until the backend bridge is running. Verified with
  `npm run -w frontend test:run -- useServingStatus.test.ts`,
  `npm run -w electron test`, and `npm run -w frontend check:types`.

**Discovered issues:**
- None open for this milestone.

### Milestone 3: Served-Instance Identity And Gateway Alias Safety

**Goal:** Make frontend display/actions and backend gateway routing safe for
simultaneous serving of the same underlying model on different llama.cpp
profiles.

**Tasks:**
- [x] Add a small frontend helper for served-instance identity using
      `model_id + profile_id + model_alias`.
- [x] Derive served-state maps once per llama.cpp panel render path:
      `servedStatusesByModelId`, selected-profile served status, and
      profile/alias keyed status.
- [ ] Stop using `model_id` alone for llama.cpp row loaded-state display,
      selected-profile status, or unload targeting.
- [ ] Update `useModelServingActions` or add a llama.cpp-specific action wrapper
      so serve/unserve can target an optional `profile_id` and `model_alias`.
- [x] Add backend serving validation that rejects ambiguous duplicate aliases
      before recording a loaded model or exposing it through the Pumas `/v1`
      gateway.
- [x] Add a backend helper that resolves the effective gateway alias from
      explicit `model_alias`, provider defaults, or model identity before both
      validation and `ServedModelStatus` recording.
- [x] Extend serving validation context with the current served-model snapshot
      so alias checks are backend-authoritative.
- [x] Add explicit model-serve error codes for duplicate alias and ambiguous
      served-model routing instead of overloading `invalid_request` or
      `unknown`.
- [x] Decide and document gateway behavior when a user requests the base
      `model_id` while multiple served instances exist. Prefer a clear
      non-critical/HTTP error unless one instance is unambiguous.

**Verification:**
- Frontend unit tests for served-instance keys and per-model status grouping.
- Frontend test that same `model_id` with CPU and GPU profiles renders two
  distinct loaded states or a clear multi-instance summary.
- Frontend test that unload sends `model_id`, `profile_id`, and `model_alias`
  for the intended served instance.
- Rust tests that duplicate aliases are rejected and distinct aliases are listed
  separately by `/v1/models`.
- Rust test that ambiguous `/v1` requests do not silently route to the first
  matching served instance.

**Status:** In progress.

**Progress:**
- 2026-05-09: Added backend-owned effective alias normalization for serve and
  validation requests, explicit duplicate-alias and ambiguous-routing error
  codes, served-snapshot validation context, canonical alias uniqueness checks,
  and deterministic `/v1` gateway conflict behavior for ambiguous base
  `model_id` requests. Verified with `cargo fmt --manifest-path
  rust/crates/pumas-core/Cargo.toml`, `cargo fmt --manifest-path
  rust/crates/pumas-rpc/Cargo.toml`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml validation_`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml openai_lookup`, and
  `npm run -w frontend check:types`.

### Milestone 4: llama.cpp-Specific Library Panel

**Goal:** Replace the generic bottom library on the llama.cpp page with a
focused compatible-model panel while preserving the existing `ModelManager`
public usage elsewhere.

**Tasks:**
- [x] Add a `LlamaCppModelLibrarySection` or equivalent component under the
      app-panel section area.
- [x] Feed it the existing model groups, runtime profile snapshot, route
      snapshot, and serving snapshot.
- [x] Render only compatible models using existing row visual patterns without
      importing the generic model manager's remote-search/download state
      machine.
- [x] Do not add llama.cpp-specific profile controls to `ModelManager`,
      `LocalModelsList`, `LocalModelRow`, or `LocalModelInstalledActions` except
      through generic extension points that do not change other app behavior.
- [x] Avoid putting cards inside cards; keep the panel as the existing app-page
      lower workspace.
- [ ] Keep search/filter behavior if practical by reusing existing pure
      filtering helpers, but do not expose remote-download mode in this page
      panel unless it remains coherent after compatibility filtering.

**Verification:**
- Component tests prove incompatible local models are not rendered on the
  llama.cpp page panel.
- Regression test that non-llama app pages still render the generic
  `ModelManager` path.
- Accessibility tests use named buttons/selectors, not generic role counts.

**Status:** Completed.

**Progress:**
- 2026-05-09: Added a display-only llama.cpp model library section and swapped
  the llama.cpp page away from the generic `ModelManager` lower panel. The new
  panel uses the llama.cpp compatibility view model and excludes incompatible
  local models without touching generic `ModelManager` behavior. Verified with
  `npm run -w frontend test:run -- LlamaCppModelLibrarySection.test.tsx` and
  `npm run -w frontend check:types`.

**Discovered issues:**
- The first panel slice intentionally passes empty runtime profiles and routes;
  Milestone 5 must wire backend-confirmed route/profile snapshots before route
  labels can be truthful.
- Search/filter behavior is not yet present in the llama.cpp-specific panel.

### Milestone 5: Row Profile Selection And Route Persistence

**Goal:** Let users choose the llama.cpp profile used for a model directly from
the row.

**Tasks:**
- [x] Add a compact profile selector listing only llama.cpp profiles.
- [x] Label profile options with name plus placement: `Emily GPU`, `Emily CPU`,
      `Emily iGPU`, `Emily Hybrid`.
- [x] On selection, persist `ModelRuntimeRoute` through existing backend RPC, or
      use an explicit save affordance if immediate persistence creates stale
      response or accidental-write issues during implementation.
- [ ] Extract or reuse a focused route save/clear helper so row selection and
      `ModelRuntimeRouteEditor` do not duplicate backend mutation logic.
- [x] Refresh route state from the backend snapshot/update feed after save.
- [x] Surface route-save errors inline without mutating served state.
- [x] Disable or explain selector state when no llama.cpp profiles exist.

**Verification:**
- Frontend tests for selector options filtered to llama.cpp profiles.
- Frontend test that selecting a profile calls route mutation RPC with the
  model id and selected profile id.
- Frontend test that failed route save leaves the previous confirmed selection
  visible.
- Rust route mutation tests if backend route behavior needs contract changes.

**Status:** In progress.

**Progress:**
- 2026-05-09: Wired the llama.cpp library rows to backend runtime profile
  snapshots and route snapshots, added a row-level llama.cpp-only profile
  selector with explicit save/clear behavior, and refreshed backend-confirmed
  route state after successful mutation. Verified with
  `npm run -w frontend test:run -- LlamaCppModelLibrarySection.test.tsx` and
  `npm run -w frontend check:types`.
- 2026-05-09: Disabled the row selector when no llama.cpp profiles exist and
  kept the empty selector label explicit. Verified with
  `npm run -w frontend test:run -- LlamaCppModelLibrarySection.test.tsx` and
  `npm run -w frontend check:types`.

**Discovered issues:**
- Route mutation logic now exists in both `ModelRuntimeRouteEditor` and
  `LlamaCppModelLibrarySection`; extract a shared route mutation helper before
  marking the duplication task complete.
- Empty-profile selector UX is covered; no open issue.

### Milestone 6: Hardware Placement Tags

**Goal:** Make selected and served hardware placement obvious and truthful.

**Tasks:**
- [ ] Add a display helper that maps profile/device settings to placement tags:
      CPU, GPU, iGPU, Hybrid, or Auto where unavoidable before save.
- [ ] Show muted selected-profile placement when the model is not loaded.
- [ ] Show stronger backend-confirmed placement when `ServedModelStatus` is
      loaded.
- [ ] Show failed load state and last error instead of a hardware tag if the
      backend reports a load failure.
- [ ] Ensure tags fit in compact rows on small widths.

**Verification:**
- Unit tests for device-to-label mapping.
- Component tests for loaded CPU/GPU/iGPU/Hybrid tags.
- Component tests for failed state taking precedence over placement display.
- Visual/manual check of row layout at desktop and narrow widths.

**Status:** Not started.

### Milestone 7: Serve From Selected Profile

**Goal:** Make the row serve action use the row-selected llama.cpp profile and
support simultaneous CPU/GPU serving behind the Pumas gateway.

**Tasks:**
- [x] When a model row has a selected profile, prefill `ModelServeDialog` with
      that profile and matching device defaults.
- [x] Add provider/profile filtering or locking to `ModelServeDialog` so
      llama.cpp-specific callers cannot drift into Ollama or unrelated runtime
      profiles.
- [ ] Allow a quick serve action where the selected route/profile is already
      valid and no alias conflict exists.
- [ ] Require or prompt for a unique alias when the same model is already served
      through another profile, and pass that alias through
      `ModelServingConfig.model_alias`.
- [ ] Preserve the existing dialog for advanced context/gpu-layer/tensor-split
      overrides.
- [ ] Ensure successful serve refreshes backend serving status and row display.
- [ ] Ensure row unload targets the selected served instance instead of the first
      status with the same `model_id`.
- [ ] Ensure runtime-profile stop/failure paths remove or mark affected
      `ServedModelStatus` entries and publish serving-status updates.

**Verification:**
- Frontend flow test: choose GPU profile, click serve, RPC request contains the
  selected profile id.
- Frontend flow test: model already served on CPU requires a distinct alias to
  serve on GPU.
- Backend/RPC tests if alias uniqueness checks move into serving validation.
- Manual release-app acceptance: serve one model on CPU, another on GPU, confirm
  `/v1/models` lists both through the same Pumas gateway.

**Status:** In progress.

**Progress:**
- 2026-05-09: Added a row serve affordance that opens the serving page with the
  selected llama.cpp route profile prefilled, and added an optional
  `ModelServeDialog` provider filter so llama.cpp row serving cannot drift into
  Ollama profiles. Verified with
  `npm run -w frontend test:run -- LlamaCppModelLibrarySection.test.tsx` and
  `npm run -w frontend check:types`.

### Milestone 8: Documentation And Release Validation

**Goal:** Close the feature with updated docs and full-path validation.

**Tasks:**
- [ ] Update relevant frontend component READMEs for the llama.cpp-specific
      library panel and placement-tag behavior.
- [ ] Update `docs/contracts/desktop-rpc-methods.md` only if RPC contracts
      change.
- [ ] Record manual acceptance results in this plan.
- [ ] Build release binaries and frontend.

**Verification:**
- `npm run -w frontend test:run -- <targeted llama.cpp panel tests>`
- `npm run -w frontend test:run -- <targeted serving-status hook tests>`
- `npm run -w frontend check:types`
- `npm run -w frontend build`
- Rust targeted tests for any backend contract changes.
- `bash launcher.sh --build-release`
- `bash launcher.sh --release-smoke`
- Manual CPU+GPU simultaneous serving acceptance path.

**Status:** Not started.

## Lifecycle And Runtime Ownership Notes

- Backend owns runtime profiles, model routes, served-model snapshots, endpoint
  status, and load errors.
- The llama.cpp library panel should subscribe through runtime-profile and
  serving-status hooks whose updates are pushed from the backend. It must not
  start timers or poll for row status.
- Row profile selection is a frontend draft until persisted; confirmed state
  comes from backend route snapshots.
- Serve/unserve actions must call backend serving RPCs and wait for confirmed
  responses or backend snapshot updates before changing loaded-state display.
- Managed runtime process ownership remains in backend runtime-profile lifecycle
  helpers.

## Edge-Case Contract Rules

- **Serving-status push subscription unavailable:** surface a blocking UI/runtime
  error for serving state. Do not start polling. Fix the push path before
  enabling row serving UI.
- **Duplicate alias requested:** backend validation rejects the serve request
  before loading or recording state. The existing served models remain
  unchanged.
- **Same model already served on another profile without a new alias:** frontend
  prompts for a distinct alias, and backend validation still enforces uniqueness.
- **Gateway request by base `model_id` when multiple instances exist:** return a
  deterministic ambiguous-model error. Do not route to the first matching
  status.
- **Gateway request by unique alias:** route to the served instance with that
  alias and rewrite provider request model to the provider-facing alias.
- **Profile deleted while a route points to it:** row shows missing profile,
  disables serve for that route, and offers reselect/clear. It must not silently
  choose another profile.
- **Profile device settings change while a model is loaded:** loaded placement
  remains the backend-confirmed `ServedModelStatus`; new profile settings apply
  only after unload/reload.
- **Runtime process stops or crashes externally:** backend publishes a serving
  update that clears or marks affected served state. UI waits for that pushed
  update and shows the backend error state.
- **User stops a runtime profile while models are loaded:** backend removes or
  marks all served instances owned by that profile and publishes a serving
  update. The UI must not infer unload from runtime-profile status alone.
- **CPU-only runtime selected for GPU/Hybrid profile:** backend returns a
  non-critical load/validation error and preserves existing served state.
- **Unload with multiple same-model instances:** unload request must include
  `model_id`, `profile_id`, and `model_alias`; ambiguous unload requests are
  rejected rather than guessed.
- **iGPU placement label:** show iGPU only when profile/runtime data can
  distinguish it. Otherwise show GPU/Hybrid/Auto without implying telemetry.
- **Alias input sanitation:** reject empty, whitespace-only, duplicate, and
  provider-invalid aliases before attempting load; backend remains
  authoritative.

## Alias Normalization Contract

Gateway aliases are part of the backend serving contract because `/v1` request
routing depends on them. Frontend checks may improve prompts, but backend
validation is authoritative.

- Treat aliases as case-insensitive for uniqueness checks by comparing a
  canonical alias key.
- Canonical alias key:
  - trim leading/trailing whitespace
  - lowercase ASCII characters
  - collapse repeated internal whitespace or punctuation runs that would be
    normalized by provider-facing name helpers
  - reject if the result is empty
- Accepted explicit alias characters for the first llama.cpp slice:
  lowercase ASCII letters, digits, `.`, `_`, `-`, and `/`.
- Reject aliases that:
  - begin or end with `/`
  - contain `//`
  - contain path traversal segments such as `.` or `..`
  - contain whitespace, control characters, backslashes, shell metacharacters,
    or URL query/fragment separators
  - exceed the chosen backend maximum length
- Autogenerated aliases should use the existing backend naming helper path, then
  pass through the same canonical validation.
- Store and expose the accepted alias consistently. Do not silently rewrite an
  explicit user alias into a different displayed alias after validation.
- Alias uniqueness is global across loaded Pumas gateway models, not scoped only
  to a profile, because `/v1` routes by model/alias.

## Served-Instance View Model Sketch

The llama.cpp page should derive a small view model from backend snapshots once
per render path and pass rows display-ready data. Rows should not perform their
own snapshot scans.

```ts
type ServedInstanceKey = string;

interface LlamaCppServedInstanceView {
  key: ServedInstanceKey;
  modelId: string;
  alias: string | null;
  profileId: string;
  profileName: string;
  provider: 'llama_cpp';
  loadState: ServedModelLoadState;
  placement: 'cpu' | 'gpu' | 'igpu' | 'hybrid' | 'auto';
  endpointUrl: string | null;
  lastError: ModelServeError | null;
}

interface LlamaCppModelRowView {
  model: ModelInfo;
  isCompatible: boolean;
  routeProfileId: string | null;
  routeProfileName: string | null;
  routeProfileMissing: boolean;
  selectedPlacement: LlamaCppServedInstanceView['placement'];
  servedInstances: LlamaCppServedInstanceView[];
  selectedProfileServedInstance: LlamaCppServedInstanceView | null;
  requiresAliasForAdditionalServe: boolean;
}
```

Implementation notes:

- `ServedInstanceKey` must be derived from `model_id + profile_id + model_alias`.
- View-model builders should use the backend-confirmed effective alias from
  `ServedModelStatus`. They should not invent a frontend fallback alias.
- `servedInstances` may contain multiple entries for one `model.id`.
- `selectedProfileServedInstance` is the entry targeted by the row's primary
  unload/serve state.
- Generic `ModelManager` rows may keep a summary badge, but llama.cpp rows must
  keep per-instance state.

## Push Stream Helper Sketch

Adding serving-status push delivery should not copy the existing Electron SSE
stream boilerplate again. Introduce a small named-stream owner/helper in
`electron/src/python-bridge.ts` or a sibling module.

Suggested shape:

```ts
interface NamedSseStreamSpec {
  label: string;
  path: string;
  expectedEventName: string;
  supportsCursor: boolean;
}

interface NamedSseStreamOwner {
  start(listener: (payload: unknown) => void): void;
  stop(): void;
}
```

The helper should own:

- request open/close
- buffer handling with `parseNamedSseChunk`
- optional cursor persistence for streams whose payloads carry cursors
- stream error notification/logging
- reconnect on the same pushed stream path only
- unsubscribe cleanup

It must not:

- call polling RPCs
- synthesize state locally
- hide stream setup failure from the renderer

Serving status, runtime profile updates, model download updates, and status
telemetry can then be either migrated incrementally or the new serving stream can
be the first user of the helper with a follow-up to collapse older duplication.

## Edge-Case Test Matrix

| Case | Backend/RPC Coverage | Electron/Hook Coverage | UI Coverage |
| ---- | -------------------- | ---------------------- | ----------- |
| Serving-status stream opens | SSE emits initial snapshot-required or current feed | `onServingStatusUpdate` forwards payload | Hook refreshes snapshot from pushed event |
| Serving-status stream setup fails | SSE error event or bridge startup error is exposed | Hook receives subscription error path | Serving UI shows blocking state and no polling timer exists |
| Duplicate alias | Validation rejects before provider load | N/A | Alias prompt shows backend error and loaded rows are unchanged |
| Same model CPU + GPU with distinct aliases | `/v1/models` lists both aliases; gateway routes each alias | Pushed updates arrive for both loads | Row shows two served instances with CPU/GPU tags |
| Base `model_id` request with multiple instances | Gateway returns ambiguous-model error | N/A | UI documentation/tooltips avoid recommending base id for duplicates |
| Unload one duplicate instance | `unserve_model` requires model/profile/alias and removes only that instance | Pushed unload update arrives | Only targeted tag disappears |
| Profile deleted while routed | Route/profile snapshot marks missing profile | Runtime-profile push updates row | Selector shows missing route and disables serve |
| Profile stopped with loaded models | Serving snapshot clears or marks all profile-owned instances | Serving push arrives after stop | Loaded tags clear without local inference |
| CPU-only runtime with GPU profile | Non-critical validation/load error; no served-state mutation | Serving error push arrives if recorded | Row shows error, not loaded GPU tag |
| iGPU cannot be distinguished | Backend/profile data lacks iGPU signal | N/A | UI shows GPU/Hybrid/Auto, not iGPU |

## Commit Cadence Notes

- Commit after each verified logical slice.
- Keep compatibility-filter/view-model changes separate from serving-action
  behavior if they can be reviewed independently.
- Keep backend contract changes in the same commit as their tests and frontend
  type updates.
- Follow `COMMIT-STANDARDS.md` conventional format.

## Optional Subagent Assignment

Use subagents only during implementation, not plan creation.

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| Frontend worker | llama.cpp compatibility, served-instance view model, row selector, tags | Component/helper patch plus tests in assigned frontend files | After Milestone 3 or 5 tests pass |
| Backend/Electron worker | Serving-status push bridge, gateway alias validation, and ambiguous routing behavior | Rust/Electron contract patch plus focused tests | Before Milestone 7 integration |

Shared files such as `api-serving.ts`, `api-runtime-profiles.ts`, and
`docs/contracts/desktop-rpc-methods.md` must have a single owner per wave.

## Re-Plan Triggers

- Backend cannot distinguish simultaneous loads of the same model by profile or
  alias without changing `ServedModelStatus` identity semantics.
- Backend-pushed serving-status updates cannot be delivered through the current
  RPC/Electron bridge without a larger transport refactor.
- Existing model metadata cannot reliably identify GGUF compatibility.
- iGPU cannot be represented truthfully from profile/runtime data.
- The generic `ModelManager` cannot support this without standards-violating
  complexity, requiring a dedicated llama.cpp list component.
- Serving alias uniqueness requires a backend contract change larger than the
  current serving facade.
- Manual CPU+GPU simultaneous serving fails because of provider/router
  constraints not represented in current profile validation.

## Recommendations

- Prefer a llama.cpp-specific panel/view model over adding llama.cpp-only
  controls to the generic `ModelManager`. This keeps the blast radius narrow and
  avoids making all app pages carry provider-specific serving concerns.
- Treat served-state identity as a small shared helper, not row-local logic.
  Every llama.cpp row, serve action, and unload action should agree on the same
  `model_id + profile_id + model_alias` key shape.
- Treat `useServingStatus` as a subscription hook, not a polling hook. It may
  call `get_serving_status` for initial load and pushed snapshot refreshes, but
  it should not own a timer.
- Keep alias uniqueness enforcement in backend serving validation. Frontend
  preflight checks are useful for prompts, but the gateway is the contract that
  must reject ambiguous routing.
- Treat hardware tags as profile/request placement labels backed by successful
  backend load state, not hardware telemetry. Telemetry can be added later, but
  the UI should first make user-selected placement and backend-confirmed loaded
  state clear.

## Completion Summary

### Completed

- Plan created and checked against local Coding Standards.
- Blast-radius review incorporated: served-instance identity, gateway alias
  validation, provider-filtered serve dialog behavior, backend-pushed serving
  status, and generic `ModelManager` containment.

### Deviations

- None.

### Follow-Ups

- Implement in validated thin vertical slices.
- Update this plan after each milestone with discovered issues and validation
  results.

### Verification Summary

- Standards reviewed from `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/`.
- Existing llama.cpp panel, model manager, local row actions, runtime-profile
  types, and serving-status hooks inspected for blast-radius planning.
- Existing serving backend and Pumas `/v1` gateway inspected for same-model
  multi-profile identity and alias-routing effects.
- Existing serving-status flow inspected: backend has an in-memory broadcast
  subscription, while the current renderer hook polls the update RPC. The plan
  now requires replacing that renderer polling with a pushed bridge.

### Traceability Links

- Module README updated: Pending implementation.
- ADR added/updated: N/A.
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: Pending PR.
