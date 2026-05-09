# Plan: Local Runtime Profiles, User-Directed Model Serving, and Ollama Version Manager Stability

## Objective

Fix the Ollama page crash triggered by the version-manager globe button and add a backend-owned local runtime profile architecture for model-serving providers. Ollama and llama.cpp should share the same runtime-profile, model-routing, status-event, and frontend settings architecture while keeping provider-specific process and API behavior behind adapters.

The next active phase extends the completed runtime-profile foundation into user-directed model serving. Users start from a model row or model modal, choose the serving profile and explicit device placement for that model, and ask Pumas to load it into a shared serving endpoint. Pumas validates obvious configuration errors, attempts the requested load, reports non-critical load failures when the selected configuration does not fit or cannot be served, and preserves already-loaded models unless the user explicitly unloads them.

## Scope

### In Scope

- Diagnose and fix the React crash that turns the Ollama page magenta when opening installable Ollama versions.
- Preserve the shared app version-management facade used by ComfyUI, Ollama, and Torch.
- Add a backend-owned local runtime profile model for managed or external model-serving endpoints/processes.
- Add provider adapters for Ollama and llama.cpp behind the same runtime profile contract.
- Add per-model routing from Pumas library models to local runtime profiles.
- Support CPU, GPU, auto, external endpoint, and future specific-device profile modes without hard-coding behavior for a specific model.
- Add a user-directed model serving workflow from model rows and the model modal.
- Add backend-owned serving status for models loaded into the shared endpoint.
- Add explicit per-load serving configuration for provider/profile, device mode, device id, GPU layers, tensor split, context size, and keep-loaded behavior where supported.
- Return recoverable, non-critical load errors for configurations that fail validation or fail to load because of memory/device/runtime constraints.
- Add frontend controls that edit backend-owned runtime/profile state through RPC.
- Extend the existing backend-to-frontend event pattern so local runtime status updates are pushed to the frontend instead of polled per profile.
- Add tests for UI crash prevention, API contracts, persisted config, process lifecycle, and routing behavior.

### Out of Scope

- Replacing existing Ollama support with another runtime.
- Automatically deciding which models to evict, downgrade, or move between devices to make a new model fit.
- Automatically selecting CPU/GPU/hybrid placement for the user beyond safe defaults shown in the form.
- Silently unloading, evicting, or reconfiguring existing served models when a new user-selected configuration fails.
- Claiming that a single Ollama daemon can enforce native per-model CPU/GPU placement unless upstream Ollama exposes a stable, documented setting for that behavior.
- Claiming llama.cpp router mode supports every feature or hardware-isolation pattern that dedicated `llama-server` processes support.
- Removing existing singleton Ollama commands in the first pass.
- Changing model-library artifact identity or Hugging Face download behavior.

## Inputs

### Problem

The Ollama page crashes when the globe/version-manager button opens installable Ollama versions. The same area also lacks sufficient settings for managing multiple local runtime models and selecting CPU/GPU behavior per model. The current implementation treats Ollama process management as singleton-oriented while model operations accept an optional `connectionUrl`. llama.cpp-capable GGUF models already exist in the library metadata surface, but Pumas does not yet expose llama.cpp runtime profiles or provider-neutral model routing.

The runtime-profile foundation now exists, but the user workflow is still incomplete. Users can configure profiles and routes, but they cannot start from a model row or modal, choose explicit model placement, and load that model into a common serving endpoint with clear success/failure status. The next feature slice must turn profiles and routes into a backend-owned serving workflow without making Pumas an automatic memory scheduler.

### Constraints

- User requested planning only before implementation.
- Backend owns persistent data and behavior-changing configuration.
- Frontend may own only transient UI state such as panel open/closed state and form edits before submit.
- Existing RPC methods and frontend bridge calls must remain compatible unless an explicit breaking change is approved.
- The feature must be model-general and cannot special-case individual model names, repositories, or quant formats.
- User placement decisions are authoritative. Pumas validates and attempts the requested configuration, but it must not override the selected CPU/GPU/hybrid settings to make a model fit.
- Load failures caused by memory, unsupported device placement, missing binaries, stopped profiles, or invalid runtime settings are normal domain outcomes and must not crash the app or corrupt existing served-model state.
- The frontend may draft serving settings locally, but loaded/served model state, last load errors, endpoint status, and active model aliases are backend-owned.
- Ollama capability labels must be truthful: Pumas can route a model to a CPU/GPU-profiled runtime, but should not present unsupported upstream Ollama behavior as a native per-model guarantee.
- llama.cpp capability labels must distinguish router profiles from dedicated process profiles because their lifecycle, isolation, and model-loading behavior differ.

### Assumptions

- The magenta page indicates an uncaught React render/runtime error in the version manager path.
- The first acceptable fix is a small vertical slice: reproduce the crash, stabilize the version-manager rendering path, and add a regression test.
- Per-model CPU/GPU selection should be implemented as explicit user-selected serving config attached to a model/profile operation, with saved route defaults used only to prefill the form.
- Managed runtime profiles may need separate Ollama processes on separate ports because process environment variables are process-wide.
- Managed llama.cpp profiles may use either router mode or dedicated `llama-server -m <model>` processes depending on the isolation and scheduling needed.
- A Pumas-owned serving facade may be required to present one stable endpoint even when provider-specific workers differ. If the first implementation keeps provider endpoints exposed, the plan must explicitly mark the unified endpoint as incomplete rather than claiming the same endpoint behavior.
- `profile_id` is the canonical internal address for local runtime operations.
- Existing optional `connectionUrl` arguments in Ollama model RPC methods remain a legacy boundary compatibility path, but internal routing resolves through validated runtime profiles.

### Dependencies

- `frontend/src/components/app-panels/OllamaPanel.tsx`
- `frontend/src/components/LocalModelRowActions.tsx`
- `frontend/src/components/LocalModelInstalledActions.tsx`
- `frontend/src/components/ModelRuntimeRouteEditor.tsx`
- `frontend/src/components/ModelMetadataModalContent.tsx`
- `frontend/src/components/ModelMetadataModalTabs.tsx`
- `frontend/src/components/app-panels/sections/OllamaModelSection.tsx`
- `frontend/src/components/app-panels/VersionManagementPanel.tsx`
- `frontend/src/components/InstallDialog.tsx`
- `frontend/src/components/InstallDialogContent.tsx`
- `frontend/src/components/VersionListItem.tsx`
- `frontend/src/hooks/useVersions.ts`
- `frontend/src/hooks/useVersionFetching.ts`
- `frontend/src/types/versions.ts`
- `frontend/src/types/api-processes.ts`
- `electron/src/preload.ts`
- `electron/src/rpc-method-registry.ts`
- `rust/crates/pumas-rpc/src/handlers/process.rs`
- `rust/crates/pumas-rpc/src/handlers/ollama.rs`
- `rust/crates/pumas-rpc/src/handlers/runtime_profiles.rs`
- New or expanded `rust/crates/pumas-rpc/src/handlers/serving.rs`
- `rust/crates/pumas-rpc/src/handlers/mod.rs`
- `rust/crates/pumas-rpc/src/wrapper.rs`
- `rust/crates/pumas-app-manager/src/ollama_client.rs`
- `rust/crates/pumas-core/src/process/launcher.rs`
- `rust/crates/pumas-core/src/process/manager.rs`
- `rust/crates/pumas-core/src/api/process.rs`
- `rust/crates/pumas-core/src/api/state_process.rs`
- `rust/crates/pumas-core/src/api/state_runtime.rs`
- New or expanded `rust/crates/pumas-core/src/api/serving.rs`
- New or expanded `rust/crates/pumas-core/src/serving/`
- `rust/crates/pumas-core/src/models/`
- `rust/crates/pumas-core/src/conversion/llama_cpp.rs`
- `rust/crates/pumas-uniffi/src/bindings/` if serving APIs become supported native binding surface.
- `rust/crates/pumas-rustler/src/lib.rs` if serving APIs become supported BEAM/NIF surface.
- `docs/architecture/MODEL_LIBRARY_ARCHITECTURE.md`
- `docs/contracts/desktop-rpc-methods.md`
- Coding standards under `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/`

### Affected Structured Contracts

- Electron bridge API types in `frontend/src/types/api-processes.ts` and related aggregate exports.
- Preload bridge methods in `electron/src/preload.ts`.
- RPC method registry and request-schema validation in `electron/src/rpc-method-registry.ts`.
- Rust RPC dispatch methods in `rust/crates/pumas-rpc/src/handlers/mod.rs`.
- Provider-specific runtime RPC response shapes, including existing Ollama responses and new llama.cpp runtime responses.
- New model-serving request/response DTOs:
  - `ModelServingConfig`
  - `ServeModelRequest`
  - `ServedModelStatus`
  - `ServingEndpointStatus`
  - `ModelServeValidationResponse`
  - `ModelServeError`
  - `ModelServeErrorSeverity`
  - explicit provider, device, placement, and error-code enums.
- New RPC/bridge methods for serving status, validation, load, and unload operations.
- Process status response shape if profile-level status is exposed in global status.
- Backend event stream payloads for runtime/profile status notifications.
- Backend event stream payloads for served-model status and non-critical load-result notifications.
- Electron bridge subscription APIs for runtime/profile events.
- Persisted runtime profile and model route config schema.

### Affected Persisted Artifacts

- New backend-owned local runtime profile config.
- New backend-owned model-to-runtime-profile route config.
- Optional backend-owned model-serving defaults, if users can save a preferred device placement per model. Defaults must be append-only and must not rewrite existing route semantics.
- Runtime serving status cache or journal if status is made durable across backend restarts. If status is in-memory only, this must be documented explicitly and rebuilt from provider inventories on startup.
- Potential profile-specific PID files and log files.
- Potential profile-specific endpoint/port allocation metadata.
- Potential llama.cpp generated model catalog or preset files for managed router profiles.
- Existing version installation metadata remains in place and should not be migrated for this feature.

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| The crash is caused by malformed version payloads, not only a frontend null check. | High | Reproduce first, inspect thrown error, add version payload regression coverage before changing UI behavior. |
| A single Ollama daemon cannot enforce per-model CPU/GPU placement. | High | Design around runtime profiles and endpoint routing instead of unsupported per-model daemon flags. |
| Multiple managed Ollama processes can collide on ports, model storage, or GPU memory. | High | Backend owns profile lifecycle, validated ports, process env, PID files, health checks, and status reporting. |
| Existing singleton `launch_ollama`, `stop_ollama`, and `is_ollama_running` callers break. | High | Preserve these commands and add append-only profile-aware commands. |
| Frontend creates stale state with optimistic profile edits. | Medium | Submit edits through backend RPC and refresh/push confirmed backend state before rendering changes as saved. |
| Polling grows as profiles multiply. | Medium | Extend the existing backend event pattern and expose one runtime status snapshot/notification path instead of frontend per-profile polling. |
| CPU/GPU labels overpromise behavior on platforms with different accelerators. | Medium | Use typed device modes and user-facing descriptions based on profile capability and backend validation. |
| `profile_id` and `connection_url` become parallel long-term routing APIs. | High | Make `profile_id` the only canonical internal route key; keep `connection_url` as adapter-only legacy input converted to a profile boundary object. |
| Runtime profiles inflate the singleton process manager with conditional branches. | High | Add a provider-neutral runtime profile service that owns profile policy and uses low-level process launch helpers instead of expanding singleton provider methods directly. |
| Ollama and llama.cpp duplicate the same architecture in separate feature silos. | High | Introduce a provider-neutral runtime profile service with provider adapters for Ollama and llama.cpp. |
| llama.cpp router mode and dedicated process mode are conflated. | Medium | Model them as separate profile modes under the llama.cpp provider with distinct lifecycle and capability flags. |
| Generated llama.cpp model catalogs or presets drift from the Pumas library. | Medium | Make catalog/preset generation backend-owned, deterministic, and evented when model routes or library artifacts change. |
| Pumas accidentally becomes an automatic memory scheduler despite the product requirement for user-selected placement. | High | Keep serve requests user-authored, treat fit checks as validation/advisory responses, and never evict or move existing models unless requested by a user command. |
| A failed load disrupts existing served models. | High | Define `loaded_models_unchanged` in the response contract, make provider adapters preserve existing model state on failure where upstream allows it, and surface any unknown provider-side side effects explicitly. |
| Frontend duplicates serving state in row/modals. | Medium | Store only form drafts locally; subscribe to backend serving snapshots/events for loaded state and last errors. |
| Shared endpoint behavior is claimed before the implementation actually provides it. | High | Add a serving endpoint status contract with an explicit mode: `pumas_gateway`, `provider_endpoint`, or `not_configured`; UI copy must reflect the current mode. |
| New serving APIs are exposed through UniFFI/Rustler without binding tests. | Medium | Keep binding exposure out of scope until there is a supported consumer, or add native and host-language binding coverage in the same slice. |

## Codebase Impact and Blast Radius

### Immediate Impact

- `OllamaPanel.tsx` already opens `VersionManagementPanel` through `showVersionManager`; the crash fix should remain inside the version-manager path unless the root cause is bad data from the version hook.
- `InstallDialog.tsx` and `InstallDialogContent.tsx` are shared by ComfyUI, Ollama, and Torch version management, so any hardening must be app-neutral and tested against shared behavior.
- `VersionListItem.tsx` currently renders a settings `IconButton` without an action. This is adjacent to the crash surface and should be inspected during implementation, but it should not be assumed to be the root cause without a failing test.
- `useVersions.ts` and `useVersionFetching.ts` own version loading state. If Ollama release data has a shape mismatch, the adapter/mapping fix belongs here or at the backend boundary, not in Ollama-specific JSX.

### Backend Runtime Blast Radius

- `ProcessManager` currently has singleton Ollama launch/stop/status behavior and scans `ollama-versions` for `ollama.pid`. Runtime profiles require a dedicated `RuntimeProfileService` or equivalent owner that uses low-level process helpers while keeping profile policy out of singleton process-manager methods.
- `BinaryLaunchConfig::ollama` currently uses the default Ollama base URL and a fixed `ollama.pid`. Runtime profiles require configurable env vars, health URLs, PID paths, and ports.
- `state_runtime.rs` aggregates Ollama resources as one app-level bucket. Profile-level resource reporting can be added later while preserving existing aggregate status.
- `pumas-rpc` has separate process and Ollama model handlers. Profile-aware lifecycle commands belong with process/runtime handling, while model list/load/create/delete commands should route through profile resolution before creating an `OllamaClient`.
- Existing broad process-pattern cleanup such as stopping every `ollama serve` process is unsafe for managed multi-profile operation and must remain a legacy singleton behavior or become profile-scoped.
- llama.cpp support should not be implemented as a second independent copy of the Ollama runtime service. Provider-specific launch flags, router APIs, and model catalog/preset handling belong in a llama.cpp adapter behind the shared runtime profile service.
- Existing `rust/crates/pumas-core/src/conversion/llama_cpp.rs` is conversion/build tooling context, not a runtime serving boundary. Runtime profile work should not overload conversion code with server lifecycle responsibilities.
- New user-directed serving behavior needs a separate service boundary, tentatively `pumas-core/src/serving/`, so `RuntimeProfileService` remains profile/process ownership and does not become a mixed UI-workflow, validation, and provider-inventory service.
- The serving service should depend on runtime profile contracts and provider adapters through narrow interfaces. It should own serve/unserve orchestration, serving snapshots, validation responses, and non-critical error shaping.
- If a Pumas gateway is introduced to provide one public endpoint, it is an app/runtime boundary and must not live in reusable model-library or conversion modules. It must bind to loopback by default and report whether the endpoint is a gateway or a direct provider endpoint.
- Provider adapter changes must be additive: Ollama register/load/unload/list-running stays in `pumas-app-manager`, while the serving service decides when those calls are made and how their errors are projected.

### Frontend Blast Radius

- Existing `OllamaModelSection.tsx` polls one `connectionUrl` every 10 seconds while running. Multiple local runtime profiles should not multiply frontend polling. The frontend should subscribe once to backend-pushed runtime/profile events and refresh a backend-owned snapshot when notified.
- Existing library GGUF rows assume one running Ollama endpoint. Per-model routing means row actions must resolve the assigned provider/profile before create/register/load/unload.
- Settings UI should avoid generic text-only controls and use existing UI primitives, semantic buttons, labels, selects, toggles, segmented controls, and accessible names.
- Model row actions should add a `Serve` action without embedding provider-specific branching in row components. The row should open a serving modal or command surface that works from backend-provided profiles and defaults.
- The existing model metadata Runtime Route tab should either be renamed/split or delegate to a serving-config editor so users can both save route defaults and issue an immediate load request from one model-centered workflow.
- The loaded-models panel should render backend `ServedModelStatus` rows, including model alias, provider/profile, device placement, endpoint mode, memory usage when known, keep-loaded state, and last non-critical error.
- Frontend code may keep draft values for device mode, GPU layers, tensor split, context size, and keep-loaded toggles. It must not mark a model as served until the backend serving snapshot or command response confirms it.

### API Compatibility Blast Radius

- Existing commands should remain:
  - `launch_ollama`
  - `stop_ollama`
  - `is_ollama_running`
  - `ollama_list_models`
  - `ollama_create_model`
  - `ollama_delete_model`
  - `ollama_load_model`
  - `ollama_unload_model`
  - `ollama_list_running`
- New commands should be append-only and profile-aware.
- Existing optional `connectionUrl` fields should remain accepted.
- New frontend types should extend the bridge contract without changing existing response fields.
- New profile-aware model commands should accept `profile_id`, not raw endpoint URLs. Raw endpoint URLs may be accepted only by legacy commands or external-profile creation/update commands.
- Provider-specific commands should stay behind a provider-neutral runtime profile facade unless an upstream API exposes provider-only capabilities that cannot be represented generically.
- New user-directed serving commands should be provider-neutral:
  - `get_serving_status`
  - `validate_model_serving_config`
  - `serve_model`
  - `unserve_model`
  - optional `list_served_models` if it is not redundant with status.
- `serve_model` responses must distinguish command transport failure from non-critical model load failure. Transport failure means the request could not be processed; non-critical load failure means the request was processed and the selected model configuration did not load.
- Existing `launch_runtime_profile(profile_id, tag?, model_id?)` remains a low-level profile lifecycle API. Row/modal serving should call `serve_model` rather than requiring React to know whether a provider needs register, load, router refresh, or dedicated process launch.
- Native binding exposure is out of scope for the first serving UI slice unless a supported host-language caller is identified. If included, the UniFFI/Rustler surfaces must be updated in the same commit as native and host-language tests.

### Standards Compliance Requirements For Next Phase

- Contract-first: add Rust DTOs and matching TypeScript bridge types before UI implementation. Use explicit enums/newtypes instead of stringly typed device, provider, endpoint-mode, and error-code fields.
- Backend-owned data: serving snapshots, loaded state, endpoint status, and last errors are backend-owned. Frontend may only hold local form drafts and presentational state.
- Boundary validation: RPC handlers validate payload shape, URL/endpoint inputs, model IDs, numeric ranges, and provider/profile compatibility before calling the serving service.
- Non-critical errors: failed model loads must be domain responses with `severity = non_critical`, stable error codes, safe messages, and enough diagnostic context to identify model/profile/provider without exposing paths or unbounded process output.
- Path and process safety: model files are resolved through the existing model library, not renderer-supplied paths. Generated llama.cpp presets and profile logs stay under backend-owned launcher-data roots.
- Accessibility: model row/modal serving controls use semantic buttons, associated labels for form controls, named icon buttons, and keyboard-accessible modal behavior.
- Testing: add one vertical acceptance path from model row/modal command to backend response/status projection before expanding provider-specific breadth.
- Documentation: update touched `src` directory READMEs and `docs/contracts/desktop-rpc-methods.md` when serving contracts are introduced.

## Standards Review Iterations

### Iteration 1: Plan Standards

- Result: Compliant after adding objective, scope, inputs, risks, structured contracts, persisted artifacts, concurrency/lifecycle notes, milestones, verification, re-plan triggers, and completion criteria.
- Adjustment: The plan is stored under `docs/plans/local-runtime-profiles-and-ollama-version-manager/plan.md` because this is cross-layer work with staged rollout risk.

### Iteration 2: Architecture Patterns

- Result: Compliant with backend-owned data after moving runtime profile and model-route persistence to backend-owned config.
- Adjustment: Frontend settings are limited to form state and confirmed backend state. No optimistic route/profile updates are allowed.
- Adjustment: Per-model CPU/GPU selection is described as routing to runtime profiles, not unsupported native Ollama per-model hardware assignment.
- Adjustment: Runtime profile policy is owned by a provider-neutral backend service instead of by frontend state, provider-specific panels, or singleton process-manager conditionals.
- Adjustment: Ollama and llama.cpp are adapters under the same runtime profile contract, not independent architecture silos.

### Iteration 3: Frontend Standards

- Result: Compliant if implementation keeps rendering declarative and adds tests using accessible selectors.
- Adjustment: Multi-profile state must not introduce frontend per-profile polling. The UI should consume a backend-pushed runtime/profile event stream and fetch confirmed snapshots after events.
- Adjustment: The version crash milestone must add a regression test that clicks the globe/version-manager control by accessible name.

### Iteration 4: Accessibility Standards

- Result: Compliant if new settings controls use semantic buttons, labels for fields, keyboard-accessible dialogs/drawers, and accessible names for icon-only controls.
- Adjustment: Any new globe/settings/profile buttons must have explicit accessible names. Icon-only controls must hide decorative icons from assistive technologies or rely on the button label.

### Iteration 5: Rust API Standards

- Result: Compliant if raw strings/ports/device modes/provider modes are parsed at the boundary into typed values.
- Adjustment: Add typed profile IDs, provider IDs, endpoint URLs, ports, device modes, llama.cpp profile modes, and lifecycle states instead of passing stringly typed modes deep into process code.
- Adjustment: Public APIs return structured errors and avoid `Result<T, String>` in Rust boundaries.

### Historical Iteration 6: Runtime Lifecycle Ownership

- Previous-wave note: this section predates the 2026-05-08 serving update. It is retained for traceability, but the 2026-05-08 update did not re-review the external multithreading/concurrency standard by request.
- Result: Profile lifecycle state remains planned under one backend owner, with guarded profile operations and profile-scoped PID/log/status ownership.
- Adjustment: Profile start/stop must prevent overlapping operations, observe task errors, avoid holding locks across blocking process work, and cleanly remove profile PID files on shutdown.
- Adjustment: Backend runtime/profile event production may internally sample external endpoint health, but sampling must be owned by the backend service and surfaced through one subscription/snapshot contract.

### Iteration 7: User-Directed Shared Endpoint Update

- Date: 2026-05-08.
- Standards reviewed for this update: plan, architecture, coding, frontend, accessibility, testing, security, interop, launcher, dependency, documentation, cross-platform, language-binding, and Rust API/security/interop/dependency/tooling/cross-platform standards.
- Excluded from this update by request: the multithreading/concurrency standards.
- Result: The next phase is compliant only if it is implemented as user-directed serving, not automatic memory scheduling.
- Adjustment: Add a backend-owned serving facade and typed serving DTOs before frontend work.
- Adjustment: Save route/profile defaults separately from immediate `serve_model` commands.
- Adjustment: Make a failed fit or runtime load failure a non-critical domain response that preserves existing served models.
- Adjustment: Expose shared endpoint status truthfully. Do not claim one stable Pumas endpoint until a gateway or equivalent endpoint facade exists.
- Adjustment: Keep binding exposure out of scope unless native and host-language verification are added in the same slice.

## Definition of Done

- Clicking the Ollama globe/version-manager button does not crash the page.
- The version-manager failure path is localized to the version manager area when bad data or an unexpected error occurs.
- A backend-owned runtime profile config exists and persists across app restart.
- Users can define or edit local runtime profiles for provider, default/auto, CPU, GPU, and external endpoints.
- Users can assign a model to a runtime profile.
- Ollama profiles and llama.cpp profiles use the same route/status/settings facade.
- Model create/load/unload/list operations use the assigned profile endpoint.
- Frontend runtime status updates arrive through a backend-owned event/snapshot path, not per-profile component polling.
- Users can start model serving from a model row or model modal.
- Users can explicitly choose the profile/provider and CPU/GPU/hybrid placement for the selected model.
- Pumas validates the selected configuration and returns non-critical errors for failed loads without crashing the app or silently modifying unrelated loaded models.
- A backend-owned served-model status snapshot shows loaded models, placement, endpoint/profile, and last error state.
- Shared endpoint behavior is either implemented through a Pumas gateway/facade or explicitly labeled as provider endpoint mode in the serving status contract.
- Existing singleton Ollama commands and current frontend flows continue to work.
- Tests cover the version crash, runtime profile config, process lifecycle, profile routing, frontend settings controls, and cleanup of any timers/polling.
- Tests cover one vertical model-row/modal serving path and one non-critical load error path.
- Release binaries and frontend build complete after implementation.

## Milestones

### Milestone 1: Reproduce and Contain Version Manager Crash

**Goal:** Make the globe/version-manager path stable without changing Ollama runtime architecture.

**Tasks:**
- [x] Capture the likely thrown React error path from the Ollama globe click: `VersionListItemState` assumed `release.tagName` existed and called `.replace()` during render.
- [x] Add a failing frontend regression test for the version release boundary using mixed snake_case and camelCase provider payloads.
- [x] Identify whether the failure is caused by version release mapping, install-dialog assumptions, or an invalid UI control.
- [x] Fix the smallest shared version-manager issue without Ollama-specific hard-coding.
- [x] Prevent release-row render crashes by filtering invalid releases at the hook boundary and adding defensive display/date fallbacks. A localized row error state was not added because malformed rows are not rendered after normalization; revisit if the backend must expose invalid provider rows to users.

**Verification:**
- `npm run -w frontend test:run -- <targeted test file>` or the repo-equivalent targeted frontend test command.
- Existing `InstallDialogContent` and version-list tests still pass.
- Manual UI check: Ollama page globe button opens version manager without magenta crash.

**Status:** Completed for automated coverage. Manual Ollama globe smoke check remains part of release validation.

**Implementation Notes:**
- 2026-05-05: The frontend version hook accepted the backend `VersionReleaseInfo` contract as snake_case only while the render path consumed normalized camelCase `VersionRelease` rows. A malformed cached/provider row or leaked API row without `tagName` could crash the version dialog before an error state could render.
- 2026-05-05: Added app-neutral release normalization in `useAvailableVersionState`, including snake_case/camelCase support and invalid-row filtering. Hardened version row display and date formatting so missing optional fields no longer crash render.
- 2026-05-05: Validated with `npm run -w frontend test:run -- useAvailableVersionState` and `npm run -w frontend check:types`.

### Milestone 2: Define Backend Contracts for Local Runtime Profiles

**Goal:** Add append-only backend and bridge contracts for provider-neutral runtime profiles and model routes.

**Tasks:**
- [x] Define Rust domain types for profile ID, provider ID, endpoint URL, port, device mode, provider mode, lifecycle state, scheduler settings, and model route.
- [x] Define persisted config schema for profiles and per-model routes.
- [x] Define the runtime/profile snapshot shape consumed by the frontend.
- [x] Define runtime/profile event payloads following the existing backend-pushed update pattern.
- [x] Define provider adapter traits for Ollama and llama.cpp.
- [x] Define llama.cpp router profile mode and dedicated process profile mode as explicit typed variants.
- [x] Add Electron/TypeScript bridge types matching the new RPC responses.
- [x] Add RPC registry entries and request validation schemas.
- [x] Preserve existing singleton Ollama commands unchanged.
- [x] Mark `connection_url` as legacy boundary compatibility and document `profile_id` as the canonical internal routing key.

**Verification:**
- Rust unit tests for config serialization/deserialization and validation.
- Rust tests for event payload serialization and snapshot defaults.
- Rust tests for provider adapter contract defaults and provider-mode parsing.
- Electron registry validation tests.
- TypeScript typecheck.

**Status:** Complete for backend and bridge contract definition. Runtime lifecycle, provider adapters, model-operation routing, and pushed event transport remain in later milestones.

**Implementation Notes:**
- 2026-05-05: Added Rust runtime profile DTOs under `pumas-core::models`, including typed profile IDs, endpoint URLs, ports, providers, provider modes, lifecycle states, device settings, scheduler settings, model routes, snapshots, update-feed events, and mutation responses.
- 2026-05-05: Added a provider-neutral `RuntimeProviderAdapter` contract and capability DTO that distinguishes Ollama serve profiles from llama.cpp router and dedicated profiles.
- 2026-05-05: Added append-only Electron/TypeScript bridge contract methods and request validation schemas for runtime profile snapshots, update feeds, profile mutations, and model-route mutations. Existing singleton Ollama commands remain unchanged.
- 2026-05-05: Added placeholder RPC handlers that expose read-only empty snapshot/update-feed responses and return structured business failures for mutation calls until the persistence service lands.
- 2026-05-05: Subagent review confirmed the existing pushed update transport is model-library-specific from Rust SSE through Electron main/preload. A generalized runtime-profile push path must be implemented in a later event-transport slice instead of overloading the model-library stream.
- 2026-05-05: Validated with `cargo test -p pumas-library runtime_profile --manifest-path rust/Cargo.toml`, `cargo test -p pumas-rpc runtime_profiles --manifest-path rust/Cargo.toml`, `npm run -w frontend check:types`, and `npm run -w electron test`.
- 2026-05-05: Added `RuntimeProfileService` as the backend owner for persisted runtime profile config at `launcher-data/metadata/runtime-profiles.json`, with atomic JSON writes, default Ollama profile seeding, route persistence, snapshot generation, cursor bumping, and non-stub mutation handlers.
- 2026-05-05: Added `PumasApi` and primary IPC dispatch methods for runtime profile snapshots, update feeds, profile upsert/delete, and route set/clear so secondary API clients do not bypass backend ownership.
- 2026-05-05: Validated the persistence slice with `cargo test -p pumas-library runtime_profile --manifest-path rust/Cargo.toml` and `cargo test -p pumas-rpc runtime_profiles --manifest-path rust/Cargo.toml`.

### Milestone 3: Implement Thin Default-Profile Vertical Slice

**Goal:** Prove the profile architecture with one default profile before adding CPU/GPU/external profile complexity.

**Tasks:**
- [x] Add `RuntimeProfileService` or equivalent backend service boundary.
- [x] Add an Ollama provider adapter behind the service.
- [x] Seed a default Ollama profile that maps to existing singleton Ollama behavior.
- [x] Add one profile-aware list path using `profile_id`.
- [x] Add one profile-aware load path using `profile_id`.
- [x] Expose one runtime/profile snapshot API.
- [x] Add one backend-pushed runtime/profile event when the default profile status changes.
- [x] Preserve app-level aggregate status and existing singleton commands.

**Verification:**
- Rust tests for default profile creation and route resolution.
- Rust RPC tests showing `profile_id` selects the default profile endpoint.
- Event bridge tests for default-profile status notification.
- Tests showing legacy `connection_url` compatibility still works at the boundary.
- Existing process tests still pass.

**Status:** Complete for the default-profile backend vertical slice. Managed lifecycle, multi-profile process isolation, full model-operation routing, llama.cpp support, and frontend settings remain in later milestones.

**Implementation Notes:**
- 2026-05-05: Added default-profile endpoint resolution to `RuntimeProfileService` and `PumasApi`, including IPC dispatch for secondary clients.
- 2026-05-05: Added append-only `ollama_list_models_for_profile` RPC/preload/TypeScript bridge method. It resolves `profile_id` through backend-owned runtime profiles and calls the Ollama client with the resolved endpoint; existing `ollama_list_models(connection_url)` remains unchanged.
- 2026-05-05: Validated with `cargo test -p pumas-library runtime_profile --manifest-path rust/Cargo.toml`, `cargo test -p pumas-rpc runtime_profiles --manifest-path rust/Cargo.toml`, `npm run -w frontend check:types`, and `npm run -w electron test`.
- 2026-05-05: Added append-only `ollama_load_model_for_profile` RPC/preload/TypeScript bridge method using the same backend `profile_id` endpoint resolution. Existing `ollama_load_model(connection_url)` remains unchanged.
- 2026-05-05: Added `OllamaRuntimeProviderAdapter` and routed Ollama profile validation through it from `RuntimeProfileService`, keeping provider-specific mode checks out of generic profile mutation flow.
- 2026-05-05: Added the runtime-profile pushed update transport: Rust SSE route `/events/runtime-profile-updates`, Electron bridge parsing/reconnect/cleanup, main-process forwarding on `runtime-profile:update`, and preload/window typing through `onRuntimeProfileUpdate`.
- 2026-05-05: Added a backend regression test proving default runtime profile snapshot initialization does not redefine app-level aggregate Ollama status; `get_status().ollama_running` still mirrors the existing singleton `is_ollama_running()` command.
- 2026-05-05: Added a backend-owned in-memory runtime profile status journal. Runtime profile snapshot and update-feed calls refresh the default Ollama profile from the existing singleton probe, emit `status_changed` events when the default profile transitions between stopped/running, and keep status cursors separate from frontend polling logic.
- 2026-05-05: Validated event transport with `cargo test -p pumas-rpc runtime_profile --manifest-path rust/Cargo.toml`, `npm run -w frontend check:types`, and `npm run -w electron test`.
- 2026-05-05: Validated status event production with `cargo test -p pumas-library runtime_profile --manifest-path rust/Cargo.toml` and recompiled the RPC runtime-profile target with `cargo test -p pumas-rpc runtime_profile --manifest-path rust/Cargo.toml`.

### Milestone 4: Implement Managed Profile Lifecycle Backend

**Goal:** Let the backend safely manage multiple provider-backed runtime profiles after the default-profile slice is proven.

**Tasks:**
- [x] Add profile-aware process lifecycle through the runtime-profile service.
- [x] Generate profile-specific ports, health URLs, PID files, and log files.
- [x] Apply profile environment variables, including CPU/GPU visibility settings where platform-supported.
- [x] Serialize start/stop operations per profile.
- [x] Report profile status, last error, endpoint URL, and running state through the snapshot/event path.
- [x] Keep broad singleton process cleanup separate from profile-scoped stop operations.
- [x] Preserve app-level aggregate status for existing UI.

**Verification:**
- Rust tests for start/stop state transitions with fake process launchers where possible.
- Rust tests for port and PID path derivation.
- Rust tests for overlapping start/stop prevention.
- Rust tests that profile-scoped stop does not target unrelated Ollama processes.
- Existing process tests still pass.

**Status:** Complete for the backend managed Ollama lifecycle slice. Successful real-process smoke coverage remains part of release validation, and llama.cpp lifecycle support remains in Milestone 6.

**Implementation Notes:**
- 2026-05-05: Added backend-managed runtime launch specs derived from persisted profiles. Specs resolve deterministic profile runtime directories under `launcher-data/runtime-profiles/{provider}/{profile_id}`, profile-scoped PID/log files, health URLs, explicit or deterministic implicit ports, and managed-port collision validation without touching singleton process launch behavior.
- 2026-05-05: Validated launch spec derivation and collision handling with `cargo test -p pumas-library runtime_profile --manifest-path rust/Cargo.toml`.
- 2026-05-05: Added profile environment derivation to launch specs, including `PUMAS_RUNTIME_PROFILE_ID`, Ollama `OLLAMA_HOST`, CPU mode GPU-hiding variables, and GPU/specific-device visibility variables when a device ID is configured.
- 2026-05-05: Validated CPU/GPU environment derivation with `cargo test -p pumas-library runtime_profile --manifest-path rust/Cargo.toml`.
- 2026-05-05: Added a per-profile operation guard inside `RuntimeProfileService` so lifecycle start/stop paths can reject overlapping operations for the same profile without holding config locks across process work. The serialization task remains open until start/stop commands use the guard.
- 2026-05-05: Validated operation serialization infrastructure with `cargo test -p pumas-library runtime_profile_service_serializes_profile_operations --manifest-path rust/Cargo.toml`.
- 2026-05-05: Extended the low-level binary launch config with profile-owned PID file, health URL, and bulk environment override builders so future profile lifecycle code can consume launch specs without putting profile policy in the process launcher.
- 2026-05-05: Added the first managed profile launch path for Ollama profiles. The core API and primary IPC dispatch now launch from a supplied installed-version directory through the backend launch spec, record starting/running/failed status events, and use the per-profile operation guard.
- 2026-05-05: Validated the launch failure/status path without spawning a process using `cargo test -p pumas-library test_launch_runtime_profile_reports_profile_scoped_failure --manifest-path rust/Cargo.toml`.
- 2026-05-05: Added append-only desktop bridge command wiring for `launch_runtime_profile(profile_id, tag?)`. The RPC handler resolves the active or requested Ollama version through the existing version manager and delegates to the backend-owned launch path; legacy `launch_ollama` remains unchanged.
- 2026-05-05: Added the first profile-scoped stop path. Core API and primary IPC dispatch stop only the selected profile PID file, never the legacy broad Ollama cleanup path, and record stopping/stopped/failed lifecycle status through the runtime profile service.
- 2026-05-05: Validated the no-PID profile-scoped stop path without touching real processes using `cargo test -p pumas-library test_stop_runtime_profile_without_pid_is_profile_scoped --manifest-path rust/Cargo.toml`.
- 2026-05-05: Added append-only desktop bridge command wiring for `stop_runtime_profile(profile_id)`. The command delegates to the profile-scoped PID-file stop path and leaves legacy `stop_ollama` unchanged.

**Discovered Issues:**
- 2026-05-05: `ProcessLauncher` is currently a static process executor, so managed profile lifecycle tests cover command construction, missing-binary failure, no-PID stop, and status transitions, but not a successful fake-spawn path. Add a fake launcher seam before expanding lifecycle policy if successful start/stop transitions need deterministic unit coverage without installed Ollama binaries.

### Milestone 5: Route Ollama Model Operations Through Profiles

**Goal:** Make Ollama model operations use the backend-owned model route rather than a single page-level endpoint.

**Tasks:**
- [x] Add model-route resolution for create/load/unload/delete/list actions.
- [x] Add profile-aware model operations that accept `profile_id`.
- [x] Keep `connection_url` accepted only as legacy compatibility input and convert it at the boundary.
- [x] Split register/create from load, or make auto-load an explicit per-route setting.
- [x] Return clear errors when a route points to a stopped or unhealthy profile.
- [x] Keep external endpoint profiles supported.

**Verification:**
- Rust RPC tests for model route resolution.
- Tests showing `connection_url` compatibility still works.
- Tests showing model-specific profile routing chooses the expected endpoint.

**Status:** Complete for backend Ollama routing. Remaining work moves to llama.cpp provider support and frontend runtime-profile controls.

**Implementation Notes:**
- 2026-05-05: Added append-only `ollama_create_model_for_profile`, `ollama_delete_model_for_profile`, and `ollama_unload_model_for_profile` commands through Rust RPC, Electron preload validation, and frontend bridge types. Existing connection-url commands remain unchanged.
- 2026-05-05: Validated profile-aware Ollama command wiring with `cargo test -p pumas-rpc runtime_profile --manifest-path rust/Cargo.toml`, `npm run -w frontend check:types`, and `npm run -w electron test`.
- 2026-05-05: Added backend-owned model route endpoint resolution with precedence `explicit profile_id > saved model route > default profile`, and routed `ollama_create_model_for_profile` through it because create has the Pumas `model_id` available at the operation boundary.
- 2026-05-05: Validated route endpoint resolution with `cargo test -p pumas-library runtime_profile_service_resolves_model_route_endpoint --manifest-path rust/Cargo.toml` and recompiled RPC with `cargo test -p pumas-rpc runtime_profile --manifest-path rust/Cargo.toml`.
- 2026-05-05: Added optional `model_id` inputs to profile-aware load/unload/delete commands so callers can use saved model routes for model-specific operations. `ollama_list_models_for_profile` remains profile-only because it lists endpoint inventory rather than a model-specific route.
- 2026-05-05: Validated route-aware load/unload/delete command wiring with `cargo test -p pumas-rpc runtime_profile --manifest-path rust/Cargo.toml`, `npm run -w frontend check:types`, and `npm run -w electron test`.
- 2026-05-05: Added operation endpoint guards that reject managed profiles unless their backend lifecycle state is `running`, while allowing external profiles because their health is owned outside the managed process lifecycle. Profile-aware Ollama model operations now use this checked endpoint path and return deterministic backend errors before falling through to network failures.
- 2026-05-05: Validated stopped managed-profile rejection and external endpoint allowance with `cargo test -p pumas-library runtime_profile_service --manifest-path rust/Cargo.toml` and recompiled RPC with `cargo test -p pumas-rpc runtime_profile --manifest-path rust/Cargo.toml`.
- 2026-05-05: Wired `ModelRuntimeRoute.auto_load` into profile-aware Ollama create. Legacy `ollama_create_model(connection_url)` still auto-loads for compatibility; `ollama_create_model_for_profile` defaults to auto-load when no route exists and skips the implicit load when a backend route sets `auto_load=false`.
- 2026-05-05: Validated auto-load route policy lookup with `cargo test -p pumas-library runtime_profile_service_reads_model_route_auto_load_policy --manifest-path rust/Cargo.toml` and recompiled RPC with `cargo test -p pumas-rpc runtime_profile --manifest-path rust/Cargo.toml`.
- 2026-05-05: Converted legacy Ollama `connection_url` inputs into `RuntimeEndpointUrl` at the RPC boundary before constructing an Ollama client. Profile-aware commands remain the canonical route path; legacy URL input is retained only for compatibility and now rejects invalid schemes as `InvalidParams`.
- 2026-05-05: Validated legacy endpoint boundary handling with `cargo test -p pumas-rpc legacy_connection_url --manifest-path rust/Cargo.toml` and recompiled the runtime-profile RPC surface with `cargo test -p pumas-rpc runtime_profile --manifest-path rust/Cargo.toml`.

### Milestone 6: Add llama.cpp Runtime Adapter

**Goal:** Add llama.cpp as a second provider under the same runtime profile architecture.

**Tasks:**
- [x] Add a llama.cpp provider adapter behind the runtime-profile service.
- [x] Support managed router profiles using `llama-server` router mode.
- [x] Support managed dedicated process profiles using `llama-server -m <model>`.
- [x] Generate deterministic model catalog or preset data for router profiles from Pumas library GGUF artifacts.
- [x] Represent llama.cpp CPU/GPU settings as typed profile/provider settings, including GPU layers/device/split controls where supported.
- [x] Report llama.cpp profile status through the shared runtime snapshot/event path.
- [x] Keep provider-specific llama.cpp capabilities behind the provider adapter unless exposed through generic runtime profile fields.

**Verification:**
- Rust tests for llama.cpp provider-mode parsing and config serialization.
- Rust tests for deterministic catalog/preset generation.
- Rust tests for router profile endpoint/status handling with fake server responses where possible.
- Rust tests for dedicated process command construction and profile-scoped PID/log paths.
- Existing Ollama runtime profile tests still pass.

**Status:** Complete for backend command construction, profile lifecycle status, catalog/preset generation, and RPC bridge wiring. Real-process llama.cpp smoke validation remains part of release validation because tests intentionally avoid starting external servers.

**Implementation Notes:**
- 2026-05-05: Added `LlamaCppRuntimeProviderAdapter` behind the existing `RuntimeProviderAdapter` trait and moved llama.cpp profile validation out of the generic service path. The adapter accepts router and dedicated provider modes, rejects provider/mode mismatches, and keeps external endpoint requirements provider-owned.
- 2026-05-05: Validated the llama.cpp adapter with `cargo test -p pumas-library llama_cpp_provider_adapter --manifest-path rust/Cargo.toml` and re-ran the existing service profile filter with `cargo test -p pumas-library runtime_profile_service --manifest-path rust/Cargo.toml`.
- 2026-05-05: Added backend-only llama.cpp router command construction. Runtime profile launch specs now carry provider-specific process args, llama.cpp router profiles derive `--host`, `--port`, `--models-dir`, `--n-gpu-layers`, and `--tensor-split` from typed profile fields, and `BinaryLaunchConfig::llama_cpp_router` targets the existing `launcher-data/llama-cpp/build/bin/llama-server` layout without a `--model` argument.
- 2026-05-05: Validated router command construction with `cargo test -p pumas-library llama_cpp_router --manifest-path rust/Cargo.toml` and `cargo test -p pumas-library runtime_profile_service_derives_llama_cpp_router_launch_specs --manifest-path rust/Cargo.toml`.
- 2026-05-05: Added core API launch support for managed llama.cpp router profiles. The shared runtime profile launch path now branches between Ollama and llama.cpp router binary configs, records the same starting/running/failed lifecycle statuses, and keeps dedicated llama.cpp launch explicitly blocked until model-bound process support is implemented.
- 2026-05-05: Validated profile-scoped llama.cpp router launch failure/status behavior with `cargo test -p pumas-library test_launch_llama_cpp_router_profile_reports_profile_scoped_failure --manifest-path rust/Cargo.toml` and re-ran the existing Ollama launch failure regression with `cargo test -p pumas-library test_launch_runtime_profile_reports_profile_scoped_failure --manifest-path rust/Cargo.toml`.
- 2026-05-05: Made RPC `launch_runtime_profile` provider-aware. Ollama profiles still resolve through the active Ollama version manager, while llama.cpp profiles resolve to the backend-owned `launcher-data/llama-cpp` build directory with a default `local-build` tag unless the caller supplies one.
- 2026-05-05: Validated provider-aware RPC launch wiring with `cargo test -p pumas-rpc runtime_profile --manifest-path rust/Cargo.toml` and re-ran the core llama.cpp router launch failure regression.
- 2026-05-05: Added deterministic llama.cpp router catalog generation from the Pumas model library. The generator lists indexed models, keeps only records whose primary artifact is GGUF, sorts entries by model id/path, and emits a `--models-preset` compatible INI payload with `load-on-startup=false` by default.
- 2026-05-05: Validated deterministic catalog output with `cargo test -p pumas-library llama_cpp_router_catalog --manifest-path rust/Cargo.toml` and re-ran the router launch/spec test filter with `cargo test -p pumas-library llama_cpp_router --manifest-path rust/Cargo.toml`.
- 2026-05-05: Wired router catalog generation into managed llama.cpp router launch. The core launch path writes a profile-scoped `models-preset.ini` under `launcher-data/runtime-profiles/llama-cpp/{profile_id}/` before process spawn and replaces the provisional `--models-dir` launch arg with `--models-preset`.
- 2026-05-05: Validated preset writing through the existing profile-scoped missing-binary launch regression with `cargo test -p pumas-library test_launch_llama_cpp_router_profile_reports_profile_scoped_failure --manifest-path rust/Cargo.toml` and re-ran the router test filter.
- 2026-05-05: Added managed dedicated llama.cpp launch preparation. Launch commands now accept an optional `model_id`; dedicated profiles require it, resolve the Pumas primary artifact, validate it is GGUF, and append `--model <path>` while preserving the same profile-scoped PID/log/status behavior as router profiles.
- 2026-05-05: Extended Electron preload, RPC validation, and frontend bridge types so `launch_runtime_profile(profileId, tag?, modelId?)` remains backward compatible while supporting dedicated model-bound launches.
- 2026-05-05: Validated dedicated command construction and bridge compatibility with `cargo test -p pumas-library llama_cpp_dedicated --manifest-path rust/Cargo.toml`, `cargo test -p pumas-rpc runtime_profile --manifest-path rust/Cargo.toml`, `npm run -w frontend check:types`, and `npm run -w electron test`.

**Discovered Issues:**
- 2026-05-05: The desktop/RPC `launch_runtime_profile` path was still coupled to the Ollama version manager before it called core launch. Resolved the same day by resolving the profile provider first and only using the Ollama version manager for Ollama profiles.

### Milestone 7: Add Frontend Local Runtime Profile Settings

**Goal:** Expose runtime profiles and per-model routing through accessible, backend-confirmed UI.

**Tasks:**
- [x] Add a frontend runtime/profile subscription hook that follows the existing model-library update subscription pattern. The bridge event source exists as `onRuntimeProfileUpdate`; the React hook still needs to consume it.
- [x] Add snapshot refresh on runtime/profile events.
- [x] Add a local runtime profile settings section.
- [x] Add profile create/edit controls for provider, provider mode, name, endpoint, port, scheduler settings, and managed/external status.
- [x] Add per-model route controls for assigning a model to auto/Ollama/llama.cpp/CPU/GPU/external profiles.
- [x] Show profile status and model running state from backend-confirmed responses.
- [x] Show provider-specific advanced controls only when the selected provider/mode supports them.
- [x] Avoid optimistic persistence; refresh or accept backend-pushed state after save.
- [x] Remove or bypass component-owned Ollama state polling for profile-backed views.

**Verification:**
- Frontend tests for rendering profile settings and saving model routes.
- Frontend tests for provider/mode-specific controls and hidden unsupported options.
- Accessibility-focused tests using named buttons/fields.
- Subscription cleanup tests for runtime/profile events.
- Typecheck and lint.

**Status:** Complete for the current implementation wave. Runtime profile snapshot/subscription state, the settings editor, per-model route controls, backend-confirmed status display, and Ollama model-section polling cleanup are available to React.

**Implementation Notes:**
- 2026-05-05: Added `useRuntimeProfileUpdateSubscription` and `useRuntimeProfiles` so React code can subscribe to backend-pushed runtime/profile update feeds, validate the feed shape, debounce notifications, and refresh the backend-owned runtime profile snapshot without adding component-level polling.
- 2026-05-05: Validated the hook/type surface with `npm run -w frontend check:types`. A focused `vitest run useRuntimeProfiles api-runtime-profiles` command found no matching tests yet, so subscription cleanup tests remain part of later frontend test work.
- 2026-05-05: Added `RuntimeProfileSettingsSection` to the Ollama panel. The section lists backend-confirmed profiles and statuses, exposes create/edit/delete controls for provider, provider mode, management mode, endpoint, port, enabled state, device mode, device id, and llama.cpp GPU layers, and refreshes the backend snapshot after every mutation rather than mutating local state optimistically.
- 2026-05-05: Validated the settings section with `npm run -w frontend check:types`.
- 2026-05-05: Added per-model runtime route controls to the model metadata modal. The new Runtime Route tab uses backend-confirmed profiles/routes/statuses, saves `ModelRuntimeRoute` assignments through the bridge, clears routes through the backend API, and refreshes the runtime profile snapshot after each mutation.
- 2026-05-05: Validated the route editor with `npm run -w frontend check:types`.
- 2026-05-05: Removed the Ollama model section's component-owned 10-second polling loop. The section now refreshes on initial/running-state changes, after local create/load/unload/delete operations, and when the backend runtime-profile update feed publishes a snapshot-required or event-bearing cursor.
- 2026-05-05: Validated polling cleanup with `npm run -w frontend check:types`.
- 2026-05-08: Fixed a runtime profile editor state bug where starting a new profile could be immediately overwritten by the default selected profile, carrying `ollama-default`'s `11434` process port into the draft and causing a managed port collision on save. The create-new draft now remains independent until saved or an existing profile is selected, managed endpoint/port copy now explains auto-allocation, and backend collision errors name the conflicting profile. Validated with `npm run -w frontend test:run -- RuntimeProfileSettingsSection`, `npm run -w frontend check:types`, and `cargo test -p pumas-library runtime_profile --manifest-path rust/Cargo.toml`.
- 2026-05-08: Locked saved profile IDs in the runtime profile editor so renaming a profile cannot accidentally create a second managed profile with the copied process port. Provider changes and switching back to managed mode also clear endpoint/port overrides so managed drafts return to automatic allocation. Validated with `npm run -w frontend test:run -- RuntimeProfileSettingsSection` and `npm run -w frontend check:types`.
- 2026-05-08: Added runtime lifecycle controls to the runtime profile settings editor. Managed Ollama and llama.cpp router profiles can now be started and stopped from their selected profile row, while managed llama.cpp dedicated profiles explicitly state that they start from a model's Serving page because a model path is required. Validated with `npm run -w frontend test:run -- RuntimeProfileSettingsSection ModelServeDialog ModelMetadataModal LocalModelInstalledActions` and `npm run -w frontend check:types`.

**Deferred Follow-up:**
- 2026-05-05: Ollama inventory changes made outside Pumas are no longer discovered by this component's timer. If automatic external inventory refresh is required, the backend runtime event producer should emit a profile/inventory update instead of reintroducing frontend polling.
- 2026-05-05: Full frontend lint is currently blocked by pre-existing issues outside this slice: unsafe `any` assignments in model manager/model hook tests, max-lines violations in several existing test/component files, and `ModelMetadataModalContent` complexity/length. The changed Ollama model section passes targeted ESLint.

**Discovered Issues:**
- 2026-05-08: The runtime profile editor originally used `selectedProfileId = null` for both "no profile selected yet" and "creating a new profile", so the initial auto-select effect could replace a new-profile draft with the default profile. Resolved by tracking create-new mode separately.
- 2026-05-08: The runtime profile editor allowed editing a saved profile's durable `profile_id`, which made save behave like clone-with-existing-port instead of rename. Resolved by making saved profile IDs read-only; display names remain editable.
- 2026-05-08: The runtime profile editor exposed launchable managed profiles but had no Start/Stop controls, leaving users to try unrelated app-sidebar launch buttons. Resolved by wiring profile-scoped launch/stop actions into the settings editor.

### Milestone 8: Integration, Documentation, and Release Validation

**Goal:** Validate the full user flow and update durable module documentation.

**Tasks:**
- [x] Update relevant module READMEs for new RPC/profile/process contracts.
- [x] Add or update contract docs if profile config becomes a durable interface.
- [x] Test default singleton Ollama flow still works.
- [x] Test llama.cpp router and dedicated profile flows with fake process/server coverage and one manual smoke path when binaries are available.
- [x] Test CPU and GPU profile assignment behavior on available hardware or documented fake-process coverage.
- [x] Build frontend and release binaries.

**Verification:**
- Full targeted frontend test set.
- Relevant Rust crate tests.
- Electron validation/typecheck.
- Frontend build.
- Release build/smoke command.

**Status:** Complete for the current implementation wave. Runtime/profile module and bridge contract documentation is updated, automated Rust/Electron/frontend coverage passes, frontend assets build, release binaries compile, and release smoke starts successfully.

**Implementation Notes:**
- 2026-05-05: Updated core, API, models, process, RPC handler, desktop RPC contract, frontend type, hook, and app-panel section docs to describe backend-owned runtime profiles, canonical `profile_id` routing, legacy endpoint boundaries, pushed runtime-profile events, profile-scoped process ownership, and generated llama.cpp router presets.
- 2026-05-05: Release smoke against an already-running older primary backend exposed a compatibility warning loop: the new runtime-profile SSE stream repeatedly called `list_runtime_profile_updates_since` through a primary instance that did not support the method yet. Added a compatibility backoff so the stream logs the unsupported primary once and retries slowly until the primary backend is restarted.
- 2026-05-05: Validated the core runtime profile surface with `cargo test -p pumas-library runtime_profile --manifest-path rust/Cargo.toml`, covering default profile seeding/status preservation, profile route resolution, stopped/external endpoint guards, CPU/GPU environment derivation, llama.cpp provider modes, router launch specs, dedicated launch prep, router catalog sorting, and generated preset writing.
- 2026-05-05: Validated RPC and Electron surfaces with `cargo test -p pumas-rpc --manifest-path rust/Cargo.toml` and `npm run -w electron test`. The full RPC suite passed with 49 unit tests and 5 active integration tests; 11 integration tests remain intentionally ignored by the existing harness.
- 2026-05-05: Validated frontend coverage with `npm run -w frontend test:run` after updating the Ollama preview test mock for the new runtime-profile subscription dependency. The suite passed with 99 test files and 410 tests.
- 2026-05-05: Validated frontend type/build and release artifacts with `npm run -w frontend check:types`, `npm run -w frontend build`, `bash launcher.sh --build-release`, and `bash launcher.sh --release-smoke`.
- 2026-05-05: No local managed llama.cpp runtime binary was found under `launcher-data/`, so llama.cpp validation stayed on deterministic backend command/catalog/preset tests rather than a real `llama-server` smoke.

**Discovered Issues:**
- 2026-05-05: `cargo test -p pumas-rpc runtime_profile --manifest-path rust/Cargo.toml` and the plural `runtime_profiles` filter currently match no RPC tests. The full `pumas-rpc` suite covers the registered RPC unit tests, but dedicated runtime-profile handler tests should be added if handler-level behavior expands.
- 2026-05-05: Release smoke can connect to an already-running primary backend from a previous build. Runtime-profile event compatibility is now throttled for older primaries, but manual verification should restart the app fully before judging new runtime-profile UI behavior.
- 2026-05-09: The install dialog could stop polling if the first progress read happened before the backend installation tracker initialized, leaving the row on the pending/default progress state. The completion path also reset transient install UI before refreshing installed versions, so a finished install could briefly or persistently render as installable.
- 2026-05-09: llama.cpp release archives may extract `llama-server` inside a nested archive directory. Existing install validation only checked fixed root/bin/build paths, so a successfully extracted runtime could be treated as incomplete and fail to update the installed-version button state.
- 2026-05-09: The new llama.cpp version directory was not ignored like `comfyui-versions/`, `ollama-versions/`, and `torch-versions/`, so a successful local install could leave runtime binaries as untracked source-tree files.
- 2026-05-09: The model-serving path for managed dedicated llama.cpp profiles still used the pre-version-manager `launcher-data/llama-cpp` local-build path, while profile Start used the installed active llama.cpp runtime version. This made Serve fail or appear inert after installing/running llama.cpp from the app page. The model-serving dialog also defaulted GGUF models to the default Ollama profile before considering a running llama.cpp profile.
- 2026-05-09: The model-serving action hook could return without user-visible feedback when the serving bridge methods were unavailable or when config construction failed. This made Start serving look inert instead of reporting the missing action path.
- 2026-05-09: The Start serving button was disabled when frontend preflight thought a model/profile was blocked, so clicking it could never surface the backend validation reason. The serving form also hid model placement/context controls for llama.cpp router profiles, leaving running router targets without the expected model settings surface.

### Milestone 9: Define User-Directed Serving Contracts

**Goal:** Freeze the cross-layer serving contract before implementing model-row/modal workflows.

**Tasks:**
- [x] Define Rust serving DTOs under `pumas-core::models` or a focused `serving` model module.
- [x] Add typed enums/newtypes for serving provider, endpoint mode, device mode, placement details, load state, error severity, and error code.
- [x] Define `ModelServingConfig` with explicit user-selected placement fields:
  - provider/profile id
  - device mode
  - device id
  - GPU layers
  - tensor split
  - context size
  - keep-loaded flag
  - model alias when supported.
- [x] Define `ServeModelRequest`, `ModelServeValidationResponse`, `ServedModelStatus`, `ServingEndpointStatus`, and `ModelServeError`.
- [x] Define response semantics for non-critical load failures, including `loaded_models_unchanged`.
- [x] Add matching TypeScript payload and bridge types in `frontend/src/types/`.
- [x] Add RPC/preload method declarations and validation schemas without registering unimplemented behavior that returns dummy data.
- [x] Decide whether native bindings are in or out of scope for the first serving phase and document the decision in this plan and binding README if needed.

**Verification:**
- Rust unit tests for serving DTO serialization, deserialization, enum labels, defaults, numeric bounds, and non-critical error shaping.
- TypeScript typecheck for new bridge and payload types.
- Electron RPC registry/preload tests for payload validation once methods are wired.
- Contract documentation update in `docs/contracts/desktop-rpc-methods.md`.

**Status:** Completed for status, validation, and Ollama serve/unserve contracts. llama.cpp support remains a later provider slice.

**Implementation Notes:**
- 2026-05-08: Added `pumas-core::models::serving` DTOs for user-authored serving config, endpoint mode, served-model state, safe non-critical load errors, validation responses, serving snapshots, and `loaded_models_unchanged` response semantics. The contract reuses existing typed runtime provider/profile/device DTOs instead of adding parallel string fields.
- 2026-05-08: Added matching renderer payload types in `frontend/src/types/api-serving.ts`, exported them through the public type barrel, and documented the backend-owned serving-state contract in frontend, model, and desktop RPC docs.
- 2026-05-08: Validated with `cargo test -p pumas-library serving --manifest-path rust/Cargo.toml` and `npm run -w frontend check:types`.
- 2026-05-08: Added implemented `get_serving_status` and `validate_model_serving_config` RPC/preload/bridge methods with Electron request schemas. Native binding exposure is documented as out of scope until a supported binding consumer exists and host-language verification can land in the same slice.
- 2026-05-08: Validated the callable contract slice with `cargo test -p pumas-library serving --manifest-path rust/Cargo.toml`, `cargo test -p pumas-rpc serving --manifest-path rust/Cargo.toml`, `npm run -w frontend check:types`, and `npm run -w electron test`.
- 2026-05-08: Registered `serve_model` and `unserve_model` after adding real Ollama provider orchestration. llama.cpp requests intentionally return a non-critical `unsupported_provider` load response until the llama.cpp serving slice is implemented.
- 2026-05-08: Validated the Ollama serving facade wiring with `cargo test -p pumas-library serving --manifest-path rust/Cargo.toml`, `cargo test -p pumas-rpc serving --manifest-path rust/Cargo.toml`, `npm run -w frontend check:types`, and `npm run -w electron test`.

**Discovered Issues:**
- 2026-05-08: The broader models README still contains a historical blanket statement that all DTOs use camelCase, while runtime-profile, package-facts, and serving contracts intentionally use snake_case. This slice documented serving explicitly, but a later docs cleanup should correct the broad statement without changing wire formats.
- 2026-05-08: Electron RPC request schemas are intentionally shallow and can only treat nested serving config as an `unknown-record`; meaningful serving validation must stay in Rust RPC/service code.
- 2026-05-08: Torch inference RPCs still bypass runtime profiles and use `connection_url`. That is outside the current Ollama/llama.cpp serving slice but should be considered before claiming a provider-general serving facade.
- 2026-05-08: `cargo test -p pumas-rpc serving --manifest-path rust/Cargo.toml` initially compiled the RPC crate but matched no focused serving tests. Resolved the same day by adding an integration test for serving status, validation, and non-critical serve failures.
- 2026-05-08: Because `pumas-core` cannot depend on `pumas-app-manager` without creating a dependency cycle, the first Ollama provider orchestration lives in the RPC serving handler while validation/status ownership lives in core. This is acceptable for the thin slice but should be revisited with an adapter inversion before adding more provider complexity.

### Milestone 10: Add Backend Serving Facade And Status Snapshot

**Goal:** Create the backend owner for user-directed serving without spreading provider-specific decisions into React or runtime-profile settings.

**Tasks:**
- [x] Add a `ServingService` or equivalent backend service boundary.
- [x] Expose serving facade methods:
  - [x] `get_serving_status`
  - [x] `list_serving_status_updates_since`
  - [x] `validate_model_serving_config`
  - [x] `serve_model` through RPC/Electron/renderer bridge
  - [x] `unserve_model` through RPC/Electron/renderer bridge
  - [x] Keep provider load/unload orchestration behind the current RPC adapter boundary until the `pumas-core`/`pumas-app-manager` dependency boundary is inverted.
- [x] Implement validation for model existence, executable artifact readiness, provider/profile compatibility, profile state, supported file format, numeric ranges, and supported provider placement fields.
- [x] Resolve model paths only through `ModelLibrary`; do not accept renderer-supplied file paths.
- [x] Add serving snapshots/events for loaded models, failed load attempts, endpoint mode, and last non-critical errors.
- [x] Keep route defaults separate from immediate serving commands.
- [x] Preserve existing runtime profile and Ollama APIs unchanged.
- [x] Report `endpoint_mode = pumas_gateway` once the Pumas `/v1` gateway is available, while keeping per-model provider endpoints in `ServedModelStatus` for routing.

**Verification:**
- Rust unit tests for validation success/failure cases.
- Rust tests proving route defaults prefill or resolve config but do not auto-load without a serve command.
- Rust tests for non-critical error response shape and `loaded_models_unchanged`.
- RPC tests for serving handlers once registered.

**Status:** Complete for the current serving facade. Status, validation, in-memory snapshot updates/events, bridge-level serve/unserve methods, route-default separation, gateway status reporting, Ollama load/unload orchestration, llama.cpp dedicated launch/unload, and llama.cpp router serving are implemented. Moving provider load/unload orchestration behind a core-owned adapter trait remains a future dependency-boundary refactor.

**Implementation Notes:**
- 2026-05-08: Added `ServingService` as the backend owner for serving snapshots and request validation. The initial snapshot is in memory and reports `endpoint_mode = not_configured` until provider endpoint or gateway behavior is implemented.
- 2026-05-08: Added `PumasApi::get_serving_status` and `PumasApi::validate_model_serving_config`, plus RPC handlers. Validation resolves models through `ModelLibrary`, checks runtime profile provider/state through the runtime profile snapshot, and returns non-critical domain errors instead of transport failures for invalid fit/request conditions.
- 2026-05-08: Added `ServingService` snapshot mutation helpers and wired `serve_model`/`unserve_model` through the RPC serving facade for Ollama. Successful Ollama loads register the GGUF in Ollama if needed, request a load using the user-selected keep-loaded setting, and publish a backend-owned `ServedModelStatus` with `endpoint_mode = provider_endpoint`.
- 2026-05-08: Added provider-specific validation for Ollama placement limitations. Ollama serving accepts `auto` or a request that matches the selected runtime profile device mode, and returns non-critical `unsupported_placement` errors for per-model device IDs, GPU layers, tensor split, and context-size settings that the current Ollama load path does not apply.
- 2026-05-08: Added validation and RPC orchestration for managed llama.cpp dedicated profiles. Stopped managed dedicated profiles are treated as launchable serving targets, `serve_model` calls the existing model-bound runtime-profile launcher, `unserve_model` stops the selected dedicated profile, and router-mode requests still return a non-critical unsupported-provider response until gateway/router semantics are implemented.
- 2026-05-08: Added focused RPC integration coverage for `get_serving_status`, `validate_model_serving_config`, and `serve_model` non-critical domain-error responses. Validated with `cargo test -p pumas-rpc serving --manifest-path rust/Cargo.toml`.
- 2026-05-08: Updated llama.cpp placement validation so dedicated profile launches accept user-authored model placement and context-size overrides, while router profiles remain profile-scoped and reject per-load placement overrides. Validated with `cargo test -p pumas-library serving --manifest-path rust/Cargo.toml`.
- 2026-05-08: Added an in-memory serving status update feed and bridge method, `list_serving_status_updates_since`, for backend-owned loaded/unloaded/load-failed events. Missed updates return `snapshot_required` rather than replaying durable history. Validated with `cargo test -p pumas-library serving --manifest-path rust/Cargo.toml`, `cargo test -p pumas-rpc serving --manifest-path rust/Cargo.toml`, `npm run -w frontend check:types`, and `npm run -w electron test`.
- 2026-05-08: Reconciled the facade checklist with the current dependency boundary. `serve_model` and `unserve_model` are exposed through RPC/Electron/renderer bridge, while provider orchestration remains in RPC handlers until adapter inversion can avoid a `pumas-core` -> `pumas-app-manager` dependency cycle. Route defaults remain separate: the serving dialog uses route/profile state only to prefill drafts, and `serve_model` carries an explicit config.

**Discovered Issues:**
- 2026-05-08: Provider orchestration still lives in `pumas-rpc` because `pumas-core` cannot depend on `pumas-app-manager` without a dependency cycle. This is acceptable for the current desktop serving facade, but a future adapter-inversion refactor should move provider load/unload behind a core-owned trait if non-RPC hosts need the same orchestration.

### Milestone 11: Implement Model Row/Modal Serve Vertical Slice

**Goal:** Prove the user workflow from a model row or modal through backend serving validation and status display.

**Tasks:**
- [x] Add a `Serve` action to installed model rows for eligible models.
- [x] Add or update a model modal serving tab/panel with profile/provider selection and device placement controls.
- [x] Prefill draft values from backend route/defaults while keeping draft state local until submit.
- [x] Call `validate_model_serving_config` before `serve_model` when the user asks for validation or when the load form is submitted.
- [x] Show non-critical load errors inline without closing the modal or marking the model as served.
- [x] Render loaded/served status only from backend status response or update events.
- [x] Add an unload action that calls `unserve_model`.
- [x] Keep controls semantic and accessible: named buttons, associated labels, keyboard-usable modal behavior, and no raw interactive divs.

**Verification:**
- Frontend test for opening the serve flow from a model row by accessible name.
- Frontend test for serving from the modal and rendering backend-confirmed loaded status.
- Frontend test for non-critical load error display with existing loaded-model state unchanged.
- Frontend test for provider/device controls and unsupported-option visibility.
- Typecheck and targeted lint for changed frontend files.

**Status:** Complete for the first row/modal serving workflow. Installed model rows and the model modal Serving tab expose the user-directed Serve dialog, which initializes loaded status from backend `get_serving_status`, validates before load, can unload through `unserve_model`, and keeps keyboard focus inside the dialog.

**Implementation Notes:**
- 2026-05-08: Added `ModelServeDialog` and row action wiring through `LocalModelsList`, `LocalModelRow`, `LocalModelRowActions`, and `LocalModelInstalledActions`. The dialog loads backend runtime profiles, keeps serving placement values as local form drafts, validates through `validate_model_serving_config`, then submits through `serve_model`.
- 2026-05-08: Validated the row action slice with `npm run -w frontend test:run -- LocalModelInstalledActions` and `npm run -w frontend check:types`.
- 2026-05-08: Added dialog unload support through `unserve_model`, initial focus on the profile selector, and Escape dismissal. Validated with `npm run -w frontend check:types`.
- 2026-05-08: The serving dialog now initializes its loaded/unload state from backend `get_serving_status` and still treats command responses as backend-confirmed state. Validated with `npm run -w frontend check:types`.
- 2026-05-08: Added a Serve action to the model modal Runtime Route tab by reusing `ModelServeDialog` from `ModelRuntimeRouteEditor`. Validated with `npm run -w frontend test:run -- ModelMetadataModal LocalModelInstalledActions` and `npm run -w frontend check:types`.
- 2026-05-08: Added focus trapping to the serving dialog. Validated with `npm run -w frontend check:types`.
- 2026-05-08: Renamed the model modal route tab to Serving and made `ModelServeDialog` choose the model's saved route profile before falling back to the default profile. The dialog now labels placement fields as model placement, shows the selected provider/mode, and initializes the model placement draft from the selected runtime target so users can see and override the launch settings before serving.
- 2026-05-08: Made the model serve dialog use an opaque launcher panel/backdrop, show an always-visible ready/cannot-serve reason, and accept the route editor's currently selected profile even before the route is saved. Validated with `npm run -w frontend test:run -- ModelServeDialog ModelMetadataModal LocalModelInstalledActions` and `npm run -w frontend check:types`.
- 2026-05-08: Reworked the model-modal serve flow from a nested floating dialog into an inline Serving tab page with a Back button. The serving page now hides placement fields that do not apply to the selected profile, defaults llama.cpp dedicated context to 4096, blocks non-launchable stopped profiles with an explicit reason, and keeps CPU-only profiles free of GPU layer, tensor split, and device-id controls. Validated with `npm run -w frontend test:run -- ModelServeDialog ModelMetadataModal LocalModelInstalledActions` and `npm run -w frontend check:types`.

**Discovered Issues:**
- 2026-05-08: The first serving dialog uses its own lightweight dialog shell rather than the existing metadata modal frame. Before broadening modal serving UX, reuse or extract the existing modal focus-trap behavior so serving controls meet the same keyboard expectations.
- 2026-05-08: Opening Serve from the model modal after selecting an unsaved runtime profile could fall back to the saved/default route, and the nested dialog could appear visually transparent over the metadata modal. Resolved by passing the selected profile into the dialog and using an opaque launcher modal surface with explicit blocking reasons.
- 2026-05-08: The model-modal serve UX still behaved like a second modal and exposed dedicated llama.cpp GPU controls even for CPU-only profiles. Resolved by rendering serving as an inline page inside the modal and deriving visible controls from the selected provider, mode, and device.

### Milestone 12: Wire Ollama Through User-Directed Serving

**Goal:** Make Ollama model register/load/unload usable through the provider-neutral serving facade.

**Tasks:**
- [x] Adapt existing Ollama create/load/unload/list-running client calls behind the serving provider interface.
- [x] Honor explicit user-selected profile and keep-loaded behavior.
- [x] Return truthful capability validation for device placement fields Ollama cannot control per model.
- [x] Preserve legacy `ollama_create_model`, `ollama_load_model`, and profile-aware Ollama commands.
- [x] Map Ollama API/load failures into non-critical serving errors where the command was valid but the model did not load.
- [x] Populate `ServedModelStatus` from Ollama running-model inventory.

**Verification:**
- Rust/app-manager or RPC tests for valid Ollama serving request mapping.
- Tests showing unsupported Ollama per-model placement controls return validation errors or warnings without pretending to apply them.
- Existing legacy Ollama API compatibility is covered by preserving legacy handlers and bridge methods; broader legacy regression remains under release validation.
- One manual smoke path when a local Ollama runtime and small GGUF model are available.

**Status:** Complete for the current Ollama serving facade. Provider-neutral `serve_model`/`unserve_model` now drive Ollama register/load/unload and backend status updates; unsupported per-model placement fields return non-critical validation errors; successful loads do a best-effort running-inventory read for memory size.

**Implementation Notes:**
- 2026-05-08: After successful Ollama load, `serve_model` now queries Ollama running inventory and records the reported model size as `ServedModelStatus.memory_bytes` when available. Inventory read failures are logged and do not turn a successful load into a failed serving command.

### Milestone 13: Wire llama.cpp Through User-Directed Serving

**Goal:** Make llama.cpp serving requests work through the same model-centered facade while preserving router/dedicated distinctions.

**Tasks:**
- [x] Support router-mode serving when the selected profile can load/register multiple GGUF models from one `llama-server` router endpoint.
- [x] Support dedicated-mode serving through model-bound `launch_runtime_profile(profile_id, tag?, model_id?)` while reporting `provider_endpoint` status rather than claiming a Pumas gateway.
- [x] Validate GGUF requirement, selected GPU layers, tensor split, context size, and device id before launch/load.
- [x] Return non-critical errors for missing binary, stopped profile, invalid GGUF, unsupported mode, and provider load failures.
- [x] Update generated router preset/catalog behavior only through backend-owned deterministic producers.
- [x] Keep conversion llama.cpp code out of serving lifecycle ownership.

**Verification:**
- Rust tests for llama.cpp serving validation and command/preset construction.
- Rust tests for router versus dedicated endpoint mode reporting.
- Rust tests for missing binary and invalid model non-critical error mapping.
- Existing llama.cpp runtime profile tests still pass.
- Manual `llama-server` smoke path when a local build is available.

**Status:** Complete for the current llama.cpp serving facade. Managed dedicated llama.cpp serve/unserve routes through the provider-neutral serving RPC facade and existing runtime-profile launcher with model-level placement overrides, router profiles can mark selected GGUF models as served through a running router endpoint without stopping the shared router on unload, and missing-runtime/provider-load failures return non-critical serving errors.

**Implementation Notes:**
- 2026-05-08: Implemented the first llama.cpp serving slice for managed dedicated profiles. Core serving validation now distinguishes launchable managed dedicated profiles from profiles that must already be running, and RPC serving records backend `ServedModelStatus` after a successful profile launch.
- 2026-05-08: Added provider-specific llama.cpp placement validation. The initial validator accepted explicit request placement only when it matched the selected profile and rejected context-size requests until the dedicated launch path had a typed context-size override.
- 2026-05-08: Added router-profile serving. Running llama.cpp router profiles now record selected models as served through the router endpoint, using the existing deterministic backend-generated router catalog/preset behavior from runtime profile launch. Router unload removes the served-model record without stopping the shared router process.
- 2026-05-08: Added focused RPC integration coverage proving a managed dedicated llama.cpp serve request returns a successful transport response with a non-critical load error when no local `llama-server` runtime can be launched. Validated with `cargo test -p pumas-rpc serving --manifest-path rust/Cargo.toml`.
- 2026-05-08: Added typed runtime-profile launch overrides for dedicated llama.cpp serving. `serve_model` now passes the selected model device, device id, GPU layers, tensor split, and context size into the dedicated launch path, which replaces profile launch args/env for that model-bound process without changing router semantics. Validated with `cargo test -p pumas-library llama_cpp_dedicated_overrides --manifest-path rust/Cargo.toml`, `cargo test -p pumas-library serving --manifest-path rust/Cargo.toml`, `cargo test -p pumas-rpc serving --manifest-path rust/Cargo.toml`, `npm run -w frontend check:types`, and `npm run -w frontend test:run -- ModelMetadataModal LocalModelInstalledActions`.
- 2026-05-09: Hardened managed llama.cpp router serving after live UI validation with the installed `b9082` runtime. `serve_model` now re-checks the resolved router endpoint before trusting cached lifecycle state, relaunches stopped or stale managed routers from the active installed llama.cpp version, and returns a non-critical provider-load error if the endpoint is still unreachable after launch.
- 2026-05-09: Made generated llama.cpp router presets model-type aware. GGUF embedding records now emit `embeddings = true`, and GGUF reranker records emit `reranking = true`, so the router-spawned child `llama-server` supports the endpoint required by the selected model.
- 2026-05-09: Changed router-mode `Start serving` from "record as served" to an eager llama.cpp router load. The handler posts to the router `/models/load` endpoint for the selected alias and records `ServedModelStatus::Loaded` only after llama.cpp accepts or reports that the model is already running.

**Discovered Issues:**
- 2026-05-08: The dedicated llama.cpp serving path initially used only the runtime profile's provider endpoint. Resolved the same day by adding the Pumas `/v1` gateway and reporting aggregate serving status as `pumas_gateway` when models are loaded.
- 2026-05-08: The first dedicated llama.cpp serving implementation exposed model placement fields but still launched with only the runtime profile's device/context arguments. Resolved the same day by adding model-bound launch overrides for dedicated profiles and keeping router profiles profile-scoped.
- 2026-05-09: Live app testing found a stale orphaned llama.cpp router could leave the runtime profile journal saying the profile was running while the newly launched process failed to bind the profile port. Resolved by health-checking the router endpoint before trusting cached state.
- 2026-05-09: Live app testing found `Qwen3-Embedding-0.6B-GGUF` registered in the router but failed `/v1/embeddings` with `This server does not support embeddings`. Resolved by emitting embedding/reranking model-type flags in backend-owned router presets.
- 2026-05-09: Live app testing showed the previous router `Start serving` path did not prove the model loaded; the first inference request performed the actual load. Resolved by calling llama.cpp `/models/load` during `serve_model`, which is the correct place to surface memory/device fit failures as non-critical domain errors.

### Milestone 14: Shared Endpoint/Gateway Decision And Validation

**Goal:** Resolve the "same server endpoint" requirement truthfully and safely.

**Tasks:**
- [x] Decide whether the first shipped endpoint is a Pumas gateway or provider endpoint mode.
- [x] If gateway mode is implemented, expose one loopback endpoint with OpenAI-compatible model listing and routing for served models.
- [x] Bind local-only by default and reject unsafe bind addresses unless a documented LAN mode exists.
- [x] Add bounded request/body/connection limits and safe error mapping at the transport boundary.
- [x] Ensure `/v1/models` or the equivalent endpoint reflects backend `ServedModelStatus`.
- [x] If gateway mode is deferred, show `provider_endpoint` status and document that one Pumas endpoint is not complete yet. Gateway mode was not deferred.

**Verification:**
- Backend or RPC tests for endpoint mode status.
- Gateway route tests if implemented.

**Status:** Complete for the first gateway slice. A first Pumas gateway is implemented on the RPC server for `/v1/models`, `/v1/chat/completions`, `/v1/completions`, and `/v1/embeddings`. It routes by served model id/alias from backend serving status, reports aggregate serving status as `pumas_gateway` when models are loaded, inherits the existing loopback-by-default RPC bind guard, uses the existing concurrency limit, and applies a 32 MiB request body limit.

**Implementation Notes:**
- 2026-05-08: Added OpenAI-compatible gateway routes to `pumas-rpc`. `/v1/models` lists loaded `ServedModelStatus` entries, and proxy routes forward JSON requests to the selected model's provider endpoint. If a served model has a provider alias, the gateway rewrites the outgoing `model` field to that alias. Validated with `cargo test -p pumas-rpc serving --manifest-path rust/Cargo.toml`.
- 2026-05-08: Added an explicit 32 MiB request body limit to the RPC/gateway server. This complements the existing in-flight request limit and loopback-by-default bind guard.

### Milestone 15: Documentation, Integration, And Release Validation For Serving

**Goal:** Close the user-directed serving phase with updated docs and full-path validation.

**Tasks:**
- [x] Update `rust/crates/pumas-core/src/api/README.md`.
- [x] Update `rust/crates/pumas-core/src/models/README.md`.
- [x] Add README for any new `pumas-core/src/serving/` directory.
- [x] Update `rust/crates/pumas-rpc/src/handlers/README.md`.
- [x] Update `frontend/src/types/README.md`.
- [x] Update relevant frontend component/app-panel README files.
- [x] Update `electron/src/README.md` if preload/bridge serving behavior changes.
- [x] Update `docs/contracts/desktop-rpc-methods.md`.
- [x] Run the vertical acceptance path from UI action to backend result/status.
- [x] Run frontend typecheck/build, Electron tests, Rust tests for changed crates, and release smoke where feasible. The final 2026-05-09 slice rebuilt release binaries/frontend and performed a live release-app UI acceptance test; release smoke was not repeated after the final live test because the served model/runtime was intentionally left running for user verification.

**Verification:**
- `cargo test -p pumas-library <serving filter> --manifest-path rust/Cargo.toml`.
- `cargo test -p pumas-rpc <serving filter> --manifest-path rust/Cargo.toml`.
- `npm run -w frontend check:types`.
- `npm run -w frontend test:run -- <targeted serving tests>`.
- `npm run -w electron test`.
- `npm run -w frontend build`.
- `bash launcher.sh --release-smoke` when release validation is in scope.

**Status:** Complete for the current serving wave. Core API/model/serving, RPC handler, frontend type/component, Electron, and desktop RPC contract docs reflect the serving status/update-feed contract. Automated focused Rust tests pass, release binaries/frontend build, and the live release app successfully served `embedding/qwen3/qwen3-embedding-06b-gguf` through the `Emily Lamacpp` profile from the model Serving UI.

**Implementation Notes:**
- 2026-05-08: Updated serving contract documentation and added `pumas-core/src/serving/README.md`. Corrected the stale models README claim that all DTOs use camelCase; newer runtime-profile, package-facts, and serving DTOs intentionally use snake_case.
- 2026-05-08: Validated with `cargo test -p pumas-library serving --manifest-path rust/Cargo.toml`, `cargo test -p pumas-rpc serving --manifest-path rust/Cargo.toml`, `npm run -w frontend build`, `npm run -w frontend test:run -- ModelMetadataModal LocalModelInstalledActions`, and `npm run -w electron test`.
- 2026-05-08: `bash launcher.sh --release-smoke` was attempted but did not complete because the release backend refused to start while another Pumas instance already owned `/media/jeremy/OrangeCream/Linux Software/repos/owned/ai-systems/Pumas-Library` (`pid 2940353`), then the bounded smoke window timed out.
- 2026-05-09: Rebuilt release binaries/frontend with `bash launcher.sh --build-release`, launched the release app with DevTools, opened `Qwen3-Embedding-0.6B-GGUF` from the model list, selected `Emily Lamacpp`, clicked `Start serving`, and verified the final Pumas gateway (`http://127.0.0.1:38941/v1/embeddings`) and direct llama.cpp router (`http://127.0.0.1:20617/v1/embeddings`) both returned HTTP 200 with a 1024-dimensional embedding.

## Lifecycle and Runtime Ownership Notes

The 2026-05-08 serving update was not reviewed against the external multithreading/concurrency standard by request. The notes below capture existing runtime ownership requirements and high-level lifecycle boundaries only.

- Backend starts, stops, and owns managed local runtime profile processes.
- Each profile must have one lifecycle owner and one serialized operation queue or lock.
- `RuntimeProfileService` or the equivalent backend owner owns profile config, provider adapter dispatch, route resolution, lifecycle state, endpoint health, event production, and snapshot generation.
- `ServingService` or the equivalent backend owner owns user-directed serve/unserve commands, serving validation, served-model snapshots, endpoint-mode status, and non-critical load error projection.
- Provider adapters own provider-specific process arguments, API calls, capability translation, and status parsing.
- `ProcessManager` and `ProcessLauncher` remain lower-level process helpers. They must not own profile routing policy.
- Start operations must validate profile config before spawning a process.
- Stop operations must be idempotent and remove stale PID files only for the target profile.
- Blocking process work must run outside async locks or inside explicit blocking tasks.
- Runtime status should be reported through backend snapshots/events, not inferred by frontend mutation.
- Serving status should be reported through backend snapshots/events, not inferred by successful button clicks or local row state.
- Frontend runtime/profile views should not add per-profile polling. Any required health sampling for external endpoints belongs inside the backend event producer.
- Frontend served-model views should not add per-model polling. External provider inventory refresh belongs in the backend serving/status producer.

## Public Facade Preservation

- Preserve existing singleton commands and bridge methods for compatibility.
- Add profile-aware commands append-only.
- Add serving commands append-only.
- Internally, singleton commands may map to the default profile after profile support exists.
- Do not require existing callers to pass a profile ID during the first implementation wave.
- Do not let `connection_url` become a second internal routing model. Convert it to a validated legacy/external profile boundary object before calling internal runtime/profile code.
- Do not require React callers to orchestrate provider-specific load sequences. The model row/modal should call the provider-neutral serving facade.
- If a Pumas gateway is not implemented in the first serving slice, the public facade must report provider endpoint mode instead of implying one shared Pumas endpoint exists.

## Optional Subagent Assignment

Use subagents only after contracts are frozen and work can be split without overlapping writes.

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| Frontend worker | Local runtime profile settings UI, runtime event subscription hook, and regression tests | Patch limited to frontend components/hooks/tests after API and event types are frozen | Backend contracts merged or branch exported |
| Backend worker | Rust profile config, runtime-profile service, Ollama adapter, event producer, and process lifecycle | Patch limited to Rust core/app-manager/rpc lifecycle files | Contract DTOs frozen |
| llama.cpp worker | llama.cpp provider adapter, catalog/preset generation, and provider tests | Patch limited to llama.cpp adapter files and provider-specific tests after shared contracts are frozen | Runtime profile service merged or branch exported |
| Serving contracts owner | User-directed serving DTOs, non-critical error envelope, endpoint-mode contract, and contract tests | Patch limited to Rust model DTOs, TypeScript bridge types, RPC validation schemas, and docs | Requirements frozen |
| Serving frontend worker | Model row/modal serve flow, loaded-model status panel, accessibility tests | Patch limited to frontend components/hooks/tests after serving contracts are frozen | Serving facade methods merged or mocked through typed bridge |
| Serving provider worker | Ollama and llama.cpp serving adapter integration under `ServingService` | Patch limited to backend serving/provider files and focused provider tests | Serving contracts and service boundary merged |
| Integration owner | Bridge types, preload, RPC registry, docs, final verification | Serial integration patch and build/test results | Worker patches complete |

Forbidden shared files for parallel workers unless explicitly assigned to the integration owner:
- `electron/src/rpc-method-registry.ts`
- `electron/src/preload.ts`
- `frontend/src/types/api-processes.ts`
- frontend runtime/profile event types
- frontend serving bridge and payload types
- `rust/crates/pumas-rpc/src/handlers/mod.rs`
- provider-neutral serving DTO/schema files
- persisted config schema files
- lockfiles and generated files

## Re-Plan Triggers

- The globe crash is caused by a broader shared version-manager architecture issue.
- Ollama exposes a stable upstream per-request or per-model device placement API that changes the correct design.
- llama.cpp router mode changes materially or lacks stable enough APIs for managed router profiles.
- Managed multi-instance Ollama conflicts with packaged-app permissions or platform process rules.
- Existing process manager design cannot support profile lifecycle without a larger process-management refactor.
- Profile settings require a migration of existing user config rather than append-only defaults.
- Tests reveal shared version manager changes regress ComfyUI or Torch.
- The current model-library-only event bridge cannot be generalized without a separate event transport plan.
- User-directed serving requires automatic eviction or placement decisions after all.
- Provider load APIs cannot preserve existing loaded-model state on failure, making the `loaded_models_unchanged` contract impossible for a backend.
- The first shared endpoint implementation would require a new long-lived listener with security or lifecycle behavior that exceeds the current feature slice.
- Frontend row/modal serving cannot be implemented without exceeding component size or complexity thresholds; split the UI into dedicated subcomponents before continuing.
- A supported native binding consumer requires serving APIs before desktop serving is complete.

## Recommendations

- Treat CPU/GPU selection as Pumas runtime-profile routing. This is more truthful and more maintainable than exposing unsupported per-model Ollama flags.
- For the next serving phase, treat CPU/GPU/hybrid selection as an explicit user-selected serving configuration. Runtime routes may prefill defaults, but Pumas should not choose placement automatically.
- Implement `serve_model` as the row/modal entrypoint. Do not make frontend code call a provider-specific sequence such as create, load, launch, and refresh.
- Treat load failures as non-critical domain responses when the request is valid but the selected configuration cannot be loaded.
- Make endpoint mode explicit in backend status so the UI never overstates shared-endpoint support.
- Make `profile_id` the canonical internal route key and keep raw endpoint URLs at compatibility/config boundaries only.
- Put local runtime policy in a provider-neutral backend service instead of expanding singleton process-manager methods or creating provider-specific silos.
- Implement Ollama and llama.cpp as provider adapters under that service.
- Model llama.cpp router profiles and dedicated process profiles as distinct typed modes.
- Reuse and generalize the backend-pushed event pattern rather than adding frontend per-profile polling.
- Make the crash fix the first thin vertical slice. It has a narrow blast radius and unblocks the broken UI before adding runtime profile complexity.
- Make the second implementation slice a default-profile `profile_id` path, then expand to CPU/GPU/external profiles.
- Freeze new backend contracts before parallel implementation. This follows the repo's architecture standards and prevents frontend/backend drift.

## Completion Summary

### Completed

- Plan written and validated against local Coding Standards.
- Milestone 1 automated crash-containment slice implemented and validated.
- Milestones 2 through 8 runtime-profile, Ollama routing, llama.cpp launch support, frontend settings, documentation, and release-validation slices implemented and validated for the previous runtime-profile wave.
- 2026-05-08 user-directed serving requirements added to the plan, with new milestones 9 through 15.
- 2026-05-08 continuation: added the first llama.cpp app/version-manager slice so users can reach a llama.cpp runtime page, install managed llama.cpp binaries through the existing version manager, and start llama.cpp runtime profiles from a provider-scoped runtime-profile editor instead of a shared provider dropdown.
- 2026-05-08 continuation: wired managed llama.cpp launches to the active installed llama.cpp version instead of the legacy `launcher-data/llama-cpp/build/bin/llama-server` local-build-only path.
- 2026-05-08 continuation: split `ModelServeDialog.tsx` and the runtime-profile settings editor into focused subcomponents/hooks/helpers so the serving/profile frontend surfaces satisfy the component size and complexity standards again.
- 2026-05-09 continuation: fixed llama.cpp install-page state by preserving polling while backend progress initializes, refreshing installed versions before clearing completed install UI, recognizing nested `llama-server` binaries from upstream archives, canonicalizing new installs to `bin/llama-server`, and ignoring local `llama-cpp-versions/` runtime data.
- 2026-05-09 continuation: fixed GGUF Serve startup by resolving managed dedicated llama.cpp serving through the active installed llama.cpp version manager path and by preferring a running llama.cpp target in the serving dialog when no model route or explicit profile is selected.
- 2026-05-09 continuation: directly tested the installed `b9082` llama.cpp binary and serving API. Dedicated CPU serving loaded `embedding/qwen3/qwen3-embedding-06b-gguf`; the stopped managed router profile then exposed two blockers: managed profiles with blank endpoint/port could not resolve their derived endpoint, and stopped router profiles were not launchable from `serve_model`.
- 2026-05-09 continuation: fixed model-start serving for managed llama.cpp router profiles by allowing stopped managed routers through validation, launching them on `serve_model`, resolving implicit managed endpoints from the same derived launch specs used by lifecycle startup, and hiding router per-model placement controls that are profile-owned rather than per-load settings.
- 2026-05-09 continuation: fixed the remaining apparent no-op state by keeping the Start serving button actionable during runtime-profile refreshes, so clicks always enter the handler and surface either progress, validation feedback, or backend errors instead of being swallowed by a disabled button.
- 2026-05-09 continuation: completed live release-app validation for `Emily Lamacpp` by serving `embedding/qwen3/qwen3-embedding-06b-gguf` from the model Serving UI. The slice fixed stale router lifecycle state, added model-type-aware llama.cpp router preset flags for embeddings/rerankers, and made `serve_model` eagerly load the selected router model before recording backend served-model status.

### Deviations

- 2026-05-08: The original plan treated CPU/GPU selection primarily as model-to-runtime-profile routing. The updated product requirement is stricter: users choose the device placement for each serve request, and Pumas validates/attempts that selection instead of scheduling or fitting models automatically.
- 2026-05-08: The original plan explicitly excluded building a new inference server. The updated requirement for one shared endpoint may require a Pumas gateway/facade. The revised plan allows that work but requires endpoint status to state whether gateway mode is implemented.
- 2026-05-08 continuation: Full frontend lint surfaced component-size and complexity issues in `ModelServeDialog.tsx` plus the runtime-profile editor. These were resolved in a dedicated frontend standards slice before adding more serving controls.

### Follow-Ups

- Keep the Milestone 1 manual Ollama globe smoke check in release validation even though automated crash-containment coverage is complete.
- Continue Milestone 9: user-directed serving contracts and non-critical error envelope.
- Decide during Milestone 14 whether the first shared endpoint is a Pumas gateway or an explicit provider-endpoint status mode.
- Re-check Ollama and llama.cpp upstream documentation before implementing provider-specific serving capability labels, because device-control and router behavior may change across versions.
- Keep native binding exposure out of scope until a supported consumer requires it or add binding verification in the same implementation slice.

### Verification Summary

- Read standards from `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/`.
- For the 2026-05-08 update, intentionally excluded the multithreading/concurrency standard by request.
- Inspected current Ollama version-manager, model UI, bridge, RPC, client, process-management surfaces, and existing llama.cpp references.
- 2026-05-08 continuation validation: targeted runtime-profile frontend tests, selected-app version tests, managed-app state tests, AppShell state/panel tests, frontend typecheck, frontend production build, and Rust `cargo check` for `pumas-core`, `pumas-app-manager`, and `pumas-rpc` passed.
- 2026-05-08 continuation validation: frontend standards split passed `npm run -w frontend lint`, `npm run -w frontend check:types`, `npm run -w frontend test:run -- RuntimeProfileSettingsSection ModelServeDialog`, and `npm run -w frontend build`.
- 2026-05-09 continuation validation: `npm run -w frontend test:run -- useInstallationManager`, `cargo test --manifest-path rust/crates/pumas-app-manager/Cargo.toml llama_cpp_nested_server_binary_is_complete`, `npm run -w frontend check:types`, `npm run -w frontend test:run -- InstallDialog InstallDialogContent VersionListItem`, `cargo check --manifest-path rust/crates/pumas-app-manager/Cargo.toml`, `cargo check --manifest-path rust/crates/pumas-rpc/Cargo.toml`, `npm run -w frontend lint -- --quiet`, `npm run -w frontend build`, `bash launcher.sh --build-release`, and `bash launcher.sh --release-smoke` passed.
- 2026-05-09 serving continuation validation: `npm run -w frontend test:run -- ModelServeDialog`, `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml test_serving_llama_cpp_missing_runtime_is_non_critical`, `npm run -w frontend check:types`, `npm run -w frontend lint -- --quiet`, `cargo check --manifest-path rust/crates/pumas-rpc/Cargo.toml`, `npm run -w frontend build`, `bash launcher.sh --build-release`, and `bash launcher.sh --release-smoke` passed.
- 2026-05-09 serving action validation: added click-path coverage proving Start serving calls `serve_model` with the selected llama.cpp profile/config and displays a message when the bridge action is unavailable. `npm run -w frontend test:run -- ModelServeDialog`, `npm run -w frontend check:types`, `npm run -w frontend lint -- --quiet`, `npm run -w frontend build`, `bash launcher.sh --build-release`, and `bash launcher.sh --release-smoke` passed.
- 2026-05-09 serving controls validation: Start serving remains actionable after profiles load so backend validation can surface a concrete reason, and llama.cpp router profiles now expose model device, GPU layer, tensor split, and context controls in the serving form. `npm run -w frontend test:run -- ModelServeDialog`, `npm run -w frontend check:types`, `npm run -w frontend lint -- --quiet`, `npm run -w frontend build`, `bash launcher.sh --build-release`, and `bash launcher.sh --release-smoke` passed.
- 2026-05-09 installed llama.cpp router validation: `llama-cpp-versions/b9082/llama-b9082/llama-server --version` passed, direct dedicated CPU serving loaded `embedding/qwen3/qwen3-embedding-06b-gguf`, and the rebuilt RPC server launched the stopped `runtime-1778279326877` llama.cpp router profile on a derived endpoint (`http://127.0.0.1:20617/`) via `serve_model`. The router endpoint returned the served model from `/v1/models`; the test runtime was then stopped.
- 2026-05-09 router-start validation: `npm run -w frontend test:run -- ModelServeDialog`, `npm run -w frontend check:types`, `npm run -w frontend lint -- --quiet`, `npm run -w frontend build`, `cargo test -p pumas-library serving::`, `cargo test -p pumas-library runtime_profile_service_resolves_implicit_managed_llama_cpp_endpoint`, `cargo check -p pumas-rpc`, `cargo build -p pumas-rpc --release`, `bash launcher.sh --build-release`, and `bash launcher.sh --release-smoke` passed.
- 2026-05-09 start-button validation: `npm run -w frontend test:run -- ModelServeDialog`, `npm run -w frontend check:types`, `npm run -w frontend lint -- --quiet`, `npm run -w frontend build`, `bash launcher.sh --build-release`, and `bash launcher.sh --release-smoke` passed.
- 2026-05-09 live Emily Lamacpp validation: `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml llama_cpp_router_catalog_sorts_and_writes_preset_entries`, `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml llama_cpp_router_model_load_url_normalizes_trailing_slash`, and `bash launcher.sh --build-release` passed. The rebuilt release app launched `Emily Lamacpp` on `http://127.0.0.1:20617/`, reported `embedding/qwen3/qwen3-embedding-06b-gguf` as backend `load_state = loaded`, and both `http://127.0.0.1:38941/v1/embeddings` and `http://127.0.0.1:20617/v1/embeddings` returned HTTP 200 with `embeddingLength = 1024`.

### Traceability Links

- Module README updated: `docs/plans/README.md`
- ADR added/updated: N/A
- Reason: This is an implementation plan, not a final architecture decision.
- Revisit trigger: Runtime profile config becomes a stable external contract or is consumed outside the desktop app.
