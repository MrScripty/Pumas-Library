# Plan: Model Library Integrity Event Refresh

## Objective

Make model-library integrity warnings update from backend-owned state changes.
When migration, reconciliation, import, delete, or metadata repair changes the
model library, the backend must publish a model-library update notification to
the frontend. The frontend then refreshes model data from the backend, and
`ISSUE` tags plus the "Library integrity warning" header disappear only when
fresh backend `list_models` or search data no longer reports the underlying
integrity issue.

## Scope

### In Scope

- Reuse the existing SQLite `model_library_update_events` feed as the durable
  backend outbox for model-library changes.
- Harden producer coverage where backend flows can change `list_models`
  results without advancing the update feed.
- Add a backend-owned notification delivery layer over the durable feed.
- Expose a realtime backend-to-frontend delivery path through `pumas-rpc`,
  Electron main/preload, and the renderer API contract.
- Add frontend subscription handling that validates notifications and refreshes
  model data from backend-owned APIs.
- Keep integrity labels derived from backend `list_models`/search projections.
- Add tests covering the full path from backend producer event to frontend
  warning removal.

### Out of Scope

- Adding a migration-specific "clear warning" command.
- Letting the frontend decide whether an integrity issue has been fixed.
- Replacing the existing model-library update feed with a separate registry.
- Making a frontend-owned cursor poller the target architecture.
- Changing the meaning of `integrity_issue_duplicate_repo_id` metadata fields.
- Reworking unrelated polling hooks that are not model-library state.

## Inputs

### Problem

The frontend currently displays duplicate/integrity warnings from backend model
metadata:

- `frontend/src/utils/libraryModels.ts` maps backend metadata fields into
  `hasIntegrityIssue`.
- `frontend/src/components/LocalModelNameButton.tsx` renders the `ISSUE` badge.
- `frontend/src/components/ModelManager.tsx` renders the library integrity
  warning header from grouped local models.

The backend injects duplicate repo issue metadata during model listing/search:

- `rust/crates/pumas-core/src/model_library/library.rs`
  `annotate_and_dedupe_records_by_repo_id` derives duplicate repo integrity
  fields at read/search time.

After migration succeeds, the frontend may still show stale issue labels
because `MigrationReportsPanel` refreshes migration reports, not the canonical
model list. The correct fix is not for migration to clear UI labels directly.
The UI labels should clear when the backend publishes that model-library state
changed, the frontend refreshes models from the backend, and the backend
projection no longer reports the issue.

### Existing Mechanism

The codebase already has a durable model-library update feed:

- `model_library_update_events` table.
- `list_model_library_updates_since` API/RPC method.
- Cursor format `model-library-updates:<event_id>`.
- `stale_cursor` and `snapshot_required` handling.
- Rust and TypeScript contract types:
  - `ModelLibraryUpdateEvent`
  - `ModelLibraryUpdateFeed`
  - `ModelLibraryChangeKind`
  - `ModelLibraryRefreshScope`

The codebase does not currently have an end-to-end realtime push path:

- `pumas-rpc` exposes `GET /health` and `POST /rpc`, not SSE/WebSocket.
- Electron main exposes request/response IPC through `api:call`.
- Preload exposes `list_model_library_updates_since`, but no subscription API.
- `PythonBridge.call()` buffers one JSON-RPC response and is not a stream
  transport.

This plan should keep the existing feed as the durable backend outbox and add a
backend-owned delivery layer. A frontend cursor poller is rejected as the target
architecture because it moves synchronization ownership into the UI.

### Producer Audit Findings

Producer coverage is mostly present because `ModelIndex::upsert`,
`ModelIndex::delete`, and `ModelIndex::replace_model_id_preserving_references`
append update events. Migration moves/splits now route through the
reference-preserving remap path. Reconciliation, imports, delete, metadata
refetch, review, notes, and settings flows generally use `index_model_dir`,
`upsert`, or `delete`.

Known gaps to address:

- `ModelLibrary::deep_scan_rebuild` can clear and rebuild index state through
  `ModelIndex::clear`; removal-only scans can change `list_models` without
  advancing the update feed.
- `ModelImporter::import_in_place` can return early when `metadata.json`
  already exists and fail to repair a missing SQLite row.
- Flow-level tests for migration, reconciliation, import, delete, metadata
  refetch, and scan cursor advancement are sparse.

### Constraints

- Follow `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/`.
- Backend owns model-library data, integrity logic, and update delivery.
- Frontend holds transient view state only and refreshes from backend state.
- Cross-language contracts must remain append-only unless a breaking change is
  explicitly planned.
- Background tasks, stream clients, and subscriptions must have clear lifecycle
  ownership and cleanup.
- Core/domain crates must not depend on transport-specific SSE/Electron types.
- Implementation must proceed in validated thin vertical slices with atomic
  commits.

### Standards Alignment

- `ARCHITECTURE-PATTERNS.md`: backend-owned data remains authoritative; the
  frontend displays backend projections and sends actions.
- `FRONTEND-STANDARDS.md`: prefer event-driven synchronization over polling;
  source-of-truth changes should push updates to consumers.
- `INTEROP-STANDARDS.md`: validate event payloads at the boundary and keep
  Rust/TypeScript contract updates together.
- `CONCURRENCY-STANDARDS.md`: use message passing and durable cursor state
  rather than shared mutable UI flags.
- `RUST-ASYNC-STANDARDS.md`: SSE producers, backend outbox tailers, and Electron
  stream clients must be lifecycle-owned and cancellable.
- `TESTING-STANDARDS.md`: cross-layer behavior requires an acceptance test from
  backend producer input to frontend consumer output.
- `PLAN-STANDARDS.md`: record scope, blast radius, lifecycle, verification,
  risks, re-plan triggers, and completion criteria.

### Assumptions

- `list_models`/search remains the canonical source for whether a model has an
  integrity issue.
- The existing update feed is the correct durable invalidation contract for
  model-library projections.
- Frontend startup can still perform an initial model fetch, so missed runtime
  notifications are recoverable.
- Server-sent events are the thinnest realtime transport because this is a
  one-way invalidation stream over the existing local HTTP backend.
- Electron main should own the backend stream connection and forward validated
  notifications to the renderer through preload, rather than letting renderer
  code discover backend ports or talk directly to the sidecar.

### Dependencies

- `rust/crates/pumas-core/src/index/model_index/model_library_updates.rs`
- `rust/crates/pumas-core/src/index/model_index.rs`
- `rust/crates/pumas-core/src/model_library/library.rs`
- `rust/crates/pumas-core/src/model_library/library/migration.rs`
- `rust/crates/pumas-core/src/model_library/importer.rs`
- `rust/crates/pumas-core/src/api/reconciliation.rs`
- `rust/crates/pumas-core/src/api/migration.rs`
- `rust/crates/pumas-core/src/api/state.rs`
- `rust/crates/pumas-rpc/src/server.rs`
- `rust/crates/pumas-rpc/src/handlers/mod.rs`
- `rust/crates/pumas-rpc/Cargo.toml`
- `electron/src/python-bridge.ts`
- `electron/src/main.ts`
- `electron/src/preload.ts`
- `frontend/src/types/api-electron.ts`
- `frontend/src/types/api-bridge-models.ts`
- `frontend/src/api/models.ts`
- `frontend/src/hooks/useModels.ts`
- `frontend/src/components/MigrationReportsPanel.tsx`
- `frontend/src/components/ModelManager.tsx`
- `frontend/src/utils/libraryModels.ts`

### Affected Structured Contracts

- Existing:
  - `ModelLibraryUpdateEvent`
  - `ModelLibraryUpdateFeed`
  - `ModelLibraryChangeKind`
  - `ModelLibraryRefreshScope`
  - `list_model_library_updates_since` RPC/API shape
- New append-only contracts:
  - `ModelLibraryUpdateNotification` backend stream payload.
  - SSE event name and payload shape, for example
    `event: model-library-update`.
  - Electron/preload subscription method, for example
    `onModelLibraryUpdate(callback): () => void`.
  - Frontend notification type and validator.

### Codebase Blast Radius Review

Checked against the current codebase on 2026-05-04.

| Area | Current Surface | Blast Radius | Standards Guardrail |
| ---- | --------------- | ------------ | ------------------- |
| Durable producer feed | `ModelIndex::upsert`, `delete`, `replace_model_id_preserving_references`, `model_library_update_events` | Changes affect every backend model list/search invalidation consumer | Keep feed append-only; add tests before changing enum values or cursor semantics |
| Producer gaps | `deep_scan_rebuild`, `ModelIndex::clear`, `ModelImporter::import_in_place` early return | Missing events can leave frontend stale even with a correct stream | Fix at backend producer boundary; do not add frontend special cases |
| Backend delivery | `pumas-rpc` Axum server and `AppState` | New route/task/subscription lifecycle affects sidecar shutdown and local HTTP CORS | Add a focused SSE/outbox module; no transport code in core/domain crates; own cancellation |
| Backend dependencies | `pumas-rpc/Cargo.toml` | SSE stream implementation may require a new crate or feature | Prefer existing Axum/Tokio primitives; justify any dependency per `DEPENDENCY-STANDARDS.md` |
| Electron bridge | `PythonBridge`, `main.ts`, `preload.ts` | Main process must manage a long-lived backend stream and renderer listeners | Main owns stream lifecycle; preload exposes subscribe/unsubscribe only; validate payloads |
| Frontend model owner | `useModels`, `App`, `ModelManager` | Refresh can race with active FTS state or overwrite filtered results | Centralize refresh in `useModels`; add stale response guards; avoid component-level mutation |
| Integrity display | `libraryModels.ts`, `LocalModelNameButton`, `ModelManager` | Warning behavior should change only through fresh backend projections | Keep display components derived; no direct clearing commands |
| Tests | Rust core/API, pumas-rpc, Electron, frontend hooks/components | Cross-layer changes can pass unit tests while failing the UI refresh path | Add producer, stream bridge, preload subscription, hook, and acceptance tests |
| Documentation | module READMEs and this plan | New backend event architecture changes ownership rules | Update module README/contract docs in the same slice as implementation |

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| Backend stream implementation introduces unowned background tasks | High | Make the outbox tailer/SSE stream lifecycle-owned by `ServerHandle`, `AppState`, or request cancellation |
| Event contract drifts between Rust, Electron, and TypeScript | High | Add append-only DTOs and validators in one slice with contract tests |
| Migration emits many per-model events | Medium | Stream cursor advancement/coalesced feed summaries; frontend debounces refreshes |
| Frontend refresh races with active search state | Medium | Add stale response guards and define current-search revalidation behavior |
| Missed events while UI is closed | Low | Initial fetch remains mandatory; stale cursor/snapshot semantics remain available |
| Frontend polling returns as permanent architecture | Medium | Mark frontend poller as rejected target; require explicit re-plan for any temporary bridge |
| SSE is blocked by packaged Electron constraints | Medium | Keep delivery behind Electron main/preload abstraction so transport can change without frontend churn |

## Definition of Done

- Backend producer gaps that can change `list_models` without update-feed
  advancement are fixed or explicitly blocked.
- Successful migration or reconciliation that changes model-library integrity
  state advances the backend model-library update feed.
- `pumas-rpc` exposes a lifecycle-owned backend event stream or equivalent
  backend-pushed delivery path over the durable feed.
- Electron main/preload exposes a validated subscribe/unsubscribe API to the
  renderer.
- Frontend receives backend-pushed model-library notifications and refreshes
  model data from backend-owned APIs.
- `ISSUE` badges and the integrity warning header disappear when fresh backend
  model data no longer includes integrity issue metadata.
- Stream cancellation, listener cleanup, stale cursor/snapshot fallback, and
  refresh debounce behavior are covered by tests.
- Rust, RPC/Electron, and TypeScript contract surfaces remain aligned.

## Milestones

### Milestone 1: Producer Coverage Hardening

**Goal:** Ensure model-library changes that affect `list_models` advance the
durable update feed.

**Tasks:**
- [x] Add flow-level tests proving migration move/split advances
  `list_model_library_updates_since`.
- [x] Add a no-op migration test proving no unnecessary event is emitted.
- [x] Fix or explicitly block `deep_scan_rebuild` removal-only update gaps.
- [x] Fix or explicitly document `import_in_place` behavior when metadata
  exists but the SQLite row is missing.
- [x] Add a test for delete cursor advancement.
- [x] Add a test for metadata refetch cursor advancement.
- [x] Add a test for reconciliation cursor advancement.

**Verification:**
- Targeted Rust tests for update-feed producer paths.
- Existing migration/reconciliation/import tests still pass.

**Status:** Complete.

### Milestone 2: Backend Notification Contract

**Goal:** Define the append-only backend notification payload that transports
durable feed advancement without exposing UI decisions.

**Tasks:**
- [x] Add a Rust DTO for model-library update notifications.
- [x] Include cursor and enough feed summary data for consumers to decide
  whether to refresh model summaries.
- [x] Preserve `list_model_library_updates_since` as the durable recovery API.
- [x] Add TypeScript DTO/validator for the Electron/preload boundary.

**Verification:**
- Rust serialization tests.
- TypeScript validator tests.
- Contract fixture or snapshot if existing test patterns support it.

**Status:** Complete.

### Milestone 3: Backend-Pushed Delivery

**Goal:** Add a lifecycle-owned backend delivery path from the durable update
feed to Electron.

**Tasks:**
- [x] Add an SSE or equivalent one-way stream route in `pumas-rpc`.
- [x] Implement a backend-owned outbox tailer or notifier that reads the update
  feed and emits notifications when the cursor advances.
- [x] Prevent overlapping feed reads and coalesce bursts.
- [x] Ensure stream/task cancellation on client disconnect and server shutdown.
- [x] Keep this code in RPC/application layers, not core/domain crates.

**Verification:**
- `pumas-rpc` tests prove a feed advancement produces a stream notification.
- Shutdown/disconnect tests prove no unowned task remains.
- `cargo test` for touched Rust crates.

**Status:** Complete.

### Milestone 4: Electron Subscription Bridge

**Goal:** Bridge backend-pushed model-library notifications into the renderer
through a validated preload API.

**Tasks:**
- [ ] Add `PythonBridge` stream subscription lifecycle methods.
- [ ] Add Electron main IPC fan-out for model-library notifications.
- [ ] Add preload `onModelLibraryUpdate` subscribe/unsubscribe API.
- [ ] Validate payloads before invoking renderer callbacks.
- [ ] Reconnect or fail visibly if the backend sidecar restarts.

**Verification:**
- Electron tests cover subscription, unsubscribe cleanup, invalid payload
  rejection, and backend restart cleanup.
- `npm run -w electron validate`.

**Status:** Not started.

### Milestone 5: Frontend Refresh Integration

**Goal:** Refresh canonical frontend model state from backend data when a
backend notification arrives.

**Tasks:**
- [ ] Add a focused frontend subscription hook that consumes the preload
  subscription API.
- [ ] Wire the hook into `useModels` or `App`, the existing model-state owner.
- [ ] Debounce refreshes and add stale fetch guards.
- [ ] Preserve or explicitly revalidate active FTS search state.
- [ ] Avoid direct warning/tag mutation.

**Verification:**
- Frontend hook tests cover notification receipt, debounce, stale response
  discard, active-search behavior, and cleanup on unmount.
- Typecheck passes.

**Status:** Not started.

### Milestone 6: Integrity Warning Acceptance

**Goal:** Prove the user-visible warning clears through backend-derived state,
not through direct UI mutation.

**Tasks:**
- [ ] Build a fixture or mocked API sequence where initial `list_models`
  returns duplicate integrity metadata.
- [ ] Simulate successful backend migration/reconciliation and backend-pushed
  update notification.
- [ ] Return fresh `list_models` data without integrity metadata.
- [ ] Verify the `ISSUE` badge and library integrity header disappear.

**Verification:**
- Frontend acceptance test covers warning present, backend notification
  received, model data refreshed, warning absent.
- Backend tests cover the corresponding producer event.
- Targeted Rust, Electron, and frontend tests pass.

**Status:** Not started.

## Execution Notes

Update during implementation:

- 2026-05-04: Planning established that backend-derived model metadata already
  owns integrity labels, and the existing model-library update feed is the
  right durable invalidation contract to reuse.
- 2026-05-04: A frontend cursor poller was considered and rejected as the
  target architecture because it keeps synchronization ownership in the
  renderer. The revised target is backend durable outbox plus backend-pushed
  delivery through RPC/Electron subscription boundaries.
- 2026-05-04: Audit found most producer paths already append events through
  `ModelIndex`, but `deep_scan_rebuild` and `import_in_place` have gap cases
  that need producer hardening or explicit contracts.
- 2026-05-04: `ModelIndex::clear` now emits `ModelRemoved` update-feed events
  for removed rows, covering `deep_scan_rebuild` removal-only state changes.
- 2026-05-04: `import_in_place` now repairs an existing metadata-backed model
  directory that is missing from SQLite by indexing it through `index_model_dir`.
- 2026-05-04: Migration move, split, and no-op flows now have update-feed
  regression tests.
- 2026-05-04: `delete_model` now has an update-feed regression test proving it
  emits `ModelRemoved`.
- 2026-05-04: `update_metadata_from_hf` now has an update-feed regression test
  covering the metadata-refetch producer boundary.
- 2026-05-04: Public `list_models` on-demand reconciliation now has an
  update-feed regression test.
- 2026-05-04: Added append-only Rust and TypeScript
  `ModelLibraryUpdateNotification` contracts with runtime validation for the
  future Electron/preload subscription boundary.
- 2026-05-04: `pumas-rpc` now exposes a backend-owned SSE stream at
  `/events/model-library-updates` that tails the durable update feed and emits
  `model-library-update` notifications.
- 2026-05-04: Broader `execute_migration_with_checkpoint` test filtering
  exposed an existing failure in
  `test_execute_migration_with_checkpoint_skips_partial_split_directories`:
  current execution reports the partial split as skipped but no longer produces
  the expected integrity error count. This should be resolved in a separate
  migration-validation slice.

## Commit Cadence Notes

- Commit after each verified logical slice.
- Keep producer hardening separate from stream delivery and frontend
  subscription work.
- Keep contract additions and their Rust/Electron/TypeScript bridge updates in
  the same commit.
- Use standard commit format from `COMMIT-STANDARDS.md`.

## Optional Subagent Assignment

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| Backend worker | Producer gaps and update-feed tests | File list, tests run, event semantics summary | Before stream contract depends on producer coverage |
| RPC/Electron worker | SSE/outbox and preload subscription bridge | File list, tests run, lifecycle cleanup summary | Before frontend hook consumes subscription API |
| Frontend worker | Subscription hook and UI acceptance | File list, tests run, refresh behavior summary | After preload contract is stable |
| Verification worker | Cross-layer regression review | Test commands, failures, residual risk notes | Before final slice commit |

## Re-Plan Triggers

- The existing model-library update feed cannot represent integrity-affecting
  changes without a breaking contract change.
- SSE cannot be made reliable in packaged Electron without broader transport
  work.
- Backend outbox tailing requires a dependency that fails
  `DEPENDENCY-STANDARDS.md` review.
- Migration changes model-library state outside the SQLite index/update-event
  transaction boundary.
- The frontend has multiple independent model state owners that cannot share a
  single refresh path without broader refactoring.
- Native bindings or RPC consumers rely on closed enum handling that would make
  append-only event kinds unsafe.

## Recommendations

- Reuse `list_model_library_updates_since` as the durable recovery and
  invalidation source.
- Implement backend-pushed delivery behind RPC/Electron subscription
  boundaries. This preserves backend ownership while keeping renderer code
  decoupled from the sidecar port and transport details.
- Do not implement a frontend cursor poller unless explicitly approved as a
  temporary bridge with a removal milestone.
- Keep integrity display code unchanged unless tests show it is rendering stale
  props after canonical model state refreshes.

## Completion Summary

### Completed

- Initial plan document committed in `a02cd40`.
- Read-only audits completed for producer coverage, frontend synchronization,
  and transport availability.
- `ModelIndex::clear` removal events implemented and covered by targeted unit
  test.
- `import_in_place` missing SQLite row repair implemented and covered by
  targeted unit test.
- Migration move, split, and no-op update-feed behavior covered by targeted
  flow tests.
- Delete update-feed behavior covered by targeted flow test.
- Metadata refresh update-feed behavior covered by targeted flow test.
- Reconciliation update-feed behavior covered by targeted API flow test.
- Backend notification DTO and frontend validator implemented and tested.
- Backend SSE delivery implemented in `pumas-rpc` and covered by integration
  test.

### Deviations

- The original plan allowed a temporary frontend cursor poller. Deeper
  codebase review found a backend-pushed route is more standards-compliant, so
  the target architecture changed.

### Follow-Ups

- Remove any draft frontend poller edits before implementation resumes.
- Resolve or update the partial-split migration validation expectation:
  `test_execute_migration_with_checkpoint_skips_partial_split_directories`
  currently fails on `report.error_count >= 1`.
- Update module READMEs when stream delivery and subscription contracts are
  implemented.

### Verification Summary

- `cargo test --manifest-path rust/Cargo.toml -p pumas-library test_clear_appends_model_library_update_events`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library test_import_in_place_indexes_existing_metadata_when_sqlite_row_missing`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library update_feed`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library no_op_does_not_emit_update_events`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library test_delete_model_advances_update_feed`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library test_update_metadata_from_hf_advances_update_feed`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library test_list_models_reconciliation_advances_update_feed`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library model_library_update_notification`
- `npm run -w frontend test:run -- api-package-facts.test.ts`
- `npm run -w frontend check:types`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-rpc test_model_library_update_event_stream_emits_after_reconcile`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library test_execute_migration_with_checkpoint_skips_partial_split_directories`
  failed; recorded as a follow-up migration-validation issue.

### Traceability Links

- Module README updated: N/A until implementation touches a module contract.
- ADR added/updated: N/A unless SSE/event delivery becomes a stable
  architecture decision requiring durable design record.
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: N/A until PR.
