# Plan: Transformers-Aligned Artifact Identity And Migration

## Objective

Fix model download identity and library organization so separate selected
artifacts from the same Hugging Face repository, such as `Q4_K_M` and
`Q5_K_M` GGUF files, are stored, tracked, displayed, and migrated as distinct
library entries while aligning Pumas metadata and path conventions with
Transformers-style model, task, and artifact naming.

## Scope

### In Scope

- Introduce a generalized selected-artifact identity for downloads, metadata,
  indexing, migration, and frontend progress tracking.
- Separate upstream repository identity from selected artifact identity.
- Normalize architecture-family names toward Transformers-style tokens, such
  as `qwen3_5` instead of `qwen35`, and allow future tokens such as
  `qwen3_6`.
- Preserve Pumas library category names, such as `llm`, `vlm`, and
  `embedding`, as a separate concept from architecture family.
- Extend the existing migration dry-run and checkpointed execution system so
  old paths and mixed artifact directories can converge to the new layout.
- Repair existing persisted metadata and file layout without model-specific
  one-off fixes.
- Update backend, frontend, and documentation contracts affected by the new
  identity model.

### Out of Scope

- Depending on the Python `transformers` package at runtime.
- Hardcoding a fix for one Qwen repository, one publisher, or one quantization
  level.
- Changing the library to a four-segment public model id unless a re-plan
  trigger is hit.
- Replacing the existing model-library migration, reconcile, or download
  systems with a parallel repair workflow.
- Downloading or redownloading model weights as part of migration planning.
- Removing compatibility fields before the GUI, API, and persisted metadata
  have a safe transition path.

## Inputs

### Problem

The current download and library identity model does not include the selected
artifact. For Hugging Face GGUF repositories, Pumas can select different files
from the same repository, but the destination directory and frontend progress
state are still primarily keyed by repository/name concepts. This collapses
distinct artifacts into the same library entry.

Observed evidence from the local library:

- `Qwen3.6-27B-NEO-CODE-HERE-2T-OT-Q5_K_M.gguf` and
  `Qwen3.6-27B-NEO-CODE-HERE-2T-OT-Q4_K_M.gguf.part` were placed in the same
  logical model directory.
- The mixed directory metadata points at one expected artifact while the file
  inventory contains another completed artifact.
- The `.pumas_download` sidecar contains the requested `Q4_K_M` selection, but
  the directory also contains a completed `Q5_K_M` file.
- The path is under `shared-resources/models/vlm/qwen35/` even though the
  artifact is a `qwen3_6` model.

Transformers guidance from the local reference checkout separates these
concepts:

- Hub repository id, for example `TheBloke/...-GGUF`.
- Artifact selector, for example `gguf_file = "...Q6_K.gguf"`.
- Optional revision and subfolder selectors.
- Architecture or config model type, for example `qwen3`, `qwen3_5`, or
  `qwen3_vl`.
- Task or pipeline labels, for example `text-generation` or
  `image-text-to-text`.

### Constraints

- Follow `/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/`.
- Keep the solution generalized across model families, publishers, file
  formats, and quantization levels.
- Preserve existing public API and GUI surfaces where feasible through
  append-only contract changes.
- Extend the existing migration dry-run, report, checkpoint, and execution
  flow instead of adding a one-off filesystem fixer.
- Migration must be idempotent, dry-run-visible, resumable, and safe around
  partial downloads.
- Existing user changes in unrelated source, package, and lock files must not
  be overwritten during planning or implementation.

### Standards Alignment

- Follow `PLAN-STANDARDS.md` for objective, scope, inputs, milestone-level
  verification, risk handling, re-plan triggers, and completion criteria.
- Follow architecture separation from `ARCHITECTURE-PATTERNS.md`:
  - Domain owns identity, naming, and migration semantics.
  - Application/API layers orchestrate download, reconciliation, and migration.
  - Frontend displays backend-owned identity and progress state.
- Follow interop rules from `INTEROP-STANDARDS.md`:
  - Rust DTOs, RPC payloads, and frontend types change in the same logical
    slice.
  - Boundary contracts remain append-only until old fields can be retired.
- Follow testing rules from `TESTING-STANDARDS.md`:
  - Unit-test pure identity and normalization helpers.
  - Add migration dry-run and execution fixtures for split artifact
    directories.
  - Add frontend state tests for concurrent artifacts from the same repo.

### Assumptions

- Pumas should keep the existing three-segment public model id shape:
  `{library_category}/{architecture_family}/{artifact_slug}`.
- The artifact slug should include publisher and repository slug information so
  equivalent filenames from different repos do not collide.
- `qwen35`, `qwen3.5`, and `qwen3_5` should converge to `qwen3_5`.
- `qwen3.6` and related evidence should converge to `qwen3_6` even if current
  Transformers does not yet define a `qwen3_6` config token.
- Existing `family` fields may need to remain as compatibility projections
  during the transition, but new logic should prefer `architecture_family`.

### Dependencies

- `rust/crates/pumas-core/src/api/state_hf.rs`
- `rust/crates/pumas-core/src/model_library/hf/download.rs`
- `rust/crates/pumas-core/src/model_library/naming.rs`
- `rust/crates/pumas-core/src/model_library/importer.rs`
- `rust/crates/pumas-core/src/model_library/library.rs`
- `rust/crates/pumas-core/src/model_library/library/migration.rs`
- `rust/crates/pumas-core/src/api/migration.rs`
- `rust/crates/pumas-core/src/api/builder.rs`
- `rust/crates/pumas-core/src/index/fts5.rs`
- `rust/crates/pumas-core/src/conversion/pipeline.rs`
- `rust/crates/pumas-core/src/conversion/manager.rs`
- `rust/crates/pumas-rpc/src/handlers/models/downloads.rs`
- `rust/crates/pumas-rpc/src/wrapper.rs`
- `rust/crates/pumas-uniffi/src/bindings/ffi_types.rs`
- `rust/crates/pumas-rustler/src/lib.rs`
- `docs/contracts/native-bindings-surface.md`
- `frontend/src/hooks/modelDownloadState.ts`
- `frontend/src/hooks/useModelDownloads.ts`
- `frontend/src/hooks/useActiveModelDownload.ts`
- `frontend/src/hooks/useDownloadCompletionRefresh.ts`
- `frontend/src/hooks/useModelLibraryActions.ts`
- `frontend/src/api/models.ts`
- `frontend/src/types/api-models.ts`
- `frontend/src/types/api-bridge-models.ts`
- `frontend/src/components/ModelManagerRemoteDownload.ts`
- `frontend/src/components/RemoteModelsList.tsx`
- `frontend/src/components/ModelManagerUtils.ts`
- `frontend/src/components/HeaderStatus.ts`
- Existing `metadata.json`, `.pumas_download`, and SQLite library/index
  projection behavior.

### Codebase Blast Radius Review

Checked against the current codebase on 2026-05-03.

| Area | Current Surface | Blast Radius | Required Guardrail |
| ---- | --------------- | ------------ | ------------------ |
| Download destination planning | `api/state_hf.rs`, `model_library/hf/download.rs`, `DownloadRequest` | New target ids affect every new HF download and recovery path | Add identity helpers outside the oversized download/API files and call them from the orchestration points |
| Download progress identity | `ModelDownloadProgress`, `pumas-rpc` download handlers, frontend API bridge types, `useModelDownloads` | Rust, RPC JSON, TypeScript, UniFFI, and Rustler can drift if fields are added unevenly | Add append-only fields in one contract slice and update all bindings/adapters together |
| Persisted download recovery | `downloads.json`, `.pumas_download`, `api/builder.rs`, importer recovery, partial resume APIs | Restart recovery may reconstruct old repo/family/name identity and re-enter the old path | Add selected-artifact identity to persisted state, then define legacy fallback behavior before moving files |
| Model metadata and indexing | `ModelMetadata`, SQLite `models.metadata_json`, FTS5 family extraction, stats by family | `family` is read by search, stats, conversions, inference defaults, and duplicate repo detection | Add `architecture_family` append-only first, keep `family` compatibility projection, then migrate readers deliberately |
| Migration dry-run/execution | `library/migration.rs`, `api/migration.rs`, migration checkpoints and reports | Existing checkpoint schema only represents whole-directory moves | Add planned action kinds before execution and preserve backward checkpoint compatibility |
| Frontend model manager | Download hooks, remote model list, local download actions, header active-download status | Repo-keyed state is assumed in tests, local overlays, delete-side cancellation, and error prompts | Introduce artifact-keyed state behind a compatibility facade before renaming all callers |
| Native bindings | `pumas-uniffi`, `pumas-rustler`, native binding contract doc | Download request/progress structs are exported to host-language consumers | Treat field additions as preview contract changes, update docs, and run native binding verification |
| Runtime/dependency references | Package facts, dependency bindings, conversion provenance, execution descriptors | Some records reference model ids and may need remapping when migration changes ids | Include dependency/reference remapping checks in migration validation, not only file moves |

### Standards Compliance Findings

- The affected Rust implementation files exceed the coding-standards
  decomposition threshold:
  - `rust/crates/pumas-core/src/model_library/library.rs`: 10937 lines
  - `rust/crates/pumas-core/src/model_library/importer.rs`: 2532 lines
  - `rust/crates/pumas-core/src/model_library/hf/download.rs`: 1892 lines
  - `rust/crates/pumas-core/src/model_library/library/migration.rs`: 927 lines
  - `rust/crates/pumas-core/src/api/state_hf.rs`: 777 lines
  - `rust/crates/pumas-core/src/api/migration.rs`: 598 lines
- The affected frontend component surface includes `ModelManager.tsx` at 306
  lines, which exceeds the UI component decomposition review threshold.
- Implementation must avoid adding substantial new logic directly to these
  files. New work should land in focused modules such as artifact identity,
  family normalization, migration action planning, and frontend download-key
  selection helpers.
- Because the change crosses Rust, RPC JSON, TypeScript, UniFFI, Rustler, and
  persisted JSON, the first implementation slice must freeze the append-only
  contract before worker-style parallelism or broad refactors begin.
- Cross-layer verification is required. Unit tests for pure helpers are not
  enough because the bug appears where backend download identity, persisted
  metadata, migration, and frontend progress state meet.

### Affected Structured Contracts

- Download request and resolved Hugging Face model request DTOs.
- Download status and progress payloads exposed through the API/RPC layer.
- Frontend download-state maps and active-download guards.
- RPC handler request parsing and response JSON for download APIs.
- UniFFI and Rustler download request/progress records.
- Persisted `.pumas_download` sidecar shape.
- Persisted `downloads.json` entries.
- Persisted `metadata.json` projection fields.
- SQLite model index fields and query projections, if the selected artifact
  identity becomes indexed.
- FTS5 family projection and family-based stats/search behavior.
- Migration dry-run report schema.
- Migration checkpoint planned-move schema.
- Native bindings surface contract documentation.
- Package fact or execution descriptor references that currently identify a
  model only by coarse model id.

### Affected Persisted Artifacts

- `shared-resources/models/models.db`
- `shared-resources/models/library.db`
- Hugging Face `downloads.json` persistence store
- Per-model `metadata.json`
- Per-download `.pumas_download`
- Existing model directories under compact family paths such as `qwen35`
- Mixed artifact directories that contain more than one selected artifact or a
  completed artifact plus a partial artifact from another selection
- Migration reports and checkpoint files generated by the existing migration
  system

### Concurrency / Race-Risk Review

- Downloads for different selected artifacts from the same repository must be
  allowed to run independently when they write to different artifact
  destinations.
- Downloads for the same selected artifact must still collapse to one active
  operation or be rejected as overlapping.
- Active and partial downloads must not be moved destructively by migration.
  They should either be migrated through an explicit partial-download relocation
  action or blocked/skipped with a clear report reason.
- Watcher-driven reconciliation, user-triggered dry-run, and checkpointed
  migration execution must agree on identity semantics so repeated runs do not
  reintroduce path drift.
- Frontend polling must key progress by selected-artifact identity, not just
  repository id, to avoid one artifact overwriting another artifact's status.

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| Selected artifact identity is only added to download paths and not metadata/index/frontend state | High | Define one domain identity struct and thread it through backend, persisted artifacts, RPC, and frontend types in one vertical slice |
| Migration moves complete files but strands partial files or sidecars | High | Add explicit dry-run actions for mixed directories and active/partial downloads before execution support |
| Family normalization becomes model-specific | High | Implement version-token normalization as a reusable naming rule, with fixtures from multiple model families when possible |
| Existing API consumers depend on the old `family` field | Medium | Keep compatibility projections and add new fields append-only until callers migrate |
| Artifact slugs collide for different repos or file selections | Medium | Include publisher/repo slug plus normalized artifact selector or stable digest |
| Frontend allows overlapping downloads to race on one destination | Medium | Key active-download guards by selected artifact id and destination path |
| Migration reports become hard to audit for split directories | Medium | Add explicit report rows for `split_artifact_directory`, source files, target artifact ids, and blocked items |

## Clarifying Questions (Only If Needed)

- None at plan creation time.
- Reason: the user has already clarified that `qwen3_6` should be used for
  these models and that the solution must remain generalized.

## Definition of Done

- `Q4_K_M` and `Q5_K_M` selections from the same Hugging Face repository are
  represented as separate library entries, downloads, metadata records, and
  frontend progress items.
- New downloads are routed to artifact-specific destinations that cannot
  overlap solely because they share a repository.
- Architecture family paths use normalized version tokens, including
  `qwen3_5` and `qwen3_6`, instead of compact or punctuation-stripped forms
  such as `qwen35`.
- Migration dry-run identifies old compact family paths, missing selected
  artifact identity, duplicate repo entries, mixed artifact directories, and
  unsafe active or partial downloads.
- Migration execution can move or split safe directories through the existing
  checkpointed migration system and reports blocked items clearly.
- Existing public API, GUI, and metadata consumers continue to work through
  compatibility fields or documented append-only transitions.
- Tests and dry-run artifacts demonstrate the fix without hardcoding the
  reported Qwen model as a special case.

## Ownership And Lifecycle Note

- `PumasApi` remains the owner of download orchestration, reconciliation,
  migration dry-run generation, and checkpointed migration execution.
- The model-library domain owns selected-artifact identity, normalized family
  naming, path generation, and migration planning.
- The frontend owns only transient presentation state and must consume
  backend-provided artifact ids for download progress and conflict prevention.
- Migration execution must not run concurrently with active writes to the same
  source or destination directory. Active downloads are either skipped,
  relocated by an explicit safe action, or blocked with a report reason.

## Public Facade Preservation Note

- Preserve the existing three-segment public model id shape where feasible:
  `{library_category}/{architecture_family}/{artifact_slug}`.
- Add selected-artifact fields append-only to backend and frontend contracts.
- Keep legacy `family` projection available during migration, but treat
  `architecture_family` as the canonical field for new path and identity logic.
- Do not remove existing import, download, search, or execution API fields in
  the first implementation pass.

## Proposed Identity Contract

Add a backend-owned selected-artifact identity with fields equivalent to:

- `repo_id`: upstream Hugging Face repository id, for example
  `DavidAU/Qwen3.6-27B-Heretic-Uncensored-FINETUNE-NEO-CODE-Di-IMatrix-MAX-GGUF`
- `revision`: upstream revision or commit, defaulting to `main` when unresolved
- `subfolder`: optional upstream subfolder selector
- `selection_kind`: `gguf_file`, `file_group`, `quant`, `full_repo`, or
  `bundle`
- `selected_filenames`: sorted selected upstream filenames
- `selected_quant`: normalized quantization token when one is selected
- `artifact_digest`: stable digest for long or multi-file selections
- `artifact_id`: stable display/storage key derived from the fields above

Recommended model id shape:

`{library_category}/{architecture_family}/{publisher}--{repo_slug}__{artifact_selector}`

Example target ids:

- `vlm/qwen3_6/davidau--qwen3_6-27b-heretic-uncensored-finetune-neo-code-di-imatrix-max-gguf__q4_k_m`
- `vlm/qwen3_6/davidau--qwen3_6-27b-heretic-uncensored-finetune-neo-code-di-imatrix-max-gguf__q5_k_m`

The example is illustrative only. The implementation must derive the same
shape from generic repository, architecture, and artifact-selection evidence.

## Milestones

### Milestone 0: Standards And Contract Guardrails

**Goal:** Freeze the cross-layer contract and decomposition boundaries before
implementation adds logic to oversized files.

**Tasks:**
- [ ] Record the final selected-artifact DTO fields and wire casing for Rust
      serde, RPC JSON, TypeScript, UniFFI, Rustler, and persisted JSON.
- [ ] Decide which fields are canonical, compatibility-only, and deprecated
      candidates:
  - canonical: `architecture_family`, selected-artifact identity fields
  - compatibility: `family`, repo-keyed frontend aliases during transition
  - deprecated candidate: repo-only download progress keying
- [ ] Add a decomposition review note for the oversized backend files and
      assign new logic to focused modules instead of expanding those files.
- [ ] Add a frontend rename/compatibility strategy so repo-keyed props can be
      migrated without breaking every model-manager caller in one uncontrolled
      edit.
- [ ] Update or queue README/contract documentation for `model_library/hf`,
      `model_library/library`, `api`, `pumas-rpc`, frontend `hooks`, frontend
      `components`, and native bindings.

**Verification:**
- Review against `PLAN-STANDARDS.md`, `CODING-STANDARDS.md`,
  `INTEROP-STANDARDS.md`, `TESTING-STANDARDS.md`, and
  `DOCUMENTATION-STANDARDS.md`.
- Contract review confirms all boundary fields are append-only or explicitly
  documented as compatibility transitions.
- File-boundary review confirms the first code slice has a focused write set
  and does not grow oversized modules unnecessarily.

**Status:** In progress

### Milestone 1: Identity Contract Vertical Slice

**Goal:** Define one selected-artifact identity contract and use it for new
download destination planning.

**Tasks:**
- [x] Add a focused domain module for selected-artifact identity and artifact
      slug generation.
- [x] Add generalized architecture-family normalization that preserves version
      separators as underscores.
- [x] Add legacy compact-token normalization for unambiguous historical forms
      such as `qwen35`, guarded by tests so it does not rewrite arbitrary
      publisher names.
- [x] Teach Hugging Face download resolution to produce selected-artifact
      identity from repo id, revision, subfolder, selected filenames, and
      quantization evidence.
- [x] Update destination path construction to use
      `{library_category}/{architecture_family}/{artifact_slug}`.
- [x] Add the selected-artifact id to `ModelDownloadResponse` and
      `ModelDownloadProgress` as an append-only field while preserving
      `repo_id`/`repoId`.
- [x] Add unit tests for artifact identity stability, sorted filename input,
      quantized GGUF selections, full-repo selections, and collision-resistant
      publisher/repo slugs.

**Verification:**
- Rust unit tests for identity and naming helpers.
- Targeted backend test or fixture showing two selected files from one repo map
  to two different target model ids.
- RPC serialization test or fixture showing the selected-artifact id survives
  the download start/status/list boundary.
- Code review confirms no model-specific branches for the reported Qwen repo.

**Status:** Complete on 2026-05-04

### Milestone 2: Persisted Metadata And Index Projection

**Goal:** Persist selected-artifact identity and normalized architecture
family without breaking existing metadata readers.

**Tasks:**
- [x] Add metadata fields for `publisher`, `architecture_family`,
      `config_model_type` or equivalent upstream model type,
      `selected_artifact_id`, `selected_artifact_files`,
      `selected_artifact_quant`, and `upstream_revision`.
- [x] Preserve legacy `family` as a compatibility projection during migration.
- [ ] Update `.pumas_download` writes and reads to include selected-artifact
      identity.
- [x] Update importer and reclassify paths so file-level evidence cannot
      incorrectly overwrite an explicit architecture family.
- [x] Update SQLite projection and reconciliation logic if the index needs to
      query by selected artifact id.
- [x] Audit FTS5, family stats, inference defaults, conversion provenance, and
      package fact hashing before changing any reader from `family` to
      `architecture_family`.

**Verification:**
- Metadata serialization and deserialization tests for new and legacy records.
- Reconciliation fixture that upgrades old metadata without losing completed
  file inventory.
- Search/stats regression check confirms family projection remains usable
  during the compatibility period.
- Review confirms compatibility behavior for older metadata files.

**Status:** In progress

### Milestone 3: Frontend Download State

**Goal:** Prevent repository-level progress collisions in the GUI.

**Tasks:**
- [x] Change frontend download status maps from repo-keyed state to
      selected-artifact-keyed state.
- [x] Keep an adapter or facade for existing repo-keyed props until
      `RemoteModelsList`, local download overlays, delete-side cancellation,
      header status, and auth prompts are migrated.
- [x] Keep repository id available as display/search metadata, not as the
      uniqueness key for active progress.
- [x] Update active-download guards to block only the same selected artifact or
      same destination path.
- [ ] Update UI labels to distinguish artifact selections when one repository
      has multiple downloadable variants.

**Verification:**
- Frontend state tests or hook tests showing two variants from one repo can be
  tracked independently.
- Regression check that duplicate starts for the same selected artifact remain
  blocked.
- Regression check for delete-side cancellation and completion refresh after
  the key change.
- Manual GUI check or screenshot during implementation if existing frontend
  standards require it for the touched flow.

**Status:** In progress

### Milestone 4: Migration Dry-Run Planning

**Goal:** Make migration reports show every required layout and metadata change
before any filesystem mutation.

**Tasks:**
- [ ] Extend dry-run analysis to report legacy compact family tokens such as
      `qwen35`.
- [ ] Detect metadata records missing selected-artifact identity.
- [ ] Detect directories containing files from multiple selected artifacts.
- [ ] Detect completed artifact plus partial artifact mixtures.
- [ ] Add planned action kinds for `move_directory`,
      `split_artifact_directory`, `rewrite_metadata_only`,
      `blocked_collision`, and `skipped_active_download`.
- [ ] Include old id, new id, selected artifact files, source path, target path,
      and block reason in report artifacts.
- [ ] Report dependency-binding, package-fact, conversion, and runtime
      descriptor references that need model-id remapping.

**Verification:**
- Migration dry-run fixture for the reported Q4/Q5 mixed directory shape.
- Additional fixture for a legacy compact family path without mixed artifacts.
- Report schema review for checkpoint compatibility and human auditability.
- Dry-run fixture verifies existing repo-id duplicate findings are not treated
  as fatal when selected-artifact ids differ.

**Status:** Not started

### Milestone 5: Checkpointed Migration Execution

**Goal:** Execute safe moves and splits through the existing migration system
with resumable checkpoints.

**Tasks:**
- [ ] Extend checkpoint state to represent split-directory actions without
      losing completed-result tracking.
- [ ] Implement safe split behavior that copies or moves only files belonging
      to each selected artifact into the correct target directory.
- [ ] Preserve sidecars and metadata by regenerating or rewriting them per
      target artifact.
- [ ] Block or skip active partial downloads unless the action can safely
      relocate the partial and sidecar together.
- [ ] Remap model-id references in dependency bindings, package facts,
      conversion provenance, and execution descriptors where the current
      codebase stores them by model id.
- [ ] Add post-migration validation for duplicate selected-artifact ids,
      missing expected files, stale compact family paths, and mixed artifact
      directories.

**Verification:**
- Checkpoint resume test for split-directory migration.
- Post-migration integrity test catches remaining mixed directories.
- Dry-run before execution and validation after execution agree on resolved
  item counts.

**Status:** Not started

### Milestone 6: Documentation And Compatibility Cleanup

**Goal:** Record the new identity semantics for future implementers and
downstream consumers.

**Tasks:**
- [ ] Update model-library README documentation for repository identity versus
      selected-artifact identity.
- [ ] Update API or frontend README documentation for download progress keys.
- [ ] Update `docs/contracts/native-bindings-surface.md` for added download
      request/progress fields.
- [ ] Add migration notes describing legacy `qwen35` to `qwen3_5` and
      punctuation-preserving version normalization.
- [ ] Document when legacy compatibility fields can be retired.

**Verification:**
- Documentation review against `DOCUMENTATION-STANDARDS.md`.
- Traceability review confirms all changed contracts are documented.
- Native binding contract review confirms preview/stable support-tier notes are
  still accurate.

**Status:** Not started

## Execution Notes

Update during implementation:

- 2026-05-03: Plan created from the Q4/Q5 overlap investigation,
  Transformers convention review, and existing migration-system review.
- 2026-05-04: Implemented the first backend identity vertical slice. Added a
  focused selected-artifact identity module, artifact-aware HF destination
  planning, append-only download progress/start response fields, native binding
  progress field updates, and frontend API type fields.
- 2026-05-04: Explorer findings recorded that package-facts/model-ref code
  already has a `selected_artifact_id` placeholder but currently returns empty
  identity in several paths. This remains a Milestone 4/5 migration and
  reference-remapping risk.
- 2026-05-04: Implemented the persisted metadata projection slice. New
  download metadata now records publisher, architecture family,
  config-model-type, selected-artifact id/files/quant, and upstream revision.
  Download finalization preserves the resolved download family instead of
  allowing file-level format evidence to rewrite it, while legacy `family`
  remains present for compatibility. FTS5 now prefers `architecture_family`
  when present and falls back to `family`.
- 2026-05-04: Implementation issue recorded for Milestone 4/5: existing FTS5
  triggers are created with `IF NOT EXISTS`, so already-initialized databases
  may need a migration/rebuild step before search-family projection uses
  `architecture_family`. New databases and explicit rebuilds use the new
  projection.
- 2026-05-04: Integrated the frontend artifact-keyed download-state slice.
  Download maps and pause/resume/cancel actions now key by
  `selectedArtifactId ?? artifactId ?? repoId`, while `repoId` remains attached
  to each status for display, delete-side cancellation, and compatibility
  fallbacks.
- 2026-05-04: Frontend display follow-up recorded: the remote search list is
  still one row per repository, so it can track multiple same-repo artifacts in
  state but only renders one active artifact status on that repo row at a time.
  A later UI slice should surface per-artifact status/labels in the quant menu
  or an equivalent artifact-level affordance.

## Commit Cadence Notes

- Commit when a logical slice is complete and verified.
- Prefer committing the first backend identity vertical slice before expanding
  into frontend state and migration execution.
- Keep migration report schema changes, checkpoint execution changes, and
  frontend state changes as separate reviewable slices unless a shared contract
  requires them to land together.
- Follow commit format and history cleanup rules from `COMMIT-STANDARDS.md`.

## Optional Subagent Assignment

Use only if implementation is intentionally parallelized.

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| Backend identity worker | Identity module, naming helpers, download destination planning | Rust code, tests, and changed file list | After Milestone 1 tests pass |
| Migration worker | Dry-run and checkpoint execution planning | Rust code, fixtures, report schema notes, and changed file list | After Milestone 4 dry-run tests pass |
| Frontend worker | Download progress keying and display updates | Frontend code, hook tests, and changed file list | After backend DTO shape is stable |

If subagents are used, assign non-overlapping write sets before work begins and
integrate one worker wave at a time.

## Re-Plan Triggers

- The existing three-segment model id shape cannot represent selected artifacts
  without ambiguity.
- API or GUI consumers require a breaking contract change instead of append-only
  compatibility fields.
- Migration execution cannot safely split mixed directories without copying
  large files or requiring an operator prompt.
- Active partial-download relocation needs stronger locking than the current
  download lifecycle provides.
- Existing SQLite schema constraints cannot index selected-artifact identity
  without a broader database migration.
- UniFFI or Rustler consumers require a non-additive record change.
- FTS5/search/statistics behavior cannot keep compatibility with both `family`
  and `architecture_family`.
- Dependency bindings, package facts, or execution descriptors cannot be
  remapped safely during migration.
- Additional Transformers conventions are found that materially change the
  identity or path design.
- The implementation discovers non-Hugging-Face artifact sources that need the
  same identity contract but cannot map to the proposed fields.

## Recommendations

- Make selected-artifact identity a backend-domain concept first, then expose
  it through API and frontend layers. This reduces the chance that the GUI,
  migration, and downloader invent incompatible keys.
- Treat legacy `family` as a compatibility field, not a canonical field. This
  keeps older records readable while stopping new path logic from mixing
  publisher, architecture, and task concepts.
- Extend the existing migration system rather than adding an ad hoc fixer. The
  current report, checkpoint, and validation machinery already provides the
  safety controls this change needs.

## Completion Summary

### Completed

- Milestone 1 backend identity contract slice.
- Milestone 2 persisted metadata/index projection slice is partially complete.
- Milestone 3 frontend download-state keying slice is partially complete.

### Deviations

- None.

### Follow-Ups

- Add a migration/rebuild step for existing FTS5 triggers so deployed
  databases pick up the `architecture_family` projection.
- Complete `.pumas_download` selected-artifact recovery reads before migration
  execution moves or splits partial downloads.
- Add artifact-level labels/status in the remote model row or quant menu so
  simultaneous same-repo artifact downloads are visibly distinguished.

### Verification Summary

- 2026-05-04: `cargo test --manifest-path rust/Cargo.toml -p pumas-library artifact_identity`
- 2026-05-04: `cargo test --manifest-path rust/Cargo.toml -p pumas-rpc test_download_start_response_includes_selected_artifact_aliases`
- 2026-05-04: `cargo check --manifest-path rust/Cargo.toml`
- 2026-05-04: `cargo check --manifest-path rust/Cargo.toml -p pumas_rustler`
- 2026-05-04: `npm run -w frontend check:types`
- 2026-05-04: `cargo test --manifest-path rust/Cargo.toml -p pumas-library artifact_identity`
- 2026-05-04: `cargo test --manifest-path rust/Cargo.toml -p pumas-library test_upsert_download_metadata_stub_persists_hf_evidence`
- 2026-05-04: `cargo test --manifest-path rust/Cargo.toml -p pumas-library test_fts5_prefers_architecture_family_projection`
- 2026-05-04: `cargo check --manifest-path rust/Cargo.toml -p pumas-library`
- 2026-05-04: `npm run -w frontend test:run -- src/hooks/useModelDownloads.test.ts src/hooks/useDownloadCompletionRefresh.test.ts src/components/ModelManagerRemoteDownload.test.ts src/hooks/useModelLibraryActions.test.ts src/components/ModelManagerUtils.test.ts src/components/RemoteModelsList.test.tsx src/components/RemoteModelListItem.test.tsx src/components/LocalModelDownloadActions.test.tsx src/components/LocalModelRowState.test.ts`
- 2026-05-04: `npm run -w frontend check:types`

### Traceability Links

- Module README updated:
  `rust/crates/pumas-core/src/model_library/hf/README.md`
- ADR added/updated: N/A at plan creation.
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: N/A at plan
  creation.

## Brevity Note

Expand this plan only where execution decisions, migration safety, or contract
compatibility require more detail.
