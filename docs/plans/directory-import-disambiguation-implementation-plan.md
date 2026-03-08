# Plan: Directory Import Disambiguation

## Objective

Add first-class directory import support to the GUI and backend while reliably distinguishing between:

- a directory that is one logical model bundle
- a directory that contains multiple separate models
- a directory that is ambiguous or unsupported

The result must unify with the current model-library systems instead of creating a parallel drag/drop, validation, or import stack.

## Scope

### In Scope

- Backend-owned classification of dropped/selected directories before import
- GUI support for dropping/selecting directories in addition to files
- Routing bundle-root directories into the existing external diffusers import path
- Routing multi-model container directories into the existing file/directory import flows
- Operator review UI for directory classification results and conflicts
- Regression protection so existing file drag/drop behavior remains unchanged

### Out of Scope

- Recursive import support for every arbitrary model-directory shape in milestone one
- New bundle formats beyond currently supported external diffusers directories
- Automatic import of ambiguous directories without operator confirmation
- Automatic decomposition of bundle internals into separate top-level models
- App mapping support changes beyond existing external-reference exclusions

## Inputs

### Problem

The current GUI drag/drop path is file-only. It filters incoming paths by file extension and sends only file imports through the existing batch import workflow. That means:

- a diffusers bundle directory is ignored entirely when dropped as a directory
- a future naive recursive directory import could incorrectly treat bundle internals as separate models
- a normal folder that contains multiple model files or child model directories could be mistaken for a single bundle unless classification is explicit

### Constraints

- Reuse the current metadata-backed model-library system and existing external diffusers registration path.
- Keep classification backend-owned so frontend drag/drop does not invent its own directory heuristics.
- Preserve the existing batch file import flow for standalone files.
- Do not create a second persistent import registry, queue, or validation lifecycle.
- Do not create a second bundle validator or a second runtime-routing contract for directory imports.
- Keep bundle classification deterministic and explainable to operators.

### Standards Alignment

- Follow layered separation from `ARCHITECTURE-PATTERNS.md`:
  - Presentation gathers paths and displays backend results.
  - Application orchestrates classification and import routing.
  - Domain owns directory classification rules and bundle/container disambiguation.
  - Infrastructure performs filesystem inspection and transport wiring.
- Follow backend-owned data rules from `ARCHITECTURE-PATTERNS.md`:
  - Classification state, candidate lists, and import decisions that affect behavior are backend-owned.
  - Frontend may hold only transient UI state such as drag/drop affordances, pending selection, and review modal state.
- Follow interop rules from `INTEROP-STANDARDS.md`:
  - Directory-classification DTOs must be defined across Rust/RPC/frontend in the same logical slice.
  - Boundary inputs from drag/drop, picker, and IPC must be validated at each crossing.
  - New interop contracts must be append-only and version-safe.
- Follow testing rules from `TESTING-STANDARDS.md`:
  - Add unit coverage for classifier behavior.
  - Add at least one cross-layer acceptance path from directory input to final import outcome.
  - Add regression coverage for unchanged file-based import behavior.

### Assumptions

- Milestone one needs to correctly classify `diffusers_directory` bundle roots versus container directories.
- A directory with `model_index.json` at its root and a supported pipeline shape is treated as one logical external bundle.
- A directory containing multiple importable child models should not be auto-imported as one model.
- Ambiguous directories should stop in a review state instead of being auto-imported.

### Dependencies

- Existing external diffusers backend capability (`import_external_diffusers_directory`)
- Existing file batch import flow in the frontend and backend
- Existing model-library metadata/index/reconciliation architecture
- Existing Electron native open dialog bridge

### Affected Structured Contracts

- New backend directory-classification DTO and RPC surface
- Frontend import workflow state and review UI contracts
- Existing `ModelImportResult` consumption in GUI import flows
- Existing drag/drop and native file-picker contracts

### Public Facade Preservation Note

- Preserve the current import facade by extending the existing dialog/workflow and API surface.
- Do not replace or break existing file import contracts in milestone one.
- Add new directory-classification and directory-import capabilities as additive contracts only.

### Affected Persisted Artifacts

- None for classification-only requests
- Existing `metadata.json` and `models.db` records for final imports

### Unification Constraints

- Directory import must extend the current import dialog/workflow, not introduce a second GUI import subsystem.
- Bundle-root imports must reuse `import_external_diffusers_directory`; they must not create a separate bundle-registration stack.
- Single-model directory and file candidates must continue to route through the existing importer/library metadata/index flow.
- Classification results must be ephemeral request data until the operator confirms import.
- Execution for imported bundles must continue to rely on the existing execution descriptor contract rather than any directory-import-specific runtime surface.

### Concurrency / Race-Risk Review

- Drag/drop and file-picker import should both flow through the same classification/import orchestration path so the same directory is not classified differently by source.
- Classification requests must be side-effect-free; only the explicit import action should create registry artifacts or metadata.
- Multi-directory imports must keep per-path results stable so the operator can review mixed outcomes without order-dependent behavior.
- Lifecycle ownership:
  - Frontend gathers raw dropped/selected paths.
  - Backend classifies each path without persistence.
  - Operator confirms routing choice when required.
  - Existing import APIs execute the selected route and persist results.

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| Frontend guesses bundle-vs-container using ad hoc heuristics | High | Add one backend classification endpoint and make the GUI consume only that result |
| Recursive import treats bundle internals as separate models | High | Detect supported bundle roots before any recursive enumeration |
| Multi-model directories get flattened into one logical import | High | Return `multi_model_container` classification with explicit child candidates |
| Ambiguous directories get silently imported the wrong way | High | Require operator confirmation or fail with explicit reasons |
| Directory drag/drop introduces a second import workflow separate from file import | Medium | Extend the existing import dialog/workflow to support classified paths instead of adding a new standalone UI flow |
| Existing file drag/drop regresses | Medium | Keep file-path behavior unchanged and add targeted regression tests |

## Classification Rules

Classification must follow one deterministic precedence order so the same path is never interpreted differently by different callers.

### Precedence Order

1. `single_bundle`
   A directory is classified as `single_bundle` when its root satisfies the supported bundle-root validator.
   For milestone one, that means a supported diffusers bundle root validated from `model_index.json`.

2. `single_model_directory`
   A directory is classified as `single_model_directory` when the root is importable as one model by existing model-library rules and does not also contain independent child import candidates that should be treated as separate models.

3. `multi_model_container`
   A directory is classified as `multi_model_container` when the root is not itself a supported single bundle/model, but contains two or more independent child import candidates.

4. `ambiguous`
   A directory is classified as `ambiguous` when classification signals conflict and automatic routing would be unsafe.
   Examples:
   - the root looks partly importable, but there are also independent child candidates
   - both bundle markers and unrelated sibling model candidates exist
   - child-candidate grouping is not deterministic

5. `unsupported`
   A directory is classified as `unsupported` when it yields no supported single-root or child-candidate interpretation.

### Traversal Rules

- Recognized bundle roots are terminal: once a directory is classified as `single_bundle`, the classifier must not enumerate its internals as sibling import candidates.
- Child enumeration must operate on immediate descendants first; recursive descent should be limited and explicit so bundle internals are never flattened into top-level imports.
- The classifier must return reasons and candidate summaries that explain why a path was treated as one bundle, one model directory, a container, or unsupported.

## Clarifying Questions (Only If Needed)

- None at plan creation time.
- Reason: the immediate requirement is clear enough to plan safely around supported bundle roots, container directories, and ambiguous directories.
- Revisit trigger: the team wants recursive auto-import of arbitrary nested directory trees in milestone one.

## Definition of Done

- Operators can drag/drop or pick directories in the GUI.
- The backend classifies each directory as `single_bundle`, `multi_model_container`, `single_model_directory`, `unsupported`, or `ambiguous`.
- Supported diffusers bundle roots import through the existing external bundle path as one logical model.
- Container directories are not mis-imported as one model bundle.
- A recognized bundle root is never decomposed into separate top-level imports during classification or routing.
- Existing file-based drag/drop and picker import continue to work unchanged.
- Classification decisions are visible and explainable in the GUI before import commits side effects.

## Milestones

### Milestone 1: Classification Contract

**Goal:** Define one backend-owned directory classification contract and routing vocabulary.

**Tasks:**
- [ ] Add a directory-classification DTO and append-only RPC/API surface
- [ ] Define canonical classification outcomes:
  - `single_bundle`
  - `single_model_directory`
  - `multi_model_container`
  - `ambiguous`
  - `unsupported`
- [ ] Define operator-visible reasons and candidate metadata for each outcome
- [ ] Record facade-preservation note: extend current import workflow rather than adding a second GUI import subsystem

**Verification:**
- Contract compiles across Rust/RPC/frontend type layers
- Review confirms classification semantics do not duplicate existing import result contracts
- Review confirms new contracts are additive and do not break existing file-import callers

**Status:** Completed on 2026-03-08

### Milestone 2: Backend Directory Classification

**Goal:** Implement deterministic directory classification in the backend without persistence side effects.

**Tasks:**
- [ ] Add a focused classifier module under `model_library`
- [ ] Detect supported bundle roots first using existing diffusers validation rules
- [ ] Detect single-model directories that are not bundle roots but are importable as one model
- [ ] Detect multi-model container directories by enumerating importable child directories/files without descending into recognized bundle roots
- [ ] Return `ambiguous` for mixed/unclear layouts instead of guessing
- [ ] Encode the precedence order in one shared classifier so drag/drop, picker import, and any future API caller cannot diverge

**Verification:**
- Unit tests cover:
  - supported diffusers bundle root
  - directory containing multiple sibling model directories
  - directory containing multiple standalone model files
  - directory containing one supported bundle plus unrelated sibling candidates
  - directory that looks importable at the root and also has child candidates
  - ambiguous mixed directory
  - unsupported/empty directory
- Classifier performs no writes and creates no metadata/index side effects
- IPC/backend boundary validation rejects malformed or non-canonical path inputs safely

**Status:** Completed on 2026-03-08

### Milestone 3: GUI Path Intake Unification

**Goal:** Extend existing drag/drop and picker intake to accept directories and route all paths through one workflow.

**Tasks:**
- [ ] Update Electron picker support to allow directory selection in addition to files
- [ ] Update drag/drop filtering so directories are preserved instead of discarded by file-extension checks
- [ ] Extend the existing import dialog/workflow state to hold classified directory entries alongside file entries
- [ ] Keep standalone file import behavior unchanged
- [ ] Ensure drag/drop and picker both call the same classification/import orchestration path

**Verification:**
- Frontend tests or targeted manual verification confirm:
  - dropping files still works
  - dropping a directory now reaches classification
  - picker can select directories
  - drag/drop and picker produce the same classification result for the same directory
- No duplicate frontend import state machine is introduced
- Frontend holds only transient review state; authoritative classification data comes from the backend

**Status:** Completed on 2026-03-08

### Milestone 4: Review and Import Routing

**Goal:** Present directory classification results to the operator and route confirmed imports into existing backend import paths.

**Tasks:**
- [ ] For `single_bundle`, route to `import_external_diffusers_directory`
- [ ] For `single_model_directory`, route to the appropriate existing directory/file import path
- [ ] For `multi_model_container`, present discovered candidates and let the operator import selected children
- [ ] For `ambiguous` and `unsupported`, block import and show explicit reasons
- [ ] Ensure result handling still uses one import-complete refresh path
- [ ] Ensure a parent container directory is never imported as one logical model when child candidates were classified separately

**Verification:**
- Integration checks confirm a diffusers bundle directory imports as one model
- Integration checks confirm a container directory yields multiple candidate imports rather than one collapsed import
- Integration checks confirm a bundle directory with internal component folders still imports as one model rather than multiple child entries
- Ambiguous directories cannot be imported without an explicit supported route
- Cross-layer acceptance check exercises: GUI/raw path intake -> IPC/API classification -> operator confirmation -> existing import path -> resulting model-library state

**Status:** Completed on 2026-03-08

### Milestone 5: Regression and Operator Surface Hardening

**Goal:** Make the directory import UX reliable, explainable, and safe.

**Tasks:**
- [ ] Add operator-visible classification details and path summaries in the import dialog
- [ ] Add regression coverage for file-only import flows
- [ ] Add regression coverage that bundle internals are never surfaced as top-level sibling models during bundle import
- [ ] Add regression coverage that a multi-model container directory is never auto-collapsed into one import
- [ ] Update docs/README/module READMEs for the new classification path

**Verification:**
- Affected Rust tests pass
- Affected frontend type/test/lint checks pass within existing repo constraints
- Manual acceptance:
  - drop a bundle root -> one bundle import
  - drop a container dir -> multi-model review
  - drop unsupported dir -> explicit failure
- Regression acceptance confirms existing file drag/drop and file-picker imports remain behaviorally unchanged

**Status:** In progress on 2026-03-08

## Re-Plan Triggers

- A new bundle format is added and classification can no longer remain diffusers-first
- Recursive nested-container import is brought into milestone one scope
- Classification needs persistence/history rather than being side-effect-free
- GUI requirements change from review-first to fully automatic directory import

## Recommendations (Only If Better Option Exists)

- Prefer a backend classification pass over any frontend-only path inspection.
  Why: it keeps bundle detection aligned with the actual executable asset rules and avoids platform-specific drift in drag/drop behavior.
  Impact: slightly more RPC work, lower long-term classification risk.

- Prefer review-first routing for directories over silent recursion.
  Why: directory layouts are much more ambiguous than file imports, and a wrong guess can create many incorrect model records quickly.
  Impact: one extra operator step, much safer import behavior.

## Completion Summary

### Completed

- Milestone 1: Added append-only import-path classification DTOs and RPC/API surface.
- Milestone 2: Added backend classifier coverage for bundle roots, root-file containers, sibling-model containers, and ambiguous layouts.
- Milestone 3: Updated drag/drop and picker intake to preserve directories and route all paths through one workflow.
- Milestone 4: Routed single bundles through external diffusers import, routed single model directories/files through the existing importer, and surfaced blocked/expanded directory results in the dialog.

### Deviations

- Milestone 1 contract was widened from directory-only classification to import-path classification so mixed file/directory intake could stay backend-owned without a frontend filesystem heuristic layer.

### Follow-Ups

- Add focused frontend regression coverage for mixed file + directory imports when the repo test harness for import dialogs is expanded.
- Consider appending security-tier hints for classified directory models if operator acknowledgment needs to occur before import for directory-root pickle layouts.

### Verification Summary

- `cargo test -p pumas-library directory_import --manifest-path rust/Cargo.toml`
- `cargo test -p pumas-rpc --manifest-path rust/Cargo.toml --no-run`
- `npm run check:types` in `frontend/`
  - result: still fails on pre-existing unrelated errors in `src/components/ModelMetadataModal.tsx` and `src/hooks/useManagedApps.test.ts`
  - directory-import changes type-check cleanly within that existing failure set

### Traceability Links

- Related capability plan: `docs/plans/external-reference-diffusers-implementation-plan.md`

## Brevity Note

This plan is intentionally focused on classification and routing decisions. It should stay separate from the broader external bundle backend plan so the GUI/path-intake work remains reviewable and does not reopen already-settled backend contracts.
