# Plan: Local Runtime Profiles and Ollama Version Manager Stability

## Objective

Fix the Ollama page crash triggered by the version-manager globe button and add a backend-owned local runtime profile architecture for model-serving providers. Ollama and llama.cpp should share the same runtime-profile, model-routing, status-event, and frontend settings architecture while keeping provider-specific process and API behavior behind adapters.

## Scope

### In Scope

- Diagnose and fix the React crash that turns the Ollama page magenta when opening installable Ollama versions.
- Preserve the shared app version-management facade used by ComfyUI, Ollama, and Torch.
- Add a backend-owned local runtime profile model for managed or external model-serving endpoints/processes.
- Add provider adapters for Ollama and llama.cpp behind the same runtime profile contract.
- Add per-model routing from Pumas library models to local runtime profiles.
- Support CPU, GPU, auto, external endpoint, and future specific-device profile modes without hard-coding behavior for a specific model.
- Add frontend controls that edit backend-owned runtime/profile state through RPC.
- Extend the existing backend-to-frontend event pattern so local runtime status updates are pushed to the frontend instead of polled per profile.
- Add tests for UI crash prevention, API contracts, persisted config, process lifecycle, and routing behavior.

### Out of Scope

- Replacing existing Ollama support with another runtime.
- Building a new inference server for Ollama-compatible APIs.
- Claiming that a single Ollama daemon can enforce native per-model CPU/GPU placement unless upstream Ollama exposes a stable, documented setting for that behavior.
- Claiming llama.cpp router mode supports every feature or hardware-isolation pattern that dedicated `llama-server` processes support.
- Removing existing singleton Ollama commands in the first pass.
- Changing model-library artifact identity or Hugging Face download behavior.

## Inputs

### Problem

The Ollama page crashes when the globe/version-manager button opens installable Ollama versions. The same area also lacks sufficient settings for managing multiple local runtime models and selecting CPU/GPU behavior per model. The current implementation treats Ollama process management as singleton-oriented while model operations accept an optional `connectionUrl`. llama.cpp-capable GGUF models already exist in the library metadata surface, but Pumas does not yet expose llama.cpp runtime profiles or provider-neutral model routing.

### Constraints

- User requested planning only before implementation.
- Backend owns persistent data and behavior-changing configuration.
- Frontend may own only transient UI state such as panel open/closed state and form edits before submit.
- Existing RPC methods and frontend bridge calls must remain compatible unless an explicit breaking change is approved.
- The feature must be model-general and cannot special-case individual model names, repositories, or quant formats.
- Ollama capability labels must be truthful: Pumas can route a model to a CPU/GPU-profiled runtime, but should not present unsupported upstream Ollama behavior as a native per-model guarantee.
- llama.cpp capability labels must distinguish router profiles from dedicated process profiles because their lifecycle, isolation, and model-loading behavior differ.

### Assumptions

- The magenta page indicates an uncaught React render/runtime error in the version manager path.
- The first acceptable fix is a small vertical slice: reproduce the crash, stabilize the version-manager rendering path, and add a regression test.
- Per-model CPU/GPU selection should be implemented as Pumas model-to-runtime-profile routing.
- Managed runtime profiles may need separate Ollama processes on separate ports because process environment variables are process-wide.
- Managed llama.cpp profiles may use either router mode or dedicated `llama-server -m <model>` processes depending on the isolation and scheduling needed.
- `profile_id` is the canonical internal address for local runtime operations.
- Existing optional `connectionUrl` arguments in Ollama model RPC methods remain a legacy boundary compatibility path, but internal routing resolves through validated runtime profiles.

### Dependencies

- `frontend/src/components/app-panels/OllamaPanel.tsx`
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
- `rust/crates/pumas-rpc/src/handlers/mod.rs`
- `rust/crates/pumas-rpc/src/wrapper.rs`
- `rust/crates/pumas-app-manager/src/ollama_client.rs`
- `rust/crates/pumas-core/src/process/launcher.rs`
- `rust/crates/pumas-core/src/process/manager.rs`
- `rust/crates/pumas-core/src/api/process.rs`
- `rust/crates/pumas-core/src/api/state_process.rs`
- `rust/crates/pumas-core/src/api/state_runtime.rs`
- `rust/crates/pumas-core/src/models/`
- `rust/crates/pumas-core/src/conversion/llama_cpp.rs`
- `docs/architecture/MODEL_LIBRARY_ARCHITECTURE.md`
- `docs/contracts/desktop-rpc-methods.md`
- Coding standards under `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/`

### Affected Structured Contracts

- Electron bridge API types in `frontend/src/types/api-processes.ts` and related aggregate exports.
- Preload bridge methods in `electron/src/preload.ts`.
- RPC method registry and request-schema validation in `electron/src/rpc-method-registry.ts`.
- Rust RPC dispatch methods in `rust/crates/pumas-rpc/src/handlers/mod.rs`.
- Provider-specific runtime RPC response shapes, including existing Ollama responses and new llama.cpp runtime responses.
- Process status response shape if profile-level status is exposed in global status.
- Backend event stream payloads for runtime/profile status notifications.
- Electron bridge subscription APIs for runtime/profile events.
- Persisted runtime profile and model route config schema.

### Affected Persisted Artifacts

- New backend-owned local runtime profile config.
- New backend-owned model-to-runtime-profile route config.
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

### Frontend Blast Radius

- Existing `OllamaModelSection.tsx` polls one `connectionUrl` every 10 seconds while running. Multiple local runtime profiles should not multiply frontend polling. The frontend should subscribe once to backend-pushed runtime/profile events and refresh a backend-owned snapshot when notified.
- Existing library GGUF rows assume one running Ollama endpoint. Per-model routing means row actions must resolve the assigned provider/profile before create/register/load/unload.
- Settings UI should avoid generic text-only controls and use existing UI primitives, semantic buttons, labels, selects, toggles, segmented controls, and accessible names.

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

### Iteration 6: Concurrency Standards

- Result: Compliant if profile lifecycle state is protected under one owner and process start/stop tasks are serialized per profile.
- Adjustment: Profile start/stop must prevent overlapping operations, observe task errors, avoid holding locks across blocking process work, and cleanly remove profile PID files on shutdown.
- Adjustment: Backend runtime/profile event production may internally sample external endpoint health, but sampling must be owned by the backend service and surfaced through one subscription/snapshot contract.

## Definition of Done

- Clicking the Ollama globe/version-manager button does not crash the page.
- The version-manager failure path is localized to the version manager area when bad data or an unexpected error occurs.
- A backend-owned runtime profile config exists and persists across app restart.
- Users can define or edit local runtime profiles for provider, default/auto, CPU, GPU, and external endpoints.
- Users can assign a model to a runtime profile.
- Ollama profiles and llama.cpp profiles use the same route/status/settings facade.
- Model create/load/unload/list operations use the assigned profile endpoint.
- Frontend runtime status updates arrive through a backend-owned event/snapshot path, not per-profile component polling.
- Existing singleton Ollama commands and current frontend flows continue to work.
- Tests cover the version crash, runtime profile config, process lifecycle, profile routing, frontend settings controls, and cleanup of any timers/polling.
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
- [ ] Add profile-aware process lifecycle through the runtime-profile service.
- [x] Generate profile-specific ports, health URLs, PID files, and log files.
- [x] Apply profile environment variables, including CPU/GPU visibility settings where platform-supported.
- [ ] Serialize start/stop operations per profile.
- [ ] Report profile status, last error, endpoint URL, and running state through the snapshot/event path.
- [ ] Keep broad singleton process cleanup separate from profile-scoped stop operations.
- [ ] Preserve app-level aggregate status for existing UI.

**Verification:**
- Rust tests for start/stop state transitions with fake process launchers where possible.
- Rust tests for port and PID path derivation.
- Rust tests for overlapping start/stop prevention.
- Rust tests that profile-scoped stop does not target unrelated Ollama processes.
- Existing process tests still pass.

**Status:** Started. Profile-scoped launch spec and environment derivation are implemented and validated; process spawning, serialized operations, and profile-scoped stop behavior remain.

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

### Milestone 5: Route Ollama Model Operations Through Profiles

**Goal:** Make Ollama model operations use the backend-owned model route rather than a single page-level endpoint.

**Tasks:**
- [ ] Add model-route resolution for create/load/unload/delete/list actions.
- [ ] Add profile-aware model operations that accept `profile_id`.
- [ ] Keep `connection_url` accepted only as legacy compatibility input and convert it at the boundary.
- [ ] Split register/create from load, or make auto-load an explicit per-route setting.
- [ ] Return clear errors when a route points to a stopped or unhealthy profile.
- [ ] Keep external endpoint profiles supported.

**Verification:**
- Rust RPC tests for model route resolution.
- Tests showing `connection_url` compatibility still works.
- Tests showing model-specific profile routing chooses the expected endpoint.

**Status:** Not started.

### Milestone 6: Add llama.cpp Runtime Adapter

**Goal:** Add llama.cpp as a second provider under the same runtime profile architecture.

**Tasks:**
- [ ] Add a llama.cpp provider adapter behind the runtime-profile service.
- [ ] Support managed router profiles using `llama-server` router mode.
- [ ] Support managed dedicated process profiles using `llama-server -m <model>`.
- [ ] Generate deterministic model catalog or preset data for router profiles from Pumas library GGUF artifacts.
- [ ] Represent llama.cpp CPU/GPU settings as typed profile/provider settings, including GPU layers/device/split controls where supported.
- [ ] Report llama.cpp profile status through the shared runtime snapshot/event path.
- [ ] Keep provider-specific llama.cpp capabilities behind the provider adapter unless exposed through generic runtime profile fields.

**Verification:**
- Rust tests for llama.cpp provider-mode parsing and config serialization.
- Rust tests for deterministic catalog/preset generation.
- Rust tests for router profile endpoint/status handling with fake server responses where possible.
- Rust tests for dedicated process command construction and profile-scoped PID/log paths.
- Existing Ollama runtime profile tests still pass.

**Status:** Not started.

### Milestone 7: Add Frontend Local Runtime Profile Settings

**Goal:** Expose runtime profiles and per-model routing through accessible, backend-confirmed UI.

**Tasks:**
- [ ] Add a frontend runtime/profile subscription hook that follows the existing model-library update subscription pattern. The bridge event source exists as `onRuntimeProfileUpdate`; the React hook still needs to consume it.
- [ ] Add snapshot refresh on runtime/profile events.
- [ ] Add a local runtime profile settings section.
- [ ] Add profile create/edit controls for provider, provider mode, name, endpoint, port, scheduler settings, and managed/external status.
- [ ] Add per-model route controls for assigning a model to auto/Ollama/llama.cpp/CPU/GPU/external profiles.
- [ ] Show profile status and model running state from backend-confirmed responses.
- [ ] Show provider-specific advanced controls only when the selected provider/mode supports them.
- [ ] Avoid optimistic persistence; refresh or accept backend-pushed state after save.
- [ ] Remove or bypass component-owned Ollama state polling for profile-backed views.

**Verification:**
- Frontend tests for rendering profile settings and saving model routes.
- Frontend tests for provider/mode-specific controls and hidden unsupported options.
- Accessibility-focused tests using named buttons/fields.
- Subscription cleanup tests for runtime/profile events.
- Typecheck and lint.

**Status:** Not started.

### Milestone 8: Integration, Documentation, and Release Validation

**Goal:** Validate the full user flow and update durable module documentation.

**Tasks:**
- [ ] Update relevant module READMEs for new RPC/profile/process contracts.
- [ ] Add or update contract docs if profile config becomes a durable interface.
- [ ] Test default singleton Ollama flow still works.
- [ ] Test llama.cpp router and dedicated profile flows with fake process/server coverage and one manual smoke path when binaries are available.
- [ ] Test CPU and GPU profile assignment behavior on available hardware or documented fake-process coverage.
- [ ] Build frontend and release binaries.

**Verification:**
- Full targeted frontend test set.
- Relevant Rust crate tests.
- Electron validation/typecheck.
- Frontend build.
- Release build/smoke command.

**Status:** Not started.

## Lifecycle and Concurrency Notes

- Backend starts, stops, and owns managed local runtime profile processes.
- Each profile must have one lifecycle owner and one serialized operation queue or lock.
- `RuntimeProfileService` or the equivalent backend owner owns profile config, provider adapter dispatch, route resolution, lifecycle state, endpoint health, event production, and snapshot generation.
- Provider adapters own provider-specific process arguments, API calls, capability translation, and status parsing.
- `ProcessManager` and `ProcessLauncher` remain lower-level process helpers. They must not own profile routing policy.
- Start operations must validate profile config before spawning a process.
- Stop operations must be idempotent and remove stale PID files only for the target profile.
- Blocking process work must run outside async locks or inside explicit blocking tasks.
- Runtime status should be reported through backend snapshots/events, not inferred by frontend mutation.
- Frontend runtime/profile views should not add per-profile polling. Any required health sampling for external endpoints belongs inside the backend event producer.

## Public Facade Preservation

- Preserve existing singleton commands and bridge methods for compatibility.
- Add profile-aware commands append-only.
- Internally, singleton commands may map to the default profile after profile support exists.
- Do not require existing callers to pass a profile ID during the first implementation wave.
- Do not let `connection_url` become a second internal routing model. Convert it to a validated legacy/external profile boundary object before calling internal runtime/profile code.

## Optional Subagent Assignment

Use subagents only after contracts are frozen and work can be split without overlapping writes.

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| Frontend worker | Local runtime profile settings UI, runtime event subscription hook, and regression tests | Patch limited to frontend components/hooks/tests after API and event types are frozen | Backend contracts merged or branch exported |
| Backend worker | Rust profile config, runtime-profile service, Ollama adapter, event producer, and process lifecycle | Patch limited to Rust core/app-manager/rpc lifecycle files | Contract DTOs frozen |
| llama.cpp worker | llama.cpp provider adapter, catalog/preset generation, and provider tests | Patch limited to llama.cpp adapter files and provider-specific tests after shared contracts are frozen | Runtime profile service merged or branch exported |
| Integration owner | Bridge types, preload, RPC registry, docs, final verification | Serial integration patch and build/test results | Worker patches complete |

Forbidden shared files for parallel workers unless explicitly assigned to the integration owner:
- `electron/src/rpc-method-registry.ts`
- `electron/src/preload.ts`
- `frontend/src/types/api-processes.ts`
- frontend runtime/profile event types
- `rust/crates/pumas-rpc/src/handlers/mod.rs`
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

## Recommendations

- Treat CPU/GPU selection as Pumas runtime-profile routing. This is more truthful and more maintainable than exposing unsupported per-model Ollama flags.
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
- Milestone 2 contract DTO/bridge slice implemented and validated.
- Milestone 2 backend persistence slice implemented and validated.

### Deviations

- None.

### Follow-Ups

- Keep the Milestone 1 manual Ollama globe smoke check in release validation even though automated crash-containment coverage is complete.
- Implement Milestone 3 as the next thin slice: default profile resolution for one profile-aware Ollama model operation while preserving legacy `connection_url` behavior.
- Implement runtime-profile pushed updates as a separate event-transport slice because the existing SSE/preload path is currently hardcoded to model-library notifications.
- Re-check Ollama and llama.cpp upstream documentation before implementing hardware mode labels, because device-control and router behavior may change across versions.

### Verification Summary

- Read standards from `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/`.
- Inspected current Ollama version-manager, model UI, bridge, RPC, client, process-management surfaces, and existing llama.cpp references.
- No implementation tests were run because this is a planning-only change.

### Traceability Links

- Module README updated: `docs/plans/README.md`
- ADR added/updated: N/A
- Reason: This is an implementation plan, not a final architecture decision.
- Revisit trigger: Runtime profile config becomes a stable external contract or is consumed outside the desktop app.
