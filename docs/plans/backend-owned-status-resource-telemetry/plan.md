# Backend-Owned Status And Resource Telemetry Plan

## Objective
Reduce idle Pumas Library CPU usage to near-zero by replacing global frontend
status/resource polling with a backend-owned cached telemetry service and
push-based updates through the existing Rust RPC, Electron bridge, and React
subscription pattern.

The implementation must preserve backend ownership of status/resource data,
avoid expensive process and system scans in request paths, and keep remaining
polling narrowly owned, low-frequency, lifecycle-managed, and justified.

## Scope

### In Scope
- Add a backend telemetry owner for launcher status, system resources, network
  status, and model-library loaded state.
- Make `get_status` and `get_system_resources` return cached snapshots instead
  of causing expensive refresh work per request.
- Add a status/resource update stream that follows the existing
  `/events/model-library-updates` and `/events/runtime-profile-updates` bridge
  conventions.
- Add Electron bridge/preload forwarding for the new status/resource update
  stream.
- Replace `frontend/src/hooks/useStatus.ts` global interval polling with an
  initial cached snapshot load plus push subscription.
- Replace or retire `frontend/src/hooks/useNetworkStatus.ts` polling by serving
  the Model Manager network indicator from the same backend-owned telemetry
  stream.
- Gate expensive per-process resource aggregation to app/process views that
  actually display those details.
- Add regression tests for timer cleanup, stream handoff, backend task
  lifecycle, and request-path cache behavior.
- Record remaining polling surfaces and whether each is acceptable, temporary,
  or part of later remediation.

### Out Of Scope
- Redesigning all app-panel metrics, plugin status, installation progress, or
  model-download progress streams in this plan.
- Changing model-library or runtime-profile event payloads except where the new
  telemetry stream reuses their bridge pattern.
- Host-level tracing setup changes such as lowering `perf_event_paranoid` or
  changing `ptrace_scope`.
- UI redesign unrelated to status/resource display.
- Preserving the current frontend status polling behavior as a compatibility
  surface.

## Inputs
- Runtime observation: with the GUI open, `pumas-rpc` was observed using about
  162% CPU with roughly 156 threads and about 3,800 file descriptors.
- File-descriptor sampling showed many `/proc/<pid>/stat` and
  `/proc/<pid>/task/<tid>/stat` handles, consistent with repeated process table
  and resource refresh work.
- `frontend/src/hooks/useStatus.ts` polls every second and invokes
  `get_status`, `get_system_resources`, `get_network_status`, and
  `get_library_status` on staggered timers.
- `rust/crates/pumas-core/src/api/system.rs::get_system_resources` creates a
  fresh `sysinfo::System::new_all()` and calls `refresh_all()` in a blocking
  task for each resource request.
- `rust/crates/pumas-core/src/system/resources.rs` already has a
  `ResourceTracker`, but `get_system_resources` bypasses its cache for CPU, RAM,
  and disk.
- `rust/crates/pumas-rpc/src/handlers/mod.rs` already exposes SSE streams for
  model-library and runtime-profile updates.
- `electron/src/python-bridge.ts`, `electron/src/main.ts`, and
  `electron/src/preload.ts` already forward backend SSE notifications into
  renderer subscriptions.

## Design

### Backend Ownership
The backend remains the source of truth for status and resources. The frontend
may request the current cached snapshot during startup and after reconnect, but
it must not own a global polling loop for backend-owned state.

Add a core telemetry service, tentatively named `StatusTelemetryService`, owned
by the primary Pumas instance. The service owns:
- current status/resource snapshot cache,
- update cursor or monotonically increasing revision,
- subscriber broadcast/watch channel,
- a lifecycle-managed sampler task,
- configurable sampling cadence and change thresholds,
- shutdown cancellation.

The service should live in core because it owns data and sampling policy. RPC
and Electron are transport adapters.

### Snapshot Shape
Create a typed telemetry snapshot DTO that can be serialized for RPC/Electron:
- `cursor` or `revision`,
- launcher status fields currently consumed by `useStatus`,
- system resource fields currently returned by `get_system_resources`,
- network availability,
- model-library loaded state,
- `sampled_at` timestamp,
- `source_state` such as `warming`, `ready`, or `degraded`.

The first implementation may reuse current DTO fields at the API boundary, but
the backend-owned snapshot should be typed internally instead of assembled by a
React hook from unrelated RPC calls.

### Sampling Policy
The sampler should:
- publish an initial snapshot shortly after startup,
- refresh cheap state when lifecycle events occur where hooks already exist,
- sample system resources at a low cadence only while the GUI or another client
  has an active telemetry subscription,
- allow an explicit cached snapshot read without forcing a refresh,
- avoid `System::new_all().refresh_all()` in request handlers,
- avoid full process refresh for the header resource display,
- use `ResourceTracker` or a narrower replacement with specific refresh calls
  for CPU, memory, disk, and GPU summary data,
- reserve per-process refresh and descendant aggregation for app/process panels.

Any remaining internal sampling loop must be backend-owned, tracked, cancellable,
and documented. That is not the same anti-pattern as frontend polling because
the backend owns the measured state and can centralize cadence, cache, and
subscriber gating.

### Push Contract
Add a backend SSE endpoint, tentatively `/events/status-telemetry-updates`, with
the same broad shape as the existing model-library stream:
- subscribe accepts an optional cursor/revision,
- the stream emits a snapshot immediately when the cursor is missing, stale, or
  older than the current snapshot,
- after the handoff, the stream emits changed snapshots or compact update
  notifications,
- disconnect stops sending and releases subscriber state,
- keep-alive remains transport-level only and does not trigger backend sampling
  work by itself.

The frontend flow becomes:
1. Load the current cached snapshot with one RPC call.
2. Subscribe through Electron to status telemetry updates.
3. Apply pushed snapshots to React state.
4. On reconnect, recover from the last cursor/revision.

### Existing Runtime-Profile Stream Note
`/events/runtime-profile-updates` currently uses a 1s loop to call
`list_runtime_profile_updates_since`. This plan does not have to fix that
stream before the status/resource CPU issue, but it must record it as a
standards debt item because it is not as canonical as the model-library
subscriber stream. If implementation touches this area, prefer converting it to
the same subscriber handoff model rather than copying the polling pattern.

## Current Anti-Patterns Found
- `frontend/src/hooks/useStatus.ts` runs a global high-frequency polling loop
  for backend-owned state.
- The status hook fans one UI poll into several backend calls, mixing launcher
  status, resources, network status, and model-library readiness.
- `get_system_resources` performs fresh full `sysinfo` initialization and
  `refresh_all()` per request.
- Resource data has two competing paths: direct request-path sysinfo refresh and
  the existing cached `ResourceTracker`.
- `spawn_blocking` hides expensive request-path work but does not remove CPU
  cost, queue pressure, or repeated `/proc` scanning.
- `get_status` does extra enrichment work such as version path sync,
  dependency checks, patch state, and shortcut state when invoked by the
  frontend status poll.
- Some existing frontend polling surfaces remain in app-specific hooks and must
  be classified before broader cleanup.

## Blast Radius

### Rust Core
- `rust/crates/pumas-core/src/api/system.rs`: convert status/resource reads to
  cached telemetry snapshots and remove request-path full refresh work.
- `rust/crates/pumas-core/src/api/state.rs`: add primary-state ownership for the
  telemetry service if that is the least invasive lifecycle owner.
- `rust/crates/pumas-core/src/api/builder.rs`: initialize telemetry service
  after process manager and model library dependencies are available.
- `rust/crates/pumas-core/src/system/resources.rs`: narrow refresh behavior and
  make cache semantics explicit; avoid using full process refresh for header
  resource snapshots.
- New candidate module `rust/crates/pumas-core/src/system/telemetry.rs`: own
  snapshot DTOs, subscriber channel, sampler task, and tests.
- `rust/crates/pumas-core/src/process/manager.rs`: gate per-process resource
  aggregation so it is not part of global header telemetry.

### Rust RPC
- `rust/crates/pumas-rpc/src/server.rs`: add the telemetry SSE route.
- `rust/crates/pumas-rpc/src/handlers/mod.rs`: add stream startup, handoff, SSE
  serialization, and error handling following model-library update conventions.
- `rust/crates/pumas-rpc/src/handlers/status.rs`: return cached snapshots from
  existing status/resource RPC methods or add one explicit cached telemetry
  method if that gives a cleaner boundary.
- `rust/crates/pumas-rpc/src/wrapper.rs`: update method wrapping only if a new
  RPC method is added.

### Electron
- `electron/src/python-bridge.ts`: add stream open/close/reconnect handling for
  status telemetry using the existing timer-controller pattern.
- `electron/src/main.ts`: forward telemetry notifications to the renderer.
- `electron/src/preload.ts`: expose `onStatusTelemetryUpdate` with validation
  and deterministic unsubscribe.
- `electron/src/rpc-method-registry.ts`: register any new cached snapshot RPC
  method.

### Frontend
- `frontend/src/hooks/useStatus.ts`: replace polling with snapshot load and
  push subscription.
- `frontend/src/hooks/useStatus.test.tsx`: replace polling expectations with
  subscription, reconnect, cleanup, and snapshot tests.
- `frontend/src/hooks/useNetworkStatus.ts` and
  `frontend/src/hooks/useNetworkStatus.test.ts`: migrate network indicator
  reads to telemetry state or retire the hook if `useStatus` becomes the single
  frontend owner.
- `frontend/src/types/api-bridge-runtime.ts`,
  `frontend/src/types/api-electron.ts`, and related API DTOs: add typed
  telemetry subscription and snapshot contracts.
- Header/status components that consume `useStatus`: verify they continue to
  render resources, loading state, network state, and library state correctly.
- `frontend/src/components/ModelManager.tsx`: verify network/rate-limit display
  still renders after `useNetworkStatus` migration.

### Documentation And Tests
- This plan file and `docs/plans/README.md`.
- Rust unit/integration tests for telemetry cache, subscriber handoff, and task
  shutdown.
- Electron tests for stream parser/reconnect/unsubscribe if an existing test
  harness covers bridge code.
- Frontend tests for absence of global status polling and correct event-driven
  updates.

## Milestones

### Milestone 0 - Baseline And Plan Package
- Record current CPU/FD evidence and code-path findings in this plan.
- Add this plan to `docs/plans/README.md`.
- Classify current polling surfaces found by `rg`.

**Verification:**
- `rg -n "setInterval|pollInterval|get_system_resources|System::new_all\\(|refresh_all\\(" frontend/src electron/src rust/crates/pumas-core/src rust/crates/pumas-rpc/src`
- Confirm the plan contains objective, scope, milestones, verification,
  ownership/lifecycle notes, risks, re-plan triggers, and completion criteria.

**Current Classification From Standards Pass:**
- `frontend/src/hooks/useStatus.ts`: global backend-owned status/resource
  polling; must be removed by this plan.
- `frontend/src/hooks/useNetworkStatus.ts`: global network-status polling for
  Model Manager; must be folded into telemetry or retired by this plan because
  network state is part of the same backend-owned status surface.
- `frontend/src/hooks/useModelDownloads.ts` and
  `frontend/src/hooks/useActiveModelDownload.ts`: download progress polling;
  related to transfer indicators but outside this status/resource CPU slice.
  Keep classified as follow-up unless post-Milestone 4 idle CPU remains high.
- `frontend/src/hooks/useInstallationProgress.ts` and
  `frontend/src/hooks/useInstallationManager.ts`: installation workflow
  polling; local workflow-owned polling and out of scope unless it runs while no
  installation workflow is active.
- `frontend/src/components/app-panels/sections/StatsSection.tsx`,
  `ModelSelectorSection.tsx`, and `TorchModelSlotsSection.tsx`: app-panel
  polling while the relevant app is running; acceptable only if mounted,
  lifecycle-cleaned, and low-frequency. This plan must avoid routing global
  header telemetry through these panel paths.
- `frontend/src/hooks/usePlugins.ts`: plugin app-status polling; out of scope
  for the CPU issue but should be revisited if app runtime status becomes part
  of a broader canonical process-status stream.
- `rust/crates/pumas-rpc/src/handlers/mod.rs` runtime-profile SSE polling:
  backend-owned but not canonical subscriber handoff; record as standards debt
  and do not copy for the new telemetry stream.

### Milestone 1 - Cached Backend Snapshot Vertical Slice
- Add backend-owned cached telemetry snapshot state.
- Convert `get_system_resources` to read the cached resource snapshot.
- Keep the frontend unchanged for this slice so the vertical change is small,
  but repeated resource RPC calls must no longer trigger full sysinfo refresh.
- Add tests that repeated cached resource reads do not require a fresh sampler
  run and do not perform request-path full refresh work.

**Verification:**
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library telemetry`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library system_resources`
- Code review confirms `System::new_all().refresh_all()` is not in the
  `get_system_resources` request path.

**Implementation Notes:**
- 2026-05-06: First backend slice introduced a primary-owned
  `ResourceTracker` for system resource reads and split summary refresh from
  process-table refresh in `ResourceTracker`.
- 2026-05-06: Standards debt found and resolved during the slice: conversion
  from `SystemResourceSnapshot` to `SystemResourcesResponse` was initially
  duplicated between public API and primary-state runtime helpers, then moved to
  a shared private API response helper before commit.
- 2026-05-06 verification for first backend slice:
  `cargo check --manifest-path rust/Cargo.toml -p pumas-library`,
  `cargo test --manifest-path rust/Cargo.toml -p pumas-library system_resources`,
  `cargo test --manifest-path rust/Cargo.toml -p pumas-library resource_snapshot`,
  and `git diff --check` passed. Code review scan confirmed
  `System::new_all().refresh_all()` no longer appears in the system resource
  request paths touched by this slice.

### Milestone 2 - Backend Subscriber Stream
- Add a telemetry update subscriber in core.
- Add an RPC SSE route with cursor/revision handoff.
- Emit an initial snapshot on subscribe when needed, then emit live updates from
  the backend-owned subscriber channel.
- Ensure subscriber connection and sampler lifecycle are independent: transport
  keep-alives must not cause sampling work.

**Verification:**
- Rust test proves subscribe-with-current-cursor does not miss an update during
  handoff.
- Rust test proves stale/missing cursor receives a snapshot.
- Rust test proves dropping the stream releases subscriber state.
- `cargo test --manifest-path rust/Cargo.toml -p pumas-rpc status_telemetry`

**Implementation Notes:**
- 2026-05-06: Added core `StatusTelemetrySnapshot` and
  `StatusTelemetryUpdateNotification` DTOs, a primary-owned telemetry service,
  a subscriber-gated sampler task, cached snapshot RPC access, and
  `/events/status-telemetry-updates` SSE.
- 2026-05-06 verification for backend stream slice:
  `cargo check --manifest-path rust/Cargo.toml -p pumas-library`,
  `cargo check --manifest-path rust/Cargo.toml -p pumas-rpc`,
  `cargo test --manifest-path rust/Cargo.toml -p pumas-library status_telemetry`,
  `cargo test --manifest-path rust/Cargo.toml -p pumas-rpc status_telemetry`,
  and `git diff --check` passed.
- 2026-05-06 issue found and resolved: telemetry snapshots are core-owned and
  initially used core `status_response`, while the existing `get_status` RPC
  handler adds ComfyUI version, dependency, patch, and shortcut enrichment. The
  RPC enrichment was moved behind a reusable helper and is now applied to
  telemetry snapshots before they are returned by RPC or emitted over SSE.

### Milestone 3 - Electron Bridge Subscription
- Add Electron bridge stream handling for telemetry updates.
- Add main-process forwarding and preload `onStatusTelemetryUpdate`.
- Reuse existing bridge reconnect/timer-controller patterns.
- Ensure unsubscribe closes the stream and clears reconnect timers.

**Verification:**
- Electron tests validate stream parsing, reconnect cursor propagation, and
  unsubscribe cleanup where the existing harness supports it.
- `npm run -w electron validate`
- Code review confirms no unowned renderer timer is introduced.

**Implementation Notes:**
- 2026-05-06: Added Electron status telemetry SSE parsing, bridge lifecycle,
  main-process forwarding, preload subscription API, cached snapshot RPC
  registry entries, and frontend TypeScript DTOs.
- 2026-05-06: The new status telemetry forwarder is subscriber-aware: preload
  subscribe/unsubscribe IPC starts the bridge stream for the first renderer
  subscriber and stops it when the last subscriber unsubscribes.
- 2026-05-06 verification:
  `npm run -w electron test`, `npm run -w frontend check:types`, and
  `git diff --check` passed.

### Milestone 4 - Frontend Status Hook Migration
- Replace `useStatus` polling with initial cached snapshot plus
  `onStatusTelemetryUpdate`.
- Replace `useNetworkStatus` polling or route it to the same telemetry state so
  network indicators do not maintain an independent global interval.
- Preserve the hook return shape for components where practical, but remove the
  `pollInterval` behavior from production code.
- Keep the initial API availability wait bounded and lifecycle-cleaned.
- Remove staged `lastResourcesFetch`, `lastNetworkFetch`, and `lastLibraryFetch`
  timers because the backend snapshot owns cadence.

**Verification:**
- `npm run -w frontend test:run -- useStatus`
- `npm run -w frontend check:types`
- Test confirms no interval is created for normal status/resource refresh.
- Test confirms unsubscribe and any startup timeout are cleared on unmount.
- Model Manager network indicator tests pass without `useNetworkStatus` owning a
  polling interval.

**Implementation Notes:**
- 2026-05-06: Replaced `useStatus` polling with initial
  `get_status_telemetry_snapshot` load plus `onStatusTelemetryUpdate`
  subscription while preserving the hook return shape and queued `refetch`
  behavior.
- 2026-05-06: Replaced `useNetworkStatus` polling with the same telemetry
  snapshot/subscription path and normalized `success_rate` from ratio to percent
  when needed.
- 2026-05-06 verification:
  `npm run -w frontend test:run -- useStatus useNetworkStatus`,
  `npm run -w frontend check:types`, `git diff --check`, and a source scan for
  status/resource polling calls in both hooks passed.

### Milestone 5 - Expensive Resource Work Gating
- Audit app/process panels that still need per-process resource details.
- Keep process aggregation behind explicit app/process data calls and avoid
  using it for global header telemetry.
- If app panels still poll, document each as local panel-owned polling or add a
  follow-up plan when the work is broader than this bug.
- Add a lightweight runtime verification script or manual checklist for idle
  CPU and FD counts after startup settle.

**Verification:**
- `rg -n "setInterval|pollInterval" frontend/src`
- `rg -n "refresh_processes_specifics|refresh_all\\(" rust/crates/pumas-core/src`
- Manual runtime check after release build: with the GUI idle for at least 30s,
  `pumas-rpc` CPU should average below 5% on the developer workstation and file
  descriptors should not grow into thousands from `/proc` stat scans.

### Milestone 6 - Release Build And Cross-Layer Verification
- Run Rust checks/tests for changed crates.
- Run frontend/electron validation.
- Build release binaries and frontend.
- Record any deviations or unresolved polling surfaces in this plan before
  final completion.

**Verification:**
- `cargo check --manifest-path rust/Cargo.toml`
- Focused Rust tests from Milestones 1 and 2.
- `npm run -w frontend test:run`
- `npm run -w frontend build`
- `npm run -w electron validate`
- `bash launcher.sh --build-release`

## Ownership And Lifecycle
- `StatusTelemetryService` is owned by the primary backend instance, not by the
  frontend, Electron bridge, or RPC handler.
- The sampler task is started by the backend composition root after dependencies
  are initialized and is shut down with the owning `PumasApi`/primary state.
- The sampler must use a cancellation token or owned task handle and must log
  cancellation/panic at the lifecycle owner.
- Backend subscriber state must be released when SSE clients disconnect.
- Frontend subscriptions must return unsubscribe functions and clear any local
  startup timeout on unmount.
- Remaining polling must be local to the smallest UI owner, justified in code or
  plan notes, and deterministically stopped.

## API Consumer Contract
- Existing frontend consumers of `useStatus` should continue receiving status,
  system resources, network availability, model-library loaded state, loading
  state, and a manual refresh entry point.
- New transport-facing telemetry stream is local to the Pumas GUI bridge unless
  later promoted as a public client API.
- Existing `get_status` and `get_system_resources` RPC methods may remain as
  cached snapshot reads for explicit refresh/debug use.
- API clients should not rely on RPC calls to force immediate resource
  resampling; sampling cadence is backend-owned.
- If a new method such as `get_status_telemetry_snapshot` is added, old frontend
  use of multiple status/resource RPC methods should be removed rather than
  kept as a parallel production path.

## Risks And Mitigations
- **Risk:** Resource values appear stale.
  **Mitigation:** emit an immediate current snapshot on subscribe and use a
  bounded low-frequency sampler while subscribers are active.
- **Risk:** Snapshot/subscription handoff misses an update.
  **Mitigation:** use cursor/revision handoff and test subscribe-since behavior.
- **Risk:** Backend task leaks after GUI shutdown.
  **Mitigation:** lifecycle-owned task handles, cancellation token, and shutdown
  tests.
- **Risk:** Hidden polling keeps CPU high.
  **Mitigation:** audit all `setInterval`, `pollInterval`, and resource refresh
  paths before completion.
- **Risk:** Full process refresh remains necessary for some app panels.
  **Mitigation:** keep that work behind explicit panel/process calls and record
  follow-up stream work separately if needed.
- **Risk:** The runtime-profile SSE polling pattern is copied.
  **Mitigation:** base new telemetry stream on model-library subscriber handoff,
  and record runtime-profile stream conversion as standards debt if not fixed in
  this plan.

## Re-Plan Triggers
- Cached `get_system_resources` cannot be implemented without changing public
  resource DTOs.
- The backend lacks a viable lifecycle owner for a telemetry sampler without a
  larger `PrimaryState`/runtime task refactor.
- Subscriber handoff cannot be made race-free with the current SSE handler
  structure.
- Frontend status consumers require synchronous refresh semantics that conflict
  with backend-owned sampling.
- Idle CPU remains above the acceptance target after the status/resource poll
  path is removed.
- Additional high-CPU source is discovered that dominates the measured idle CPU
  after this plan's Milestone 4.

## Completion Criteria
- The global frontend status/resource polling loop is removed from production
  status UI.
- The separate frontend network-status polling loop is removed or migrated to
  the same telemetry subscription.
- Backend status/resource reads use cached snapshots and do not perform
  `System::new_all().refresh_all()` in request handlers.
- Status/resource updates reach the frontend through a backend-owned push stream
  and Electron subscription.
- Sampler and stream lifecycle are tested for startup, disconnect, and shutdown.
- Remaining polling surfaces are classified and either accepted, documented as
  local panel-owned polling, or moved to a follow-up plan.
- Idle `pumas-rpc` CPU after startup settle is below 5% on the developer
  workstation and does not show runaway `/proc` stat file descriptors.
- Plan completion notes record verification commands and any deviations.

## Standards Compliance Review
- `PLAN-STANDARDS.md`: includes objective, scope, milestones, verification,
  ownership/lifecycle notes, risks, re-plan triggers, and completion criteria.
- `ARCHITECTURE-PATTERNS.md`: keeps backend-owned state in the backend and uses
  backend push to update frontend display state.
- `FRONTEND-STANDARDS.md`: removes global high-frequency polling for backend
  state; any remaining polling is small-owner, low-frequency, cleaned up, and
  explicitly classified.
- `CONCURRENCY-STANDARDS.md` and `languages/rust/RUST-ASYNC-STANDARDS.md`:
  background sampler ownership, cancellation, panic handling, and request-path
  blocking work are explicit plan requirements.
- `TESTING-STANDARDS.md`: uses thin vertical slices with milestone-level unit,
  integration, frontend, and runtime verification.
- `DOCUMENTATION-STANDARDS.md`: records API consumer contract, lifecycle,
  failure/reconnect behavior, and completion criteria.

### Standards Iteration Notes
- The codebase scan found two global frontend polling surfaces for the same
  backend-owned status domain: `useStatus.ts` and `useNetworkStatus.ts`. The
  plan was updated so both are in scope.
- Download, installation, plugin, and app-panel polling surfaces were classified
  separately so this fix does not become a broad frontend polling rewrite before
  proving the high-CPU vertical slice.
- Rust resource-refresh scans confirmed the plan's request-path blast radius:
  `api/system.rs::get_system_resources`, `system/resources.rs::ResourceTracker`,
  and legacy state runtime helpers are the places that must be reviewed during
  implementation.
- Electron bridge scans confirmed the existing subscriber/reconnect pattern can
  be extended without adding a second transport architecture.
- Frontend/Electron look-over found that existing event streams are started
  globally after backend startup. The telemetry stream must not copy that
  behavior blindly; it needs subscriber-aware bridge lifecycle so renderer
  absence does not keep telemetry sampling active.
- Frontend look-over found a network status contract mismatch risk:
  `NetworkStatusResponse.success_rate` is computed as `0.0..1.0` in Rust while
  UI wording treats it as a percent. The telemetry DTO must normalize or
  explicitly document the unit.
- Frontend look-over found resource DTO duplication between `api-system.ts` and
  `apps.ts`. The telemetry slice should reuse an existing shape rather than add
  a third resource DTO.

## Completion Summary
Not started.

### Deviations
None yet.

### Verification Summary
Not run yet.
