# Plan: Model Library Integrity Event Refresh

## Objective

Make model-library integrity warnings update when backend-owned model-library
state changes, so frontend `ISSUE` tags and the "Library integrity warning"
header disappear after a successful migration or reconciliation only when fresh
backend data no longer reports the underlying integrity issue.

## Scope

### In Scope

- Reuse the existing model-library update feed as the backend-owned change
  signal for model list and integrity projection changes.
- Ensure successful migration and reconciliation flows emit model-library
  update events when they change indexed model rows, metadata, validation
  state, or model ids.
- Add a frontend synchronization path that listens for backend model-library
  change notifications, validates the contract, and refreshes model data from
  the backend.
- Keep integrity labels derived from backend `list_models`/search projections.
- Add tests that cover the full path from backend change signal to frontend
  model-list refresh.

### Out of Scope

- Adding a migration-specific "clear warning" command.
- Letting the frontend decide whether an integrity issue has been fixed.
- Replacing the existing model-library update feed with a separate event
  system.
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
because `MigrationReportsPanel` executes migration and refreshes migration
reports, but does not refresh the backend-owned model list. The correct fix is
not for migration to clear UI labels directly. The UI labels should clear when
the frontend receives or observes a backend model-library change, refetches
models, and the backend projection no longer reports the issue.

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

This plan should extend and consume that existing feed instead of creating a
new integrity-warning mechanism.

### Constraints

- Follow `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/`.
- Backend owns model-library data and integrity logic.
- Frontend holds transient view state only and refreshes from backend state.
- Cross-language contracts must remain append-only unless a breaking change is
  explicitly planned.
- Background work must have clear lifecycle ownership and cleanup.
- Implementation must proceed in validated thin vertical slices with atomic
  commits.

### Standards Alignment

- `ARCHITECTURE-PATTERNS.md`: backend-owned data remains authoritative; the
  frontend displays backend projections and sends actions.
- `FRONTEND-STANDARDS.md`: prefer event-driven synchronization over polling;
  source-of-truth changes should push or signal updates to consumers.
- `INTEROP-STANDARDS.md`: validate event payloads at the boundary and keep
  Rust/TypeScript contract updates together.
- `CONCURRENCY-STANDARDS.md`: use message passing or durable cursor state
  rather than shared mutable UI flags.
- `RUST-ASYNC-STANDARDS.md`: any watcher, bridge, or background task must be
  lifecycle-owned and cancellable.
- `TESTING-STANDARDS.md`: cross-layer behavior requires an acceptance test from
  backend producer input to frontend consumer output.
- `PLAN-STANDARDS.md`: record scope, risks, lifecycle, verification, re-plan
  triggers, and completion criteria.

### Assumptions

- `list_models`/search remains the canonical source for whether a model has an
  integrity issue.
- The existing update feed is the correct durable invalidation contract for
  model-library projections.
- Frontend startup can still perform an initial model fetch, so missed runtime
  events are recoverable.
- If no realtime transport exists yet, a short-lived cursor poller may be used
  only as an explicitly documented bridge with cleanup and overlap prevention.

### Dependencies

- `rust/crates/pumas-core/src/index/model_index/model_library_updates.rs`
- `rust/crates/pumas-core/src/model_library/library.rs`
- `rust/crates/pumas-core/src/model_library/library/migration.rs`
- `rust/crates/pumas-core/src/api/migration.rs`
- `rust/crates/pumas-core/src/api/state.rs`
- `rust/crates/pumas-rpc/src/handlers/models/imports.rs`
- `rust/crates/pumas-rpc/src/handlers/models/migration.rs`
- `electron/src/preload.ts`
- `electron/src/rpc-method-registry.ts`
- `frontend/src/api/models.ts`
- `frontend/src/types/api-package-facts.ts`
- `frontend/src/hooks/useModels.ts`
- `frontend/src/components/MigrationReportsPanel.tsx`
- `frontend/src/components/ModelManager.tsx`
- `frontend/src/utils/libraryModels.ts`

### Affected Structured Contracts

- `ModelLibraryUpdateEvent`
- `ModelLibraryUpdateFeed`
- `ModelLibraryChangeKind`
- `ModelLibraryRefreshScope`
- `list_model_library_updates_since` RPC/API shape
- Any new backend push or bridge event carrying a cursor, feed summary, or
  model-library invalidation signal

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| No existing realtime backend-to-frontend event transport exists | High | Reuse the durable cursor feed first; add a lifecycle-owned bridge or documented temporary poller as a thin slice |
| Migration emits too many per-model events | Medium | Emit one bounded summary event or coalesce frontend refreshes with debounce |
| Frontend refresh races with active search state | Medium | Preserve existing stale-response guards and define whether library events refresh full list, current search, or both |
| Cursor becomes stale while UI is closed | Low | Treat `snapshot_required` as a full model-list refresh |
| Event contract drifts between Rust and TypeScript | High | Update Rust DTOs, RPC bridge types, TS validators, and contract tests in one slice |
| Direct migration UI coupling is reintroduced | Medium | Keep migration report refresh separate from model-list refresh; integrity display remains derived from fresh model data |

## Definition of Done

- Successful migration or reconciliation that changes model-library integrity
  state advances the backend model-library update feed.
- Frontend receives or observes the backend model-library update and refreshes
  model data from backend-owned APIs.
- `ISSUE` badges and the integrity warning header disappear when fresh backend
  model data no longer includes integrity issue metadata.
- Stale cursor or missed event cases trigger a full snapshot refresh.
- Subscription, polling bridge, or event listener cleanup is covered by tests.
- Rust, RPC/Electron, and TypeScript contract surfaces remain aligned.

## Milestones

### Milestone 1: Event Feed Coverage Audit

**Goal:** Confirm which model-changing operations currently append
`model_library_update_events` and identify gaps relevant to integrity warnings.

**Tasks:**
- [ ] Audit migration execution paths for model row, metadata, validation, and
  id/path changes.
- [ ] Audit reconciliation, import, delete, metadata refetch, and scan flows
  for update-event emission.
- [ ] Record any operation that can change `list_models` output without
  advancing the update cursor.

**Verification:**
- Code inspection notes identify each producer path and whether it emits an
  event.
- Existing update-feed unit tests are mapped to the audited producer paths.

**Status:** Not started

### Milestone 2: Backend Producer Slice

**Goal:** Ensure successful migration/reconciliation changes that affect
frontend model-list or integrity projections emit durable backend update events.

**Tasks:**
- [ ] Add missing update-event emission at the application/domain boundary
  where model-library changes are committed.
- [ ] Prefer one coalesced event for batch migration completion when feasible.
- [ ] Use existing `ModelLibraryChangeKind`, `ModelFactFamily`, and
  `ModelLibraryRefreshScope` values where they fit.
- [ ] If the existing enum set cannot express the change, add append-only enum
  values and update all contract surfaces in the same slice.

**Verification:**
- Rust tests prove the update cursor advances after a migration or
  reconciliation that changes indexed model state.
- Rust tests prove no event is emitted for a no-op migration.
- Existing migration tests still pass.

**Status:** Not started

### Milestone 3: Frontend Synchronization Slice

**Goal:** Add a frontend hook or model-state integration that refreshes model
data when the backend model-library feed advances.

**Tasks:**
- [ ] Add a focused hook, for example `useModelLibraryUpdates`, that owns
  cursor state, event/feed validation, debounce, and cleanup.
- [ ] Integrate the hook with `useModels` or `App` so model-list refresh is
  centralized.
- [ ] On `snapshot_required` or stale cursor, perform a full model refresh.
- [ ] Avoid migration-specific callbacks for clearing issue labels.
- [ ] Preserve stale async response guards so old refreshes cannot overwrite
  newer model state.

**Verification:**
- Frontend tests cover update receipt, debounced refresh, stale cursor snapshot
  refresh, and cleanup on unmount.
- Typecheck passes for API bridge and frontend model types.

**Status:** Not started

### Milestone 4: Backend-To-Frontend Delivery

**Goal:** Provide a standards-compliant delivery mechanism from backend-owned
model-library changes to the frontend.

**Tasks:**
- [ ] Prefer an existing app event bridge if one is available.
- [ ] If no push bridge exists, implement the smallest lifecycle-owned bridge
  over the existing cursor feed.
- [ ] If a temporary poller is required, document why, prevent overlapping
  polls, clear timers on unmount, and add a re-plan trigger for replacing it.
- [ ] Keep transport payloads minimal: cursor, snapshot requirement, and enough
  event metadata to decide whether model summaries need refresh.

**Verification:**
- Integration or component test proves backend feed advancement causes a
  frontend model refresh.
- Tests cover listener or timer cleanup.
- No unowned background task is introduced in Rust or Electron.

**Status:** Not started

### Milestone 5: Integrity Warning Acceptance

**Goal:** Prove the user-visible warning clears through backend-derived state,
not through direct UI mutation.

**Tasks:**
- [ ] Build a fixture or mocked API sequence where initial `list_models`
  returns duplicate integrity metadata.
- [ ] Simulate successful backend migration/reconciliation and update-feed
  advancement.
- [ ] Return fresh `list_models` data without integrity metadata.
- [ ] Verify the `ISSUE` badge and library integrity header disappear.

**Verification:**
- Frontend acceptance test covers warning present, backend update received,
  model data refreshed, warning absent.
- Backend tests cover the corresponding producer event.
- Run targeted Rust and frontend tests for touched modules.

**Status:** Not started

## Execution Notes

Update during implementation:

- 2026-05-04: Planning established that backend-derived model metadata already
  owns integrity labels, and the existing model-library update feed is the
  right invalidation contract to reuse.

## Commit Cadence Notes

- Commit after each verified logical slice.
- Keep backend producer fixes separate from frontend synchronization work.
- Keep contract additions and their Rust/TypeScript bridge updates in the same
  commit.
- Use standard commit format from `COMMIT-STANDARDS.md`.

## Optional Subagent Assignment

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| Backend worker | Audit and patch update-event producer gaps | File list, tests run, event semantics summary | Before frontend hook depends on event shape |
| Frontend worker | Implement hook and component tests | File list, tests run, cleanup behavior summary | After backend event/feed contract is stable |
| Verification worker | Cross-layer acceptance and regression review | Test commands, failures, residual risk notes | Before final slice commit |

## Re-Plan Triggers

- The existing model-library update feed cannot represent integrity-affecting
  changes without a breaking contract change.
- No safe backend-to-frontend delivery mechanism exists and a temporary poller
  would become permanent architecture.
- Migration changes model-library state outside the SQLite index/update-event
  transaction boundary.
- The frontend has multiple independent model state owners that cannot share a
  single refresh path without broader refactoring.
- Native bindings or RPC consumers rely on closed enum handling that would make
  append-only event kinds unsafe.

## Recommendations

- Reuse `list_model_library_updates_since` as the durable invalidation source.
  This keeps the fix aligned with the existing backend contract and avoids a
  parallel integrity-warning system.
- Treat direct migration-to-UI refresh callbacks as a fallback only. They solve
  the immediate panel case but do not handle other backend changes that can
  alter integrity labels.
- If push transport is missing, implement the frontend side behind a hook whose
  public behavior is event-driven. A temporary cursor poller can live inside
  that hook with explicit cleanup and a replacement trigger.

## Completion Summary

### Completed

- Not started.

### Deviations

- None.

### Follow-Ups

- None yet.

### Verification Summary

- Not run.

### Traceability Links

- Module README updated: N/A until implementation touches a module contract.
- ADR added/updated: N/A unless the delivery mechanism introduces a new
  backend-to-frontend event architecture.
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: N/A until PR.
