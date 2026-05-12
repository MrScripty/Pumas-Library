# Detailed Milestones

## Milestones

### Milestone 0: Provider Model Refactor

**Goal:** Turn the existing partial sharing across Ollama, llama.cpp, and Torch
into an explicit provider model before ONNX adds a third runtime-profile
provider and widens the blast radius.

**Tasks:**
- [x] Confirm worktree hygiene before implementation starts. Resolve, commit,
      stash, or explicitly allow dirty implementation files before editing code.
- [x] Document the existing shared systems in the implementation ADR: app/plugin
      registry, version/process management, runtime profiles, model library,
      serving state, OpenAI gateway, and frontend runtime/profile UI. For each
      system, mark whether ONNX extends it, refactors it, or only uses it as a
      reference.
- [x] Decide the app/runtime descriptor strategy before ONNX app wiring:
      update the existing hard-coded Rust/frontend/plugin identity lists in one
      contract slice, or replace them with a validated descriptor-driven
      composition root. Document why the chosen path is simpler and how drift is
      tested.
- [x] Finalize the contract ownership matrix before coding against new provider
      shapes. Every changed boundary contract must name its owner, runtime
      validator/decoder, producer tests, consumer tests, and persisted-artifact
      compatibility policy.
- [x] Add a decomposition review for touched files that exceed standards
      thresholds: files over 500 lines, UI components over 250 lines, or
      modules/services with more than one clear responsibility. Split new ONNX
      work into focused modules instead of expanding large mixed-responsibility
      files.
- [x] Define the first vertical acceptance path before broad implementation:
      managed ONNX profile -> provider-scoped route -> serve request -> gateway
      `/v1/models` and `/v1/embeddings` against a fake or fixture Rust ONNX
      provider. Add the failing-first acceptance test at the earliest slice
      where the public gateway contract can be exercised.
- [x] Define provider capabilities/behavior for supported artifact formats,
      OpenAI gateway endpoints, provider modes, device/placement support,
      launch-on-serve support, unload behavior, and provider-side model id
      policy.
- [x] Create a backend provider registry used by runtime profiles, serving
      adapters, gateway routing, and launch strategy selection. The registry is
      the extension point for Ollama, llama.cpp, and ONNX Runtime.
- [x] Establish the provider registry composition root and lifecycle owner.
      Feature modules may request provider behavior from the registry, but must
      not construct HTTP clients, process launchers, ONNX session managers, or
      concrete provider implementations ad hoc.
- [x] Build reusable provider HTTP clients and gateway clients at composition
      roots with explicit timeout/body/error policy. Provider serving adapters
      consume those clients; request handlers must not build clients directly.
- [x] Include alias defaulting, served-instance identity, route identity,
      endpoint support, request model-id rewriting, managed launch strategy, and
      provider-specific unload behavior in the provider behavior contract so
      serving/gateway handlers do not need provider matches.
- [ ] Separate provider concepts in code and docs: app/plugin identity, runtime
      provider, runtime profile, launch strategy, model route, serving adapter,
      gateway endpoint capability, model compatibility, and frontend provider
      descriptor. Do not use one enum or helper as a hidden proxy for multiple
      concepts.
- [x] Document capability ownership and route-contract replacement in an ADR
      before implementation branches depend on the new architecture.
- [ ] Replace provider-specific dispatch match blocks and non-provider fallbacks
      with provider behavior calls. The old Ollama-vs-llama.cpp branching style
      must not remain as the extension point.
- [ ] Migrate existing Ollama and llama.cpp runtime-profile behavior onto the
      provider behavior/registry path before ONNX load/unload is wired. Preserve
      user-visible behavior; do not preserve the legacy internal branching.
- [x] Replace `ModelRuntimeRoute` with a provider-scoped route type keyed by
      provider and model id. Update Rust DTOs, TypeScript bridge types, IPC
      parameters, runtime profile snapshots, mutation handlers, route lookup,
      auto-load lookup, and frontend route helpers in the same contract slice.
- [x] Replace `clear_model_runtime_route` and `model_runtime_route_auto_load`
      contracts so they accept provider plus model id. Update Electron/RPC
      method parameters and frontend call sites in the same slice.
- [x] Add a one-way runtime-profile config schema migration/cleanup that rewrites
      persisted routes to the new provider-scoped shape where unambiguous and
      drops ambiguous legacy global routes with an explicit event/error record.
      Do not keep a dual old/new route reader after cleanup.
- [ ] Represent expensive lifecycle and capability invariants with typed
      contracts/newtypes where practical, rather than passing raw strings,
      booleans, or unchecked numbers through internal APIs.
- [ ] Parse raw route, endpoint, alias, provider, mode, and placement inputs
      into validated boundary types once. Internal route/serving/profile code
      must consume validated types rather than re-validating strings.
- [x] Refactor serving artifact validation so supported formats are derived
      from provider behavior instead of the current unconditional GGUF check.
- [x] Refactor `serve_model` and `unserve_model` dispatch so load/unload paths
      are selected by provider without a non-llama.cpp-implies-Ollama fallback.
- [x] Introduce provider serving adapters for Ollama and llama.cpp before adding
      the ONNX adapter. The RPC handler should keep only boundary parsing,
      validation orchestration, and response shaping.
- [x] Add provider to served-instance lookup/unload identity where ambiguity is
      possible. Add tests where the same model id is served by more than one
      provider/profile and unload/gateway lookup remains deterministic.
- [x] Add gateway endpoint capability checks before proxying `/v1/*` requests.
- [x] Add a shared gateway HTTP client with explicit timeouts instead of
      constructing a new client per proxied request.
- [x] Add a typed gateway endpoint capability model for `/v1/models`,
      `/v1/chat/completions`, `/v1/completions`, and `/v1/embeddings`. ONNX
      must not receive chat/completion traffic unless a later plan adds that
      capability.
- [x] Add endpoint-specific request body and timeout policy for gateway routes.
      `/v1/embeddings` must have an explicit limit and error shape rather than
      inheriting a broad global proxy limit by accident.
- [x] Add typed model compatibility values for executable artifact format and
      serving task. Replace GGUF-only and raw extension checks in shared serving
      paths with provider compatibility checks.
- [x] Extract model-library executable-format/provider-compatibility projection
      helpers out of the large library implementation path where possible.
      Updates to custom ONNX runtime projections and generic ONNX embedding
      compatibility must be isolated and separately tested.
- [x] Add a typed managed launch strategy abstraction for binary process,
      reserved sidecar, and external-only profiles. Use it for existing Ollama
      and llama.cpp launch behavior before adding the superseding Rust
      in-process ONNX managed runtime behavior.
- [x] Add frontend provider descriptors consumed by profile settings, compatible
      model lists, route mutations, and serve dialog selection. Move
      llama.cpp-specific route-row behavior behind provider-specific view models
      that feed shared route primitives.
- [x] Split already-large files through narrow delegating modules before adding
      ONNX branches: runtime provider behavior, route persistence/migration,
      serving provider adapters, gateway proxy helper, runtime launch strategy,
      and frontend provider row/view-model components.
- [x] Add or update README files for new provider, gateway, launch-strategy,
      route-migration, serving-adapter, or frontend provider-descriptor
      directories. Required sections must contain concrete rationale or an
      explicit `None` with reason and revisit trigger.
- [ ] Make managed ONNX Runtime profiles the implementation target for the
      first complete slice. External ONNX profiles may be supported only through
      the same provider behavior and route contracts.
- [x] Add backend tests that prove Ollama and llama.cpp behavior now flows
      through the new provider behavior and provider-scoped route contracts,
      while unsupported provider/path combinations fail cleanly.
- [x] Add contract serialization tests for provider capabilities and any new
      served-model fields so Rust and frontend wire shapes stay aligned.

**Verification:**
- `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml serving`
- `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`
- `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`
- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `npm run -w frontend check:types`
- Tests prove no old global route shape is serialized in new snapshots.
- ADR exists for provider capabilities/provider-scoped routes and links back to
  this plan.
- Tests prove same model id can have separate routes and served instances for
  different providers without cross-provider fallback.
- Tests prove Ollama and llama.cpp use the provider registry/adapter path for
  profile validation, serving dispatch, gateway model-id rewriting, and unload.
- Tests prove unsupported provider endpoint combinations fail before proxying.

**Status:** In progress. Initial worktree hygiene found dirty model-library
implementation files, which were committed before ONNX implementation resumed.
Provider-model documentation setup is complete in
`docs/adr/0001-onnx-runtime-provider-model.md`. Backend provider behavior
contracts and built-in registry values now exist in
`rust/crates/pumas-core/src/providers/`, with tests for Ollama and llama.cpp.
Runtime-profile validation now consumes the registry for provider-mode and
managed/external support checks without changing user-visible profile behavior.
Runtime profile capability DTOs now project from provider behavior values.
Runtime profile routes are now provider-scoped across Rust DTOs, RPC/Electron
parameters, frontend bridge types, llama.cpp route helpers, endpoint lookup,
auto-load lookup, and one-way persisted config migration. Serving dispatch now
flows through provider behavior and focused existing-provider adapter modules.
`unserve_model` no longer has the non-llama.cpp-implies-Ollama fallback.
Serving and gateway request model-id rewriting now consume provider behavior
policy instead of transport-layer Ollama/llama.cpp matches. This completes the
model-id rewriting portion of provider behavior migration; launch strategy
selection remains pending.
Gateway proxy routes now map `/v1/*` paths to typed `OpenAiGatewayEndpoint`
values and reject unsupported provider/endpoint combinations before proxying.
The current built-in Ollama and llama.cpp behavior remains unchanged because
both declare support for the currently routed endpoints; ONNX can register
embeddings-only support without inheriting chat/completion routing.
Gateway proxying now uses a shared timeout-bound HTTP client owned by the RPC
server composition root instead of constructing a client for each forwarded
request. Endpoint-specific body/timeout policy remains pending.
Gateway proxying now parses raw request bodies through typed endpoint policy,
rejects oversized bodies with a Pumas-shaped HTTP 413 response before provider
forwarding, and applies an explicit per-request timeout. The current
per-endpoint body ceilings preserve the existing 32 MiB gateway limit until the
Rust ONNX provider supplies a narrower endpoint contract.
Served-instance identity now includes provider when recording, finding, and
unloading served models. `UnserveModelRequest` accepts an optional provider
field, frontend unload calls send the backend-recorded provider, and serving
state tests cover two providers serving the same model/profile/alias without
cross-provider unload.
Serving alias defaulting now consumes provider behavior policy instead of
matching directly on `RuntimeProviderId`: Ollama keeps generated Ollama model
names and llama.cpp keeps the library model id. Managed launch strategy remains
pending.
`unserve_model` dispatch now consumes provider behavior unload policy instead
of matching directly on the served provider id.
`serve_model` dispatch now consumes provider behavior serving-adapter kind
instead of matching directly on the requested provider id. Existing Ollama and
llama.cpp load routines are now behind focused adapter modules.
OpenAI-compatible gateway handlers and proxy helpers have been extracted from
the oversized RPC handlers module into a focused gateway module while preserving
the public route exports. The broader large-file split task remains open for
route migration, launch strategy, and frontend provider row work.
Ollama serving load/unload has been extracted into a focused adapter module.
llama.cpp serving load/unload, router behavior, and shared compatibility helpers
have also been extracted into focused modules. The serving handler now owns the
JSON-RPC boundary, validation orchestration, provider behavior dispatch, and
shared response shaping. Launch strategy extraction remains a separate open
task.
Serving validation now receives a typed `ExecutableArtifactFormat` parsed once
from the primary model file path instead of a raw extension string. The shared
provider artifact compatibility check consumes that typed value, and touched
Ollama serving and dedicated llama.cpp launch paths use the same provider-owned
artifact parser.
The existing `handlers/README.md` now documents the extracted OpenAI gateway
and serving adapter modules. The provider README documents the provider
behavior contract and executable artifact parser. No new source directories
were added for launch strategy, route migration, or frontend descriptors in
this slice.
llama.cpp router catalog projection now uses an isolated executable-artifact
projection helper backed by `ExecutableArtifactFormat::from_path` instead of a
raw GGUF extension check. ONNX-specific model-library projections remain
isolated to later ONNX slices.
Runtime-profile management-mode validation now consumes provider launch kinds
through `ProviderBehavior::supports_management_mode`. The provider contract has
a previously reserved sidecar launch kind, now superseded for ONNX by the Rust
in-process runtime strategy recorded in the 2026-05-11 re-plan note below.
The runtime-strategy abstraction itself remains pending.
The backend provider registry is now consumed by runtime-profile validation and
capability projection, serving adapter selection, gateway endpoint routing,
provider-side request model-id policy, artifact compatibility, alias defaulting,
unload behavior, and launch-kind validation. The registry is ready to accept an
ONNX Runtime behavior entry; composition-root lifecycle ownership remains a
separate open task.
RPC server state now owns the provider registry for gateway and serving handler
boundaries, so those handlers no longer construct built-in registries ad hoc.
Core serving/runtime-profile services still need a separate injection slice
before the composition-root lifecycle ownership task can be closed.
Core serving validation now consumes the provider registry owned by
`ServingService` instead of constructing a built-in registry inside shared
validation. Runtime-profile validation still needs the same service-owned
registry treatment before the composition-root lifecycle task can be closed.
Core runtime-profile validation now consumes the provider registry owned by
`RuntimeProfileService`, and the primary API builder is the composition root for
the core provider registry passed into both runtime-profile and serving
services. Service-level default constructors no longer construct built-in
registries in production code. This closes the provider-registry composition
root task for existing core services; reusable provider clients and managed
launch strategy remain open under their separate tasks.
Reusable provider-client work has started with llama.cpp router serving:
`LlamaCppRouterClient` is owned by RPC `AppState` and the router serving
adapter consumes it for readiness/load/unload requests with explicit operation
timeouts.
Ollama provider-client reuse is now complete for existing RPC paths:
`OllamaClient` accepts reusable `OllamaHttpClients`, RPC `AppState` owns an
`OllamaClientFactory`, and Ollama serving/app handlers use that factory instead
of constructing client stacks in request handling. Existing-provider gateway and
provider HTTP client ownership is now composition-rooted; plugin proxy HTTP
client construction is unrelated to runtime provider serving.
Provider capability DTOs and provider-scoped served-model status fields now
have focused JSON serialization/round-trip tests in Rust, locking the current
wire names before ONNX adds new provider and artifact values.
Runtime-profile provider-specific validation now dispatches through
`RuntimeProviderAdapters` owned by `RuntimeProfileService`, with `PumasApiBuilder`
composing the existing Ollama and llama.cpp adapters. Runtime-profile
launch-spec derivation still contains provider-specific branching and remains
open under the managed launch-strategy abstraction task.
Runtime-profile launch specs now carry `RuntimeProfileLaunchStrategy`. Existing
Ollama and llama.cpp managed profiles map to binary-process launch kinds, and
lifecycle launch config construction consumes that strategy instead of matching
provider ids directly. The previously reserved Python sidecar launch kind is
now superseded for ONNX by the Rust in-process runtime strategy recorded in the
2026-05-11 re-plan note below.
Managed launch target selection now lives in `ProviderBehavior` as per-mode
`ProviderManagedLaunchStrategy` entries. Runtime-profile launch-spec derivation
consumes the composed provider registry for launch targets, so existing Ollama
and llama.cpp launch mapping no longer lives in a runtime-profile provider
match. This closes the managed-launch portion of the provider behavior contract;
ONNX in-process runtime lifecycle wiring remains a later provider lifecycle
slice.
Provider behavior now separately declares the managed runtime app id used for
version-manager lookup plus the existing launch failure messages for missing
version manager or active version. RPC runtime-profile launch no longer matches
provider ids to select Ollama versus llama.cpp version managers, and current
user-visible launch errors are preserved.
Frontend runtime provider descriptors now live in
`frontend/src/utils/runtimeProviderDescriptors.ts`. Runtime profile settings,
llama.cpp compatible model rows, provider-scoped route mutations, and serve
dialog compatibility checks consume descriptor data for existing Ollama and
llama.cpp providers. ONNX frontend panel wiring remains deferred until the ONNX
provider/app identity milestones.
Serve-dialog default context size and launch-on-serve initial profile fallback
now consume runtime provider descriptors instead of hard-coded llama.cpp checks.
Current llama.cpp behavior is preserved because llama.cpp is the only existing
provider descriptor that declares launch-on-serve support and a default context
size.
Managed launch-spec derivation has been extracted from the oversized
`runtime_profiles.rs` into `runtime_profiles/launch_specs.rs`. This covers the
runtime launch-strategy portion of the large-file split task; route
persistence/migration and frontend provider row decomposition remain open under
that task.
Backend provider-path verification now includes focused tests proving managed
launch-spec derivation consumes the composed provider registry and serving
validation consumes provider-declared artifact compatibility instead of
hard-coded provider/format fallback. Existing provider-scoped route tests
already cover same-model-id routing and served-instance identity for separate
providers.
Core serving placement validation has been extracted into
`rust/crates/pumas-core/src/serving/placement.rs`, and the serving README now
documents the focused module. The large-file split task remains open for route
persistence/migration and frontend provider row/view-model decomposition.
Core serving placement rule selection now consumes
`ProviderBehavior::serving_placement_policy` instead of matching directly on
provider ids. Existing Ollama requests use the profile-only policy, existing
llama.cpp requests use the llama.cpp runtime policy, and router loaded-context
checks compare against the request provider rather than a hard-coded provider
id.
Core serving gateway alias validation and effective-alias derivation have been
extracted into `rust/crates/pumas-core/src/serving/gateway_alias.rs`, and the
serving README now documents alias policy ownership. This keeps alias boundary
rules out of the serving service entrypoint before ONNX adds embedding-serving
routes.
Provider behavior now owns managed runtime path segments and implicit base
ports, so launch-spec derivation no longer maps provider ids to launch layout
for those values. Launch-spec env/arg derivation now consumes
`RuntimeProfileLaunchStrategy` instead of matching directly on provider ids;
non-binary runtime strategies fail explicitly until a provider lifecycle slice
implements that target.
Serving validation now consumes provider-owned launch-on-serve support for
stopped managed profiles instead of hard-coding llama.cpp as the only accepted
provider path. Existing Ollama remains rejected for stopped managed serving
requests, while llama.cpp router/dedicated behavior is preserved.
Runtime-profile lifecycle launch preparation now branches on
`RuntimeProfileLaunchStrategy` for llama.cpp router/dedicated prep instead of
checking provider id plus provider mode. ONNX in-process runtime preparation
remains unwired until the Rust ONNX lifecycle slice.
Runtime-profile route config initialization, one-way legacy route migration,
and route validation now live in
`rust/crates/pumas-core/src/runtime_profiles/route_config.rs`. The runtime
profile README documents the persistence boundary, and provider-scoped route
migration remains verified through the public runtime profile service tests.
The llama.cpp compatible-model list and row renderers have been extracted into
`frontend/src/components/app-panels/sections/LlamaCppModelLibraryList.tsx` and
`frontend/src/components/app-panels/sections/LlamaCppModelRow.tsx`, and
quick-serve helpers now live in `llamaCppQuickServe.ts`. This reduces
`LlamaCppModelLibrarySection.tsx` below the large-component threshold while
leaving route persistence and serving orchestration in the section. The
sections README documents these boundaries so ONNX can add sibling list/row
work without expanding the llama.cpp section.
The named large-file split work is complete for the ONNX provider-model
prerequisite: runtime provider behavior, route config migration, serving
adapters, gateway proxy helpers, runtime launch strategy/spec derivation, and
frontend provider row/view-model components now have focused delegate modules.
Large unrelated legacy files remain outside this plan's ONNX write surface.
Re-plan accepted on 2026-05-11: ONNX Runtime execution will use Rust bindings
and an in-process Rust provider/session manager instead of a Python sidecar.
The existing reserved `PythonSidecar(OnnxRuntime)` planning direction is
superseded by a Rust in-process runtime strategy to be added in the next
provider contract slice.

### Milestone 1: Rust ONNX Runtime Skeleton

**Goal:** Add a Rust-owned ONNX provider/session boundary with validated
embedding request/response contracts and a fake backend before wiring real ONNX
Runtime execution.

**Tasks:**
- [x] Create a focused Rust ONNX provider/session module or crate with README
      contract sections before expanding internals.
- [x] Keep ONNX session manager construction at the Rust composition root.
      Serving/gateway handlers may receive traits/handles, but must not create
      ONNX sessions, tokenizer state, or global managers ad hoc.
- [x] Define validated Rust request/session types for model path, model id,
      embedding input, dimensions, batch size, token limit, and execution
      provider options.
- [x] Implement a session abstraction that can be unit-tested with a fake ONNX
      backend before real ONNX Runtime integration.
- [x] Implement fake load, unload, status/list, and embedding execution through
      the same Rust provider adapter shape the real ONNX backend will use.
- [x] Add bounded inference concurrency so ONNX Runtime threading and Rust async
      request handling cannot create unbounded work under embedding load.
- [x] Define shutdown ordering: stop accepting new load/inference work, cancel
      or drain queued work with bounded timeout, unload sessions, and report
      cleanup failures through logs/status.
- [x] Return OpenAI-compatible error bodies from the Pumas gateway for ONNX
      embedding failures.

**Verification:**
- `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`
- `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`
- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`
- Tests cover invalid path/root escape, invalid model id, invalid payload
  shape, request-size limits, fake backend ordering, and session unload cleanup.
- Tests use isolated temp roots and no shared mutable process-global state
  unless explicitly serialized.

**Status:** In progress. The first Rust ONNX skeleton slice added
`rust/crates/pumas-core/src/onnx_runtime/` with README coverage, validated
contract types, a fake backend, a bounded `OnnxSessionManager`, and focused
unit tests. RPC `AppState` now owns the bounded ONNX session manager for fake
serving. The gateway ONNX adapter now maps validation, not-loaded, and backend
failures into bounded OpenAI-compatible error bodies. `OnnxSessionManager`
shutdown now closes the manager to new work, drains operation permits with a
bounded timeout, unloads known sessions, and rejects later operations with a
typed backend error.

### Milestone 2: ONNX Embedding Execution

**Goal:** Run real ONNX embedding inference with explicit, configurable
post-processing semantics.

**Tasks:**
- [x] Add Rust ONNX Runtime/tokenizer/numeric dependencies to the owning Rust
      crate/module only after a dependency review. Candidate ONNX Runtime Rust
      binding: `ort`, pending version/native-library strategy decision.
- [x] Record dependency justification for selected Rust ONNX Runtime,
      tokenizer, and numerical crates: in-house alternative,
      maintenance/license, transitive cost, CPU/GPU package choice, and Rust
      owner.
- [x] Pin dependencies through the owning Rust manifest/lock strategy used by
      the repo, and verify focused Rust build/test commands do not depend on
      unrelated runtime paths.
- [x] Record dependency tree, license, security-audit, and package-size impact.
      If ONNX Runtime introduces separate CPU/GPU packages, document the chosen
      default and the re-plan trigger for GPU support.
- [x] Load tokenizer from a validated model directory in Rust.
- [x] Load ONNX session from a validated model directory in Rust.
- [x] Keep ONNX Runtime native-library/provider selection explicit in Rust
      configuration or startup logs so CPU/GPU package behavior is observable
      and does not silently vary by platform.
- [x] Tokenize string or string-array `input`.
- [x] Run ONNX Runtime inference with bounded batch size and token length.
- [x] Implement a configurable embedding postprocess strategy covering pooling,
      optional layer normalization, optional Matryoshka truncation, and optional
      L2 normalization.
- [x] Apply output tensor selection to real ONNX outputs during inference.
- [x] Default conservatively from model metadata/config when possible and fail
      with explicit configuration errors when the output contract is ambiguous.
- [x] Support optional `dimensions` only when the loaded model/postprocess
      strategy can produce a compatible vector length.
- [x] Use checked arithmetic before tensor allocation, vector truncation, or
      response-size calculations derived from request payloads.
- [x] Return one embedding row per input item.
- [x] Add tests for response shape, vector dimensions, batch ordering, and
      rejected invalid dimensions.
- [x] Add deterministic numerical tests with tolerances for post-processing.
- [x] Add shape tests for real ONNX fixtures. Do not make broad performance or
      quality claims without benchmark evidence.
- [ ] Add a throughput/resource-limit check for representative batch sizes if
      ONNX inference becomes a hot path or any performance claim is made.

**Verification:**
- `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`
- `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`
- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`
- Dependency tree/audit output recorded in Execution Notes or PR notes.
- Rust dependency build/check command recorded in Execution Notes or PR notes.
- A local smoke call against a known ONNX embedding fixture:
  `POST /v1/embeddings` returns HTTP 200 and expected vector length.
- Resource-limit tests prove oversized batches/tokens/dimensions fail before
  unbounded allocation.

**Status:** In progress. Dependency review is recorded in
`dependency-review.md`. The first manifest slice added explicit CPU-first
workspace dependencies consumed only by `pumas-core`: `ort` `2.0.0-rc.12` with
explicit `std`/`ndarray`/`tracing`/`download-binaries`/`copy-dylibs`/
`tls-native`/`api-24` features, and `tokenizers` `0.23.1` with only `onig`.
`ndarray` is present transitively through `ort`, not as a direct dependency.
Focused `cargo check`, `cargo test ... onnx`, and dependency-tree checks passed.
Dependency tree, license, package-size, and attempted security-audit evidence
is recorded in `dependency-review.md`; `cargo-audit` is not installed in this
environment, so a successful advisory audit or approved release-time
alternative remains a release gate. Native-library packaging validation remains
open for Milestone 8's release/launcher smoke.
The tokenizer slice added `OnnxTokenizer`, which resolves a sibling
`tokenizer.json` from a validated ONNX model path, verifies it stays under the
configured model root, tokenizes ordered embedding inputs, returns `i64`
input-id and attention-mask rows, and rejects empty or oversized tokenized
inputs before tensor construction. Focused ONNX tests cover successful
tokenizer load/tokenization, missing tokenizer files, and token-limit
rejection. The real session-loader slice added `OnnxRuntimeSession`, which
uses the validated model path, sibling tokenizer, explicit CPU execution
provider, bounded ONNX Runtime thread options, and session input/output
introspection. Focused tests cover the validated model-directory contract and
map invalid ONNX bytes to a typed backend error; a successful real-model smoke
remains open until a known ONNX embedding fixture is available. ONNX inference
remains open. The postprocess slice added a pure configurable postprocessor for
mean pooling, optional layer normalization, optional truncation, and optional L2
normalization. Deterministic tests cover masked mean pooling, batch ordering,
truncation-before-normalization, layer normalization, invalid dimensions, and
shape mismatch rejection. The module-size guard slice extracted the backend
trait and bounded session manager into `onnx_runtime/manager.rs`, reducing
`onnx_runtime/mod.rs` from 498 to 372 lines before real inference wiring. Real
ONNX output tensor selection remains open for the inference slice. Tokenizer
discovery now supports package-root `tokenizer.json` files for ONNX graphs in
nested directories such as `onnx/model_fp16.onnx`, while preserving root
containment validation. Model config discovery uses the same package-root
search for `config.json`; real session loading now defaults source embedding
dimensions from matching `hidden_size`/`n_embd` metadata and rejects explicit
load dimensions that conflict with that metadata. Real ONNX output tensor
selection remains open for the inference slice. An opt-in real fixture smoke
test now loads the local Nomic package through ONNX Runtime when
`PUMAS_ONNX_REAL_MODEL_ROOT` is supplied, validating the package-root metadata,
768-dimensional config, expected input names, and at least one output name
without making normal focused tests depend on the large model file.
FP16 tensor extraction support is now explicitly planned through `ort`'s `half`
feature plus a direct `half` dependency owned by `pumas-core`, because the local
Nomic ONNX fixture is the FP16 export.
The real backend slice added `RealOnnxEmbeddingBackend` and real
`OnnxRuntimeSession::embed` execution. It pads tokenized input into
`input_ids`, `attention_mask`, and optional `token_type_ids`, runs ONNX Runtime,
selects either a named output or first floating tensor, extracts `f32`/`f16`/
`bf16` hidden states with checked shape/value counts, applies the existing
postprocessor, and returns one embedding row per input. The opt-in local Nomic
FP16 smoke verifies two ordered inputs, 256-dimensional Matryoshka truncation,
finite values, and non-zero token usage through actual ONNX Runtime inference.
The follow-up no-behavior split moved tensor padding to `tensors.rs` and output
selection/extraction to `output.rs`, reducing `real.rs` from 452 to 219 lines
before the serving integration slice.
RPC composition now uses the real backend in production through
`OnnxEmbeddingBackendKind::real()`, while focused RPC tests explicitly inject
the fake backend variant to preserve deterministic gateway/serving assertions.
Real ONNX session startup now logs the selected execution provider, native
library strategy, graph optimization level, thread counts, model id, and
loaded input/output counts without logging full model paths or request text.
The public gateway smoke now exercises the real Rust ONNX backend through
`/v1/embeddings` with the local Nomic FP16 fixture and verifies HTTP 200,
OpenAI-compatible response shape, 256-dimensional finite embeddings, and
non-zero token usage without calling raw ONNX provider internals.

### Milestone 3: Plugin And Runtime Profile Contracts

**Goal:** Make ONNX Runtime a typed Pumas runtime provider without affecting
Ollama or llama.cpp profiles.

**Tasks:**
- [x] Add `launcher-data/plugins/onnx-runtime.json`.
- [x] Add `RuntimeProviderId::OnnxRuntime` and
      `RuntimeProviderMode::OnnxServe`.
- [x] Add ONNX Runtime to the frontend app registry with a sidebar icon,
      display name, description, default connection URL/port, and status
      defaults.
- [x] Update Rust `AppId`, version-manager registration, plugin metadata,
      frontend app registry, selected-version hooks, managed-app decoration,
      app-shell panel props, and panel renderer in one app identity slice unless
      Milestone 0 replaced them with a descriptor-driven composition root.
      If Rust ONNX execution does not use a version manager, document the
      explicit no-version-manager app identity contract in this slice instead
      of inventing a dummy install state.
- [ ] Extend managed app decoration/state so the ONNX icon reflects installed
      or available runtime support plus runtime-profile/session states from
      backend-owned ONNX provider state.
- [ ] Extend selected app version/process state hooks only as far as the
      selected lifecycle slice requires. If ONNX uses runtime profiles instead
      of standalone process hooks, keep the icon state derived from profile
      statuses rather than adding duplicate process state.
- [x] Register ONNX Runtime capabilities in the provider capability/behavior
      boundary created in Milestone 0.
- [ ] Remove assumptions that provider enums and runtime-profile DTOs are
      append-only. Replace route DTOs and provider behavior contracts cleanly
      where the old shape is wrong for ONNX.
- [x] Update Rust and TypeScript contracts in the same logical slice and verify
      serde/JSON casing for every new enum value and field.
- [ ] Add or update executable schema/fixture tests for runtime profile
      snapshots, provider capabilities, route mutations, and plugin metadata
      before frontend or RPC consumers depend on the new fields.
- [ ] Update runtime profile validation, default profile creation policy,
      endpoint resolution, status snapshots, and provider-mode compatibility
      rules.
- [ ] Add managed runtime specs for ONNX in-process lifecycle, status/health
      projection, and environment/configuration values.
- [x] Extract runtime-profile launch/runtime strategy so managed profiles can
      launch binary runtimes or initialize in-process runtimes without forcing
      ONNX through Ollama/llama.cpp binary constructors or generic lifecycle
      branches.
- [ ] Make launch/shutdown idempotent and cancellation-aware. Every background
      task, process handle, health poll, and restart flow must have one owner
      that tracks handles and observes cancellation/panic/failure paths.
- [ ] Ensure launch/stop/restart flows do not hold synchronous locks across
      awaits or blocking process/file work. Use the repo's blocking-work pattern
      for unavoidable process and filesystem operations.
- [ ] Keep platform-specific executable/venv path resolution behind existing
      process or platform abstractions. Do not inline OS checks in handlers or
      UI components.
- [ ] Update frontend runtime profile types and provider-mode option maps.
- [ ] Add contract tests for serialization and provider-mode compatibility.

**Verification:**
- `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profile`
- Runtime-profile schema migration/cleanup test rewrites or drops legacy global
  routes and persists only the provider-scoped route shape afterward.
- Lifecycle tests cover stale PID files, duplicate launch/stop, failed health
  checks, restart after failure, and managed state isolation.
- Contract fixture tests cover omitted defaults, unknown/unsupported enum
  values, invalid provider/mode combinations, and persisted artifact rewrite
  behavior.
- `npm run -w frontend check:types`
- Focused frontend tests for ONNX app registry entry, icon state derivation,
  selected-version/profile state, and provider/mode option rendering.
- App identity tests prove plugin metadata, Rust version-manager key, frontend
  app id, selected-version state, and rendered panel remain aligned.

**Status:** In progress. The Rust provider contract now includes
`RuntimeProviderId::OnnxRuntime`, `RuntimeProviderMode::OnnxServe`,
`.onnx` executable artifact support, an embedding-only ONNX provider behavior,
an `in_process_runtime` managed launch target, and contract tests. Frontend
runtime provider types/descriptors now include `onnx_runtime` and `onnx_serve`
with focused descriptor tests and typecheck coverage. Plugin metadata now
includes `onnx-runtime` with an explicit `in-process` installation type,
`.onnx` compatibility, runtime-profile/model-library panel declarations, no
version-manager capability, and Rust/TypeScript plugin schema support. The app
identity slice now registers `AppId::OnnxRuntime`, prevents ONNX Runtime from
creating a `VersionManager` or process manager, adds the frontend sidebar app
entry, keeps selected-version hooks from querying an ONNX version manager, and
routes the app shell through the explicit fallback panel until the dedicated
ONNX panel lands in Milestone 6. ONNX runtime profile lifecycle, backend-derived
icon/session state, and full schema/fixture coverage remain open.

### Milestone 4: Serving Validation And Load/Unload

**Goal:** Let users serve ONNX embedding models through backend-owned serving
state.

**Tasks:**
- [x] Extend serving validation so ONNX Runtime profiles accept primary `.onnx`
      embedding artifacts.
- [x] Use provider behavior to decide whether a `.onnx` artifact is compatible
      with generic embedding serving; do not infer that every ONNX/custom ONNX
      app artifact belongs to the ONNX Runtime embedding provider.
- [x] Reject non-ONNX artifacts and unsupported model types with non-critical
      domain errors.
- [x] Validate ONNX placement through provider capabilities: reject llama.cpp
      specific `gpu_layers`, `tensor_split`, and `context_size` controls unless
      ONNX gains an explicit equivalent later.
- [x] Use the provider-side model id policy from Milestone 0 so gateway aliases
      are not overloaded as ONNX session names accidentally.
- [x] Parse raw ONNX serving requests into validated boundary types before the
      ONNX provider adapter consumes them. Internal ONNX load/unload code no
      longer re-validates raw model ids, aliases, or paths after boundary
      parsing.
- [ ] Extend the validated serving-boundary type pattern to existing Ollama and
      llama.cpp adapters where they still consume raw request strings, ports,
      dimensions, or paths.
- [x] Add ONNX load workflow compensation when session load succeeds but status
      confirmation or backend served-state recording fails.
- [ ] Audit remaining request-cancellation windows across provider adapters so
      load/unload operations do not split durable state updates across
      cancellation points unless the step is transactional, idempotent, or has
      explicit compensation.
- [x] Instrument ONNX serving load/unload workflows with tracing spans or
      equivalent structured logs at lifecycle owners so partial failures and
      cancellations are observable without reading ONNX Runtime internals.
      ONNX runtime-profile restart is not yet an active workflow; the lifecycle
      slice must instrument it when introduced.
- [x] Resolve the effective ONNX serving profile from the saved runtime route
      when the serve dialog does not provide an explicit profile. Explicit
      profile choices still override the saved route for that request.
- [ ] Apply the same saved-route profile selection to the ONNX model row when
      the ONNX panel/model list lands in Milestone 6.
- [x] Resolve serve-dialog saved routes by `(provider, model_id)`, not by model
      id alone, and avoid default-profile fallback for ONNX when no ONNX route
      exists.
- [x] Remove default-profile fallback from any remaining backend/core ONNX model
      endpoint operation path that can silently select the wrong provider.
- [x] Return a clear serve-dialog validation message when an ONNX model has no
      saved route and no explicit ONNX profile selection.
- [x] Add ONNX provider adapter calls from `serve_model` to the Rust ONNX
      session manager.
- [x] Move existing Ollama and llama.cpp serving paths behind provider serving
      adapters before adding ONNX load/unload so the RPC handler only performs
      boundary parsing, validation orchestration, and response shaping.
- [x] Confirm the Rust ONNX provider status/list includes the model before
      recording loaded status.
- [x] Add unload support through the Rust ONNX session manager and served
      status removal.
- [x] Make load and unload idempotent where possible: duplicate load returns
      the existing loaded state, duplicate unload returns an unchanged snapshot,
      and partial ONNX provider failures do not leave stale loaded status.
- [x] Preserve user-visible Ollama and llama.cpp outcomes through the new
      provider-scoped route and provider behavior paths. Do not preserve their
      legacy internal dispatch or global-route implementation.

**Verification:**
- `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml serving`
- `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`
- Tests include duplicate load/unload, ONNX provider load failure, profile restart,
  stale endpoint, invalid alias, missing ONNX route, explicit profile override,
  provider-scoped route resolution, absence of default-profile fallback for
  ONNX, and invalid artifact cases.
- Replay/recovery tests cover persisted provider-scoped routes and served-state
  cleanup after process failure or app restart.
- Affected integration tests are run with normal parallelism enabled and repeated
  at least once to detect temp root, port, environment, or persisted-state
  leakage.
- Rust ONNX load/unload smoke test against a real or fixture ONNX embedding model.

**Status:** In progress. Serving validation accepts ONNX requests only when the
selected ONNX profile is running and the primary executable artifact is `.onnx`.
Provider behavior drives ONNX artifact compatibility, and ONNX rejects
llama.cpp-specific placement overrides with non-critical domain errors. The RPC
serving boundary now loads/unloads ONNX through the Rust fake session manager
and records/removes backend served status. The ONNX serving adapter now
confirms the Rust session manager lists the loaded model before recording
backend served status. ONNX session model ids now come from provider behavior
instead of the gateway alias, with focused RPC serving coverage proving an
explicit alias is not used as the ONNX session name. Real ONNX Runtime
execution and route/profile fallback cleanup remain open. Duplicate ONNX loads
now return the existing confirmed loaded state without bumping served-state
cursor, and ONNX unload removes stale served status if the session is already
absent. ONNX serving load/unload now emits structured lifecycle logs with safe
provider/model/profile/error fields and without full model paths, request
payloads, secrets, or embedding inputs. ONNX serving load/unload request
handling now parses executable artifact paths, provider-side model ids, gateway
aliases, and unload identities into validated local boundary values before
calling the ONNX session manager or served-state reconciliation helpers.
The serve dialog now uses provider-scoped saved routes for ONNX profile
selection, preserves explicit profile overrides, and refuses to fall back to the
default/first profile for ONNX when no saved route exists. Core model endpoint
resolution now uses provider behavior to decide whether a provider may fall
back to the global default profile; ONNX disables that fallback while Ollama and
llama.cpp preserve their existing behavior. ONNX load now explicitly unloads
the session if post-load status confirmation or served-state recording fails,
reducing stale session/state divergence for recoverable workflow failures.
Earlier committed M4 prerequisite slices extracted Ollama and llama.cpp serving
into provider adapter modules and kept their existing public serving outcomes
covered by focused core/RPC serving tests while removing legacy dispatch
fallbacks.
Gateway embedding routing has started under Milestone 5.

### Milestone 5: Pumas Gateway Routing

**Goal:** Expose served ONNX models to external applications through the
existing Pumas `/v1` gateway.

**Tasks:**
- [x] Update gateway provider routing for `onnx_runtime`.
- [x] Ensure provider request model id uses the Rust ONNX session model
      name/alias needed by `/v1/embeddings`.
- [x] Move provider request model-id rewriting out of the gateway helper and
      into provider behavior. The gateway should not match on individual
      providers to decide how to rewrite `model`.
- [x] Route `/v1/embeddings` to the Rust ONNX gateway adapter with bounded
      request body behavior preserved.
- [x] Validate OpenAI-compatible request JSON at the gateway boundary before
      dispatch, including model field shape, endpoint support, body limit, and
      provider capability.
- [x] Add endpoint-specific body-limit tests so `/v1/embeddings` rejects
      oversized embedding payloads before entering ONNX Runtime.
- [x] Use provider endpoint capabilities to keep `/v1/chat/completions` and
      `/v1/completions` unavailable for ONNX embedding-only models unless a
      future provider capability says otherwise.
- [x] Reuse the shared gateway HTTP client from Milestone 0.
- [x] Ensure gateway request handling uses bounded body reads, connection
      limits/timeouts, and no per-request client construction.
- [x] Preserve timeout and error mapping semantics so provider failures return
      bounded OpenAI-compatible error bodies and do not hang external callers.
- [x] Add request correlation or structured logging at the gateway/provider
      boundary without logging embedding input text, tokens, secrets, or full
      model paths.
- [x] Add gateway tests for success, unknown model, ambiguous alias, and
      provider error pass-through.

**Verification:**
- `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`
- Gateway tests include unsupported endpoint, provider timeout, malformed JSON,
  body too large, duplicate alias, and provider error pass-through.
- Gateway tests prove request handlers use the shared provider/gateway client
  path and do not construct per-request HTTP clients.
- Gateway acceptance test exercises the real Pumas `/v1` facade instead of the
  raw ONNX provider internals.
- Manual curl:
  `GET /v1/models` includes the ONNX model alias.
- Manual curl:
  `POST /v1/embeddings` returns OpenAI-compatible embedding JSON.

**Status:** In progress. The first M5 slice adds
`rust/crates/pumas-rpc/src/handlers/openai_gateway_onnx.rs` as the in-process
ONNX `/v1/embeddings` gateway adapter. The generic gateway still performs body
limit, JSON, model lookup, and provider capability checks first; ONNX requests
then map the gateway model to the served library model id, validate string or
string-array embedding input plus optional dimensions, reject unsupported
`encoding_format`, execute through the bounded Rust ONNX session manager, and
return OpenAI-compatible embedding JSON. Ollama and llama.cpp proxy behavior is
unchanged. Existing shared gateway tests were extracted to
`openai_gateway_tests.rs` so `openai_gateway.rs` remains below the 500-line
standards threshold. Focused verification passed:
`cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
`cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`,
and `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`.
Remaining M5 work includes manual curl evidence after frontend/serve workflow
is available.
The follow-up handler-contract slice added direct gateway handler tests for the
ONNX public `/v1/embeddings` path: a served and loaded ONNX model returns
OpenAI-compatible embedding JSON through the in-process adapter, ONNX rejects
`/v1/chat/completions` through provider endpoint capabilities, and an ONNX
served status without a loaded session maps to a bounded OpenAI-compatible
`model_not_found` error. Verification passed:
`cargo fmt --manifest-path rust/Cargo.toml --all -- --check` and
`cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`.
The next focused M5 test slice added public gateway handler coverage for
oversized embedding bodies, unknown models, and ambiguous aliases. Oversized
embedding request bodies now have a test proving they return HTTP 413 before
JSON parsing or ONNX session dispatch, unknown models return HTTP 404, and
duplicate gateway aliases return HTTP 409 with the provider-scoped ambiguity
code. Verification passed:
`cargo fmt --manifest-path rust/Cargo.toml --all -- --check` and
`cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`.
The structured-logging slice added ONNX gateway/provider boundary logs for
successful routing and provider failures with provider id, model id, gateway
model, profile id, input count, dimensions, and error code only. It does not
log embedding input text, tokens, secrets, or model paths. Verification passed:
`cargo fmt --manifest-path rust/Cargo.toml --all -- --check` and
`cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`.
The real gateway facade smoke added an opt-in test that injects the production
real ONNX backend into isolated RPC state, loads the local Nomic package via
`PUMAS_ONNX_REAL_MODEL_ROOT` and `PUMAS_ONNX_REAL_MODEL_PATH`, records the
backend-owned served status, and calls the public `/v1/embeddings` gateway
handler. It verifies HTTP 200, OpenAI-compatible JSON, the public gateway alias,
256 finite embedding values, and non-zero token accounting. Focused
verification passed:
`cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
`cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`,
`cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`, and
`PUMAS_ONNX_REAL_MODEL_ROOT=<absolute local Nomic package>
PUMAS_ONNX_REAL_MODEL_PATH=onnx/model_fp16.onnx cargo test --manifest-path
rust/crates/pumas-rpc/Cargo.toml
openai_proxy_smokes_real_onnx_embedding_fixture -- --nocapture`. The focused
RPC gateway commands require permission to bind PumasApi's local loopback IPC
listener in this sandbox; the sandboxed attempt failed with
`Operation not permitted` before the test was rerun with that allowance.
The timeout/error mapping slice added gateway handler tests for malformed JSON
rejection before provider dispatch, upstream provider error status/body
preservation, and provider timeout mapping to a bounded Pumas-shaped gateway
error. The timeout test advances the existing 120-second endpoint policy with
paused Tokio time instead of sleeping in real time. Verification passed:
`cargo fmt --manifest-path rust/Cargo.toml --all -- --check` and
`cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`.

### Milestone 6: Frontend Integration

**Goal:** Add the first-class ONNX Runtime app panel: sidebar entry, profile
manager, ONNX-compatible model list, route assignment, and serve actions while
keeping backend-owned state authoritative.

**Tasks:**
- [ ] Add ONNX Runtime to `AppPanelRenderer` so selecting the ONNX app icon
      opens a real ONNX app panel rather than the default coming-soon panel.
- [ ] Decide whether the hard-coded app registry remains acceptable for ONNX.
      If it remains, update every hard-coded registry/decorator/renderer state
      path in the same slice. If a descriptor approach is cleaner, replace the
      registry rather than adding another partial special case.
- [ ] If the hard-coded frontend app registry remains, add a focused drift test
      that fails when an app exists in plugin metadata/Rust app identity but is
      missing selected-version state, managed decoration, app-shell props, or
      renderer selection.
- [ ] Keep new ONNX panel/components under standards thresholds where practical
      and split view-model, route mutation, and rendering responsibilities
      before components become multi-responsibility.
- [ ] Keep provider descriptors and view models as data/policy translation
      layers. React components render declared capabilities and dispatch actions;
      they must not encode provider compatibility, route identity, endpoint
      support, or launch policy directly.
- [ ] Compose the ONNX panel with connection/version/status affordances that
      match the selected lifecycle slice, `RuntimeProfileSettingsSection`
      scoped to `onnx_runtime`, and an ONNX-compatible model library section
      below the profile settings.
- [ ] Build ONNX-compatible model view-model helpers that filter `.onnx`
      artifacts and exclude GGUF-only llama.cpp rows.
- [ ] Create shared executable-format/provider-compatibility helpers consumed
      by ONNX rows, llama.cpp rows, and the serve dialog. Do not duplicate
      extension checks in each component.
- [ ] Update shared `ModelInfo` format typing and model-library view models so
      ONNX is a first-class executable format, not an unchecked string special
      case.
- [ ] Show `.onnx` compatible models for ONNX Runtime without adding ONNX rows
      to llama.cpp-only views.
- [ ] Add per-row ONNX profile selection and save controls using the existing
      backend runtime route APIs after they are replaced with provider-scoped
      route APIs.
- [ ] Remove frontend helpers that assume a route is keyed only by model id.
      ONNX and llama.cpp model rows must both read and write provider-scoped
      routes.
- [ ] Update `ModelServeDialog` initial profile selection so it consults
      provider-scoped routes and provider/format compatibility. It must not
      fall back from ONNX to a llama.cpp/default profile when an ONNX route is
      missing.
- [ ] Replace one-shot serving-status reads in serve-dialog alias/loaded-state
      logic with the existing serving-status subscription hook, or document why
      a one-shot read is a non-authoritative validation aid only.
- [ ] Add ONNX quick-serve and serving-options actions that use the saved ONNX
      route/profile by default and persist draft route changes before serving.
- [ ] Update runtime profile settings controls for `onnx_runtime` and
      `onnx_serve`.
- [ ] Update serve dialog filters and labels for ONNX embedding serving.
- [ ] Replace GGUF-only serve-dialog checks with provider/format compatibility
      helpers that can handle ONNX without regressing llama.cpp behavior.
- [ ] Display backend-confirmed loaded state and endpoint mode from serving
      snapshots/events.
- [ ] Keep runtime profile and serving state backend-owned. Do not add
      optimistic loaded/unloaded UI state; render only confirmed snapshots or
      explicitly transient form/submission state.
- [ ] Use semantic controls, associated labels, accessible names for icon
      buttons, focus management for dialogs, and keyboard interaction tests for
      any new interactive controls.
- [ ] Use `button` for actions, `label`/`aria-label` for controls, decorative
      icons marked `aria-hidden`, and no generic clickable elements unless the
      required ARIA and keyboard behavior is implemented.
- [ ] Prefer existing runtime/serving event subscriptions. Any new polling must
      document why events are not feasible and include deterministic cleanup
      tests.
- [ ] Add accessible controls and tests using semantic selectors.
- [ ] Avoid direct DOM mutation for normal rendering. If any direct DOM access is
      unavoidable, isolate it behind a small owner with teardown and focused
      tests.

**Verification:**
- `npm run -w frontend test:run`
- `npm run -w frontend check:types`
- `npm run -w frontend build`
- Frontend tests use role/label/title selectors for new controls and cover
  keyboard interaction, stale async responses, and timer cleanup if polling is
  introduced.
- Tests use `userEvent` for user flows and named role/label selectors for new
  route/profile controls; generic role-count assertions are updated if added
  accessibility attributes change the role tree.
- Accessibility lint/typecheck/build must pass with no warnings introduced by
  the ONNX panel or route controls.
- ONNX panel tests cover app icon selection, profile creation/edit/save,
  ONNX-compatible model filtering, profile route save/clear, quick serve using
  the saved route, serving-options launch with the selected route, and
  backend-confirmed served-state display.
- Llama.cpp panel tests are updated to prove GGUF route assignment still works
  through the provider-scoped route contract, not through legacy global routes.
- Serve dialog tests cover missing ONNX route, provider-filtered initial
  selection, alias requirement updates from serving-status subscriptions, and
  absence of fallback to llama.cpp/default profiles.

**Status:** Not started.

### Milestone 7: Documentation And External App Contract

**Goal:** Make the new serving path discoverable for Emily and other local
clients.

**Tasks:**
- [ ] Update `docs/contracts/desktop-rpc-methods.md` with ONNX Runtime gateway
      behavior.
- [ ] Add README coverage for Rust ONNX provider/session modules with lifecycle,
      endpoint/gateway behavior, limits, and troubleshooting.
- [ ] Update relevant runtime/profile README files if new provider modules are
      added.
- [ ] Add or update an ADR if provider capabilities become a durable runtime
      provider registry or materially change the runtime architecture.
- [ ] Add README or ADR traceability for every new extracted source directory
      that owns provider behavior, route migration, launch strategies, gateway
      proxying, serving adapters, or frontend provider descriptors.
- [ ] Document plugin manifest semantics and runtime-profile persisted JSON
      compatibility where structured producer contracts change.
- [ ] Add or update persisted-artifact validation tooling if runtime-profile
      JSON, plugin manifests, or example payloads gain schema-backed shapes that
      can drift.
- [ ] Add external app examples for `/v1/models` and `/v1/embeddings`.
- [ ] Add Emily config guidance that points at the Pumas gateway, not the raw
      provider internals, for normal usage.

**Verification:**
- Documentation links resolve.
- New/changed source directories have README sections required by
  Documentation Standards, including explicit `None` statements where a section
  is not applicable.
- README sections contain project-specific rationale for purpose, constraints,
  decisions, invariants, and revisit triggers; generic file-inventory
  placeholders are not accepted.
- Example curl commands are validated against a local served model or clearly
  marked as shape examples when no model is available.

**Status:** Not started.

### Milestone 8: Release Validation

**Goal:** Validate the full cross-layer feature in the packaged launcher path.

**Tasks:**
- [ ] Run focused Rust and frontend checks from prior milestones.
- [ ] Run focused Rust dependency/build checks from the ONNX execution owner,
      not only from root convenience scripts.
- [ ] Build the release app.
- [ ] Launch the app and serve an ONNX embedding model through an ONNX Runtime
      profile.
- [ ] Verify external gateway calls from a separate process.
- [ ] Verify unload removes the model from `/v1/models`.
- [ ] Verify launcher-compatible install/build/release-smoke paths package or
      locate ONNX Runtime native dependencies without mutating normal user
      state.
- [ ] Record dependency audit, license review, package size/transitive cost,
      and CPU/GPU packaging decision.
- [ ] Record release artifact impact, checksum/SBOM expectations, and whether
      ONNX Runtime native libraries change installer/package size or platform
      support.
- [ ] Update changelog or release notes for the user-visible ONNX serving
      feature.
- [ ] Record results in Execution Notes and Completion Summary.

**Verification:**
- `bash launcher.sh --build-release`
- Existing launcher release smoke command, if available for this repo.
- Release app smoke: ONNX model loaded, `/v1/models` lists alias,
  `/v1/embeddings` returns expected dimension, unload removes alias.

**Status:** Not started.
