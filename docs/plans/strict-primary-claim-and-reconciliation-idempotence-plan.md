# Plan: Strict Primary Claim and Reconciliation Idempotence

## Objective

Eliminate steady-state reconciliation writes for unchanged models and enforce a
single-primary-per-launcher-root runtime contract so all other instances attach
as clients.

## Scope

### In Scope

- Reconciliation idempotence for already-imported models
- Strict cross-process primary claim for a launcher root
- Startup and attach behavior in Rust and UniFFI entrypoints
- Registry and architecture documentation updates
- Concurrency and regression coverage for reconcile and startup races

### Out of Scope

- IPC transport replacement
- Multi-host or distributed coordination
- Unrelated model-library schema or runtime feature work
- UI redesign beyond any necessary wording updates

## Inputs

### Problem

- Targeted reconciliation currently permits write-side normalization on
  already-imported models.
- `kitten-tts` and `sd-turbo` exposed a concrete idempotence bug where
  reconcile re-persisted derived dependency state.
- Primary selection is currently best-effort, so concurrent starters can create
  multiple primaries for the same launcher root.

### Constraints

- Prefer extending existing registry, IPC, reconciliation, and model-library
  systems.
- Preserve existing public facades where feasible.
- Do not start watcher, reconciliation, or other primary-owned background work
  before primary ownership is secured.
- Documentation must match actual runtime guarantees after implementation.

### Assumptions

- The strict singleton guarantee is scoped per launcher root.
- Existing embedders prefer compatibility-preserving behavior changes where
  possible.
- Registry-backed coordination remains local-machine only.

### Dependencies

- `registry.db` and `LibraryRegistry`
- `PumasApi::new`, `PumasApi::discover`, and `start_ipc_server`
- UniFFI startup constructors
- Model-library reconcile and index paths
- Coding standards for planning, testing, tooling, and documentation

### Affected Structured Contracts

- Reconciliation scope and dirtiness contract
- Watcher-to-reconcile event routing contract
- Primary claim and startup lifecycle contract
- SQLite-canonical model-state contract for derived `metadata.json`

### Affected Persisted Artifacts

- `models.db`, `models.db-wal`, `models.db-shm`
- Per-model `metadata.json`
- Dependency binding/profile/history tables
- Registry instance-claim rows and launcher-root ownership records

### Concurrency/Race-Risk Review

- Startup claim, watcher startup, and reconcile scheduling must remain
  single-owner per launcher root.
- Watcher-triggered reconcile must not loop on Pumas-owned derived writes.
- Dirty markers, cooldowns, and startup freshness must remain coherent when
  watcher events overlap with direct API-triggered reconcile.
- Any suppression or lifecycle state introduced for watcher idempotence must be
  bounded, primary-owned, and cleaned up on shutdown.

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| Hidden reconcile write paths remain after fixing the known autobind bug | High | Audit all reconcile-time persistence paths and add unchanged-state regressions |
| Strict claim logic leaves stale ownership after crash | High | Define claim expiry and recovery semantics and test crash and stale takeover paths |
| Compatibility impact for callers using `PumasApi::new()` directly | High | Make facade decision explicit before implementation and stage behavior changes if required |
| Primary and client race still exists during claim-to-IPC handoff | High | Define an atomic claim lifecycle with bounded attach and retry behavior and explicit startup sequencing |
| Docs drift from implemented behavior | Medium | Update architecture and registry docs in the same milestone as behavior changes |

## Clarifying Questions (Only If Needed)

- None. Current repo state is sufficient to plan.

## Definition of Done

- Repeating reconcile on unchanged models causes no metadata rewrite, no
  binding churn, and no history growth.
- Exactly one process can own primary status for a launcher root at a time.
- Concurrent starters converge deterministically to one primary and the rest
  clients.
- Watcher, reconciliation, and process-owning background tasks are started only
  by the winning primary.
- Architecture and registry docs describe the enforced singleton contract
  accurately.
- Regression coverage exists for unchanged reconcile and concurrent startup and
  claim behavior.

## Ownership And Lifecycle Note

- The winning primary owns watcher startup, reconcile scheduling, IPC server
  startup, and any background repair work for its launcher root.
- Client instances must not start primary-owned background work.
- Startup repair work must be evidence-driven and skipped when the launcher
  root is already clean.
- Any watcher suppression for Pumas-owned derived writes must be in-memory,
  primary-local, and automatically expired or cleared on shutdown.

## Public Facade Preservation Note

- Facade-first preservation is required for `PumasApi`, IPC handlers, and
  UniFFI constructors unless a re-plan trigger is hit.
- Internal startup/reconcile semantics may change to enforce idempotence and
  singleton guarantees, but host-facing APIs should remain compatible.

## Milestones

### Milestone 1: Lock Runtime Contract

**Goal:** Define the intended singleton and reconcile-idempotence contract
before implementation.

**Tasks:**
- [ ] Record the strict runtime contract for single primary per launcher root.
- [ ] Record the reconcile contract for unchanged imported models: no persisted
  side effects.
- [ ] Record that SQLite is the source of truth and `metadata.json` is a
  derived projection that should change only when derived content changes.
- [ ] Define watcher handling for Pumas-owned derived writes and the ownership
  and expiry rules for any suppression state.
- [ ] Decide facade-first behavior for `PumasApi::new()`, `discover()`, and
  UniFFI constructors.
- [ ] Add ownership and lifecycle notes for claim acquisition, background task
  startup, shutdown, stale-claim cleanup, and retry and attach flow.
- [ ] Identify affected structured contracts and persisted artifacts in this
  plan file.

**Verification:**
- Plan review against `PLAN-STANDARDS.md`
- Concurrency review against `CONCURRENCY-STANDARDS.md`
- Cross-check contract against current architecture docs and startup code paths

**Status:** Complete

### Milestone 2: Audit and Fix Reconcile Write Semantics

**Goal:** Make reconciliation idempotent when model state is already current.

**Tasks:**
- [ ] Audit all reconcile-triggered persistence paths, including metadata
  projection, dependency profile persistence, dependency binding persistence,
  external validation refresh, and reclassify-triggered rewrites.
- [ ] Classify each path as must-write or must-no-op when unchanged.
- [ ] Add watcher self-write suppression so Pumas-owned derived metadata writes
  do not re-dirty unchanged model scopes.
- [ ] Add targeted reconcile prechecks so unchanged model scopes can exit
  without index or reclassify work.
- [ ] Implement no-op behavior for unchanged derived runtime state and other
  steady-state projections.
- [ ] Add focused regressions for repeated reconcile on `kitten-tts`,
  `sd-turbo`, and one normal model.
- [ ] Verify history tables and metadata files remain stable after repeated
  reconcile.

**Verification:**
- Targeted Rust unit and integration tests for repeated reconcile
- Cross-layer acceptance coverage from watcher event to reconcile outcome
- File and database state assertions showing no new writes on unchanged state
- Existing formatting and tooling checks per repo standards

**Status:** Not started

### Milestone 3: Enforce Strict Cross-Process Primary Claim

**Goal:** Ensure only one primary can own a launcher root at any given time.

**Tasks:**
- [ ] Design an atomic claim protocol using the existing registry as the single
  coordination source.
- [ ] Define claim record semantics, stale detection, recovery, and attach and
  retry behavior.
- [ ] Change startup sequencing so primary-owned background work starts only
  after claim success.
- [ ] Update Rust entrypoints so direct construction cannot silently create
  competing primaries for the same root.
- [ ] Update UniFFI startup to use the same claim and attach contract.
- [ ] Ensure losing contenders either attach to the winner or fail cleanly
  without touching model-library ownership paths.

**Verification:**
- Concurrent startup tests with multiple processes racing one launcher root
- Crash and stale-claim recovery tests
- Assertions that only one process starts watcher, reconciliation, and
  IPC-primary ownership paths

**Status:** Not started

### Milestone 4: Align Documentation and Consumer Guidance

**Goal:** Make the codebase and docs reflect the same enforced contract.

**Tasks:**
- [ ] Update architecture docs to replace best-effort singleton wording with
  the implemented guarantee.
- [ ] Update registry docs to describe claim semantics, recovery behavior, and
  client attachment expectations.
- [ ] Document constructor semantics for embedders and host applications.
- [ ] Update module READMEs for API and model-library directories so lifecycle,
  invariants, and SQLite-canonical / derived-metadata contracts remain explicit.
- [ ] Add troubleshooting guidance for DB lock symptoms, stale claims, and
  startup race diagnostics.
- [ ] Ensure traceability pointers required by documentation standards are
  included.

**Verification:**
- Documentation review against implemented behavior
- Traceability review per documentation standards
- Consistency pass across architecture, registry, and API-facing docs

**Status:** Not started

## Execution Notes

Update during implementation:
- 2026-03-10: Plan created before continuing code changes so singleton and
  reconcile contracts are explicit in-repo.
- 2026-03-11: Added explicit SQLite-canonical / derived-metadata contract,
  watcher lifecycle ownership, and structured-contract coverage to align the
  plan with Coding-Standards planning and documentation requirements.

## Commit Cadence Notes

- Commit after each verified logical slice:
- contract and plan scaffolding
- reconcile-idempotence fixes
- primary-claim enforcement
- final documentation and regression completion

## Optional Subagent Assignment

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| None | None | None | None |

## Re-Plan Triggers

- `PumasApi::new()` cannot preserve compatibility without ambiguous ownership
  semantics
- Registry claim design requires persisted schema or migration work beyond the
  current scope
- Additional reconcile-time writers are discovered that materially change
  milestone sequencing
- Cross-platform stale-claim recovery proves less reliable than assumed
- Documentation contract changes require ADR-level recording beyond a plan
  update

## Recommendations (Only If Better Option Exists)

- Use the existing registry as the only cross-process claim authority to avoid
  split-brain coordination.
- Treat primary-owned lifecycle work as a formal invariant: watcher,
  reconciliation scheduler, and server registration start only after claim

## Completion Summary

### Completed

- Milestone 1 planning contract capture and standards alignment updates.

### Deviations

- None yet.

### Follow-Ups

- Complete Milestone 2 watcher/reconcile idempotence implementation.
- Complete Milestone 3 strict primary-claim enforcement.
- Complete Milestone 4 architecture and consumer documentation alignment.

### Verification Summary

- Plan structure reviewed against `PLAN-STANDARDS.md`.
- Lifecycle and race notes aligned with `CONCURRENCY-STANDARDS.md`.
- Traceability expectations aligned with `DOCUMENTATION-STANDARDS.md`.

### Traceability Links

- Module README updated: `rust/crates/pumas-core/src/api/README.md`
- Module README updated: `rust/crates/pumas-core/src/model_library/README.md`
- ADR added/updated: N/A
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: pending implementation completion
  success.
- Preserve public facades where possible, but prefer explicit behavior over
  silent fallback if compatibility conflicts with singleton safety.
- Update docs in the same implementation slice as the behavior they describe to
  keep the codebase strongly aligned.

## Completion Summary

### Completed

- None yet.

### Deviations

- None yet.

### Follow-Ups

- None yet.

### Verification Summary

- None yet.

### Traceability Links

- Module README updated: TBD
- ADR added or updated: N/A unless implementation expands beyond plan scope
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`

## Brevity Note

This plan stays concise, but it includes the required contract, lifecycle,
concurrency, verification, and traceability elements so implementation remains
aligned with the coding standards.
