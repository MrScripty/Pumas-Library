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

**Implementation Notes:**
- 2026-05-06 audit: status/resource hooks no longer contain `setInterval`,
  `pollInterval`, or direct `get_status`, `get_system_resources`,
  `get_network_status`, or `get_library_status` polling calls.
- 2026-05-06 audit: remaining frontend intervals are workflow-local
  installation/download polling, plugin app status polling, short-lived
  `AppIndicator` UI animation intervals, and mounted app-panel polling for
  loaded models/stats/Torch state. These are classified as outside the global
  status/resource CPU bug, but download progress polling remains a good
  follow-up candidate because it is related to transfer indicators.
- 2026-05-06 audit: process-table refresh remains isolated to explicit
  process-resource paths and app-resource aggregation while apps are running;
  global header resource snapshots no longer call process refresh.
- 2026-05-06 fix: lowered verbose `aggregate_ollama_resources` per-PID resource
  logs from info to debug so telemetry sampling while Ollama is running does not
  create noisy normal-operation logs.

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

**Implementation Notes:**
- 2026-05-06 final verification passed:
  `cargo check --manifest-path rust/Cargo.toml`,
  `npm run -w frontend test:run`, `npm run -w electron test`,
  `npm run -w frontend build`, and `bash launcher.sh --build-release`.
- 2026-05-06 release build completed successfully, including release
  `pumas-rpc`, frontend assets, and Electron main-process build.

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
Completed on 2026-05-06.

The implementation replaced the high-frequency frontend status/resource polling
path with backend-owned status telemetry. System resource requests now use a
primary-owned cached `ResourceTracker`, global header resource reads no longer
refresh the process table, the Rust RPC backend exposes a status telemetry SSE
stream, Electron forwards that stream only while renderer subscribers exist, and
React status/network hooks consume telemetry snapshots plus pushed updates.

Remaining polling surfaces were audited and classified. Installation/download
workflow polling, plugin app-status polling, short-lived app indicator
animation timers, and mounted app-panel polling remain outside this bug fix.
Download progress polling remains the strongest follow-up candidate because it
is related to transfer indicators.

### Deviations
- The backend telemetry service samples status while subscribers exist rather
  than being fully event-triggered for every status source. This is intentional
  for the first validated slice because it centralizes cadence in the backend,
  gates work by subscribers, and removes frontend-owned polling. Future work can
  convert more individual status sources to direct event producers.
- Runtime-profile SSE still uses its pre-existing backend polling loop. It was
  recorded as standards debt and was not copied for status telemetry.

### Verification Summary
- `cargo check --manifest-path rust/Cargo.toml`
- `cargo check --manifest-path rust/Cargo.toml -p pumas-library`
- `cargo check --manifest-path rust/Cargo.toml -p pumas-rpc`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library system_resources`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library resource_snapshot`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library status_telemetry`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-rpc status_telemetry`
- `npm run -w electron test`
- `npm run -w frontend test:run`
- `npm run -w frontend test:run -- useStatus useNetworkStatus`
- `npm run -w frontend check:types`
- `npm run -w frontend build`
- `bash launcher.sh --build-release`
- `git diff --check`

## Post-Completion Idle CPU Remediation

Added on 2026-05-06 after a second runtime trace showed the first telemetry
slice removed the global status/resource poll, but did not bring the idle app
to the expected near-zero CPU baseline.

### Trace Evidence

- Running GUI backend: `pumas-rpc` PID `2517866`.
- `pidstat -p 2517866 -t 1 10` showed process average CPU around `122%`.
- CPU was spread across many `tokio-runtime-worker` threads, each consuming
  small but persistent CPU.
- The backend process had about `164` threads while idle.
- `notify-rs` threads were present, but disk utilization was low and did not
  look like the primary bottleneck.
- Four established Electron-to-backend localhost connections were present.
- `list_model_downloads` returned five paused downloads with `speed: 0.0`.
- Deeper host tracing was blocked by current Linux permissions:
  `ptrace` denied, `/proc/<pid>/task/<tid>/stack` denied, and
  `perf_event_paranoid = 4`.

### Identified Problems

1. Download state still has frontend polling.
   `frontend/src/hooks/useModelDownloads.ts` polls every `800ms` and keeps
   polling when retained paused/partial statuses remain. This can keep the GUI
   and backend active even when no transfer is moving.

2. The header download indicator has duplicate polling.
   `frontend/src/hooks/useActiveModelDownload.ts` polls `list_model_downloads`
   every second while mounted. This duplicates the model-download hook and can
   show stale or mismatched progress when duplicate/partial download records
   exist.

3. Runtime-profile updates still use a backend polling stream.
   `rust/crates/pumas-rpc/src/handlers/mod.rs` polls every second in
   `next_runtime_profile_update_event`, then calls
   `list_runtime_profile_updates_since`.

4. Runtime-profile update reads refresh process status.
   `rust/crates/pumas-core/src/api/runtime_profiles.rs` refreshes the default
   Ollama profile status during update reads. That can call
   `is_ollama_running`, which can enter `spawn_blocking` and scan the process
   table through `ps -eo pid=,args=`.

5. The backend runtime is too broad for an idle desktop process.
   The default multi-thread Tokio runtime and blocking pool make the app look
   like it is prepared to multithread everything. Pumas needs concurrency for
   GUI responsiveness, API clients, downloads, indexing, and runtime processes,
   but idle status, paused downloads, and page-mounted displays must not keep
   large worker pools busy.

### Remediation Objective

Bring idle Pumas Library CPU back under the documented target by removing the
remaining global and backend-loop polling paths, making download and
runtime-profile updates canonical push streams, and bounding backend runtime
concurrency at the process composition root.

### Remediation Scope

In scope:

- Replace download progress polling with a backend-owned download update
  stream that supports snapshot plus cursor handoff.
- Move the header download indicator and model-library download badges onto the
  same download state subscription.
- Remove the runtime-profile SSE polling loop and expose a core-owned
  runtime-profile update subscription.
- Stop runtime-profile update reads from refreshing process status as a side
  effect.
- Gate or cache process table scans so they are not part of idle update-feed
  reads.
- Configure `pumas-rpc` with an explicit Tokio runtime worker count and bounded
  blocking pool at the composition root.
- Add tests and runtime checks that prove no idle frontend interval or backend
  update loop remains for these domains.

Out of scope:

- Changing the model-library selector snapshot API.
- Redesigning every app-panel or plugin-specific status check unless idle CPU
  remains high after this remediation.
- Requiring host kernel tracing permission changes as part of normal
  verification.
- Preserving obsolete GUI RPC surfaces if the replacement subscription contract
  is complete. The Rust client API must still use explicit typed contracts and
  avoid hidden internal RPC.

### Remediation Blast Radius

Rust core:

- `rust/crates/pumas-core/src/model_library/hf/download.rs`: source of truth
  for download state, retained statuses, progress, completion, pause, and
  resume events.
- `rust/crates/pumas-core/src/api/hf.rs` and
  `rust/crates/pumas-core/src/api/state_hf.rs`: public download commands,
  download listing, and any new download event snapshot/stream functions.
- `rust/crates/pumas-core/src/api/runtime_profiles.rs`: runtime-profile update
  reads and status refresh side effects.
- `rust/crates/pumas-core/src/api/state.rs`: legacy IPC dispatch paths for
  runtime-profile update reads and Ollama-running status enrichment must follow
  the same no-idle-refresh rule until the legacy route is removed.
- `rust/crates/pumas-core/src/api/state_process.rs`,
  `rust/crates/pumas-core/src/process/manager.rs`, and
  `rust/crates/pumas-core/src/platform/process.rs`: process status refresh,
  process-table scans, and cached process facts.
- `rust/crates/pumas-core/src/api/builder.rs` and
  `rust/crates/pumas-core/src/api/runtime_tasks.rs`: lifecycle ownership for
  new broadcasters, cancellation handles, and bounded task ownership.

Rust RPC:

- `rust/crates/pumas-rpc/src/main.rs`: explicit Tokio runtime configuration.
- `rust/crates/pumas-rpc/src/server.rs`: stream lifecycle and shutdown
  behavior.
- `rust/crates/pumas-rpc/src/handlers/mod.rs`: runtime-profile SSE loop
  removal.
- `rust/crates/pumas-rpc/src/handlers/models/downloads.rs`: download stream
  endpoint and snapshot/recovery semantics.
- `rust/crates/pumas-rpc/src/handlers/status.rs`: only revisit if status
  telemetry or runtime-status enrichment remains a measured CPU source after
  the download/runtime-profile fixes.

Electron bridge:

- `electron/src/python-bridge.ts`, `electron/src/main.ts`,
  `electron/src/preload.ts`, and `electron/src/rpc-method-registry.ts`: replace
  download and runtime-profile polling consumers with subscription setup,
  unsubscribe cleanup, and renderer fanout.

Frontend:

- `frontend/src/hooks/useModelDownloads.ts`: replace interval polling with
  snapshot plus download-event subscription.
- `frontend/src/hooks/useActiveModelDownload.ts`: remove separate polling and
  derive active/retained state from the canonical download subscription.
- Model-library rows, HF search download actions, and header transfer displays:
  refresh from backend-owned pushed state and do not infer transfer truth from
  local timers.
- Existing tests for download badges, header transfer indicators, and hook
  cleanup must be updated to assert subscription behavior and absence of idle
  intervals.
- `frontend/src/hooks/README.md`: replace current guidance that accepts
  download polling with the new subscription contract and lifecycle rule.

Documentation and plans:

- This plan remains the controlling remediation record.
- Add implementation notes and deviations here after each vertical slice.
- Update plan index text if remediation becomes a separate follow-up plan.

### Remediation Milestones

#### R0 - Baseline And Guardrails

Status: Planned.

- Record the measured idle CPU, thread count, socket count, and retained
  download state before implementation.
- Add a lightweight manual trace checklist to this plan so future verification
  can be repeated without privileged tracing.
- Run `rg` checks for `setInterval`, `pollInterval`, `list_model_downloads`,
  `next_runtime_profile_update_event`, `spawn_blocking`, and process-table
  scans in the affected paths.

Verification:

- Existing test suite still passes before code edits.
- Plan records exact baseline commands and known permission limits.

#### R1 - Backend Download Event Contract

Status: Completed on 2026-05-06.

- Add a core-owned download update snapshot and subscription contract with a
  monotonic cursor.
- The contract must carry enough identity for every UI consumer to distinguish
  repository, selected artifact, quant/variant, partial state, and transfer
  status without path guessing.
- Initial subscription must support recovery from a cursor and then transition
  to live events without a handoff race.
- Paused, completed, failed, and partial states must emit terminal or retained
  events that do not require frontend polling to discover stability.

Verification:

- Rust unit tests for cursor ordering, paused/completed events, and artifact
  identity.
- Rust integration test showing snapshot plus subscribe-since does not miss an
  update inserted between snapshot and subscription setup.

Implementation notes:

- Added core download snapshot and update notification DTOs around the existing
  `ModelDownloadProgress` shape.
- `HuggingFaceClient` now owns a monotonic download revision and broadcast
  sender, publishes snapshots from state transitions, and throttles chunk-level
  progress publishing.
- `PumasApi` exposes download snapshot, subscription, and cursor recovery
  helpers for the RPC/Electron slice.
- Fixed a discovered cancellation-state bug: `cancel_download` could abort the
  task before a terminal `Cancelled` state was written or published.

Verification completed:

- `cargo check --manifest-path rust/Cargo.toml -p pumas-library`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library test_cancel_download_aborts_tracked_task -- --nocapture`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library download_notification_since -- --nocapture`

#### R2 - Download Stream Through RPC And Electron

Status: Completed on 2026-05-06.

- Expose the backend download subscription through the existing RPC/SSE
  transport.
- Bridge it through Electron with subscriber-aware lifecycle and explicit
  unsubscribe cleanup.
- Avoid opening backend streams when no renderer has subscribed.

Verification:

- RPC tests for stream startup, recovery from cursor, and shutdown.
- Electron tests for one backend stream shared by multiple renderer
  subscribers and closed after the last unsubscribe.

Implementation notes:

- 2026-05-06: Added the RPC `/events/model-download-updates` SSE endpoint with
  initial snapshot delivery.
- 2026-05-06: Added subscriber-counted Electron bridge/preload forwarding for
  model-download update notifications.

Verification completed:

- `cargo check --manifest-path rust/Cargo.toml -p pumas-rpc`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-rpc test_model_download_update_event_stream_emits_initial_snapshot -- --nocapture`
- `npm run -w electron validate`
- `npm run -w electron test`
- `npm run -w frontend check:types`

#### R3 - Frontend Download Poll Removal

Status: Completed on 2026-05-06.

- Replace `useModelDownloads.ts` interval polling with initial snapshot plus
  pushed download events.
- Replace `useActiveModelDownload.ts` with a selector over the same canonical
  download store.
- Ensure paused full-repo partial records do not keep a polling loop alive.
- Ensure completed selected artifacts clear active transfer indicators and
  retain only accurate partial/problem badges.

Verification:

- Frontend tests prove no production `setInterval` is used for download state.
- Hook tests cover paused, completed, failed, duplicate artifact, and reconnect
  recovery cases.
- Manual UI check confirms the header transfer indicator does not show activity
  when all downloads are paused or complete.

Implementation notes:

- `useModelDownloads` now loads one startup snapshot and subscribes to
  `onModelDownloadUpdate`.
- `useActiveModelDownload` now derives the active header indicator from the
  same pushed snapshot contract.
- The hook README now documents download update subscriptions instead of
  download polling.

Verification completed:

- `npm run -w frontend test:run -- useModelDownloads useActiveModelDownload`
- `npm run -w frontend check:types`
- `rg -n "setInterval|list_model_downloads|poll" frontend/src/hooks/useModelDownloads.ts frontend/src/hooks/useActiveModelDownload.ts frontend/src/hooks/README.md`

#### R4 - Runtime-Profile Push Updates

Status: Completed on 2026-05-06.

- Replace the `next_runtime_profile_update_event` one-second loop with a
  runtime-profile update broadcaster owned by core state.
- Update callers so `list_runtime_profile_updates_since` is a history/recovery
  read, not an idle refresh trigger.
- Prevent update-feed reads from refreshing Ollama or llama.cpp process status
  as a side effect.

Verification:

- Rust tests for subscribe-since handoff and no missed runtime-profile update.
- Code search confirms the runtime-profile SSE path no longer sleeps in a loop.
- Tests or instrumentation confirm runtime-profile update reads do not call
  process-table scans.

Implementation notes:

- `RuntimeProfileService` now owns a broadcast channel for update feeds.
- Runtime-profile status changes publish event feeds directly.
- Runtime-profile config mutations publish `snapshot_required` feeds so clients
  can refresh after profile/route shape changes without polling.
- `/events/runtime-profile-updates` now performs one startup recovery read from
  `list_runtime_profile_updates_since(cursor)` and then waits on the broadcast
  receiver.
- Snapshot and update-feed reads no longer refresh the default Ollama process
  status, including the legacy IPC dispatch path.

Verification completed:

- `cargo check --manifest-path rust/Cargo.toml -p pumas-rpc`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library runtime_profile_status_changes_emit_update_events -- --nocapture`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library runtime_profile_config_mutations_push_snapshot_required_update -- --nocapture`
- `rg -n "RUNTIME_PROFILE_UPDATE_STREAM_POLL|tokio::time::sleep\\(|setInterval\\(|list_runtime_profile_updates_since\\(|refresh_default_ollama_profile_status" rust/crates/pumas-rpc/src/handlers/mod.rs rust/crates/pumas-core/src/api/runtime_profiles.rs rust/crates/pumas-core/src/api/state.rs frontend/src`

#### R5 - Process Status Ownership

Status: In progress.

- Move Ollama/llama.cpp process liveness into an explicit process-status owner
  with cached facts and a clear refresh trigger.
- Refresh can occur on explicit user action, child-process lifecycle events, or
  a low-frequency backend-owned monitor with subscribers. It must not occur
  because a UI page is mounted or an update feed is idle.
- Process-table fallback scans must be rate-limited and observable.

Verification:

- Unit tests for cache TTL/rate limiting if a TTL is used.
- Integration tests for runtime-profile display using cached status.
- `rg` confirms `find_processes_by_cmdline` is not reachable from idle
  update-stream loops.

Implementation notes:

- First slice moved ComfyUI, Ollama, and Torch liveness reads in
  `ProcessManager` to cached facts updated by startup detection, launch/stop
  operations, and explicit refresh methods.
- `is_running`, `is_ollama_running`, and `is_torch_running` are now
  non-scanning reads, so status telemetry sampling no longer reaches
  process-table fallback scans for basic app liveness.
- Process-table fallback remains in explicit refresh/cleanup paths such as
  startup detection, `refresh_*_running`, and stop-by-pattern cleanup.

Discovered issue:

- Managed child-process exit is not yet observed through a wait-handle owner.
  If a managed ComfyUI, Ollama, or Torch process exits outside an explicit stop
  path, the cached liveness fact can remain stale until an explicit refresh.
  Resolve this with launch-owned child exit observation or another
  lifecycle-owned backend mechanism rather than restoring UI/request polling.

Verification completed for first slice:

- `cargo check --manifest-path rust/Cargo.toml -p pumas-library`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library liveness_read_uses_cache_until_explicit_refresh -- --nocapture`

#### R6 - Runtime And Thread Budget

Status: Planned.

- Replace the default `#[tokio::main]` runtime setup in `pumas-rpc` with an
  explicit runtime builder at the composition root.
- Set a conservative worker-thread count and bounded blocking pool sized for
  desktop GUI responsiveness and API client access.
- Heavy categories such as downloads, hashing, indexing, extraction, and model
  runtime management must use explicit owned services, semaphores, or queues
  rather than relying on an oversized generic runtime.
- Document any intentionally long-lived threads, such as file watchers or
  runtime child-process monitors.

Verification:

- Rust checks and RPC tests pass under the explicit runtime.
- Release smoke test confirms downloads and API calls do not starve.
- Idle thread count is measured and recorded. The target is fewer than `64`
  backend threads while idle unless a higher count is justified in this plan.

#### R7 - Release Verification And Completion

Status: Planned.

- Build release binaries and frontend after all remediation slices.
- Run the GUI idle for at least 60 seconds after startup settle with no active
  downloads, installs, migrations, or runtime launches.
- Measure CPU with `pidstat -p <pumas-rpc-pid> -t 1 30`.
- Record backend thread count, socket count, and active stream count.

Acceptance:

- Idle `pumas-rpc` CPU averages below `5%` on the developer workstation.
- No frontend production hook globally polls download state, status resources,
  network status, or runtime-profile updates.
- Runtime-profile and download update streams use snapshot plus cursor handoff.
- Paused and completed downloads do not produce continuous backend RPC traffic.
- No idle path calls `ps -eo pid=,args=` once per second.

### Concurrency And Runtime Requirements

- Runtime creation belongs only at process composition roots.
- Library code must not create hidden Tokio runtimes.
- Every spawned background task must have a named owner, cancellation path, and
  shutdown behavior.
- `spawn_blocking` is allowed for real blocking work, but not as a way to hide
  repeated status refreshes caused by idle UI loops.
- Shared state updates should use message passing or owned services before
  ad-hoc shared mutable state.
- Frontend state for backend-owned domains must be snapshot plus pushed events,
  not optimistic timers.
- Subscriber streams must clean up when the last consumer disconnects.

### Parallel Implementation Plan

Parallel work is feasible after the download and runtime-profile event
contracts are frozen.

- Main integrator owns contracts, plan updates, runtime/thread budget, and final
  release verification.
- Worker A can implement core download event history and tests.
- Worker B can implement Electron/frontend download subscription replacement
  after Worker A's DTO contract is available.
- Worker C can implement runtime-profile broadcaster and process-status cache
  work after the core event-owner pattern is clear.

Workers must not edit the same contract files concurrently. Any discovered
standards issue, stub, or unexpected coupling must be recorded in this plan
before broadening implementation scope.

### Standards Re-Iteration Notes

Reviewed standards on 2026-05-06:

- `PLAN-STANDARDS.md`: this addendum adds objective, scope, blast radius,
  milestone-level verification, risks through re-plan triggers, acceptance
  criteria, and concurrency lifecycle ownership for the remediation work.
- `ARCHITECTURE-PATTERNS.md`: the remediation keeps backend-owned download and
  runtime-profile data in the backend and pushes updates to the frontend.
- `FRONTEND-STANDARDS.md`: the plan removes high-frequency frontend polling for
  backend-owned download state and prevents retained paused state from keeping
  intervals alive.
- `CONCURRENCY-STANDARDS.md`: the plan replaces serial polling loops with
  owner-published events and requires bounded task ownership.
- `languages/rust/RUST-ASYNC-STANDARDS.md`: runtime creation moves to the
  composition root, spawned tasks require cancellation, and blocking work must
  be isolated behind owned services or bounded queues.
- `TESTING-STANDARDS.md`: each slice has vertical verification from Rust core
  through RPC/Electron/frontend where applicable, plus runtime acceptance.
- `DOCUMENTATION-STANDARDS.md`: deviations, trace limitations, and implementation
  discoveries must be recorded in this plan.

Standards findings to enforce during implementation:

- The existing frontend download hooks are standards-noncompliant for
  backend-owned state because they poll globally while paused retained rows
  exist.
- The runtime-profile SSE loop is standards-noncompliant because it polls the
  backend every second and refreshes process status as part of an update-feed
  read.
- Process-table scans are acceptable only behind explicit process-status
  ownership, cache/rate limits, or direct user action.
- A large default runtime is not itself a correctness bug, but it hides
  ownership boundaries and makes idle behavior harder to reason about. The
  implementation must make thread and blocking budgets explicit.
- The previous status telemetry sampler remains acceptable only if it is
  subscriber-gated and not the dominant CPU source after the new remediation
  slices. If it remains a measured source, this plan must be reopened before
  declaring completion.

### Additional Re-Plan Triggers

- Idle CPU remains above `5%` after download and runtime-profile polling are
  removed.
- Explicit Tokio runtime sizing causes download, API, or model-runtime
  starvation under normal use.
- The download event contract cannot represent artifact identity well enough to
  distinguish repo variants and quant files.
- Runtime-profile process status cannot be cached without making UI status
  materially misleading.
- A privileged trace later identifies a different dominant CPU source.
