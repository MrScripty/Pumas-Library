# Plan: Pumas Artifact Load Target Resolution

## Objective
Implement a Pumas-owned artifact load-target resolver that lets Pantograph pass
a selected `PumasModelRef` to Pumas and receive either a Pumas-approved local
load target or typed readiness diagnostics.

## Scope

### In Scope
- Public request/response DTOs and diagnostics in `pumas-core` model contracts.
- One shared model-library resolver core with explicit `OwnerFresh` and
  `ReadOnlyIndexed` modes.
- Read-only, owner, API, RPC, and local-client entry points as thin adapters
  over the shared resolver core.
- Tests for serde fixtures, exact selected-artifact behavior, external
  references, read-only non-mutation, mode enforcement, and exposed boundaries.
- README updates for touched source directories.
- Pantograph integration at the Rust composition boundary before worker
  dispatch.

### Out of Scope
- Managed download/materialization orchestration.
- Pantograph scheduler, retry, queue, or worker lifecycle policy.
- Python worker path resolution or direct Pumas calls.
- Migration of existing Ollama, ONNX, llama.cpp, and serving validation
  `get_primary_model_file` call sites.
- Public UniFFI or generated host-language binding exposure unless explicitly
  required by a concrete host-language consumer.

## Inputs

### Problem
Pantograph can identify a selected Pumas artifact but cannot safely derive the
local path to load without duplicating Pumas storage, external-reference, and
selected-artifact semantics.

### Constraints
- `PumasModelRef` is the authoritative selected-artifact reference.
- Caller-observed package facts are stale-check inputs only.
- `ReadOnlyIndexed` mode must not write metadata, update SQLite/cache state,
  start watchers, repair projections, or perform owner-only freshness work.
- `PumasReadOnlyLibrary` must reject `OwnerFresh` with a typed boundary error
  or diagnostic.
- The hot path must not wrap broad model-level APIs, regenerate package facts,
  deep-scan roots, or compute fingerprints just to resolve a target.
- External-reference assets are valid load targets when Pumas validates them.
- Boundary surfaces must parse and validate typed DTOs and return typed errors
  or diagnostics for normal unavailable states.

### Assumptions
- Existing selected artifact metadata, selector/index state, package-facts
  cache rows, and external-reference records can support the first read-only
  resolver without a schema migration.
- Pantograph can consume an owner-fresh Pumas API call before worker dispatch.
- UniFFI exposure is not needed for Pantograph's first integration.

### Dependencies
- `rust/crates/pumas-core/src/models/`
- `rust/crates/pumas-core/src/model_library/`
- `rust/crates/pumas-core/src/model_library/package_facts/`
- `rust/crates/pumas-core/src/index/model_index/`
- `rust/crates/pumas-core/src/api/`
- `rust/crates/pumas-core/src/ipc/`
- `rust/crates/pumas-rpc/`
- Pantograph runtime integration.

### Affected Structured Contracts
- `ResolveModelArtifactLoadTargetRequest`
- `ResolveModelArtifactLoadTargetResponse`
- `PumasArtifactLoadTarget`
- `PumasArtifactLoadTargetDiagnostic`
- `PumasArtifactLoadTargetDiagnosticCode`
- `PumasArtifactLoadPathKind`
- `PumasArtifactLoadTargetResolutionMode`
- RPC/IPC/local-client method payloads if exposed.

### Affected Persisted Artifacts
- None expected in the first read-only/core slice.
- Existing model index, metadata, package-facts cache, and external-reference
  records are read as inputs.
- Any required schema, cache, or metadata mutation is a re-plan trigger.

### Ownership And Lifecycle Note
`ModelLibrary` owner mode may perform freshness work already owned by the
library instance. `PumasReadOnlyLibrary` owns no lifecycle work and must reject
`OwnerFresh`. API, RPC, IPC, and local-client layers do not own resolver policy;
they validate/transport typed requests and delegate to the core resolver.

### Public Facade Preservation Note
This adds a new public capability. Existing model/package-facts/descriptor APIs
remain intact and must not be repurposed as the new selected-artifact resolver.

## Definition Of Done
- Pantograph can request a load target for a selected artifact without knowing
  Pumas library roots.
- Pumas returns typed readiness diagnostics while preserving
  `ModelArtifactState` and `ModelEntryPathState` fidelity.
- Read-only surfaces are proven non-mutating and reject owner-fresh mode.
- The resolver supports library-owned and validated external-reference paths.
- The implementation has passing tests for resolver behavior, serde fixtures,
  mode enforcement, exposed boundaries, and Pantograph worker-envelope use.
- Changed source READMEs document new responsibilities and contracts.
- Each verified vertical slice is committed atomically.

## Milestones

### Milestone 0: Plan Package
**Goal:** Promote the accepted proposal into the Pumas documentation layout.

**Tasks:**
- [x] Create a slugged plan directory.
- [x] Copy the accepted Pantograph proposal into the plan directory.
- [x] Add this implementation plan and directory README.
- [x] Update `docs/plans/README.md`.

**Verification:**
- `git diff --check`
- Documentation-only status shows only expected plan files.

**Status:** Complete

### Milestone 1: DTO Contract Slice
**Goal:** Add typed request/response contracts without resolver behavior.

**Tasks:**
- [x] Add model DTOs and diagnostics using existing Pumas contracts.
- [x] Export the DTOs through the existing model module pattern.
- [x] Add serde fixture round-trip tests.
- [x] Update model contract documentation.

**Verification:**
- `cargo fmt --manifest-path rust/Cargo.toml --all`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library --test artifact_load_target_contract_fixtures`
- `cargo check --manifest-path rust/Cargo.toml -p pumas-library`

**Status:** Complete

### Milestone 2: Read-Only Resolver Core
**Goal:** Resolve exact selected artifacts from indexed/cache state without
mutation.

**Tasks:**
- [x] Add focused `model_library/artifact_load_target.rs` resolver module.
- [x] Reuse or extract lower-level selected-artifact helpers without wrapping
  broad model-level APIs.
- [x] Implement initial `ReadOnlyIndexed` behavior and typed unavailable diagnostics.
- [ ] Add full non-mutation tests and core resolver state tests.
- [x] Update model-library documentation.

**Verification:**
- Resolver tests for ready, missing, partial, invalid, stale, needs-detail,
  ambiguous, kind mismatch, and external-reference cases as feasible from
  current fixtures.
- Read-only non-mutation test.
- `cargo fmt`, targeted tests, and `cargo check`.

**Slice Verification:**
- `cargo fmt --manifest-path rust/Cargo.toml --all`
- `cargo test --manifest-path rust/Cargo.toml -p pumas-library read_only_library`
- `cargo check --manifest-path rust/Cargo.toml -p pumas-library`

**Status:** In Progress

### Milestone 3: Owner Freshness Surface
**Goal:** Route owner-mode resolution through the same resolver core.

**Tasks:**
- [ ] Add `ModelLibrary` owner-fresh entry point.
- [ ] Allow owner freshness only through owner-owned surfaces.
- [ ] Prove owner-fresh and read-only behavior diverge only by explicit mode.

**Verification:**
- Tests showing `OwnerFresh` may refresh external-reference state while
  `ReadOnlyIndexed` does not.
- Tests showing no full package-facts regeneration or fingerprint computation.

**Status:** Pending

### Milestone 4: API, RPC, And Local Client Surfaces
**Goal:** Expose the resolver through typed thin adapters.

**Tasks:**
- [ ] Add Pumas API/state entry point.
- [ ] Add RPC/IPC/local-client method if required by Pantograph's selected
  integration path.
- [ ] Enforce allowed modes per surface, especially read-only rejection of
  `OwnerFresh`.
- [ ] Update API/RPC/IPC documentation and tests.

**Verification:**
- Boundary tests reject malformed or disallowed mode payloads.
- API/RPC/local-client tests for ready and non-ready responses.

**Status:** Pending

### Milestone 5: Pantograph Integration
**Goal:** Resolve load targets before worker dispatch and pass only approved
targets to workers.

**Tasks:**
- [ ] Update Pantograph Rust integration to call the Pumas resolver at the
  composition boundary.
- [ ] Map non-ready responses to terminal planning/readiness diagnostics.
- [ ] Keep Python worker envelope free of Pumas roots, handles, and repair data.

**Verification:**
- Pantograph tests for ready Diffusers target, non-ready diagnostics, and worker
  envelope shape.

**Status:** Pending

## Issue Register
- M2-001: Read-only resolver currently proves ready cache, kind mismatch, and
  read-only mode rejection. Missing, partial, invalid, stale, needs-detail,
  external-reference, and explicit non-mutation coverage still need to be added
  before Milestone 2 can close.

## Risks And Mitigations
| Risk | Mitigation |
| ---- | ---------- |
| Resolver logic drifts across surfaces | One shared core with explicit modes |
| Read-only mode mutates state | Non-mutation tests plus boundary mode rejection |
| Hot path hydrates broad facts | Avoid broad model-level APIs and test optional fingerprint behavior |
| Existing persisted state is insufficient | Re-plan before schema/cache changes |
| External-reference paths are treated as library-root-only | Preserve `StorageKind` and `AssetValidationState` and test external paths |

## Re-Plan Triggers
- Schema, cache, or metadata migration is required.
- Correctness requires deep scans or package-facts regeneration.
- A surface cannot enforce allowed resolver modes.
- Owner freshness requires new background lifecycle management.
- Pantograph requires a language-binding surface rather than Rust/API access.

## Completion Summary
- Pending implementation.
