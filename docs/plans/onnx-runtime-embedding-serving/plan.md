# Plan: ONNX Runtime Embedding Serving

## Objective

Add first-class ONNX Runtime embedding serving to Pumas so local ONNX embedding
models such as `nomic-embed-text-v1.5` can be loaded, tracked, and exposed to
external applications through the existing OpenAI-compatible Pumas `/v1`
gateway.

The intended external contract is:

```text
POST http://127.0.0.1:<pumas-rpc-port>/v1/embeddings
```

Pumas remains the backend-owned source of truth for served-model state, model
aliases, runtime profile lifecycle, and gateway routing. ONNX Runtime execution
is hosted by a Rust provider adapter inside the Pumas runtime process, not by a
Python sidecar.

The intended GUI contract is:

- The Pumas Library sidebar includes an ONNX Runtime app icon.
- Selecting ONNX Runtime opens a first-class ONNX app panel.
- The ONNX panel includes a runtime profile manager like the Ollama and
  llama.cpp panels.
- Beneath profile settings, the ONNX panel lists local ONNX-compatible models.
- Each ONNX-compatible model row lets the user select and save the ONNX Runtime
  profile that should serve that model.
- Serving a model uses the saved ONNX route/profile unless the user explicitly
  chooses a different profile from the serving options flow.
- Once served, external applications call the Pumas `/v1` gateway and receive
  OpenAI-compatible embedding responses.

## Scope

### In Scope

- Add a Rust ONNX Runtime provider adapter for embedding models.
- Add an in-process ONNX session manager and embedding engine behind the
  provider serving/gateway contracts.
- Add Rust-owned load, unload, status, device/execution-provider reporting, and
  health/lifecycle checks for ONNX Runtime profiles.
- Add `onnx-runtime` plugin metadata and model compatibility for `.onnx`.
- Add `onnx_runtime` runtime provider and `onnx_serve` provider mode to typed
  backend/frontend contracts.
- Refactor shared provider systems into a cleaner provider model before ONNX is
  wired as the third runtime-profile provider.
- Replace provider-specific match/fallback paths with provider behavior,
  serving adapters, launch strategies, endpoint capability checks, and frontend
  provider descriptors.
- Replace model runtime routes with provider-scoped routes so ONNX profile
  assignment does not inherit the current one-route-per-model limitation.
- Extend serving validation so ONNX Runtime profiles accept ONNX embedding
  models and reject incompatible artifacts.
- Extend Pumas serving so `serve_model` loads ONNX models through the Rust ONNX
  provider adapter and records `ServedModelStatus`.
- Reuse the existing Pumas `/v1/embeddings` gateway by routing served ONNX
  models to an in-process ONNX embedding gateway adapter.
- Add the ONNX Runtime sidebar entry, app panel, runtime profile manager,
  compatible model list, per-model profile route selection, quick serve/options
  actions, and backend-confirmed served-state display.
- Document the external app contract and Emily configuration guidance.
- Add focused Rust, TypeScript, and gateway integration tests.

### Out of Scope

- Routing ONNX models through llama.cpp.
- Supporting ONNX text generation, reranking, vision, audio, or diffusion in
  the first slice.
- Replacing the existing Torch sidecar.
- Adding a Python ONNX sidecar.
- Rewriting the existing Pumas gateway.
- Maintaining legacy provider dispatch, legacy global model-route semantics, or
  dual old/new runtime-profile config readers after implementation lands.
- Automatically selecting an embedding model for a caller.
- Automatically adding task prefixes such as `search_query:` or
  `search_document:` to caller input. Callers own semantic input formatting.
- LAN exposure by default. External app access remains loopback by default.
- Changing Emily's existing memory schema or embedding dimension without a
  separate migration plan.

## Inputs

### Problem

Pumas can already expose served llama.cpp GGUF embedding models through the
OpenAI-compatible `/v1/embeddings` gateway, but ONNX embedding models are only
recognized as model-library artifacts. There is no first-class ONNX Runtime
provider, no Rust ONNX execution adapter, and no serving workflow that records
ONNX models as backend-owned served instances.

The specific user need is to serve an ONNX `nomic-embed-text-v1.5` model so
Emily and other external apps can use it through a stable local endpoint.

### Constraints

The detailed constraints, standards reviewed, standards compliance guardrails,
standards gates, assumptions, dependencies, affected contracts, persisted
artifacts, and lifecycle ownership notes are maintained in
[inputs-and-standards.md](inputs-and-standards.md).

Key constraints:

- Pumas `/v1` remains the external facade. External apps should not depend on
  raw provider endpoints for normal usage.
- Backend-owned runtime profile and served-model state remain authoritative.
- ONNX must be added through provider behavior, provider-scoped routes, serving
  adapters, and gateway endpoint capabilities instead of new provider-specific
  branches.
- Boundary validation must happen before file, network, process, or model-load
  operations.
- ONNX Runtime dependencies stay in the Rust crate that owns ONNX execution;
  root/workspace manifests may change only when that crate is the owner.
- Dirty implementation files must be resolved, committed, stashed, or
  explicitly allowed before implementation begins.

### Assumptions

- Pumas can host ONNX Runtime in Rust through Rust bindings such as the `ort`
  crate after a dependency review confirms packaging and native-library
  strategy.
- Existing Torch sidecar patterns are not the implementation target for ONNX.
- A provider registry can be introduced before ONNX serving without changing
  user-visible Ollama and llama.cpp outcomes.
- Existing ONNX model-library awareness can be reused after compatibility logic
  is separated from custom ONNX app metadata.

Detailed assumptions are in [inputs-and-standards.md](inputs-and-standards.md).

### Dependencies

Internal dependencies include `pumas-core` runtime profiles, serving state,
model-library metadata, ONNX provider/session modules, `pumas-rpc`
serving/gateway handlers, frontend app shell/runtime profile/model route UI,
plugin metadata, and Electron bridge types.

External dependencies include the selected Rust ONNX Runtime binding, Rust
tokenization/numeric support, OpenAI-compatible embedding payloads, native ONNX
Runtime library packaging, and platform filesystem behavior.

The full dependency inventory is in
[inputs-and-standards.md](inputs-and-standards.md).

### Affected Structured Contracts

The affected contracts include runtime provider ids/modes/capabilities,
provider-scoped runtime routes, served-model status/identity, gateway payloads,
Rust ONNX provider/session contracts, ONNX embedding payloads, plugin/app
metadata, frontend bridge types, and model-library executable format
projections.

The detailed ownership matrix is in
[provider-model-and-contracts.md](provider-model-and-contracts.md).

### Affected Persisted Artifacts

The runtime profile config moves to a provider-scoped route schema. ONNX
runtime profile state remains in backend-owned runtime profile and serving
state; no Python sidecar runtime directory is introduced. Model-library records
must classify `.onnx` as a first-class executable format.

Detailed persisted-artifact notes are in
[inputs-and-standards.md](inputs-and-standards.md).

## Impact Review

The codebase review and blast-radius analysis are maintained in
[impact-review.md](impact-review.md).

Implementation must treat these findings as blockers:

- Route identity, served identity, gateway lookup, and route mutations are
  currently too model-id-centric for ONNX.
- Runtime profile, serving, gateway, launcher, and frontend app-shell code
  contain hard-coded Ollama/llama.cpp assumptions.
- App identity is split across Rust, plugin metadata, RPC composition, and
  frontend registry code.
- `model_library/library.rs`, runtime profile service, serving handler,
  gateway handler, launcher code, and llama.cpp model-library UI are already
  large enough that ONNX branches should be delegated to smaller modules.
- Rust ONNX execution still needs explicit concurrency, cancellation, shutdown,
  and runtime-profile provider ownership.

## Provider Model

The cleaner provider model is documented in
[provider-model-and-contracts.md](provider-model-and-contracts.md).

The implementation must separate:

- App/plugin identity
- Runtime provider behavior
- Runtime profile persistence
- Launch strategy
- Model route
- Served instance identity
- Serving adapter
- Gateway endpoint capability
- Model compatibility
- Frontend provider descriptor

ONNX must land as the first new provider on this cleaned-up model, not as a
third pile of special cases next to Ollama and llama.cpp.

## Risks

The full risk table is maintained in [risks.md](risks.md).

Highest risks:

| Risk | Impact | Mitigation |
| ---- | ------ | ---------- |
| ONNX model output/pooling semantics vary across exports. | High | Add Rust session introspection, explicit postprocess configuration, and shape/numerical tests. |
| Existing gateway and serving paths assume only Ollama and llama.cpp. | High | Introduce provider behavior, provider-scoped route identity, provider-scoped served identity, and gateway endpoint capability checks before ONNX wiring. |
| ONNX model path or request validation is incomplete. | High | Validate paths, aliases, batch sizes, token counts, dimensions, and model ids at Rust boundaries before side effects. |
| ONNX work expands already-large files. | High | Extract provider behavior, launch strategy, gateway helper, route migration, model compatibility, and frontend view-model components before or during ONNX wiring. |
| Legacy compatibility code remains after the feature lands. | High | Treat legacy dispatch, global route semantics, and dual old/new readers as implementation blockers. |

## Definition of Done

- ONNX Runtime appears as a distinct runtime provider in backend and frontend
  contracts.
- The Pumas Library sidebar shows an ONNX Runtime app icon with status derived
  from ONNX runtime profile and backend-owned ONNX session state.
- The ONNX Runtime app panel contains a runtime profile manager and an
  ONNX-compatible local model list.
- Users can create, edit, save, launch, and stop ONNX Runtime profiles from the
  ONNX panel according to the selected lifecycle slice.
- Users can assign an ONNX Runtime profile to each ONNX-compatible model from
  the ONNX model list.
- Runtime profile routes are provider-scoped. Existing one-route-per-model
  semantics are removed from code and persisted config after migration/cleanup.
- ONNX models can be loaded through a managed ONNX Runtime profile created and
  controlled from the ONNX panel.
- Serving from the ONNX model list uses the saved ONNX route/profile by default.
- Pumas records successfully loaded ONNX embedding models as `ServedModelStatus`.
- `GET /v1/models` on the Pumas gateway lists served ONNX embedding models by
  gateway alias.
- `POST /v1/embeddings` on the Pumas gateway proxies to ONNX Runtime and
  returns OpenAI-compatible embeddings.
- `POST /v1/chat/completions` and `/v1/completions` against ONNX-served models
  return Pumas-shaped unsupported-endpoint errors.
- ONNX serve and unload dispatch is selected by provider, not by an
  Ollama-vs-llama.cpp fallback.
- No legacy two-provider dispatch, global route fallback, or old
  runtime-profile config shape remains in active code paths.
- The Rust ONNX provider validates model paths, model names, request sizes, and
  execution configuration at boundaries.
- Frontend profile and serving flows support ONNX Runtime without duplicating
  backend-owned served state.
- Docs explain how external apps and Emily should call the Pumas gateway.
- Focused Rust, TypeScript, and release/build checks pass.
- The implementation satisfies the standards compliance guardrails, with any
  deviations recorded in execution notes and converted into re-plan triggers
  before merging.

## Milestones

Detailed milestone tasks and verification commands are maintained in
[milestones.md](milestones.md).

| Milestone | Name | Purpose |
| --------- | ---- | ------- |
| 0 | Provider Model Refactor | Establish provider behavior, provider registry, provider-scoped route/served identity, gateway capabilities, launch strategies, model compatibility, frontend descriptors, and app/runtime identity strategy before ONNX wiring. |
| 1 | Rust ONNX Runtime Skeleton | Create the Rust ONNX provider/session boundary, fake embedding backend, validation types, and in-process gateway scaffolding. |
| 2 | ONNX Embedding Execution | Add Rust ONNX Runtime/tokenizer/numeric execution with bounded inference, explicit postprocess semantics, request limits, and shape/numerical tests. |
| 3 | Plugin And Runtime Profile Contracts | Add ONNX plugin/app identity, provider/mode contracts, runtime profile validation, managed launch specs, and frontend runtime profile typing. |
| 4 | Serving Validation And Load/Unload | Serve ONNX artifacts through Rust provider adapters, provider-scoped route resolution, and backend-owned served status. |
| 5 | Pumas Gateway Routing | Route `/v1/models` and `/v1/embeddings` through the Pumas facade with endpoint-specific capability, body, timeout, and error behavior. |
| 6 | Frontend Integration | Add the ONNX app panel, profile manager, ONNX-compatible model list, provider-scoped route controls, serve actions, and backend-confirmed state display. |
| 7 | Documentation And External App Contract | Document Rust ONNX provider architecture, runtime profile contracts, gateway examples, and Emily usage. |
| 8 | Release Validation | Run focused release/build/package validation and update user-visible release notes. |

## Execution Notes

Implementation notes, prior plan iteration history, commit cadence, optional
parallel worker plan, and completion-summary template are maintained in
[execution-and-coordination.md](execution-and-coordination.md).

## Commit Cadence Notes

- Commit the Rust ONNX provider skeleton and tests as the first verified slice.
- Commit Rust provider/profile contracts separately from frontend UI when
  feasible.
- Commit gateway routing with Rust tests before release validation.
- Keep code, tests, and documentation together when they describe one completed
  behavior.
- Follow `COMMIT-STANDARDS.md`.

## Optional Parallel Worker Plan

The optional parallel worker plan is maintained in
[execution-and-coordination.md](execution-and-coordination.md). Use it only
after Milestone 0 freezes the shared contracts and worker write sets can be
kept non-overlapping.

## Re-Plan Triggers

Re-plan when:

- `nomic-embed-text-v1.5` ONNX artifacts require custom tokenizer or
  post-processing not represented by the planned Rust embedding configuration.
- ONNX Runtime dependency packaging conflicts with the existing release process.
- Pumas gateway behavior for embeddings differs materially from the expected
  OpenAI-compatible shape.
- Emily requires dimension or schema changes rather than only endpoint
  configuration.
- A shared provider model proves larger than this feature's acceptance path and
  needs a separate migration plan.
- Dirty implementation files cannot be isolated from the ONNX work.
- Provider-scoped route migration would lose user runtime-profile assignments
  without a safe rewrite or explicit cleanup policy.
- Gateway endpoint capability checks require a broader gateway contract change.
- Frontend hard-coded app registry paths cannot be kept in sync with app/plugin
  metadata through focused tests.
- ONNX Runtime Rust dependency, native-library, or packaging behavior cannot be
  validated in the owning Rust crate/release path.
- Standards gates reveal file-size, ownership, dependency, concurrency,
  accessibility, or documentation violations that cannot be resolved inside the
  current milestone.

The full trigger list is in
[execution-and-coordination.md](execution-and-coordination.md).

## Recommendations

- Keep Pumas as the only external endpoint that Emily and other apps configure.
  This preserves backend-owned served state and keeps raw provider internals as
  implementation details.
- Introduce provider-scoped routes before ONNX UI work so the frontend does not
  build against a route shape that will immediately be replaced.
- Treat ONNX embedding output semantics as configuration-backed behavior, not
  hard-coded assumptions for one model export.
- Keep ONNX dependencies owned by the Rust ONNX provider crate/module and make
  CPU/GPU package strategy an explicit release decision.
- Split the large runtime-profile, serving, gateway, model-library, and
  frontend route UI surfaces as part of the provider-model work instead of
  expanding them with ONNX branches.

## Completion Summary

### Completed

- Milestone 0 provider-model refactor is in progress with multiple verified
  slices committed.
- 2026-05-11 re-plan accepted: ONNX Runtime execution targets Rust bindings and
  an in-process Rust provider/session manager, not a Python sidecar.

### Deviations

- Original sidecar-oriented milestones were superseded by the Rust ONNX Runtime
  plan before sidecar code was committed.

### Follow-Ups

- None recorded.

### Verification Summary

- Plan split completed; no implementation verification has run.

### Traceability Links

- Directory overview: [README.md](README.md)
- Inputs and standards: [inputs-and-standards.md](inputs-and-standards.md)
- Impact review: [impact-review.md](impact-review.md)
- Provider model and contracts:
  [provider-model-and-contracts.md](provider-model-and-contracts.md)
- Risks: [risks.md](risks.md)
- Detailed milestones: [milestones.md](milestones.md)
- Execution and coordination:
  [execution-and-coordination.md](execution-and-coordination.md)
