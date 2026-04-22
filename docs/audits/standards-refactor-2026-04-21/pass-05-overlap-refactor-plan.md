# Pass 05 - Overlap-Aware Refactor Plan

## Overlap Model
The audit found a maximum overlap depth of 4. The deepest targets, such as `frontend/src/types/api.ts`, `electron/src/preload.ts`, `rust/crates/pumas-rpc/src/handlers/mod.rs`, `rust/crates/pumas-core/src/api/state.rs`, and `rust/crates/pumas-core/src/model_library/library.rs`, are affected by:

1. contract ownership drift;
2. boundary validation gaps;
3. decomposition/state ownership issues;
4. verification/tooling gaps.

The plan therefore makes four passes over the refactor strategy. Each pass unlocks the next and avoids splitting files before the durable boundaries are known.

## Depth 1 - Establish Contracts and Governance

Goal: create the stable target shape before moving code.

Tasks:

- Add a standards adoption map under `docs/` linking each external standard to the project rule, enforcement status, and exceptions.
- Add missing READMEs for directories listed in pass 1.
- Add a desktop/RPC method registry contract:
  - method name;
  - request schema;
  - response schema;
  - owner module;
  - stability tier;
  - auth/trust boundary;
  - frontend exposure policy.
- Add release artifact contract for Electron app, Rust native libraries, generated bindings, checksums, and SBOMs.
- Add crate role map for Rust workspace.

Exit criteria:

- New contract docs are present and reviewed.
- No source behavior has to change yet except tests/tooling that validate contracts.
- Every later refactor target has an owning boundary.

## Depth 2 - Validate Boundaries and Parse Once

Goal: make ingress safe before internal decomposition.

Tasks:

- Electron:
  - add `ipc-validation.ts`;
  - validate `api:call`, dialog options, and shell URL payloads as `unknown`;
  - reject unregistered RPC method names before forwarding.
- Rust RPC:
  - introduce typed request structs for highest-risk methods first: imports, downloads, path open, config, process launch, deletion;
  - parse `serde_json::Value` into typed commands at handler boundaries.
- Rust paths:
  - add validated path newtypes for launcher root, library root, model paths, external import paths, and open-path requests;
  - add symlink/space/canonical path tests.
- Torch:
  - add Pydantic validators and shared path validator;
  - add LAN binding policy validation.
- Bindings:
  - classify exported APIs by support tier and validate host-facing path/string inputs.

Exit criteria:

- Boundary tests prove malformed inputs are rejected.
- Internal services start accepting typed validated commands for new/refactored paths.
- No broad file split occurs until command boundaries are typed.

## Depth 3 - Decompose Along Ownership Boundaries

Goal: reduce large files by extracting real responsibilities.

Rust extraction sequence:

1. `model_library/library.rs`
   - facade stays as `ModelLibrary`;
   - extract `catalog`, `metadata_store`, `migration`, `projection`, `integrity`, `dependency_resolution`, `download_recovery`, and test fixtures.
2. `model_library/importer.rs`
   - split validation, copy/link execution, sharding, recovery, and result projection.
3. `model_library/hf/download.rs`
   - split planning, HTTP transfer, destination locking, progress state, resume/recovery.
4. `index/model_index.rs`
   - split schema/migrations, CRUD, search/query, dependency profiles, overlays.
5. `api/state.rs` and `api/reconciliation.rs`
   - split typed IPC dispatch from reconciliation state machine and durable effects.
6. `pumas-uniffi/src/bindings.rs`
   - split by domain once supported binding surface is classified.

Frontend extraction sequence:

1. `frontend/src/types/api.ts`
   - replace hand-maintained monolith with generated/grouped contract modules.
2. `frontend/src/App.tsx`
   - extract composition/root state wiring from layout.
3. `ModelManager.tsx`, `LocalModelsList.tsx`, `InstallDialog.tsx`, `ConflictResolutionDialog.tsx`
   - separate workflow owner hooks from presentational components.
4. Large app-panel sections
   - move polling and mutation workflows into hooks or backend events.

Python/launcher extraction sequence:

1. `torch-server/serve.py`
   - fresh app factory.
2. `torch-server/model_manager.py`
   - explicit state machine and manager lock.
3. launcher docs and dependency installer
   - clarify run/build artifact semantics.

Exit criteria:

- Each extracted module has a README or updated parent README.
- New modules expose fewer than roughly 7 public functions or have an explicit decomposition exception.
- Existing public API remains stable or has changelog/migration notes.

## Depth 4 - Enforce and Ratchet

Goal: prevent regression after the structural cleanup.

Tasks:

- Add `.editorconfig`.
- Add `lefthook.yml`:
  - pre-commit: format, lint, typecheck, decision traceability, staged schema/artifact validation;
  - pre-push: tests and targeted audit checks.
- Add CI:
  - Linux and Windows required builds/tests;
  - Rust fmt/clippy/test/doc/all-features/no-default-features;
  - frontend lint/typecheck/test;
  - Electron validate;
  - launcher tests;
  - Python tests/lint if Python tooling is adopted;
  - release smoke with documented GUI strategy.
- Add Rust `[workspace.lints]` and member `[lints] workspace = true`.
- Add unsafe policy:
  - deny by default;
  - relax only in OS/FFI modules;
  - require `SAFETY:` comments.
- Re-enable or scope frontend max-lines/complexity rules.
- Add dependency ownership verification:
  - package-local commands must declare their own tools;
  - Cargo duplicate/audit checks;
  - package manager frozen lockfile checks.
- Add Criterion benchmarks for model index/search, metadata migration/projection, and download planning if performance claims are made.

Exit criteria:

- CI blocks new violations for areas already remediated.
- Legacy exceptions are listed with owner, rationale, and revisit trigger.
- Refactor plan can transition from broad audit to ordinary tracked issues.

## Recommended Milestone Breakdown

### Milestone 1 - Governance and Contract Baseline
- Add standards adoption map.
- Add missing high-level READMEs.
- Define RPC/desktop contract registry.
- Fix package dependency ownership for TypeScript/Electron tooling.

### Milestone 2 - Boundary Hardening
- Electron IPC validation.
- Rust typed RPC commands for path/import/download/destructive methods.
- Torch validators and app factory fix.
- Path validation newtypes.

### Milestone 3 - Rust Model Library Decomposition
- Extract `ModelLibrary` submodules.
- Extract importer/download/index modules.
- Move test fixtures out of production files where possible.

### Milestone 4 - Frontend State and Accessibility
- Generate/group API types.
- Split `App.tsx` and model manager flows.
- Replace generic interactive elements and `window.confirm` where feasible.
- Consolidate or justify polling.

### Milestone 5 - Async, Unsafe, and Binding Surfaces
- Task supervisor and shutdown.
- Unsafe isolation/lints.
- Binding surface classification and split.
- Host-language verification matrix.

### Milestone 6 - Enforcement and Release Readiness
- Hooks and CI matrix.
- Rust/Node/Python audit checks.
- Release artifact/checksum/SBOM workflow.
- Performance benchmarks for hot paths.

## First Slice Recommendation
Start with Milestone 1 plus the Electron `api:call` allowlist. That combination creates a source of truth for method names and immediately reduces the highest-risk trust-boundary gap without destabilizing the large Rust model-library refactor.
