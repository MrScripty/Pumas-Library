# Plan: External-Reference Diffusers Support

## Objective

Add external-reference diffusers-directory support to the existing Pumas model-library system without creating parallel registries, importers, validators, or runtime-routing contracts.

## Scope

### In Scope

- Backend metadata/schema changes for external-reference directory-root assets
- Backend import flow for external diffusers directories
- Asset validation and validation-state persistence
- Execution descriptor contract for runtime consumers
- Reconciliation, reindex, and delete/unregister changes needed to keep external assets reliable
- API and frontend read-surface updates required to expose `storage_kind`, `bundle_format`, and validation health
- Regression protection for existing file-based import, indexing, resolution, and delete behavior

### Out of Scope

- SDXL-specific decomposition or executable submodel support
- LoRA, ControlNet, refiner, or adapter composition
- App mapping support for external-reference bundles
- New standalone registry/database for external assets
- New dependency contract separate from the existing dependency-resolution system
- New runtime backend-selection field parallel to `recommended_backend` and `runtime_engine_hints`

## Inputs

### Problem

Pumas currently assumes the executable model asset is either a file or a library-owned directory whose runtime path can be reduced to a primary file. Diffusers bundles break that assumption because the executable unit is the bundle root directory. A correct implementation must preserve existing model-library durability and indexing behavior while adding external-reference support as a first-class asset shape.

### Constraints

- Reuse the current metadata-backed model-library + SQLite index architecture in `pumas-core`.
- Preserve stable `model_id` generation from library-owned registry artifacts.
- Keep backend as the source of truth for import validation and external-asset health.
- Do not create overlapping validation semantics with the dependency-resolution contract.
- Prevent external-reference assets from entering existing app-mapping flows.
- Respect current cross-layer verification requirements from the testing standards.
- Keep module changes decomposed enough to avoid overloading `importer.rs` and `library.rs`.

### Assumptions

- Milestone one supports only `storage_kind=external_reference`, `bundle_format=diffusers_directory`, and `task_type_primary=text-to-image`.
- `source_path` and `entry_path` are equal for milestone one, but both are persisted separately.
- Execution descriptor behavior in milestone one is fail-hard for `degraded` and `invalid` asset validation states.
- Dependency-resolution data is reused from the current dependency contract rather than redefined.

### Dependencies

- Existing `ModelMetadata` persistence and indexing flow
- Existing model-library reconciliation flow
- Existing dependency-resolution contract and APIs
- Existing RPC model surfaces and frontend type contracts
- Documentation updates for affected model-library modules

### Affected Structured Contracts

- `ModelMetadata` persisted fields and effective metadata projections
- `ModelImportResult`
- New execution-descriptor DTO and RPC surface
- Existing list/get/search model surfaces
- Existing frontend API types for model metadata display

### Affected Persisted Artifacts

- Per-model `metadata.json`
- `models.db` indexed `metadata_json` projection
- Library-owned registry directories under the model library root

### Concurrency / Race-Risk Review

- Reconciliation currently adopts metadata-missing model directories and stages partial downloads. External-reference assets must not be mistaken for orphans or partial downloads.
- Validation refresh after startup/reindex must update current-state-only `validation_state` and `validation_errors` atomically with metadata/index updates.
- Import and revalidate flows must be idempotent so restart/reindex does not duplicate registry artifacts or drift `model_id`.
- Lifecycle ownership:
  - Import flow creates the registry artifact and initial validation result.
  - Reconciliation/reindex refreshes persisted asset validation state.
  - Delete/unregister removes library-owned artifacts only for `external_reference`.
  - Execution descriptor resolution reads persisted health and fails hard for non-`valid` assets in milestone one.

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| External support is implemented as a second registry path | High | Extend `ModelMetadata`, `save_metadata`, `index_model_dir`, and existing RPC surfaces instead of adding a separate asset store |
| Asset validation and dependency validation use the same labels without distinction | High | Name and document asset-level validation fields separately in metadata and execution descriptor; reuse dependency contract as a nested payload only |
| Existing file-based imports regress due to shared importer changes | High | Add a dedicated external-directory import path and regression tests for current file and in-place imports |
| Reconciliation treats external-reference assets as orphans or missing-file models | High | Gate orphan/adoption and reclassification logic on persisted `storage_kind` and registry-artifact invariants |
| Delete flow removes user-owned bundle contents | High | Branch delete/unregister behavior on `storage_kind` inside existing `ModelLibrary` delete semantics |
| External assets leak into app mapping | Medium | Add an explicit mapping exclusion for `storage_kind=external_reference` and test it |
| `importer.rs` and `library.rs` become oversized catch-all modules | Medium | Extract focused modules for external asset validation and execution descriptor orchestration |

## Clarifying Questions (Only If Needed)

- None at plan creation time.
- Reason: Contract semantics, milestone scope, and acceptance criteria are now specific enough to sequence implementation safely.
- Revisit trigger: The team decides degraded assets should be executable in milestone one or wants app mapping in scope.

## Definition of Done

- External-reference diffusers bundles are represented as normal library models with extended metadata, not as a separate registry system.
- Import, validation, indexing, reconciliation, and delete/unregister behavior are integrated into existing model-library flows.
- Runtime consumers obtain bundle execution data only through the new execution descriptor contract.
- Existing file-based model behavior remains unchanged and covered by regression verification.
- Documentation and module READMEs reflect the new contract and lifecycle.

## Milestones

### Milestone 1: Contract and Architecture Alignment

**Goal:** Define the new asset shape within existing model-library contracts and document how it integrates with current systems.

**Tasks:**
- [ ] Extend `ModelMetadata` and related Rust/TS contracts with `source_path`, `entry_path`, `storage_kind`, `bundle_format`, `pipeline_class`, `import_state`, asset-level `validation_state`, and asset-level `validation_errors`
- [ ] Define explicit asset-level enum/value semantics distinct from dependency validation semantics
- [ ] Add execution-descriptor DTOs and versioning policy at the contract layer
- [ ] Update `model_library` documentation to state the new external-reference invariants, mapping exclusion, and execution-descriptor contract
- [ ] Record facade-preservation note: preserve existing model-library public surfaces where possible and add new capability via append-only contracts

**Verification:**
- Contract type additions compile across Rust and frontend type layers
- Documentation updates reflect actual persisted and consumer-facing semantics
- Review confirms no duplicate top-level backend-routing field is introduced

**Status:** Completed on 2026-03-08

### Milestone 2: Registry Artifact and Import Path

**Goal:** Add a dedicated external-directory import path that creates a library-owned registry artifact while preserving external bundle layout.

**Tasks:**
- [ ] Add a focused external-asset import module/service rather than extending copy-based import logic inline
- [ ] Implement registry-artifact creation under the library root with stable `model_id` derivation
- [ ] Update `ModelImportResult` to return `model_id` and status only
- [ ] Ensure existing copy import and existing in-place import continue to use current behavior unchanged
- [ ] Update module READMEs for any new or substantially changed backend directories

**Verification:**
- Unit tests cover successful external-directory registration without copying or renaming bundle contents
- Regression tests cover existing file import and existing in-place import behavior
- Restart/reindex preserves `model_id` and registry artifact stability

**Status:** Completed on 2026-03-08

### Milestone 3: Validation Lifecycle and Reconciliation

**Goal:** Integrate asset validation into current backend-owned metadata and reconciliation flows without adding a parallel health subsystem.

**Tasks:**
- [ ] Add a dedicated diffusers-directory validator module with deterministic path and component checks
- [ ] Persist current-state-only asset `validation_state` and `validation_errors`
- [ ] Integrate validation refresh into startup/reindex/reconciliation flow
- [ ] Gate orphan adoption, partial-download handling, and reclassification logic so external-reference assets are not misclassified
- [ ] Define asset-health update behavior when the external directory is moved, deleted, or becomes incomplete

**Verification:**
- Unit tests cover valid, degraded, and invalid asset validation outcomes
- Integration tests cover restart/reindex producing stable `model_id`, `entry_path`, and `validation_state`
- Acceptance check verifies metadata/index output reflects validation degradation after external asset drift

**Status:** Completed on 2026-03-08

### Milestone 4: Execution Descriptor and Consumer Routing

**Goal:** Introduce one runtime-facing execution descriptor that reuses current metadata and dependency systems and displaces primary-file-first routing for bundle assets.

**Tasks:**
- [ ] Add `resolve_model_execution_descriptor(model_id)` to the backend/RPC surface
- [ ] Compose the descriptor from persisted metadata plus existing dependency-resolution output
- [ ] Enforce milestone-one fail-hard behavior for `degraded` and `invalid` external assets
- [ ] Update torch and other runtime-facing consumers to avoid `primary_file` routing for external-reference bundles
- [ ] Keep existing `primary_file` surfaces for file-based models only where still valid

**Verification:**
- Integration tests verify `valid` external assets return bundle-root `entry_path`
- Integration tests verify `degraded` and `invalid` external assets fail hard
- Cross-layer acceptance check verifies producer metadata -> execution descriptor -> consumer-visible output consistency

**Status:** Completed on 2026-03-08

### Milestone 5: Operator Surfaces and Safety Gates

**Goal:** Expose external-reference status to operators while explicitly blocking unsupported flows.

**Tasks:**
- [ ] Extend list/get/search surfaces to expose `storage_kind`, `bundle_format`, and asset `validation_state`
- [ ] Update frontend types and metadata display surfaces for external assets
- [ ] Add explicit mapping exclusion for external-reference assets in existing mapping flows
- [ ] Update delete/unregister behavior and operator-facing wording for external ownership
- [ ] Add targeted documentation for operator-visible semantics and blocked app mapping

**Verification:**
- UI/API contract tests verify operator-visible metadata fields
- Integration tests verify external-reference assets are excluded from mapping previews/apply flows
- Integration tests verify delete/unregister remove only library-owned registry artifacts

**Status:** In progress

## Execution Notes

Update during implementation:
- 2026-03-08: Plan created against current Rust/Electron codebase after reviewing import, metadata, dependency, reconciliation, delete, and runtime-routing paths for duplication risk.
- 2026-03-08: Added external diffusers bundle metadata contracts, dedicated external bundle validation/import flow, runtime execution descriptor resolution, safe external-reference delete semantics, and mapping exclusion for external assets.
- 2026-03-08: Exposed backend/RPC/frontend API surfaces for external diffusers registration and execution descriptor resolution. Full frontend directory-import UX remains deferred.

## Commit Cadence Notes

- Commit after each milestone or smaller logical slice is complete and verified.
- Keep metadata/schema, runtime-routing, and UI/operator-surface changes in separate reviewable commits when possible.
- Follow commit format/history cleanup rules from `COMMIT-STANDARDS.md`.

## Optional Subagent Assignment

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| None | None | None | Revisit only if backend contract work and frontend/operator-surface work can proceed independently without coupling risk |

## Re-Plan Triggers

- The team changes milestone-one execution policy for `degraded` assets
- App mapping for external-reference assets is brought into scope
- Metadata changes require a SQLite schema projection change beyond current `metadata_json` usage
- The chosen registry-artifact location cannot preserve stable `model_id` behavior
- Runtime consumers require a different compatibility policy than append-only execution descriptor changes

## Recommendations (Only If Better Option Exists)

- Prefer a metadata-extension approach over any sidecar asset database.
  Why: it preserves the current source-of-truth model-library architecture, reduces migration risk, and keeps reconcile/index behavior unified.
  Impact: lower implementation risk, no added registry subsystem.
- Extract focused modules for external asset validation and execution descriptor orchestration instead of growing `importer.rs` and `library.rs` further.
  Why: it aligns with file-size and responsibility standards and makes long-term maintenance safer.
  Impact: slightly more upfront refactor work, lower future drift.

## Completion Summary

### Completed

- Milestone 1: external-asset metadata enums/fields, execution descriptor DTO, and contract documentation.
- Milestone 2: dedicated external diffusers registration path that creates a library-owned registry artifact without copying bundle contents.
- Milestone 3: backend-owned diffusers validation plus persisted validation refresh through index/query/descriptor flows.
- Milestone 4: runtime execution descriptor plus torch consumer routing off the descriptor instead of primary-file-first logic.
- Milestone 5 (partial): external-reference mapping exclusion, delete safety, and API/type exposure for operator-facing metadata.

### Deviations

- The backend/API capability is implemented ahead of a dedicated frontend directory-import workflow.
- Reason: the existing UI import path is file-centric and would need additional metadata-entry UX to register external bundles cleanly.
- Follow-up trigger: when the team is ready to add a directory picker and external-bundle review/import dialog in the frontend.

### Follow-Ups

- Add a dedicated frontend directory-import flow for external diffusers bundles.
- Consider richer operator-visible metadata presentation for external-reference health in the metadata modal/list views.

### Verification Summary

- `cargo test -p pumas-library model_library:: --manifest-path rust/Cargo.toml`
- `cargo test -p pumas-library model_library::mapper::tests::test_preview_mapping_skips_external_reference_assets --manifest-path rust/Cargo.toml`
- `cargo test -p pumas-rpc --manifest-path rust/Cargo.toml --no-run`
- `npm run check:types` in `frontend/` still fails on pre-existing unrelated TypeScript issues in `ModelMetadataModal.tsx` and `useManagedApps.test.ts`.

### Traceability Links

- Module README updated: `docs/plans/README.md`
- ADR added/updated: N/A
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: N/A

## Brevity Note

This plan is intentionally scoped to execution decisions, integration points, and risk controls. Expand detail only when implementation uncovers a re-plan trigger.
