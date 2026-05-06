# Plan: Fast Model Snapshot And Explicit Client API

## Objective

Implement a fast, safe Pumas model selector path and explicit local client
architecture that lets Pantograph consume Pumas model facts without per-model
hydration or hidden RPC inside the Rust API.

## Scope

### In Scope

- Materialized selector snapshot rows with canonical Pumas model references,
  selected artifact state, entry path state, and validation/detail state.
- Direct in-process and read-only Rust snapshot access backed by indexed
  SQLite/cache state.
- Core-owned typed model-library update subscription with atomic recovery from
  a snapshot cursor.
- Explicit Pumas instance, local client, and read-only reader API roles.
- Same-device local instance discovery and an explicit local-client transport
  adapter.
- Batch hydration and cheap descriptor APIs after the fast selector slice is
  proven.
- RPC/SSE/Electron forwarding alignment so GUI paths consume core event
  contracts.

### Out of Scope

- Distributed or multi-host discovery.
- Pantograph runtime scheduling, session, queue, graph execution, or diagnostic
  policy.
- Replacing SQLite as the local source of indexed and durable model facts.
- Preserving transparent `PumasApi` auto-client behavior.
- Making GUI RPC the preferred API for Pantograph or other non-GUI clients.

## Inputs

### Problem

Pantograph currently needs selector and graph-authoring facts that require
multiple expensive Pumas calls per model. The existing `PumasApi` also blurs
deployment topology by sometimes acting as a direct primary API and sometimes
as an IPC-backed client. That design is hard to reason about, slower than
needed, and unsafe for external consumers that need clear identity and update
semantics.

### Constraints

- Direct Rust API calls must stay typed and in-process unless the caller
  explicitly chooses a local client transport.
- Selector snapshots must not scan model directories, read metadata JSON,
  regenerate package facts, resolve dependencies, or perform per-row IPC.
- `indexed_path` is display/debug data only.
- `entry_path` is executable only when `entry_path_state == Ready` and
  `artifact_state == Ready`.
- Snapshot-to-subscription handoff must not miss events committed after the
  snapshot cursor is produced.
- Local-client endpoints must remain same-device only and authenticated by a
  registry token or equivalent local credential.
- Documentation and implementation must follow the Coding Standards plan,
  documentation, Rust API, concurrency, testing, and commit standards.

### Assumptions

- Existing model index and metadata/cache state can provide enough facts for a
  first selector projection without deep package-facts regeneration.
- SQLite transaction boundaries can provide coherent direct/read-only snapshot
  semantics.
- The existing durable model-library update feed can be reused or adapted as
  the recovery source for subscription handoff.
- Pantograph can initially consume direct/read-only snapshots before the full
  local-client transport slice is complete.
- Legacy transparent `PumasApi` behavior can be broken or deprecated rather
  than preserved.

### Dependencies

- `rust/crates/pumas-core/src/index/`
- `rust/crates/pumas-core/src/model_library/`
- `rust/crates/pumas-core/src/api/`
- `rust/crates/pumas-core/src/registry/`
- `rust/crates/pumas-core/src/ipc/`
- `rust/crates/pumas-rpc/src/server.rs`
- `electron/src/python-bridge.ts`
- `electron/src/preload.ts`
- `frontend/src/hooks/useModelLibraryUpdateSubscription.ts`
- Pantograph `puma-lib` integration and fixture expectations.

### Affected Structured Contracts

- Rust public entry points currently represented by `PumasApi`.
- New `PumasLibraryInstance`, `PumasReadOnlyLibrary`, and `PumasLocalClient`
  surfaces.
- `ModelLibrarySelectorSnapshotRequest`
- `ModelLibrarySelectorSnapshot`
- `ModelLibrarySelectorSnapshotRow`
- `PumasModelRef`
- `ModelEntryPathState`
- `ModelArtifactState`
- `ModelLibraryUpdateSubscription`
- `ModelLibraryUpdateFeed`
- Local instance registry endpoint records.
- RPC/SSE/Electron update forwarding payloads.

### Affected Persisted Artifacts

- Model index SQLite tables or projections used for selector rows.
- Durable model-library update feed rows and cursors.
- Local library registry SQLite instance rows and endpoint records.
- Existing metadata and package-facts cache rows used as selector inputs.

### Ownership And Lifecycle Note

`PumasLibraryInstance` owns writes, migrations, downloads, reconciliation,
watchers, update production, and optional local service publication.
`PumasReadOnlyLibrary` owns no background work and must not mutate state or
claim the instance registry. `PumasLocalClient` owns only a connection or
subscription stream to a running instance and must cleanly close streams on
drop/shutdown.

Subscription startup must use `subscribe_model_library_updates_since(cursor)`.
The instance must replay recoverable events after the cursor, return the
cursor after recovery, and then transition the same subscriber to live events.
Reconnect uses `list_model_library_updates_since(cursor, limit)` and falls
back to a full snapshot on stale cursor.

### Public Facade Preservation Note

This is an API-breaking cleanup. Transparent `PumasApi` auto-client behavior is
not preserved as a compatibility requirement. If `PumasApi` remains during the
transition, it must be explicitly deprecated, narrowed, or converted into a
non-transport alias so consumers are not misled about ownership mode.

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| Selector projection lacks a fact Pantograph needs | High | Start with Pantograph's `puma-lib` needs and add contract tests for graph-facing model refs |
| Snapshot path accidentally calls deep resolution | High | Add tests/tracing guards that fail on metadata JSON loads, filesystem scans, dependency resolution, or per-row IPC |
| Entry path is used when state is not ready | High | Add tests proving `entry_path` is executable only when entry and artifact states are both `Ready` |
| Snapshot/subscription race misses updates | High | Implement cursor-based subscription handshake before relying on push updates |
| Read-only snapshots observe incomplete writes | High | Use SQLite transactions and document read-only consistency semantics |
| `PumasApi` split touches more callers than expected | High | Inventory callers before source changes and migrate by role in separate commits |
| Local client transport exposes an unintended network surface | Medium | Prefer platform IPC; restrict loopback TCP to localhost plus registry token |
| Local-client target depends on transport choice | Medium | Measure selected Unix socket, named pipe, or TCP transport separately from direct SQLite |
| Batch hydration becomes a loop over slow APIs | Medium | Share internal loaded facts and test that batch paths avoid public per-model loops |

## Clarifying Questions

- None.
- Reason: Pantograph has accepted the revised direction, and remaining
  transport choices can be handled inside the relevant implementation slice.
- Revisit trigger: The selected local-client transport cannot meet security or
  performance requirements.

## Definition of Done

- Root proposal has been moved into this slugged plan directory with a
  standards-compliant README and implementation plan.
- Direct/read-only selector snapshots return 50-100 warm indexed rows in
  `<= 5ms` without filesystem scans, metadata JSON loads, RPC, or deep
  per-model resolution.
- Selector rows expose `PumasModelRef`, selected artifact identity/path,
  entry path, entry path state, and artifact state.
- Tests prove `entry_path` is executable only when entry and artifact state are
  both `Ready`.
- Pantograph can consume the selector snapshot lazily for `puma-lib` without
  hydrating every listed model.
- Core-owned subscriptions accept a snapshot cursor and replay recovered
  events before live events.
- GUI RPC/SSE/Electron update forwarding is backed by the same core update
  contract.
- Explicit local-client discovery works without hidden `PumasApi` RPC.
- Local-client snapshot timing is measured against the selected same-device
  transport and avoids per-row calls.
- Batch hydration and cheap descriptor APIs share loaded facts and preserve
  selected-model detail completeness.
- Each completed logical slice is verified and committed atomically.

## Milestones

### Milestone 0: Plan Package And Traceability

**Goal:** Put the accepted proposal into a standards-compliant plan package.

**Tasks:**
- [x] Move the Pantograph proposal into a slugged directory.
- [x] Add a directory README with purpose, constraints, invariants, and
  consumer contract.
- [x] Add this implementation plan with thin vertical slices.
- [x] Update `docs/plans/README.md` and superseded-plan references to the new
  directory path.

**Verification:**
- Plan package follows `PLAN-STANDARDS.md` and `DOCUMENTATION-STANDARDS.md`.
- `git status --short` shows only expected documentation changes.

**Status:** Complete

### Milestone 1: API Role Inventory And Contract Freeze

**Goal:** Freeze the public role names and identify every current caller before
changing source behavior.

**Tasks:**
- [x] Inventory all `PumasApi::new`, `PumasApi::builder`, and `PumasApi::discover`
  callers.
- [x] Classify each caller as owning instance, explicit local client, or
  read-only consumer.
- [x] Add or update architecture docs that define `PumasLibraryInstance`,
  `PumasReadOnlyLibrary`, and `PumasLocalClient`.
- [x] Record the compatibility break and migration path for transparent
  client-mode behavior.

**Verification:**
- No caller remains unclassified.
- Architecture docs state that direct Rust APIs do not secretly use RPC.
- Compile is not required if this slice is documentation/inventory only.

**Status:** Complete

### Milestone 2: Fast Selector Vertical Slice

**Goal:** Deliver the smallest useful selector path Pantograph can consume
without broad transport work.

**Tasks:**
- [x] Add selector snapshot DTOs and contract tests.
- [x] Add materialized selector row storage or a query projection backed by
  indexed SQLite/cache state.
- [x] Populate rows for existing indexed models with `PumasModelRef`,
  selected artifact identity/path, entry path, entry path state, artifact
  state, display fields, and detail state.
- [x] Add direct in-process selector snapshot access through the current owner
  surface. Final `PumasLibraryInstance` naming/export remains in Milestone 7.
- [x] Add `PumasReadOnlyLibrary` selector snapshot access with no background
  work or registry claim.
- [x] Add tests proving non-ready entry/artifact states are not executable.
- [x] Add benchmark or timing test for 50-100 warm direct/read-only rows.
- [x] Update Pantograph-facing docs or fixtures showing lazy `puma-lib`
  consumption from selector rows.

**Commit Sub-Slices:**
- Slice 2.1: Add selector DTOs, executable-state contract tests, and
  documentation for `PumasModelRef` semantics.
- Slice 2.2: Add SQLite-backed selector projection or storage and populate it
  for existing indexed models.
- Slice 2.3: Add direct in-process selector snapshot API and prove it avoids
  deep resolution.
- Slice 2.4: Add read-only selector snapshot API with no lifecycle ownership.
- Slice 2.5: Add performance measurement/report and Pantograph-facing fixture
  or documentation updates.

**Verification:**
- Targeted Rust tests for DTO mapping, state semantics, and read-only behavior.
- Warm direct/read-only selector timing reports `<= 5ms` for common pages.
- Tests or tracing guards prove no filesystem scan, metadata JSON load, RPC,
  dependency resolution, or package-facts regeneration on the snapshot path.
- Correctness tests are gating. The `<= 5ms` target should be recorded through
  a benchmark or timing report unless stable performance-test infrastructure
  exists for this crate.
- Atomic commit after successful verification.

**Status:** Complete

### Milestone 3: Selector Materialization Lifecycle

**Goal:** Keep selector rows current through model-library state changes.

**Tasks:**
- [x] Update import completion to populate/update selector rows.
- [x] Update download completion and selected-artifact changes to refresh
  selector rows.
- [x] Update migration/reconciliation paths to refresh selector rows.
- [x] Update metadata refresh paths to invalidate or refresh selector rows.
- [x] Emit model-library update feed events when selector-visible facts change.

**Verification:**
- Tests cover import, download completion, metadata refresh, migration, and
  reconciliation updates.
- Integrity/problem rows surface explicit stale, missing, partial, ambiguous,
  or needs-detail states instead of deep-resolving inline.
- Atomic commit after successful verification.

**Status:** Complete

### Milestone 4: Core Subscriber With Atomic Handoff

**Goal:** Make model-library updates a core typed subscription with no
snapshot-to-live race.

**Tasks:**
- [x] Move or wrap model-library update publication behind a core event bus.
- [x] Add `subscribe_model_library_updates_since(cursor)`.
- [x] Make subscription startup replay durable events after the cursor before
  yielding live events.
- [x] Return `cursor_after_recovery` before live event processing.
- [x] Preserve `list_model_library_updates_since(cursor, limit)` for reconnect
  recovery.
- [x] Define stale cursor behavior that forces a fresh selector snapshot.

**Verification:**
- Test commits an update after snapshot cursor creation and before
  subscription activation; subscriber receives the update.
- Direct Rust subscription and durable recovery observe the same ordered event
  sequence.
- Disconnect/reconnect tests recover missed events or report stale cursor.
- Atomic commit after successful verification.

**Status:** Complete

### Milestone 5: GUI Forwarding From Core Events

**Goal:** Make existing GUI push behavior consume the canonical core event
contract instead of a parallel event path.

**Tasks:**
- [x] Update RPC/SSE endpoint implementation to subscribe to the core update
  bus.
- [x] Update Electron forwarding to preserve cursor/recovery semantics.
- [x] Update preload/frontend type validation if payload shape changes.
- [x] Keep frontend refresh debounced and subscriber-owned, not component-level
  polling.

**Verification:**
- Backend/RPC tests cover SSE event payload and recovery behavior.
- Electron tests cover subscribe/unsubscribe cleanup and invalid payloads.
- Frontend tests cover debounced refresh from update notifications.
- Atomic commit after successful verification.

**Status:** Complete

### Milestone 6: Explicit Local Client Discovery

**Goal:** Support same-device external clients without hidden RPC inside the
Rust API.

**Tasks:**
- [ ] Define local instance registry endpoint records with pid, root, status,
  transport kind, endpoint, and connection token.
- [ ] Add explicit `PumasLocalClient::connect`.
- [ ] Expose local-client selector snapshot as one transport request per
  snapshot.
- [ ] Expose local-client subscription as one stream per subscription.
- [ ] Choose platform transport order: Unix socket on Linux/macOS, named pipe
  on Windows, localhost TCP fallback when needed.
- [ ] Measure local-client selector latency against the selected transport.

**Verification:**
- A second process discovers and connects to a running instance explicitly.
- Direct Rust constructors do not silently become transport clients.
- Local-client snapshot avoids per-row calls and reports selected-transport
  timing against the `<= 25ms` initial target.
- Security tests or assertions cover localhost/token restrictions for TCP
  fallback.
- Atomic commit after successful verification.

**Status:** Not started

### Milestone 7: Public API Split And Compatibility Cleanup

**Goal:** Remove or narrow transparent `PumasApi` behavior after replacement
entry points exist.

**Tasks:**
- [ ] Introduce or finalize `PumasLibraryInstance`, `PumasReadOnlyLibrary`, and
  `PumasLocalClient` exports.
- [ ] Migrate internal callers from `PumasApi` to explicit roles.
- [ ] Remove, deprecate, or narrow transparent `ApiInner::Client` dispatch.
- [ ] Update UniFFI/bindings guidance to use explicit roles.
- [ ] Update crate docs and examples.

**Verification:**
- Compile and targeted Rust API tests pass.
- Public docs no longer describe hidden transparent client behavior as the
  preferred contract.
- API break is recorded in docs/release notes if release notes exist for this
  cycle.
- Atomic commit after successful verification.

**Status:** Not started

### Milestone 8: Batch Hydration And Cheap Descriptor Split

**Goal:** Keep selected-model detail access complete while eliminating slow
multi-model public loops.

**Tasks:**
- [ ] Add batch package-facts summary resolution backed by shared loaded facts.
- [ ] Add batch execution descriptor resolution backed by shared loaded facts.
- [ ] Add batch inference-settings access.
- [ ] Split cheap execution descriptor fields from dependency resolution.
- [ ] Keep dependency resolution opt-in for selected models or explicit batch
  requests.
- [ ] Update Pantograph integration guidance for optional multi-select
  hydration.

**Verification:**
- Batch tests prove per-model failures are represented without failing the
  whole batch unnecessarily.
- Tests or tracing guards show batch APIs do not loop over slow public
  single-model APIs.
- Selected-model hydration still returns full details, dependencies, and
  inference settings.
- Atomic commit after successful verification.

**Status:** Not started

### Milestone 9: Final Standards Pass And Release Build

**Goal:** Close the plan with complete verification and traceability.

**Tasks:**
- [ ] Run Rust formatting and targeted/full Rust checks appropriate to the
  changed crates.
- [ ] Run Electron/frontend type, lint, and test checks for changed surfaces.
- [ ] Build release binaries and frontend.
- [ ] Update this plan's execution notes, completion summary, deviations,
  follow-ups, and verification summary.

**Verification:**
- `cargo fmt`/Rust checks pass for changed Rust crates.
- Frontend/Electron checks pass for changed JS/TS surfaces.
- Release build and frontend build pass.
- Final commit captures documentation closeout or release artifact updates.

**Status:** Not started

## Execution Notes

Update during implementation:
- 2026-05-06: Proposal moved into a standards-compliant plan directory and
  converted into this implementation plan.
- 2026-05-06: Milestone 1 caller inventory recorded in `caller-inventory.md`.
  Architecture, core, IPC, UniFFI, and native-binding docs now describe hidden
  `PumasApi` convergence as transitional compatibility rather than the target
  API contract.
- 2026-05-06: Milestone 2 Slice 2.1 added fast selector snapshot DTOs,
  selector entry/artifact readiness states, detail freshness state, and
  `PumasModelRef.model_ref_contract_version`. Contract tests now prove
  selector snapshots use snake_case wire labels and `entry_path` is executable
  only when both entry and artifact states are `ready`.
- 2026-05-06: Slice 2.2 codebase inspection found that the first selector
  projection can be implemented as a single `ModelIndex` SQLite query over
  `models` plus `model_package_facts_cache`, using metadata JSON fallbacks for
  selected artifact identity and cached summary JSON when valid. Materialized
  selector columns remain a performance follow-up if JSON extraction cannot
  meet the direct/read-only timing target.
- 2026-05-06: Milestone 2 Slice 2.2 added
  `ModelIndex::list_model_library_selector_snapshot`, projecting selector rows
  from the indexed `models` table and package-facts summary cache. The query
  prefers selected-artifact-scoped summary rows when present and falls back to
  existing empty selected-artifact cache rows. Partial download flags,
  validation state, and import state now produce non-executable selector
  artifact/entry states without filesystem inspection.
- 2026-05-06: Milestone 2 Slice 2.3 exposed selector snapshots through
  `ModelLibrary::model_library_selector_snapshot` and a primary-only
  transitional `PumasApi::model_library_selector_snapshot`. This direct surface
  intentionally does not add RPC/client dispatch; explicit local-client
  transport remains in Milestone 6 and final role naming remains in Milestone 7.
- 2026-05-06: Milestone 2 Slice 2.4 added `PumasReadOnlyLibrary`, backed by
  `ModelIndex::open_read_only`. It opens an existing `models.db` with SQLite
  read-only flags and `query_only`, exposes only selector snapshots, and does
  not create schema, claim an instance, reconcile, or start watchers.
- 2026-05-06: Milestone 2 Slice 2.5 added a 100-row warm selector timing test,
  Pantograph-facing selector contract documentation, and a selector snapshot
  fixture. Local debug-test timing reported direct `0.878ms` and read-only
  `0.694ms` for 100 warm rows.
- 2026-05-06: Milestone 3 implemented selector lifecycle through live
  projection instead of a materialized selector table. Existing import,
  download, migration, reconciliation, and metadata-refresh paths already
  update the underlying model index rows that selector snapshots read. Added
  lifecycle tests for model-row updates and package-summary cache updates.
- 2026-05-06: Milestone 3 found and fixed an update-feed gap: changed
  package-facts summary cache rows affected selector output but did not emit
  model-library update events. Summary cache changes now emit
  `PackageFactsModified` events with `refresh_scope = Summary`.
- 2026-05-06: Milestone 4 first slice added
  `subscribe_model_library_updates_since(cursor)` as a recovery-first typed
  handoff. It pages the durable update feed from the snapshot cursor, returns
  `cursor_after_recovery`, reports stale cursors with `snapshot_required`, and
  explicitly leaves `live_stream_ready = false` until a core live bus is added.
  Codebase inspection confirmed no existing core broadcast bus; current GUI
  SSE polls from `None` and will be handled in later slices.
- 2026-05-06: Milestone 4 second slice added a core `tokio::broadcast`
  update bus owned by `ModelIndex`. Durable SQLite append remains
  authoritative; update events publish only after append success, and
  transactional paths publish only after commit. Tests cover direct model row
  publication and transactional replace publication.
- 2026-05-06: Milestone 4 third slice added an owner-side direct Rust
  `ModelLibraryUpdateSubscriber` and
  `PumasApi::subscribe_model_library_update_stream_since(cursor)`. The handle
  attaches to the live bus before durable recovery, returns the recovered
  handshake first, drains duplicate live events already covered by
  `cursor_after_recovery`, and then yields live events through `next_event()`.
  GUI/SSE transport adaptation remains intentionally out of scope for this
  slice.
- 2026-05-06: Milestone 4 testing found that after a clean reconciliation,
  manually adding a test model directory is not visible to `list_models()`
  unless reconciliation is explicitly marked dirty or rebuilt. This matches the
  dirty-state owner model but is relevant for future filesystem watcher and
  service-discovery tests that create files outside normal import/download
  flows.
- 2026-05-06: Milestone 5 changed the RPC model-library SSE endpoint from
  durable-feed polling to the core owner-side subscriber. The endpoint accepts
  an optional `cursor` query parameter, emits recovered events before live
  events, and preserves the existing `ModelLibraryUpdateNotification` payload
  shape. Electron now retains the latest notification cursor for reconnects and
  resets it when the update stream is explicitly stopped. Frontend validation
  did not require changes because the notification payload shape stayed stable.
- 2026-05-06: Milestone 5 testing found an unrelated RPC wrapper mismatch:
  `refresh_model_index` returns an object from its handler, but
  `wrap_response()` treats it like a boolean and reports `success: false`.
  Record this as a follow-up before relying on that RPC method in external
  clients or tests.

## Commit Cadence Notes

- Commit after each milestone's verified logical slice.
- Keep schema, Rust code, tests, and matching docs together when they are part
  of the same slice.
- Keep API compatibility cleanup separate from the first selector slice.
- Keep local-client transport work separate from direct/read-only snapshot
  work.
- Use standard commit format from `COMMIT-STANDARDS.md`.

## Concurrent Worker Plan

Use subagents only after Milestone 1 has classified callers, the integration
branch is clean, and shared DTO/schema ownership has been assigned. Each worker
must use an isolated worktree or temporary clone and may commit only inside its
assigned workspace.

### Worker Wave 1: Independent Read And Event Slices

| Owner/Agent | Assigned Scope | Primary Write Set | Allowed Adjacent Write Set | Read-Only Context | Forbidden/Shared Files | Output Contract | Report Path | Handoff Checkpoint |
| ----------- | -------------- | ----------------- | -------------------------- | ----------------- | ---------------------- | --------------- | ----------- | ------------------ |
| Worker A | Selector storage/query and Rust DTO tests | `rust/crates/pumas-core/src/index/`, selector-specific files under `rust/crates/pumas-core/src/model_library/`, focused Rust tests | Module README updates for touched selector/index directories | Existing metadata, package-facts, import, and migration code | Public exports, shared DTO modules, registry schema, lockfiles, generated bindings | Patch and tests for Milestone 2 selector projection without deep resolution | `docs/plans/pumas-fast-model-snapshot-and-client-api/reports/worker-a-selector.md` | Worker tests pass and report lists changed files |
| Worker B | Core update bus and subscription race tests | Core update-feed/subscription files under `rust/crates/pumas-core/src/api/` and `rust/crates/pumas-core/src/model_library/`, focused Rust tests | Module README updates for touched update-feed directories | Existing SSE/Electron update forwarding and durable feed code | Public exports, shared DTO modules, registry schema, lockfiles, generated bindings | Patch and tests for Milestone 4 atomic cursor handoff | `docs/plans/pumas-fast-model-snapshot-and-client-api/reports/worker-b-subscription.md` | Race test passes and report lists changed files |

### Worker Wave 2: Transport And GUI Forwarding

Start only after Wave 1 has been integrated and verified.

| Owner/Agent | Assigned Scope | Primary Write Set | Allowed Adjacent Write Set | Read-Only Context | Forbidden/Shared Files | Output Contract | Report Path | Handoff Checkpoint |
| ----------- | -------------- | ----------------- | -------------------------- | ----------------- | ---------------------- | --------------- | ----------- | ------------------ |
| Worker C | RPC/SSE/Electron/frontend forwarding from core events | `rust/crates/pumas-rpc/src/`, `electron/src/`, `electron/tests/`, `frontend/src/hooks/`, `frontend/src/types/`, focused tests | Frontend hook README updates if touched | Core update bus contracts from integrated Wave 1 | Core DTO definitions unless assigned by integration owner, lockfiles, generated bindings | Patch and tests for Milestone 5 GUI forwarding | `docs/plans/pumas-fast-model-snapshot-and-client-api/reports/worker-c-gui-forwarding.md` | JS/TS focused tests pass and report lists changed files |
| Worker D | Explicit local client discovery transport | `rust/crates/pumas-core/src/registry/`, `rust/crates/pumas-core/src/ipc/`, new local-client files, focused Rust tests | Module README updates for touched registry/ipc directories | Integrated selector and subscription contracts | Shared DTO exports unless assigned by integration owner, lockfiles, generated bindings | Patch and tests for Milestone 6 explicit attach and selected-transport timing | `docs/plans/pumas-fast-model-snapshot-and-client-api/reports/worker-d-local-client.md` | Explicit attach test passes and report lists changed files |

### Serial Integration Ownership

The integration owner, not parallel workers, owns:

- shared public DTO names and exports;
- schema migrations that affect more than one worker;
- crate-level public API re-exports;
- lockfiles and generated bindings;
- conflict resolution;
- plan status, execution notes, and completion summary updates.

### External-Change Escalation Rule

If a worker needs edits outside its primary or allowed adjacent write set, it
must record the need in its report instead of changing the file. The
integration owner decides whether to expand the write set, handle the change
serially, or re-plan.

### Integration Sequence And Cleanup

After each worker wave:

- Read every worker report.
- Verify changed files match assigned write sets.
- Integrate worker branches one at a time.
- Run the wave verification after integration.
- Commit conflict resolution separately if it is not already owned by one
  worker.
- Update this plan with integrated commits, verification, deviations, and
  follow-ups.
- Confirm worker workspaces have no uncommitted changes before removing them.

## Re-Plan Triggers

- Pantograph cannot build safe graph-facing references from selector rows.
- Direct/read-only snapshot cannot meet `<= 5ms` without deep changes to the
  index schema.
- Atomic subscription handoff cannot be implemented on the existing durable
  feed.
- Splitting `PumasApi` requires a broader crate restructure than expected.
- Selected platform IPC cannot satisfy same-device security or performance
  requirements.
- Existing migrations or download/reconciliation flows cannot update selector
  rows without risking data loss.

## Recommendations

- Implement Milestone 2 before expanding local-client transport. This proves
  the immediate Pantograph path and keeps early blast radius low.
- Keep `PumasReadOnlyLibrary` narrow. It should only expose snapshot-style
  reads and never become a second owner lifecycle.
- Treat `PumasLocalClient` as a transport adapter over core contracts, not as a
  second API semantics layer.

## Completion Summary

### Completed

- Milestone 0 planning package setup:
  - moved the proposal into `docs/plans/pumas-fast-model-snapshot-and-client-api/proposal.md`;
  - added the directory README;
  - added this standards-compliant implementation plan;
  - updated plan index and superseded-plan references.
- Milestone 1 API role inventory and contract freeze:
  - classified current source, test, example, RPC, UniFFI, and documentation
    construction references;
  - documented migration blockers and anti-patterns in `caller-inventory.md`;
  - updated active docs to frame transparent `PumasApi` convergence as
    transitional compatibility.
- Milestone 2 Slice 2.1 selector contract foundation:
  - added `ModelLibrarySelectorSnapshot`, `ModelLibrarySelectorSnapshotRow`,
    request DTOs, entry/artifact readiness enums, and detail freshness enum;
  - extended `PumasModelRef` with `model_ref_contract_version` to clarify that
    the version is the model-reference contract version, not a model revision;
  - documented selector invariants in the models module README.
- Milestone 2 Slice 2.2 selector index projection:
  - added the first SQLite-backed selector snapshot projection under
    `ModelIndex`;
  - joined valid package-facts summaries without regenerating facts;
  - preserved rows when summaries are missing or invalid;
  - derived non-ready artifact and entry states from persisted partial,
    validation, and import metadata.
- Milestone 2 Slice 2.3 direct owner API:
  - added `ModelLibrary::model_library_selector_snapshot`;
  - added a primary-only transitional `PumasApi` method with no hidden IPC
    fallback;
  - covered the direct API path and proved it does not regenerate missing
    package-facts summaries.
- Milestone 2 Slice 2.4 read-only API:
  - added `PumasReadOnlyLibrary`;
  - added `ModelIndex::open_read_only`;
  - proved read-only selector access works against an existing index and does
    not create a missing database.
- Milestone 2 Slice 2.5 timing and consumer fixture:
  - added a warm 100-row direct/read-only timing test;
  - recorded timing results in `reports/selector-snapshot-performance.md`;
  - added `selector-snapshot-contract.md` and
    `fixtures/selector-snapshot-row.json` for Pantograph-facing lazy selector
    consumption.
- Milestone 3 selector lifecycle:
  - kept selector rows as a live projection over index/cache rows rather than
    adding a second materialized refresh lifecycle;
  - added tests proving selector output reflects model row updates and package
    summary cache updates without a selector-specific refresh job;
  - changed package-facts summary cache writes to emit model-library update
    events.
- Milestone 4 recovery-first subscription handoff:
  - added `ModelLibraryUpdateSubscription`;
  - added direct owner/API recovery from a snapshot cursor;
  - tested the snapshot-gap case and stale cursor behavior;
  - recorded that the live bus remains pending rather than hiding that gap
    behind the current polling SSE path.
- Milestone 4 core update bus:
  - added a broadcast sender/receiver to `ModelIndex`;
  - published model-library update events after durable append/commit;
  - exposed `ModelLibrary::subscribe_model_library_update_events`;
  - tested live publication for model upsert and transactional model-id
    replace.

### Deviations

- None yet.

### Follow-Ups

- None yet.

### Verification Summary

- Documentation-only planning slice.
- Checked against `PLAN-STANDARDS.md`, `DOCUMENTATION-STANDARDS.md`, and
  `templates/PLAN-TEMPLATE.md`.
- Milestone 1 verification was documentation/inventory-only; no compile was
  required because no source behavior changed.
- Milestone 2 Slice 2.1 verification:
  - `cargo fmt --manifest-path rust/Cargo.toml --all`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library model_library_selector`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library package_facts`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library model_ref`
- Milestone 2 Slice 2.2 verification:
  - `cargo fmt --manifest-path rust/Cargo.toml --all`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library selector_snapshot`
- Milestone 2 Slice 2.3 verification:
  - `cargo fmt --manifest-path rust/Cargo.toml --all`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library selector_snapshot`
- Milestone 2 Slice 2.4 verification:
  - `cargo fmt --manifest-path rust/Cargo.toml --all`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library read_only`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library selector_snapshot`
- Milestone 2 Slice 2.5 verification:
  - `cargo fmt --manifest-path rust/Cargo.toml --all`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library selector_snapshot_reports_warm_100_row_timing -- --nocapture`
- Milestone 3 verification:
  - `cargo fmt --manifest-path rust/Cargo.toml --all`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library selector_snapshot`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library package_facts`
- Milestone 4 recovery-first subscription verification:
  - `cargo fmt --manifest-path rust/Cargo.toml --all`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library subscribe_model_library_updates_since`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library model_library_update`
- Milestone 4 core update bus verification:
  - `cargo fmt --manifest-path rust/Cargo.toml --all`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library model_library_update_broadcast`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library model_library_update`
- Milestone 4 owner-side stream handoff verification:
  - `cargo fmt --manifest-path rust/Cargo.toml --all`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library subscribe_model_library_update_stream_since`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-library model_library_update`
- Milestone 5 GUI forwarding verification:
  - `cargo fmt --manifest-path rust/Cargo.toml --all`
  - `cargo check --manifest-path rust/Cargo.toml -p pumas-rpc`
  - `cargo test --manifest-path rust/Cargo.toml -p pumas-rpc model_library_update_event_stream`
  - `npm run -w electron validate`
  - `npm run -w electron test`
  - `npm run -w frontend test:run -- useModels.test.ts ModelManagerIntegrityRefresh.test.ts api-package-facts.test.ts`

### Traceability Links

- Proposal: `docs/plans/pumas-fast-model-snapshot-and-client-api/proposal.md`
- Directory README:
  `docs/plans/pumas-fast-model-snapshot-and-client-api/README.md`
- Module README updated: N/A until implementation touches source modules.
- ADR added/updated: N/A.
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: N/A until PR
  creation.
