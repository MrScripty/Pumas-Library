# Plan: Feasible Execution Candidates

## Objective

Add a Pumas model-library contract that returns feasible execution candidates and basic fact-based technical information from durable model facts without introducing a live runtime registry or making answer-quality claims.

## Scope

### In Scope

- Add an additive backend/API contract for feasible execution candidates
- Reuse existing model metadata, dependency bindings, and execution-descriptor flows
- Include basic comparable technical facts such as size and simple RAM/VRAM estimates derived from verifiable facts
- Optionally project best-candidate summaries into execution-descriptor output where useful for hosts
- Add regression coverage and contract documentation

### Out of Scope

- Any Pumas-owned live runtime/process registry
- Session-aware warmup, retention, or eviction policy
- Prompt-semantic or answer-quality model recommendation
- Pantograph-specific scheduling logic
- A new standalone registry separate from the existing model-library/index/dependency systems

## Inputs

### Problem

Pumas already knows model metadata, dependency feasibility, backend hints, and basic resource-relevant facts, but host applications currently need to assemble their own view of which model/backend combinations are technically runnable and what durable technical facts are relevant for client-side runtime decisions. That leads to duplicated selection logic and weak interoperability. Pumas should provide feasible execution candidates plus basic fact-based technical information, while leaving runtime configuration and live placement decisions to hosts.

### Constraints

- Extend current Rust/Electron-era model-library architecture rather than add a parallel subsystem
- Reuse existing `recommended_backend`, `runtime_engine_hints`, dependency bindings, and execution descriptor flows
- Keep recommendations grounded in verifiable technical facts only
- Preserve compatibility for current `resolve_model_dependency_requirements` and `resolve_model_execution_descriptor` callers
- Keep live runtime ownership outside Pumas
- Do not model inference-specific tuning details such as sequence length, batch size, offload policy, or live memory occupancy

### Assumptions

- Initial candidate ordering may rely on deterministic backend/dependency facts and simple comparable memory estimates rather than benchmark-style heuristics
- Candidate generation can be additive and optional for current API consumers
- Resource estimates may be partial; contracts should carry estimate confidence/source explicitly
- Platform-aware feasibility should continue to flow through existing dependency-resolution and metadata systems
- Current system/process memory usage remains a client concern and is out of scope for candidate generation

### Dependencies

- Existing `ModelMetadata` fields including `recommended_backend` and `runtime_engine_hints`
- Existing dependency profile and binding resolution paths
- Existing execution descriptor contract and API surface
- Existing model-library metadata projection/index flow
- Existing test/documentation standards in the Pumas repo

### Affected Structured Contracts

- New feasible execution candidate DTO(s)
- `resolve_model_execution_descriptor` output if candidate summary is attached there
- Additive API surface for candidate resolution
- Effective metadata projection if new durable estimate fields are exposed

### Affected Persisted Artifacts

- Potential `metadata.json` additions only if durable resource-estimate fields are stored
- Potential `models.db` `metadata_json` projection changes only if those fields are persisted
- No new registry/database is allowed in this plan

### Concurrency / Race-Risk Review

- Candidate generation must remain a pure read of durable model/index/dependency state in milestone one
- No background polling or process lifecycle ownership is added in this plan
- If external asset validation refresh updates metadata concurrently, candidate resolution must either use the current validated snapshot or fail deterministically
- Lifecycle ownership:
  - Model-library indexing/validation remains the producer of durable facts
  - Candidate resolution is a read-time consumer only
  - Host applications own any live runtime admission, warmup, and eviction behavior

### Risks

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| Candidate output drifts into unverifiable “best model” claims | High | Constrain outputs to dependency/backend feasibility and basic technical facts; document non-goals explicitly |
| Proposal introduces a parallel runtime-selection contract | High | Extend execution descriptor/dependency flows; do not add a Pumas runtime registry |
| Resource estimates are too vague or presented as runtime guarantees | High | Expose them as simple comparable estimates with confidence/source and keep runtime interpretation in clients |
| Existing execution-descriptor callers regress | Medium | Keep candidate resolution additive and preserve current descriptor fields/semantics |
| Conflicting backend hints produce unstable ranking | Medium | Define deterministic tie-breaking and explicit conflict handling |

## Clarifying Questions (Only If Needed)

- None at plan creation time.
- Reason: objective, boundaries, and host/Pumas ownership are specific enough to sequence safely.
- Revisit trigger: the team decides Pumas should own measured runtime telemetry or host-specific placement policy.

## Definition of Done

- Pumas exposes a deterministic additive API for feasible execution candidates
- Candidate data is derived from durable technical facts only
- Contracts include backend feasibility, size/resource facts, simple RAM/VRAM estimates where available, estimate confidence/source, and exclusion reasons
- Existing execution descriptor and dependency APIs remain compatible
- Documentation explains the boundary between Pumas candidate generation and host runtime selection/configuration

## Milestones

### Milestone 1: Contract And Boundary Definition

**Goal:** Define the candidate contract and lock the architectural boundary.

**Tasks:**
- [ ] Add candidate DTOs with fields for backend key, dependency feasibility, model size, estimated RAM/VRAM where available, estimate confidence/source, hard exclusion reasons, and ordering reasons
- [ ] Define deterministic ordering semantics using dependency/backend facts and simple comparable technical facts only
- [ ] Record a facade-preservation note: preserve current dependency and execution-descriptor contracts; add candidate resolution as an append-only capability
- [ ] Document non-goals: no answer-quality scoring, no live runtime registry, no host-session policy, and no inference-configuration logic

**Verification:**
- Contract additions compile across Rust and any affected bindings/types
- Review confirms no new parallel runtime facade is introduced
- Documentation reflects the actual fact-based contract and non-goals

**Status:** Not started

### Milestone 2: Candidate Resolution From Existing Durable Facts

**Goal:** Produce feasible candidates by extending current metadata and dependency systems.

**Tasks:**
- [ ] Implement a candidate-resolution path that reuses `recommended_backend`, `runtime_engine_hints`, model type/task type, dependency bindings, and platform/backend filtering
- [ ] Define how hard exclusions are derived from dependency mismatch, unsupported backend/platform, or invalid asset state
- [ ] Derive simple comparable memory/resource facts from durable facts such as `size_bytes`, model architecture hints, backend, and known memory-placement implications
- [ ] Keep deterministic handling for conflicting or partial metadata

**Verification:**
- Unit tests cover deterministic candidate ordering for the same model/platform inputs
- Tests cover conflicting backend hints, missing dependency bindings, and invalid external-asset states
- Tests cover complete and partial memory-estimate outputs without inferring runtime configuration
- Regression tests verify existing dependency-resolution behavior remains unchanged

**Status:** Not started

### Milestone 3: Execution Descriptor Integration

**Goal:** Expose candidate information through existing consumer-facing flows without breaking current clients.

**Tasks:**
- [ ] Decide whether execution descriptor includes a best-candidate summary, a full candidate list, or neither in milestone one
- [ ] Keep `resolve_model_execution_descriptor` backward-compatible for current consumers
- [ ] Add additive API methods for direct candidate resolution where full detail is needed
- [ ] Ensure execution descriptor semantics remain source-of-truth for model entry-path/runtime contract, not host placement policy or inference configuration

**Verification:**
- Contract tests verify existing descriptor fields and meanings remain stable
- Integration tests verify candidate-aware consumers can opt in without changing old callers
- Binding/API review confirms additive compatibility

**Status:** Not started

### Milestone 4: Durable Estimate Refinement And Documentation

**Goal:** Make candidate outputs explainable and safe for host consumption.

**Tasks:**
- [ ] Decide which estimate inputs should remain computed-at-read versus persisted in metadata/index
- [ ] Add confidence/source fields so hosts can distinguish precise, partial, and heuristic estimates
- [ ] Document how hosts should combine Pumas feasible candidates and basic technical facts with app-local runtime state and inference configuration
- [ ] Add module/doc updates for model-library and API consumers

**Verification:**
- Documentation examples show correct Pumas-versus-host responsibility split
- Tests cover confidence/source outputs for complete and partial estimate cases
- Plan review confirms no persistence changes are added unless justified by durable source-of-truth needs

**Status:** Not started

## Execution Notes

Update during implementation:
- 2026-03-21: Plan created for upstream discussion after Pantograph-side design review clarified that Pumas should provide feasible execution candidates from durable facts, while Pantograph should own live technical-fit selection for workflow runs.
- 2026-03-21: Scope refined to keep Pumas focused on feasible candidates plus basic fact-based technical information such as model size and simple RAM/VRAM estimates, while client applications own inference configuration and live resource interpretation.

## Commit Cadence Notes

- Commit after each logical slice is complete and verified.
- Keep contract, resolution logic, and documentation slices reviewable and separate where practical.
- Follow repo commit standards.

## Optional Subagent Assignment

| Owner/Agent | Scope | Output Contract | Handoff Checkpoint |
| ----------- | ----- | --------------- | ------------------ |
| None | None | None | Revisit only if contract and implementation streams can be split cleanly |

## Re-Plan Triggers

- The team decides Pumas should own live runtime telemetry or process state
- Candidate generation requires new persisted artifacts not justified by current source-of-truth systems
- Existing execution descriptor consumers require an API-breaking change
- The team wants Pumas to solve inference-configuration details or live memory admission instead of exposing basic facts only

## Recommendations (Only If Better Option Exists)

- Prefer “feasible execution candidates” and “technical fit” terminology over “best model”.
  Why: it keeps the contract grounded in verifiable facts and avoids false quality claims.
  Impact: no scope increase; improves long-term contract clarity.
- Keep candidate generation as a pure read-time capability in milestone one.
  Why: it avoids introducing lifecycle ownership, polling, or new persisted state before the contract is proven useful.
  Impact: lower implementation risk and easier compatibility preservation.
- Prefer basic fact/estimate terminology such as “model size” and “estimated RAM/VRAM” over inference-specific claims such as “context headroom”.
  Why: clients own runtime configuration and should interpret Pumas facts in the context of their own inference settings.
  Impact: keeps the contract narrow, defensible, and easier to adopt across hosts.

## Completion Summary

### Completed

- Proposal document created for upstream review.

### Deviations

- None at proposal time.

### Follow-Ups

- Decide whether execution descriptor should carry a summary candidate field in milestone one or whether the candidate API should stay separate initially.
- Decide which resource-estimate inputs, if any, deserve durable metadata/index persistence beyond already-projected model size facts.

### Verification Summary

- Proposal aligned with Pumas `docs/plans/` conventions and the shared plan template.
- Contract references cross-checked against current execution descriptor and dependency-resolution surfaces.

### Traceability Links

- Module README updated: N/A at proposal stage
- ADR added/updated: N/A
- PR notes completed per project standards: N/A at proposal stage

## Brevity Note

Keep the implementation concise and contract-focused. Expand only where execution decisions or compatibility risk require more detail.
