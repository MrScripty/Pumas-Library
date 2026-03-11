# Plan: Transparent Client-Mode Dispatch for `PumasApi`

## Objective

Implement full IPC-backed client-mode dispatch in `pumas-core` so low-level
Rust callers receive a usable `PumasApi` handle whether the process becomes the
primary or attaches to an existing primary for the same launcher root.

## Scope

### In Scope

- Real IPC-backed `ApiInner::Client`
- Method-by-method audit and parity implementation across public `PumasApi`
  modules
- Missing IPC handlers and request/response shapes required for parity
- Constructor behavior updates after parity is verified
- Documentation updates for the low-level Rust API contract
- Regression and race coverage for attach behavior and API parity

### Out of Scope

- IPC transport replacement
- Distributed or multi-host coordination
- Unrelated model-library, launcher, or app-manager refactors
- Frontend changes beyond documentation alignment

## Inputs

### Problem

- Strict singleton ownership is already enforced per launcher root.
- UniFFI constructors already converge to a working primary or client handle.
- Raw Rust `PumasApi::new()` still returns `PrimaryInstanceBusy` instead of a
  usable client-backed handle.
- `ApiInner::Client` exists conceptually but is not a full transparent
  implementation.

### Constraints

- Preserve the `PumasApi` public facade where feasible.
- Do not weaken the strict single-primary claim lifecycle.
- Keep primary-owned lifecycle work exclusive to the primary.
- Avoid partial client mode that silently works for some methods and fails
  unpredictably for others.
- Update docs in the same implementation slice as behavior changes.

### Assumptions

- Transparent client behavior is the intended long-term Rust API contract.
- Existing IPC transport is sufficient if missing handlers are added.
- Some methods may remain intentionally primary-only, but only as explicit,
  documented exceptions.

### Dependencies

- `rust/crates/pumas-core/src/lib.rs`
- `rust/crates/pumas-core/src/api/*.rs`
- `rust/crates/pumas-core/src/ipc/*.rs`
- `rust/crates/pumas-core/src/registry/*.rs`
- Existing strict-primary-claim implementation and docs

### Affected Structured Contracts

- `PumasApi` constructor behavior
- `ApiInner` runtime dispatch semantics
- IPC method coverage and payload contracts
- Primary-only versus proxyable method boundaries
- Error mapping consistency between local and remote execution

### Affected Persisted or Runtime Artifacts

- Registry claim rows in `registry.db`
- IPC server lifecycle for the winning primary
- Client-side in-memory `IpcClient` handles
- No new persistent storage is expected unless IPC schema/versioning requires it

### Concurrency and Race-Risk Review

- Client attach must respect the existing `claiming` to `ready` lifecycle.
- Losing constructors must not start primary-owned background tasks.
- Client reconnect and failure semantics must remain coherent when the primary
  exits.
- Race coverage must include overlapping primary startup and attach after claim
  promotion.

## Current Method Classification Matrix

### Already IPC-Backed

- Model library:
  `list_models`, `search_models`, `get_model`, `delete_model_with_cascade`,
  `import_model`, `import_models_batch`, `rebuild_model_index`,
  `reclassify_model`, `reclassify_all_models`, `get_inference_settings`,
  `update_inference_settings`
- HuggingFace:
  `search_hf_models`, `start_hf_download`, `get_hf_download_progress`,
  `cancel_hf_download`, `list_interrupted_downloads`, `recover_download`,
  `lookup_hf_metadata_for_file`, `get_hf_repo_files`
- Network/system/process/conversion:
  `is_online`, `get_disk_space`, `get_status`, `get_system_resources`,
  `is_torch_running`, `stop_torch`, `start_conversion`,
  `get_conversion_progress`, `cancel_conversion`, `list_conversions`,
  `is_conversion_environment_ready`, `ensure_conversion_environment`,
  `supported_quant_types`, `backend_status`, `ensure_backend_environment`

### Missing IPC Coverage and Must Be Evaluated

- Model library:
  `get_library_status`, `resolve_model_dependency_requirements`,
  `resolve_model_execution_descriptor`, `audit_dependency_pin_compliance`,
  `list_models_needing_review`, `submit_model_review`,
  `reset_model_review`, `get_effective_model_metadata`,
  `import_external_diffusers_directory`, `import_model_in_place`,
  `adopt_orphan_models`, `get_link_health`, `clean_broken_links`,
  `get_links_for_model`, `preview_model_mapping`, `apply_model_mapping`,
  `sync_models_incremental`, `sync_with_resolutions`,
  `reclassify_model` follow-on report operations,
  `generate_model_migration_dry_run_report`, `execute_model_migration`,
  `list_model_migration_reports`, `delete_model_migration_report`,
  `prune_model_migration_reports`
- HuggingFace:
  `search_hf_models_with_hydration`, `get_hf_download_details`,
  `pause_hf_download`, `resume_hf_download`, `list_hf_downloads`,
  `resume_partial_download`, `refetch_metadata_from_hf`,
  `lookup_hf_metadata_for_bundle_directory`, `set_hf_token`,
  `clear_hf_token`, `get_hf_auth_status`
- Process/system/network:
  `connectivity_state`, `check_connectivity`, `network_status`,
  `get_network_status_response`, `is_comfyui_running`,
  `get_running_processes`, `set_process_version_paths`, `stop_comfyui`,
  `is_ollama_running`, `stop_ollama`, `launch_ollama`, `launch_torch`,
  `launch_version`, `get_last_launch_log`, `get_last_launch_error`,
  `has_background_fetch_completed`, `reset_background_fetch_flag`,
  `check_launcher_updates`, `apply_launcher_update`

### Candidate Primary-Only Methods

- `network_manager`, `model_library` accessors
- Local shell/open helpers:
  `open_path`, `open_url`, `open_directory`
- Patch/restart/system-probe helpers:
  `get_launcher_version`, `restart_launcher`, `is_patched`, `toggle_patch`,
  `check_git`, `check_brave`, `check_setproctitle`
- Synchronous classification/helpers that may depend on local file layout:
  `validate_file_type`, `classify_model_import_paths`,
  `get_cross_filesystem_warning`, `set_model_link_exclusion`,
  `get_link_exclusions`

### Constructor Contract Targets

- `PumasApi::new()` must not auto-client until the proxyable method set is
  implemented and verified.
- During implementation, `PrimaryInstanceBusy` remains acceptable for raw Rust
  callers.
- After parity is complete, `PumasApi::new()` should:
  - become primary when claim succeeds
  - wait and attach as a client when another process owns or is starting the
    same launcher root
- Any explicit primary-only constructor, if needed, must be named as such and
  documented separately.

## Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| Public methods have undocumented primary-only assumptions | High | Inventory and classify every public `PumasApi` method before changing constructor behavior |
| IPC handler coverage is incomplete, producing partial client parity | High | Add explicit parity matrix and close all gaps before enabling transparent Rust auto-client |
| Error behavior differs between local and remote execution | High | Add parity tests for representative success and failure paths |
| Client-mode refactor causes broad regressions across API modules | Medium | Use shared dispatch helpers and commit module slices atomically after targeted verification |
| Constructor contract flips too early | Medium | Delay `PumasApi::new()` behavior change until client parity milestones are verified |

## Definition of Done

- `ApiInner::Client` stores a real IPC client and supports transparent dispatch
  for all supported public `PumasApi` methods.
- Every public `PumasApi` method is explicitly classified as proxyable or
  primary-only.
- Proxyable methods work in both primary and client mode with equivalent
  behavior and error semantics.
- Primary-only methods return explicit, documented errors in client mode.
- `PumasApi::new()` returns a working primary or client-backed handle without
  split-brain risk.
- Rust integration tests cover primary attach, client attach, and representative
  API parity.
- Architecture and crate docs accurately describe the low-level Rust contract.

## Milestones

### Milestone 1: Establish the Client-Parity Contract

**Goal:** Define the intended low-level Rust API contract before implementation.

**Tasks:**
- [ ] Inventory all public `PumasApi` methods across API modules.
- [ ] Classify each method as `proxyable`, `primary-only`, or `needs decision`.
- [ ] Record the target Rust constructor behavior and client-mode expectations.
- [ ] Identify all missing IPC handlers and schema gaps required for parity.
- [ ] Add the method classification matrix and target contract to this plan.

**Verification:**
- Method matrix completed with no unclassified public methods.
- Review against current IPC dispatch coverage in
  `rust/crates/pumas-core/src/api/state.rs`.
- Plan artifact conforms to planning/documentation standards.

**Status:** In progress

### Milestone 2: Build Shared Client Dispatch Infrastructure

**Goal:** Make client mode real at the core dispatch layer.

**Tasks:**
- [ ] Refactor `ApiInner::Client` to hold `Arc<IpcClient>`.
- [ ] Add shared helpers for local-or-remote dispatch to avoid ad hoc branching.
- [ ] Add consistent client-side request encoding, response decoding, and error
  mapping helpers.
- [ ] Implement or refine constructors/discovery helpers that can create real
  client-backed `PumasApi` values without changing public behavior yet.
- [ ] Ensure primary-only helpers fail coherently rather than panicking when
  invoked from client mode.

**Verification:**
- Targeted tests for helper behavior and error mapping.
- Compile/test pass proving a client-backed `PumasApi` can execute a minimal
  IPC-backed method set.
- No undocumented `self.primary()` assumptions remain in the infrastructure
  layer.

**Status:** Not started

### Milestone 3: Deliver Module-by-Module API Parity

**Goal:** Make the public API work in client mode before changing constructor
behavior.

**Tasks:**
- [ ] Implement model-library API parity.
- [ ] Implement HF/download API parity.
- [ ] Implement process/system/network/conversion parity where appropriate.
- [ ] Add missing IPC handlers in `rust/crates/pumas-core/src/api/state.rs`.
- [ ] For intentionally primary-only methods, return explicit client-mode
  errors and document the exception.

**Verification:**
- Targeted parity tests for representative methods in each module.
- Negative-path tests for primary-only methods.
- Regression check that primary behavior remains unchanged.

**Status:** Not started

### Milestone 4: Promote Transparent Rust Constructor Behavior

**Goal:** Change the low-level Rust constructor only after parity is proven.

**Tasks:**
- [ ] Update `PumasApi::new()` to return a client-backed handle when another
  primary already owns or is starting the launcher root.
- [ ] Decide and document whether `discover()` remains separate or converges on
  the same behavior.
- [ ] Reuse the existing strict registry claim lifecycle for wait/attach
  behavior.
- [ ] Remove the standard-caller dependency on handling
  `PrimaryInstanceBusy` manually, except where explicitly retained for a
  specialized primary-only path.

**Verification:**
- Integration tests where one process becomes primary and another receives a
  working client-backed `PumasApi`.
- Race tests for `claiming` to `ready` attach behavior.
- Confirmation that no losing constructor starts primary-owned lifecycle work.

**Status:** Not started

### Milestone 5: Align Documentation and Usage Guidance

**Goal:** Make the codebase and docs reflect one coherent contract.

**Tasks:**
- [ ] Update low-level Rust API docs to describe transparent dual-mode behavior.
- [ ] Update IPC and architecture docs to reflect Rust parity with wrapper
  attach behavior.
- [ ] Document any remaining primary-only operations explicitly.
- [ ] Add concise troubleshooting guidance for attach failures and
  shared-instance loss semantics.

**Verification:**
- Consistency pass across architecture and crate docs.
- Documentation traceability review against implemented behavior.
- Final documentation changes satisfy documentation standards.

**Status:** Not started

## Execution Notes

Update during implementation:
- 2026-03-10: Added the method classification matrix before changing any Rust
  constructor behavior so parity work has an explicit checklist.

## Commit Cadence Notes

- Commit after each verified logical slice:
- client-parity contract artifact
- dispatch infrastructure
- module parity slices
- constructor behavior change
- documentation alignment and final regressions

## Re-Plan Triggers

- A meaningful portion of the public API cannot be cleanly proxied over current
  IPC contracts
- Primary-only assumptions in process/system APIs require an explicit API split
- IPC schema changes expand beyond incremental handler additions
- Constructor compatibility implications for Rust embedders differ materially
  from current assumptions

## Recommendations (Only If Better Option Exists)

- Treat this as an API-parity project first and a constructor-behavior change
  second.
- Do not flip `PumasApi::new()` to auto-client until module parity is verified.
- Centralize dual-mode dispatch helpers instead of scattering `match self.inner`
  across modules.
- Keep the strict registry claim lifecycle unchanged and build transparent
  client behavior on top of it.

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
