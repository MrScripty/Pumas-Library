# Proposal: Pumas Artifact Load Target Resolution For Pantograph

## Context

Pantograph uses Pumas Library as the canonical owner of model identity, model
storage, selected artifact metadata, package facts, and library availability.
Pantograph does not own model files, model-library roots, or external-reference
asset resolution. Pantograph should trust Pumas-provided artifact references
instead of rediscovering, inferring, or validating Pumas storage layout from
local paths.

Pantograph's image-generation execution path now requires a clean handoff from
Pumas package facts to a PyTorch/Diffusers worker load target:

- Scheduler/admission selects backend, runtime variant, device, and model.
- Inference planning consumes scheduler-selected decisions and Pumas package
  facts.
- The PyTorch worker can load a Diffusers pipeline only from a concrete local
  directory.

The missing contract is a Pumas-owned API that turns a selected model artifact
reference into an execution-ready local load target, or returns typed
unavailability diagnostics.

## Problem

Pantograph currently has enough information to know which Pumas artifact was
selected, but not enough to safely load it without taking ownership of Pumas
storage semantics.

A root-relative artifact entry such as:

```text
image/stable-diffusion/tiny-sd
```

is useful for stable identity and diagnostics, but it does not by itself answer:

- whether the artifact is currently materialized locally;
- which Pumas library root or external-reference record owns it;
- whether the artifact is a loadable directory or a pending/missing artifact;
- whether the selected artifact id/path still matches the current model record;
- what exact local path should be passed to a runtime worker;
- whether the artifact is valid for the requested runtime family, such as a
  Diffusers bundle.

Pantograph should not solve this by configuring Pumas roots and joining paths.
That would duplicate Pumas ownership and would not handle Pumas-supported
external-reference assets correctly. Pantograph should also not pass a Pumas
handle to the Python worker and let the worker resolve it. That would move
library resolution and diagnostics into the wrong layer.

## Goal

Add a Pumas-owned artifact load-target resolver API.

The API should let Pantograph give Pumas a selected artifact reference and
receive one of:

- an execution-ready local load target for the exact selected artifact; or
- typed diagnostics explaining why the artifact cannot currently be loaded.

Pantograph will treat this Pumas response as authoritative.

## Non-Goals

Pumas should not own Pantograph scheduler policy, runtime ranking, worker
execution, lifecycle events, retry policy, or workflow diagnostics formatting.

Pantograph should not infer executable backend selection from Pumas package
hints. Pumas facts remain factual model/package evidence. Pantograph's scheduler
still decides runtime selection.

The Python worker should not call Pumas, inspect model library roots, or repair
missing paths. It should receive an already-approved local load target from Rust.

## Standards Review

This plan was checked against the standards under
`/media/jeremy/OrangeCream/Linux Software/repos/owned/developer-tooling/Coding-Standards/`,
with special attention to:

- `PLAN-STANDARDS.md`
- `CODING-STANDARDS.md`
- `ARCHITECTURE-PATTERNS.md`
- `DOCUMENTATION-STANDARDS.md`
- `TESTING-STANDARDS.md`
- `CONCURRENCY-STANDARDS.md`
- `SECURITY-STANDARDS.md`
- `INTEROP-STANDARDS.md`
- `LANGUAGE-BINDINGS-STANDARDS.md`
- `CROSS-PLATFORM-STANDARDS.md`
- `TOOLING-STANDARDS.md`
- `languages/rust/RUST-API-STANDARDS.md`
- `languages/rust/RUST-ASYNC-STANDARDS.md`
- `languages/rust/RUST-SECURITY-STANDARDS.md`
- `languages/rust/RUST-INTEROP-STANDARDS.md`
- `languages/rust/RUST-LANGUAGE-BINDINGS-STANDARDS.md`
- `languages/rust/RUST-CROSS-PLATFORM-STANDARDS.md`
- `languages/rust/RUST-TOOLING-STANDARDS.md`

The implementation must preserve Pumas' existing layer ownership: domain DTOs
and serde contracts live in model-contract code, resolution policy lives in
model-library code, and API/RPC/local-client/language-binding layers remain
thin transport or host adapters.

This file is a cross-repository proposal owned by Pantograph. Before Pumas code
implementation begins, the accepted plan should be copied or promoted into the
Pumas repository's standard documentation layout, for example
`docs/plans/pumas-artifact-load-target-resolution/`, so implementation status,
worker coordination, verification evidence, and later re-plans live with the
code being changed.

## Scope And Blast Radius

In scope:

- new request/response DTOs and diagnostics in the Pumas model-contract layer;
- one shared artifact load-target resolver core in the model-library layer;
- thin `ModelLibrary`, `PumasApi`, RPC state, `PumasLocalClient`, and optional
  `PumasReadOnlyLibrary` entry points that delegate to the same resolver core;
- lower-level helper extraction where existing helpers are too broad because
  they trigger package-facts hydration, primary-file selection, cache writes, or
  owner-only freshness work;
- README/API contract updates for any touched source directories whose
  responsibilities, public API, or structured producer contracts change;
- focused Pumas tests for DTO serialization, resolver behavior, read-only
  non-mutation, owner freshness, and transport/binding surfaces that are
  exposed;
- Pantograph integration at the Rust composition boundary before worker
  dispatch.

Out of scope for the first integration:

- managed download/materialization orchestration;
- Pantograph scheduler policy, runtime ranking, retry policy, or worker
  lifecycle design;
- Python worker access to Pumas handles, roots, or storage layout;
- migration of existing Ollama, ONNX, llama.cpp, or serving validation
  `get_primary_model_file` call sites, except to avoid creating new duplicated
  primary-file logic that would make their future migration harder;
- database schema migrations unless implementation discovery proves the current
  index/cache cannot represent the required states;
- public language-binding exposure unless a host-language consumer needs this
  surface in the same delivery slice.

Affected code areas and expected ownership:

| Area | Expected Change | Standards Constraint |
| --- | --- | --- |
| `pumas-core/src/models/` | Add serde-stable DTOs and diagnostics; reuse `PumasModelRef`, `PackageArtifactKind`, `ModelArtifactState`, `ModelEntryPathState`, `StorageKind`, and `AssetValidationState`. | Public wire contracts must be append-only, explicitly named, and round-trip tested. |
| `pumas-core/src/model_library/` | Add focused resolver module, update README, keep `library.rs` as a thin facade. | Avoid further `library.rs` bloat; one resolver core owns policy. |
| `pumas-core/src/model_library/external_assets.rs` | Reuse or extract validation helpers without forcing read-only callers through mutating refresh paths. | Owner-fresh and read-only modes must be behaviorally distinct. |
| `pumas-core/src/package_facts/` and selector/index helpers | Read selected artifact and cached state only; extract small helpers if needed. | Do not wrap broad APIs that hydrate full facts or compute fingerprints on the hot path. |
| `pumas-core/src/api/`, `pumas-rpc/`, and IPC/local client code | Add thin request forwarding and typed response handling. | Validate boundary payloads, keep transport logic thin, and test executable boundary contracts. |
| `pumas-uniffi/` or generated bindings | Change only if the capability is intentionally exposed to host languages. | Binding surfaces must be curated, documented, regenerated, and host-tested. |
| Pantograph runtime integration | Call the Pumas API before worker dispatch and pass only the approved target into the worker envelope. | Pantograph must not duplicate Pumas path resolution. |

## Structured Contracts And Persisted Artifacts

The new request/response types are structured producer and API consumer
contracts. They must use explicit serde names for public wire fields and enum
variants, remain append-only after first release, and have fixture-based
round-trip tests.

Stage 1 should not require new persisted artifacts. It should read existing
model records, selected artifact metadata, selector/index state, package-facts
cache entries, and external-reference asset records. `ReadOnlyIndexed` mode must
not update SQLite, metadata files, package-facts caches, external-reference
validation state, or any other persisted artifact.

If implementation discovery shows that existing persisted state cannot
distinguish required resolver states, that is a re-plan trigger. Any schema or
persisted artifact change must define compatibility, migration/regeneration
rules, staged validation, and fixture updates before code changes continue.

If RPC, IPC, local-client, or language-binding surfaces are exposed, those
method names, payload shapes, error categories, lifecycle expectations, and
serialization formats are executable boundary contracts. They must be updated in
the same implementation slice as their tests and documentation.

## Proposed API

Add an artifact materialization/load-target API to Pumas Library:

```rust
pub async fn resolve_model_artifact_load_target(
    &self,
    request: ResolveModelArtifactLoadTargetRequest,
) -> Result<ResolveModelArtifactLoadTargetResponse>;
```

Proposed request shape, using existing Pumas contracts:

```rust
pub struct ResolveModelArtifactLoadTargetRequest {
    pub model_ref: PumasModelRef,
    pub expected_artifact_kind: Option<PackageArtifactKind>,
    pub caller_observed_entry_path: Option<String>,
    pub caller_observed_package_facts_contract_version: Option<u32>,
    pub resolution_mode: PumasArtifactLoadTargetResolutionMode,
    pub consumer: PumasArtifactConsumer,
}

pub enum PumasArtifactLoadTargetResolutionMode {
    OwnerFresh,
    ReadOnlyIndexed,
}

pub struct PumasArtifactConsumer {
    pub consumer_name: String,
    pub task_kind: Option<String>,
    pub runtime_family: Option<String>,
}
```

Exact names can change. The important part is that Pantograph sends exactly one
authoritative selected-artifact reference: `PumasModelRef`, including its
existing `selected_artifact_id` and `selected_artifact_path` fields when a
specific artifact was selected. `caller_observed_entry_path` and
`caller_observed_package_facts_contract_version` are stale-check inputs only;
they must not override the selected artifact encoded in `PumasModelRef`.

This proposal intentionally does not introduce parallel artifact DTOs such as a
new `PumasArtifactKind` or `PumasArtifactRef`. Pumas should reuse
`PackageArtifactKind`, `PumasModelRef`, and existing model-library selector
state contracts unless a new type has deliberately different semantics.

## Proposed Response Shape

```rust
pub struct ResolveModelArtifactLoadTargetResponse {
    pub artifact_state: ModelArtifactState,
    pub entry_path_state: ModelEntryPathState,
    pub target: Option<PumasArtifactLoadTarget>,
    pub diagnostics: Vec<PumasArtifactLoadTargetDiagnostic>,
}

pub struct PumasArtifactLoadTarget {
    pub model_ref: PumasModelRef,
    pub artifact_kind: PackageArtifactKind,
    pub local_load_path: String,
    pub load_path_kind: PumasArtifactLoadPathKind,
    pub library_root_id: Option<String>,
    pub storage_kind: StorageKind,
    pub validation_state: AssetValidationState,
    pub content_fingerprint: Option<String>,
    pub package_facts_contract_version: Option<u32>,
}

pub enum PumasArtifactLoadPathKind {
    Directory,
    File,
}

pub struct PumasArtifactLoadTargetDiagnostic {
    pub code: PumasArtifactLoadTargetDiagnosticCode,
    pub field_path: Option<String>,
    pub message: String,
}

pub enum PumasArtifactLoadTargetDiagnosticCode {
    MissingModel,
    MissingSelectedArtifact,
    SelectedArtifactMismatch,
    ArtifactMissing,
    ArtifactPartial,
    ArtifactNeedsDetail,
    ArtifactPathMissing,
    ArtifactPathNotLoadable,
    ArtifactKindMismatch,
    InvalidArtifact,
    InvalidPackageFacts,
    StalePackageFacts,
    LibraryUnavailable,
}
```

The response should be serde-stable and append-only. Pantograph can map
`ModelArtifactState`, `ModelEntryPathState`, and these diagnostics into its
scheduler/readiness/planner diagnostics without parsing message text.

If Pumas wants a convenience status field in addition to the existing state
fields, it should be derived from `ModelArtifactState` and
`ModelEntryPathState`, not replace them. Pantograph needs the original state
fidelity for partial downloads, stale facts, ambiguous artifacts, and
needs-detail cases.

`content_fingerprint` is optional and should be populated only from already
available indexed/cache state. The resolver must not compute full package-facts
source fingerprints on the hot execution path just to fill this field.

## Contract Rules

- Pumas owns resolving model refs, selected artifact refs, library roots,
  external-reference asset records, and local filesystem load paths.
- Pumas owns whether the selected artifact is currently loadable.
- Pumas owns validating that `local_load_path` is a Pumas-approved local path
  for the selected artifact. That path may be inside Pumas-managed storage or
  may be an approved external-reference asset. The ready target should expose
  typed `StorageKind` and `AssetValidationState` values so consumers do not
  assume all load targets live under one library root.
- Pumas owns checking whether the artifact kind matches the caller's expected
  artifact kind.
- Pumas should return typed unavailable states instead of throwing opaque
  string errors for normal missing/not-downloaded/stale cases.
- Pumas should implement resolver logic once and expose it through multiple
  surfaces, rather than adding parallel owner, read-only, RPC, and local-client
  implementations that can drift.
- Pantograph should not join root paths, scan directories, infer from file
  names, or repair selected artifact refs.
- Pantograph should pass the resolved `local_load_path` to runtime workers only
  after Pumas returns ready artifact and entry-path states with a target.
- Runtime workers should not receive Pumas roots or call Pumas directly.

## Diffusers Image Generation Use Case

For Pantograph image generation, the request would look conceptually like:

```rust
ResolveModelArtifactLoadTargetRequest {
    model_ref: scheduler_selected_model_ref_with_selected_artifact,
    expected_artifact_kind: Some(PackageArtifactKind::DiffusersBundle),
    caller_observed_entry_path: Some(package_facts.artifact.entry_path),
    caller_observed_package_facts_contract_version: Some(
        package_facts.package_facts_contract_version,
    ),
    resolution_mode: PumasArtifactLoadTargetResolutionMode::OwnerFresh,
    consumer: PumasArtifactConsumer {
        consumer_name: "pantograph".to_string(),
        task_kind: Some("image_generation".to_string()),
        runtime_family: Some("pytorch.diffusers".to_string()),
    },
}
```

If ready, Pumas returns:

```rust
PumasArtifactLoadTarget {
    artifact_kind: PackageArtifactKind::DiffusersBundle,
    local_load_path: "/.../Pumas-Library/shared-resources/models/image/...".to_string(),
    load_path_kind: PumasArtifactLoadPathKind::Directory,
    storage_kind: StorageKind::LibraryOwned,
    validation_state: AssetValidationState::Valid,
    ...
}
```

Pantograph then includes this resolved directory in the Rust-owned worker
envelope. The Python worker loads that directory and does not resolve Pumas
state.

## Availability Semantics

The resolver should map directly to existing Pumas model-library states:

- `ModelArtifactState::Ready` and a ready/loadable entry path means the target
  is currently loadable as requested. For `PackageArtifactKind::DiffusersBundle`,
  that means the target path is a Pumas-approved local directory and Pumas
  recognizes it as the selected Diffusers artifact.
- `ModelArtifactState::Missing` means the model or selected artifact is known
  but not locally materialized, or the selected artifact cannot be found.
- `ModelArtifactState::Partial` means some required artifact content is present
  but incomplete. Pantograph should treat this as not ready and surface the
  diagnostic without attempting a partial load.
- `ModelArtifactState::Invalid` means Pumas can locate the artifact but package
  validation does not consider it loadable.
- `ModelArtifactState::Ambiguous` means Pumas cannot identify one exact selected
  artifact. Pantograph should fail the workflow and refresh or ask the user to
  select an artifact explicitly.
- `ModelArtifactState::NeedsDetail` means Pumas has only summary/index state and
  must perform or schedule detail resolution before it can return a load target.
- `ModelArtifactState::Stale` means the caller's observed facts or selected
  artifact no longer match Pumas' current model record.

`ModelEntryPathState` should be returned alongside `ModelArtifactState` so
Pantograph can distinguish a missing artifact from a missing, stale, ambiguous,
or invalid load path.

The resolver may need resolver-specific derivation for
`ModelArtifactState::Missing` and related states. The existing selector
projection may derive ready, partial, invalid, stale, ambiguous, and
needs-detail states from metadata and download fields without covering every
exact selected-artifact load-target case. The resolver implementation should
not assume the selector projection already covers every state needed at this
execution boundary.

An expected-kind mismatch is an additional diagnostic over the state fields:
the artifact may exist, but if Pantograph requested a Diffusers directory and
the selected artifact is GGUF, Pumas should return an
`ArtifactKindMismatch` diagnostic and no load target.

## Authority And Stale Checks

`PumasModelRef` is the authoritative selected-artifact reference in the request.
If `model_ref.selected_artifact_id` or `model_ref.selected_artifact_path` is
present, Pumas should resolve exactly that artifact or return typed diagnostics.
If both are absent, Pumas may resolve the model only when the model has exactly
one unambiguous loadable artifact for the requested kind. Missing selected
artifact fields should become `MissingSelectedArtifact` only when exact artifact
identity is required, when multiple artifacts could match, or when the
artifact-kind request cannot be resolved without ambiguity.

`caller_observed_entry_path` and
`caller_observed_package_facts_contract_version` are optional observations from
Pantograph's cached package facts. They exist only to help Pumas return precise
stale-facts diagnostics. They must not select a different artifact, repair the
request, or override `PumasModelRef`.

If the caller-observed entry path disagrees with the selected artifact resolved
from `PumasModelRef`, Pumas should return `ModelArtifactState::Stale` or a
specific `SelectedArtifactMismatch` diagnostic rather than silently switching to
either side.

## Resolver Ownership And Modes

Pumas should add one internal resolver core, for example
`resolve_artifact_load_target_core`, and route every public surface through it.
The core should take explicit mode/configuration inputs rather than relying on
call-site-specific behavior:

- `OwnerFresh` mode may perform owner-only freshness work such as external asset
  revalidation and metadata/index refresh before resolving a target.
- `ReadOnlyIndexed` mode must not write metadata, update SQLite rows, start
  watchers, run reconciliation, or repair stale projections. It should resolve
  only from indexed/cache state and return typed states such as `NeedsDetail`,
  `Stale`, `Partial`, or `Invalid` when owner-side freshness work is required.

This distinction matters because `PumasReadOnlyLibrary` intentionally opens the
existing SQLite index without claiming lifecycle ownership. Read-only load-target
checks are useful for consumers, but they cannot have the same freshness
guarantees as owner-instance resolution.

Surfaces must enforce their allowed modes at the boundary.
`PumasReadOnlyLibrary` must reject `OwnerFresh` with a typed boundary error or
diagnostic. It must not silently downgrade the request, mutate state, or attempt
owner-only freshness work.

## Implementation Boundaries

Add DTOs in the public model-contract layer and keep the resolver implementation
in a focused model-library module such as
`model_library/artifact_load_target.rs`. `ModelLibrary`, `PumasApi`, RPC state,
`PumasLocalClient`, and `PumasReadOnlyLibrary` should be thin adapters over the
same resolver core.

The resolver should not call or wrap `resolve_model_package_facts`,
`resolve_model_package_facts_summary`, or `resolve_model_execution_descriptor`.
Those APIs are model-level or package-facts surfaces and may perform package
inspection, dependency resolution, cache writes, primary-file selection, or
external-asset refresh behavior that does not match this exact selected-artifact
boundary.

The resolver may reuse small lower-level helpers when they already encode Pumas
ownership rules, such as artifact-kind classification, selected-file projection,
external-reference validation, and selector/package-facts cache parsing. If a
helper currently implies model-level primary-file selection or full
package-facts hydration, split out a smaller helper instead of depending on the
broader API.

Existing `get_primary_model_file` consumers in Ollama, ONNX, llama.cpp launch,
and serving validation are outside the first Pantograph integration, but they
represent future convergence targets. The new resolver should not add another
copy of primary-file logic; it should establish the shared path these call sites
can migrate to later.

## Standards Compliance Requirements

Architecture and module shape:

- Keep the resolver's domain logic synchronous where practical. Async should
  remain at API, RPC, IPC, database, and filesystem boundaries.
- Do not add resolver policy to transport handlers, Pantograph adapters, Python
  workers, generated bindings, or UI code.
- Add a focused module such as `model_library/artifact_load_target.rs` instead
  of expanding `library.rs` with another large implementation block.
- Keep implementation modules `pub(crate)` unless a type or function is part of
  the intended public Pumas contract.
- If a touched source directory gains new responsibilities or public contract
  semantics, update its `README.md` in the same slice.

Input validation and path safety:

- Treat API/RPC/IPC/local-client payloads as untrusted at the boundary and parse
  them into typed DTOs before resolver use.
- Accept and compare paths using `Path`/`PathBuf` internally. Do not construct
  load targets through string joins or hardcoded separators.
- Return only Pumas-approved local paths that come from validated
  library-owned storage or validated external-reference assets.
- Preserve `StorageKind` and `AssetValidationState` in the response so callers
  do not infer path authority from string prefixes.
- Do not use `unwrap()` or `expect()` in production request paths for missing
  artifacts, malformed refs, filesystem failures, or stale persisted state.

Concurrency and lifecycle:

- `ReadOnlyIndexed` mode must not start watchers, background refresh tasks,
  reconciliation jobs, or mutation-capable owner workflows.
- `OwnerFresh` mode may perform owner-only freshness work, but any background
  task must have a lifecycle owner, cancellation path, and observed error
  handling.
- Async request paths must not run blocking filesystem, SQLite, or metadata
  work directly on runtime worker threads; isolate unavoidable blocking work
  using the existing Pumas patterns for database/filesystem access.
- Do not hold async locks across blocking work or across unrelated awaits.

Performance:

- Resolving a load target must be bounded by indexed/cache lookups and targeted
  selected-artifact checks. It must not deep-scan model roots or regenerate full
  package facts on the hot path.
- `content_fingerprint` must remain optional unless it is already available
  from indexed/cache state.
- Any claim that the new path improves latency or throughput must be backed by
  Criterion benchmarks or existing project benchmark tooling.

Interop and language bindings:

- Do not expose this through UniFFI or generated host bindings by default.
- If a binding is added, keep core logic binding-neutral, add FFI-safe wrapper
  DTOs only where required, regenerate generated bindings, document the support
  tier, and add both native Rust and host-language smoke tests.

Cross-platform behavior:

- Use platform-neutral filesystem APIs and test with paths that contain spaces.
- If canonicalization or filesystem identity checks are part of validation,
  tests must compare canonical paths rather than display strings.
- Do not add inline platform-specific business logic for this resolver.

## Staged Implementation

### Stage 1: Read-Only Resolver

Add the request/response DTOs and implement lookup from existing Pumas indexed
model records, selected artifact metadata, package facts, and external-reference
asset records. Do not trigger downloads or repairs from this API.

Implement the shared resolver core and the read-only/indexed path first. Current
executable APIs are model-level and may choose a primary file or directory; this
resolver needs an exact selected-artifact contract. It also needs lower-level
lookup or explicit error translation so normal unavailable states such as
missing, partial, invalid, needs-detail, and stale return typed responses
instead of opaque errors.

Expose the resolver through the surfaces Pantograph actually consumes:
`ModelLibrary`, the Pumas API/RPC state, `PumasLocalClient`, and
`PumasReadOnlyLibrary` if read-only consumers need load-target checks without
owning lifecycle.

Stage 1 verification:

- DTO serde fixture round trips for request and response shapes.
- Resolver unit tests for ready, missing, partial, invalid, stale,
  needs-detail, kind mismatch, ambiguous, and external-reference states.
- A read-only non-mutation test that snapshots relevant metadata/index/cache
  state before and after resolution.
- API/RPC/local-client tests for any public surface introduced in this stage.
- README updates for changed source directories and any host-facing contract
  sections required by documentation standards.
- Repository Rust gates appropriate to the changed crates, at minimum
  formatting, tests for touched crates, and `cargo check`/Clippy where already
  used by the Pumas workspace.

### Stage 1.5: Owner Freshness Path

Wire the same resolver core into owner-mode resolution. Owner-mode resolution
may refresh external-reference asset state and use freshly loaded metadata/index
state before returning a ready target. It should still avoid full package-facts
regeneration and should return typed non-ready diagnostics for normal
unavailability.

Stage 1.5 verification:

- Tests showing owner-fresh resolution can return a ready target after allowed
  external-reference refresh while `ReadOnlyIndexed` returns a typed non-ready
  response for the same stale indexed input.
- Tests showing owner-fresh resolution still does not force full package-facts
  regeneration or fingerprint computation.
- Concurrency/lifecycle review for any refresh task, watcher interaction, or
  blocking filesystem/database work touched by this stage.

### Stage 2: Pantograph Integration

Pantograph calls the resolver at the composition boundary before worker
dispatch. Inference planning remains side-effect free; the resolver call belongs
in the host/runtime integration layer that already has Pumas access.

Stage 2 verification:

- Pantograph tests proving a ready Diffusers target reaches the Rust-owned
  worker envelope.
- Pantograph tests proving non-ready responses become terminal
  readiness/planning diagnostics before worker dispatch.
- Worker-envelope tests proving the Python worker receives only the approved
  local load target and no Pumas root, handle, or selected-artifact repair data.

### Stage 3: Materialization Hooks

If Pumas later supports managed download/materialization, the same API can
return the existing not-materialized state plus actionable metadata or a
separate materialization handle. Pantograph still should not perform path
repair.

Stage 3 verification:

- A separate materialization plan before adding handles, queues, polling,
  downloads, retries, or background tasks.
- Persistence compatibility and migration/regeneration rules for any new
  materialization artifacts.
- Lifecycle tests for any background work, cancellation, retries, and overlap
  prevention.

## Test Expectations

Pumas should add tests for:

- valid Diffusers bundle returns `ModelArtifactState::Ready`, a ready entry
  path state, and a directory load target;
- valid GGUF artifact returns `ModelArtifactState::Ready`, a ready entry path
  state, and a file load target when requested as GGUF;
- requesting Diffusers for a GGUF artifact returns `ArtifactKindMismatch`;
- missing selected artifact id/path returns `MissingSelectedArtifact` when
  exact artifact identity is required or the model has ambiguous artifacts;
- stale selected artifact returns `ModelArtifactState::Stale` plus a precise
  stale/mismatch diagnostic;
- known but not downloaded model returns `ModelArtifactState::Missing` or the
  existing Pumas state that represents not materialized;
- partially downloaded artifacts return `ModelArtifactState::Partial`;
- summary-only artifacts return `ModelArtifactState::NeedsDetail`;
- invalid package facts return `InvalidPackageFacts` or `InvalidArtifact`;
- external-reference assets can return a Pumas-approved local load path with
  typed `StorageKind` and `AssetValidationState`; tests must not require all
  paths to live under the Pumas library root;
- read-only/indexed resolution does not write metadata or repair stale external
  references, while owner-fresh resolution may refresh external-reference state
  before returning a ready target;
- `PumasReadOnlyLibrary` rejects `OwnerFresh` with a typed boundary
  error/diagnostic instead of silently downgrading or mutating;
- `content_fingerprint` can be absent and is not computed by forcing full
  package-facts fingerprinting on the load-target path;
- path handling works when model roots or external-reference paths contain
  spaces, and validation compares canonical filesystem identity when the
  resolver canonicalizes paths;
- transport/boundary tests reject malformed payloads without panics or
  partially resolved targets;
- serde fixtures round-trip the request and response shapes.

Pantograph should add tests after the API exists for:

- `Ready` Diffusers target reaches the PyTorch worker envelope;
- non-ready target becomes terminal readiness/planning diagnostics before
  worker dispatch;
- Python worker receives no Pumas handle and performs no path resolution.

If the API is exposed through UniFFI or another language-binding layer, Pumas
must also add binding wrapper tests, regenerate generated bindings, and add at
least one host-language smoke test for the supported or experimental surface.

## Risks And Mitigations

| Risk | Mitigation |
| --- | --- |
| Resolver logic drifts between owner, read-only, RPC, local client, and binding surfaces. | Route all surfaces through one resolver core with explicit `resolution_mode`. |
| The implementation accidentally becomes another primary-file selector. | Resolve exact selected artifacts first; only fall back to unambiguous single-artifact resolution when allowed by the request semantics. |
| Read-only consumers mutate metadata, SQLite, or cache state. | Add a non-mutation test and keep owner freshness behind `OwnerFresh`. |
| The hot path regenerates package facts or fingerprints. | Treat `content_fingerprint` as optional and only use indexed/cache values. |
| External-reference assets are treated as if they must live under a Pumas root. | Preserve `StorageKind` and `AssetValidationState`; test approved external paths separately from library-owned paths. |
| API/RPC/IPC payloads become stringly typed or message-text parsed. | Use typed DTOs, explicit diagnostics, serde fixtures, and boundary validation tests. |
| Binding exposure leaks unstable internals. | Defer bindings unless a real host-language use case exists; if exposed, document tier and add host tests. |
| Existing serving launch paths remain duplicated indefinitely. | Keep them out of the first slice but document them as future convergence targets and avoid adding new incompatible helper logic. |

## Re-Plan Triggers

Re-plan before continuing implementation if any of these are discovered:

- existing indexed/cache state cannot represent the required ready, missing,
  partial, invalid, stale, needs-detail, ambiguous, and external-reference
  states without a schema or persisted artifact change;
- selected artifact identity cannot be resolved from `PumasModelRef` and
  existing metadata without introducing a new artifact reference contract;
- a transport or binding consumer requires a different wire shape than the core
  DTOs can safely provide;
- owner freshness requires new background tasks, polling, download queues,
  retries, or lifecycle management;
- a public surface cannot enforce its allowed resolver modes without ambiguous
  downgrade behavior or hidden mutation;
- resolving load targets requires deep directory scans or full package-facts
  regeneration to produce correct answers;
- a cross-platform path validation case cannot be handled with the current
  path/storage abstractions;
- implementation causes `library.rs` or transport handlers to absorb resolver
  policy rather than staying as facades/adapters.

## Completion Criteria

The implementation is complete when these acceptance criteria are met:

- Pantograph can request a load target for a selected Pumas artifact without
  knowing Pumas library roots.
- Pumas returns typed load-target readiness diagnostics while preserving
  existing `ModelArtifactState` and `ModelEntryPathState` fidelity.
- Pantograph passes only Pumas-approved local load targets to workers.
- No Pantograph code joins Pumas root paths or infers load paths from model ids,
  artifact names, package hints, or filesystem scans.
- The API is reusable for image generation, text generation, audio generation,
  ONNX, GGUF, and future model families.
- The implementation has passing tests for resolver behavior, serde fixtures,
  read-only non-mutation, owner freshness where enabled, and every exposed
  API/RPC/IPC/binding boundary.
- Changed source-directory READMEs document new responsibilities, API consumer
  contracts, and structured producer contracts where required.
- The final implementation passes the repository's standard Rust formatting,
  lint/check, and test gates for the touched crates.
