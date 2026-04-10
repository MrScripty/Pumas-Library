# Plan: HF Classification and Library Repair Remediation

## Objective

Produce a standards-compliant, evidence-backed remediation for incorrect Hugging
Face classification and incorrect local library organization, with standards
cleanup first, a saved 30+ model HF audit artifact, a local-library findings
inventory, and non-model-specific fixes that make existing library state and
migration reports converge to correct results.

## Scope

### In Scope

- Standards remediation required by the prior cross-module work
- Refreshing and saving an HF audit of at least 30 random models without
  downloading weights
- Auditing the current local library for stale metadata, duplicate repo entries,
  path/family drift, and resolver conflicts
- Root-cause analysis that separates HF metadata issues from local-library
  repair issues
- Non-model-specific backend and frontend fixes needed to classify, persist,
  reconcile, and display models correctly
- Verification that migration dry-run reflects repaired state rather than stale
  index or metadata drift

### Out of Scope

- Downloading model weights as part of the HF audit
- One-off manual fixes for individual model directories as the primary solution
- Breaking changes to the public `PumasApi`, RPC facade, or existing GUI import
  surfaces unless a re-plan trigger is hit
- Broad repo-wide standards cleanup unrelated to the touched classification,
  reconciliation, audit, and GUI surfaces

## Inputs

### Problem

The current work has not yet met the requested outcome:

- Incorrectly organized models are still visible in the running library
- Migration dry-run still reported collisions and moves in the live GUI
- Prior fixes addressed only part of the problem space
- The user explicitly requires evidence from at least 30 random HF models plus
  concrete findings from the already-downloaded local library

### Constraints

- Follow `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/`
- Do not download model weights for the HF audit
- Prefer extending the existing model-library, index, importer, reconciliation,
  and GUI systems over introducing parallel workflows
- Keep backend-owned data and facade-first compatibility semantics intact
- Preserve SQLite-canonical state and derived `metadata.json` projection rules

### Assumptions

- The existing `hf_metadata_audit.rs` example can be reused, but its output
  must be saved and tied to the next remediation pass
- The local library contains a mix of stale metadata, duplicate repo entries,
  and path-normalization drift from earlier classification behavior
- The running GUI may be attached to an already-running primary process, so
  verification must account for live-process state instead of only source code

### Dependencies

- `rust/crates/pumas-core/examples/hf_metadata_audit.rs`
- `rust/crates/pumas-core/src/model_library/`
- `rust/crates/pumas-core/src/api/`
- `frontend/src/components/`
- Existing migration dry-run and reconcile flows
- Coding, documentation, testing, and plan standards in the standards repo

### Affected Structured Contracts

- HF audit JSON and Markdown artifact shape
- `PumasApi` reconcile and migration-report lifecycle behavior
- Model classification and metadata projection fields:
  - `model_type`
  - `pipeline_tag`
  - `task_type_primary`
  - `input_modalities`
  - `output_modalities`
  - `huggingface_evidence`
  - `review_reasons`
- GUI rendering contracts for model kind, organization, and migration-report
  display

### Affected Persisted Artifacts

- `shared-resources/models/models.db`
- `shared-resources/models/library.db`
- per-model `metadata.json`
- migration report JSON and Markdown artifacts
- audit report Markdown and JSON artifacts saved under the repo docs/output path

### Concurrency/Race-Risk Review

- The primary process owns watcher startup, reconcile scheduling, and migration
  report generation; verification must not assume a cold start unless one is
  actually performed
- Read-only HF auditing must remain separate from mutating local-library repair
  flows
- Backfill and repair flows must be idempotent so repeated reconcile, dry-run,
  and GUI polling do not create new drift
- Migration dry-run should reflect reconciled state before snapshotting report
  artifacts
- Any bulk repair command must define who starts it, who stops it, and how it
  avoids overlapping with watcher-driven reconcile work

### Current Standards Compliance Findings

- No written plan existed before the prior multi-file, cross-layer changes.
- The following touched files exceed the coding-standards soft decomposition
  threshold and therefore require an explicit decomposition review before more
  feature work is added:
  - `rust/crates/pumas-core/src/model_library/library.rs`
  - `rust/crates/pumas-core/src/api/models.rs`
  - `rust/crates/pumas-core/src/model_library/model_type_resolver.rs`
  - `rust/crates/pumas-core/src/lib.rs`
  - `frontend/src/components/ModelManager.tsx`
- The prior work changed runtime and classification behavior without updating
  all affected module-level docs:
  - `rust/crates/pumas-core/src/README.md` does not satisfy the documentation
    template sections required for a `src/` directory
  - `frontend/src/components/README.md` does not satisfy the documentation
    template sections required for a `src/` directory
  - `rust/crates/pumas-core/src/api/README.md` and
    `rust/crates/pumas-core/src/model_library/README.md` should be updated to
    reflect the reconcile, migration, audit, and backfill semantics actually in
    force after remediation
- The new startup/reconcile regression tests were added to `lib.rs`, which
  increases root-file size and should be reviewed for extraction into a more
  focused test location

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| More classification logic is added to already-oversized files before decomposition cleanup | High | Make standards remediation the first milestone and block further feature expansion until the review/extraction pass is done |
| HF audit results are not durable or traceable | High | Save JSON and Markdown artifacts in-repo and reference them from docs and the plan |
| Resolver fixes improve new imports but do not repair old library state | High | Add a dedicated bulk backfill/repair path and verify against the current local library |
| Live GUI behavior differs from source-only tests because an existing primary process is attached | High | Include verification against the running-library workflow and document process-state assumptions |
| Manual model-specific fixes creep in | Medium | Restrict remediation to reusable resolver, projection, repair, and display logic |
| Documentation drifts again after behavior changes | Medium | Update touched module READMEs in the same milestone as code refactors and behavior changes |

## Definition of Done

- A standards-compliant plan exists in `docs/plans/` and the directory README
  references it.
- Standards remediation for the touched surfaces is completed or explicitly
  justified in updated module docs.
- A saved HF audit covers at least 30 random models across multiple task
  families without downloading weights.
- A saved local-library findings report explains the concrete causes of current
  misorganization and incorrect migration-report output.
- Non-model-specific fixes exist for the validated causes.
- Verification demonstrates that classification, persistence, local repair, GUI
  display, and migration dry-run agree on the repaired state, or any residual
  gaps are explicitly documented.

## Ownership And Lifecycle Note

- `PumasApi` primary ownership remains responsible for watcher startup,
  reconciliation, migration-report generation, and any repair/backfill execution
  that mutates persisted library state.
- The HF audit harness remains read-only with respect to model payloads and
  should run in a temporary metadata-only workspace.
- Any bulk repair flow must either run under the primary-owned API path or
  document why a standalone maintenance path is safe and non-overlapping.

## Public Facade Preservation Note

- Preserve the existing `PumasApi`, RPC, and GUI import/search surfaces where
  feasible.
- Prefer facade-first internal refactors: extract helpers and modules without
  changing host-facing contracts unless a re-plan trigger is hit.

## Milestones

### Milestone 1: Standards Remediation And Traceability

**Goal:** Bring the touched surfaces into compliance before adding more feature
logic.

**Tasks:**
- [ ] Perform and document a decomposition review for:
  - `rust/crates/pumas-core/src/model_library/library.rs`
  - `rust/crates/pumas-core/src/api/models.rs`
  - `rust/crates/pumas-core/src/model_library/model_type_resolver.rs`
  - `rust/crates/pumas-core/src/lib.rs`
  - `frontend/src/components/ModelManager.tsx`
- [ ] Extract the newly added startup/reconcile regression fixtures and tests
      out of `rust/crates/pumas-core/src/lib.rs` into a more focused test
      location if the review confirms that is the cleaner boundary
- [ ] Extract classification, migration-prep, or audit-adjacent helpers from
      oversized modules where the current responsibility boundaries are blurred
- [ ] Update `rust/crates/pumas-core/src/README.md` to satisfy the required
      documentation template sections
- [ ] Update `frontend/src/components/README.md` to satisfy the required
      documentation template sections
- [ ] Update `rust/crates/pumas-core/src/api/README.md` and
      `rust/crates/pumas-core/src/model_library/README.md` so reconcile,
      migration, audit, and persisted-contract behavior matches the code

**Verification:**
- README review against `DOCUMENTATION-STANDARDS.md`
- File-boundary review against `CODING-STANDARDS.md`
- Targeted tests still pass after any extraction move

**Status:** Not started

### Milestone 2: Refresh HF And Local-Library Evidence

**Goal:** Save the evidence base that the next remediation pass will use.

**Tasks:**
- [ ] Re-run the HF metadata audit with at least 30 random models across
      multiple task families without downloading weights
- [ ] Save the audit JSON and Markdown artifacts under a repo-traceable path
- [ ] Record the exact seed, sampling method, categories, and timestamp
- [ ] Audit the current local library for:
  - stale `pipeline_tag` and task projection drift
  - duplicate repo entries
  - path/family normalization drift
  - resolver conflicts
  - GUI display mismatches
- [ ] Produce a local-library findings Markdown report that lists concrete
      examples and groups them by root cause rather than by one-off model fixes

**Verification:**
- Saved audit artifact shows at least 30 sampled HF repos
- Saved local-library findings report references real on-disk model examples
- Evidence is traceable to the current code path rather than ad hoc manual notes

**Status:** Not started

### Milestone 3: Fix Resolver, Projection, And Repair Root Causes

**Goal:** Implement non-model-specific fixes for the causes validated in the
evidence pass.

**Tasks:**
- [ ] Fix resolver precedence and conflict handling for generic
      architecture/config rules that currently override stronger modality/task
      evidence
- [ ] Add any still-missing task and modality projection behavior validated by
      the refreshed HF audit
- [ ] Implement a bulk metadata backfill/repair path for existing library
      entries using stored `pipeline_tag` and `huggingface_evidence`
- [ ] Tighten duplicate cleanup and path-normalization repair so old
      `unknown/...` and stale-family entries self-heal through the supported
      repair flows
- [ ] Ensure migration dry-run consumes repaired/reconciled state rather than
      stale projections

**Verification:**
- Unit tests for resolver and projection edge cases
- Replay/recovery/idempotency checks for repair flows per
  `TESTING-STANDARDS.md`
- Cross-layer acceptance check from stored metadata input to repaired record and
  migration-report output

**Status:** Not started

### Milestone 4: Align GUI And Final Verification

**Goal:** Ensure the running product surfaces the repaired backend state
correctly.

**Tasks:**
- [ ] Verify GUI organization and model-kind display against repaired backend
      records
- [ ] Fix any remaining frontend interpretation gaps that cause correct backend
      data to display incorrectly
- [ ] Re-run migration dry-run against the repaired library state and confirm
      collisions/moves match reality
- [ ] Re-run the HF audit sample and compare before/after issue counts
- [ ] Update the final findings doc with residual issues, if any, and explicit
      rationale for anything deferred

**Verification:**
- Frontend targeted tests where interpretation logic changed
- Backend acceptance path from repaired metadata to GUI-visible organization
- Final migration dry-run artifact review

**Status:** Not started

## Execution Notes

Update during implementation:
- 2026-04-10: Plan created after standards review of the prior audit and repair
  work.

## Commit Cadence Notes

- Commit when a logical slice is complete and verified.
- Follow commit format/history cleanup rules from `COMMIT-STANDARDS.md`.

## Optional Subagent Assignment

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| None | None | None | None |

## Re-Plan Triggers

- Refactor work reveals a better module boundary that changes milestone order
- The refreshed HF audit shows a materially different failure distribution than
  the existing report
- A required fix would break the existing `PumasApi`, RPC, or GUI facade
- Bulk backfill/repair needs a schema or persisted-contract change not covered
  by the current plan

## Recommendations

- Recommendation 1: Keep the read-only HF audit workflow and the mutating
  local-library repair workflow explicitly separate.
  Why: it avoids repeating the earlier sequencing failure where code changes
  got ahead of the saved evidence base.
  Impact: small up-front documentation and verification cost, lower risk of
  drifting fixes.

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

- Module README updated: Not started
- ADR added/updated: N/A for now
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: Not started

## Brevity Note

Keep the plan concise. Expand detail only where execution decisions or risk
require it.
