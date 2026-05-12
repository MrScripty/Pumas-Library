# Execution And Coordination

## Execution Notes

Update during implementation:
- 2026-05-11: Split the monolithic ONNX Runtime embedding serving plan into a
  standards-compliant plan directory. `plan.md` is now the execution index;
  detailed inputs/standards, impact review, provider contracts, risks,
  milestones, and coordination notes live in separate linked Markdown files.
- 2026-05-11: Plan created from user request to add ONNX Runtime embedding
  serving for external apps, following the local coding standards plan
  structure.
- 2026-05-11: Plan reviewed against runtime profile, serving, gateway, process,
  plugin, and frontend blast radius. Added Milestone 0 for provider capability
  boundaries, provider-aware validation/unload/gateway dispatch, shared gateway
  client, and clearer frontend integration constraints.
- 2026-05-11: Plan iterated against local Coding Standards for planning,
  architecture, security, concurrency, dependencies, frontend/accessibility,
  interop, documentation, launcher, release, cross-platform, and Rust-specific
  API/async/security/tooling requirements. Added a standards compliance
  guardrail section and milestone-level verification requirements.
- 2026-05-11: Plan re-reviewed with the constraint that legacy code and
  backwards-support paths must not remain. The plan now requires provider-scoped
  routes, clean provider behavior replacement, managed ONNX lifecycle as the
  target slice, one-way runtime-profile route cleanup, and removal of old
  global-route/two-provider fallback paths.
- 2026-05-11: Plan iterated again against the local Coding Standards directory.
  Added implementation-blocking compliance gates, code-area findings, vertical
  acceptance-test requirements, validated boundary type requirements,
  lifecycle/cancellation checks, dependency ownership evidence, README/ADR
  traceability, release artifact/SBOM expectations, and standards-compliant
  parallel worker coordination rules.
- 2026-05-11: Plan reviewed against current code blast radius. Added concrete
  findings for runtime profile DTOs/service, route resolution, serving core,
  RPC serving handlers, gateway proxy, runtime launch lifecycle, model-library
  metadata, frontend app registry, runtime profile controls, model route UI,
  state synchronization, and oversized files. Added simplification and
  performance requirements so ONNX is implemented through provider behavior,
  provider-scoped identity, launch strategy extraction, shared gateway client,
  and frontend provider compatibility helpers instead of new special cases.
- 2026-05-11: Plan updated to make the cleaner provider model an explicit
  architecture deliverable. Milestone 0 is now the Provider Model Refactor and
  requires documenting shared systems, separating app/plugin identity from
  runtime provider behavior, adding a provider registry, migrating Ollama and
  llama.cpp through provider behavior/adapters first, adding provider-scoped
  routes/served identity, endpoint capability checks, managed launch strategies,
  model compatibility types, and frontend provider descriptors before ONNX
  serving is wired.
- 2026-05-11: Plan iterated again for standards compliance after the provider
  model update. Added explicit public-facade and composition-root constraints,
  executable contract ownership matrix, provider-registry lifecycle ownership,
  package-local sidecar dependency/format checks, README traceability gates,
  integration-test isolation/repeat requirements, frontend declarative
  rendering and semantic selector requirements, and additional re-plan triggers
  for standards gate failures or contract ownership ambiguity.
- 2026-05-11: Plan re-reviewed against the current code after the cleaner
  provider-model changes. Added additional blast-radius findings for hard-coded
  Rust/frontend/plugin app identity, absence of a real provider registry
  composition root, per-request/per-operation HTTP client construction, Torch
  sidecar limits as a reference pattern, gateway endpoint-specific body policy,
  the very large model-library implementation surface, and frontend
  serve-dialog state drift. Updated milestones to require app/runtime descriptor
  strategy, reusable provider clients, model-library compatibility extraction,
  endpoint-specific gateway limits, app identity drift tests, and serving-status
  subscription use before ONNX is wired.
- 2026-05-11: Implementation start hygiene check ran before code edits.
  `git status --short --untracked-files=all` found dirty implementation files
  under `rust/crates/pumas-core/src/model_library/`: `artifact_identity.rs`,
  `library.rs`, `library/migration.rs`, and `mod.rs`. The first confirmed
  slice is Milestone 0 worktree hygiene plus provider-model documentation setup.
  Per the plan gate, code implementation is paused until those dirty
  implementation files are resolved, committed, stashed, or explicitly allowed
  for this plan.
- 2026-05-11: Dirty model-library implementation files were committed in
  `2c6dea94` before ONNX implementation resumed. Focused verification for that
  pre-existing dirty slice passed: model-library migration dry-run tests and
  Rust workspace formatting via `rust/Cargo.toml`.
- 2026-05-11: Completed the first ONNX Milestone 0 documentation slice. Added
  ADR 0001 for the provider model, existing shared-system treatment,
  app/runtime descriptor strategy, provider capability and route ownership,
  first vertical acceptance path, and decomposition review. Next implementation
  slice is the first backend provider behavior/registry contract test and
  implementation.
- 2026-05-11: Added the first backend provider behavior registry slice in
  `rust/crates/pumas-core/src/providers/`. The slice defines typed provider
  behavior for existing Ollama and llama.cpp providers, including provider
  modes, device modes, local artifact formats, serving tasks, OpenAI gateway
  endpoints, launch kinds, model-id policy, and unload behavior. This slice does
  not yet route runtime-profile validation, serving, gateway, or launcher code
  through the registry; that is the next slice.
- 2026-05-11: Integrated the provider registry into runtime-profile validation
  for provider-mode and managed/external support checks while preserving the
  existing Ollama and llama.cpp adapters for provider-specific validation. The
  registry is still not used by serving adapters, gateway routing, or launcher
  strategy selection.
- 2026-05-11: Moved runtime profile capability DTO construction onto provider
  behavior projection for existing providers. This keeps the current
  `RuntimeProviderCapabilities` shape stable while removing the duplicate
  hard-coded provider mode/device lists from runtime profile capability
  constructors.
- 2026-05-11: Replaced model-only runtime routes with provider-scoped route
  records across Rust DTOs, runtime profile mutations, endpoint lookup,
  auto-load lookup, RPC/Electron params, frontend bridge types, llama.cpp route
  helpers, and serve-dialog initial route selection. Runtime-profile config
  schema version is now `2`; legacy schema `1` routes are rewritten when the
  referenced profile makes the provider unambiguous, and ambiguous legacy routes
  are dropped during the one-way cleanup.
- 2026-05-11: Moved serving artifact validation from an unconditional GGUF check
  to provider behavior compatibility. Existing Ollama and llama.cpp serving
  behavior still accepts GGUF, while unknown artifact extensions now fail
  through the provider compatibility path that ONNX can extend later.
- 2026-05-11: Removed the `unserve_model` non-llama.cpp-implies-Ollama fallback.
  Unload now no-ops when no served instance exists and dispatches from the
  backend-recorded served provider for Ollama and llama.cpp. Full serving
  adapter extraction remains pending.
- 2026-05-11: Moved provider-side request model-id rewriting into
  `ProviderBehavior::provider_request_model_id`. The OpenAI gateway and
  llama.cpp router load path now ask the built-in provider registry for
  model-id policy instead of matching Ollama versus llama.cpp in transport
  handlers. This is the first thin serving/gateway slice toward provider
  adapters; alias defaulting, endpoint capability checks, shared clients, and
  full load/unload adapter extraction remain pending.
- 2026-05-11: Added gateway endpoint capability enforcement before proxying.
  Gateway proxy requests now map the requested `/v1/*` path to
  `OpenAiGatewayEndpoint` and verify the served model's provider behavior
  declares support before forwarding the request. Built-in Ollama and llama.cpp
  behavior is unchanged because both providers currently declare support for the
  routed endpoints. Shared gateway HTTP client and endpoint-specific body/timeout
  policy remain pending.
- 2026-05-11: Added a shared gateway HTTP client to the RPC server composition
  root. OpenAI-compatible proxy handlers now reuse the `AppState` client with an
  explicit 120 second timeout instead of constructing a new client per request.
  Endpoint-specific body/timeout policies are still pending.
- 2026-05-11: Added typed gateway endpoint policies for chat completions,
  completions, and embeddings. The proxy handler now accepts raw bytes, applies
  endpoint-specific body ceilings before JSON parsing/provider forwarding,
  returns a Pumas-shaped HTTP 413 error response when exceeded, and
  applies the endpoint request timeout to the forwarded request. The explicit
  per-endpoint body ceilings preserve the existing 32 MiB gateway limit until a
  narrower Rust ONNX provider endpoint contract lands.
- 2026-05-11: Added provider to served-instance unload identity. Backend
  serving state now compares provider when replacing, finding, and unloading
  served models; unload events carry the provider; `UnserveModelRequest`
  accepts provider; and frontend unload calls send the provider from
  backend-owned served status. This keeps same-model-id served instances
  deterministic before ONNX adds another provider.
- 2026-05-11: Added gateway alias defaulting policy to `ProviderBehavior` and
  moved serving request alias defaulting onto that policy. Ollama still derives
  provider-safe Ollama model names from model display names, and llama.cpp still
  defaults the gateway alias to the library model id. Full serving adapter
  extraction, launch strategy selection, and unload behavior consumption remain
  pending.
- 2026-05-11: Moved `unserve_model` provider dispatch onto
  `ProviderBehavior::unload_behavior`. The handler still calls the existing
  Ollama and llama.cpp unload routines, but the extension point is now provider
  behavior rather than a direct provider-id match. Load dispatch and full
  serving adapter extraction remain pending.
- 2026-05-11: Added `ProviderServingAdapterKind` to provider behavior and moved
  `serve_model` load dispatch onto that behavior. The handler still delegates
  to the existing Ollama and llama.cpp load routines, but both load and unload
  dispatch now select through provider behavior rather than direct provider-id
  matches. Full provider serving adapter extraction remains pending.
- 2026-05-11: Extracted OpenAI-compatible gateway handlers and proxy helpers
  from the oversized RPC handlers module into
  `rust/crates/pumas-rpc/src/handlers/openai_gateway.rs`. The public route
  handlers remain re-exported from `handlers::mod` so the server route contract
  is unchanged. During extraction, removed duplicate gateway sort/return lines
  found in the moved code; behavior is covered by the existing gateway lookup
  and endpoint-policy tests.
- 2026-05-11: Extracted the Ollama serving load/unload implementation into
  `rust/crates/pumas-rpc/src/handlers/serving_ollama.rs`. The JSON-RPC serving
  handler now imports Ollama adapter entry points while retaining boundary
  parsing, validation orchestration, non-critical response shaping, and provider
  behavior dispatch. llama.cpp adapter extraction remains pending and must stay
  separate from launch-strategy redesign.
- 2026-05-11: Extracted llama.cpp serving into focused modules:
  `serving_llama_cpp.rs` for adapter entry points,
  `serving_llama_cpp_router.rs` for router-specific behavior, and
  `serving_llama_cpp_shared.rs` for runtime/model-id compatibility helpers.
  This completes the existing-provider serving adapter extraction without
  mixing in ONNX behavior or the later launch-strategy abstraction. The new
  serving modules are below the 500-line standards threshold.
- 2026-05-11: Moved serving artifact compatibility to a typed boundary value.
  `PumasApi::validate_model_serving_config` now parses the primary model file
  path into `ExecutableArtifactFormat` once, `ServingValidationContext`
  carries the typed format instead of a raw extension string, and the shared
  provider compatibility check consumes provider behavior. Touched Ollama
  serving and dedicated llama.cpp launch paths now use the same provider-owned
  `ExecutableArtifactFormat::from_path` parser.
- 2026-05-11: Updated `rust/crates/pumas-rpc/src/handlers/README.md` for the
  extracted OpenAI gateway and serving adapter modules. Existing provider README
  coverage now includes the executable artifact parser. No new source
  directories were introduced for launch strategy, route migration, or frontend
  provider descriptors in this slice.
- 2026-05-11: Extracted the llama.cpp router catalog's executable artifact
  projection into a small helper that returns `ExecutableArtifactFormat` plus
  path. Router catalog generation now consumes the same provider-owned artifact
  parser as serving validation instead of checking raw `.gguf` extensions in
  place. ONNX-specific model-library projections remain deferred to ONNX
  implementation slices.
- 2026-05-11: Made runtime-profile management-mode validation consume provider
  launch kinds. `ProviderBehavior::supports_management_mode` now maps
  `binary_process` and the then-reserved sidecar launch kind to managed profiles,
  and `external_only` to external profiles. This is the launch-strategy contract
  slice only; ONNX managed launch now needs the Rust in-process runtime strategy
  recorded by the 2026-05-11 re-plan.
- 2026-05-11: Reconciled Milestone 0 registry usage after the launch-kind
  validation slice. The built-in provider registry is now consumed by
  runtime-profile validation/capability projection, serving adapter selection,
  gateway endpoint routing, provider request model-id policy, alias defaulting,
  unload behavior, artifact compatibility, and launch-kind validation.
  Composition-root lifecycle ownership remains open.
- 2026-05-11: Added RPC composition-root ownership for provider behavior.
  `AppState` now owns the provider registry used by OpenAI gateway and serving
  handlers, replacing handler-local `ProviderRegistry::builtin()`
  construction. Core serving/runtime-profile services still construct built-ins
  internally and need a separate injection slice before the lifecycle ownership
  task is complete.
- 2026-05-11: Injected provider behavior into core serving validation.
  `ServingService` now owns a provider registry and `PumasApi` validates serving
  requests through that service instance instead of a static validator that
  constructs `ProviderRegistry::builtin()`. Added a focused test proving a
  composed registry controls artifact/provider validation. Runtime-profile
  validation remains the next core injection sub-slice.
- 2026-05-11: Injected provider behavior into core runtime-profile validation
  and made the primary API builder the composition root for the core provider
  registry. `RuntimeProfileService` now owns an injected registry, `PumasApi`
  builds one registry and passes it to both runtime-profile and serving
  services, and service-level default constructors no longer construct built-in
  registries in production code. Reusable provider clients and the managed
  launch-strategy abstraction remain separate open tasks.
- 2026-05-11: Started the reusable provider-client slice by extracting
  llama.cpp router serving HTTP operations into a reusable
  `LlamaCppRouterClient` owned by RPC `AppState`. The serving adapter now
  consumes the state-owned client and per-operation timeout policy instead of
  building `reqwest::Client` values inside request handling. Remaining direct
  provider-client construction is still present in Ollama serving/app handlers
  and must be handled in a separate slice before the provider-client task is
  complete.
- 2026-05-11: Completed the existing-provider client reuse slice for Ollama.
  `OllamaClient` now accepts reusable `OllamaHttpClients`, RPC `AppState` owns
  an `OllamaClientFactory`, and Ollama serving/app handlers obtain endpoint
  clients from that state-owned factory instead of constructing client stacks
  in request handling. The remaining `reqwest::Client::builder()` usage in RPC
  handlers is plugin proxy code, not runtime provider serving.
- 2026-05-11: Added contract serialization coverage for provider capabilities
  projected from provider behavior and provider-scoped served-model status
  fields. The tests round-trip the Rust DTOs through JSON so provider,
  provider-mode, device-mode, profile, endpoint, and served identity wire names
  are locked before ONNX adds new values.
- 2026-05-11: Moved runtime-profile provider-specific validation dispatch
  behind a composed `RuntimeProviderAdapters` registry owned by
  `RuntimeProfileService`. `PumasApiBuilder` now composes both provider
  behavior and runtime-profile adapters, and a focused test proves injected
  adapters control validation. Runtime-profile launch-spec derivation still has
  provider-specific branching and remains part of the managed launch-strategy
  abstraction task.
- 2026-05-11: Added the typed runtime-profile launch strategy abstraction under
  `rust/crates/pumas-core/src/runtime_profiles/`. Managed Ollama and llama.cpp
  launch specs now carry `RuntimeProfileLaunchStrategy::BinaryProcess(...)`,
  external profiles map to `ExternalOnly`, and the previously reserved sidecar
  path is superseded for ONNX by the Rust in-process runtime strategy recorded
  by the re-plan. Runtime lifecycle launch-config construction now consumes the
  typed strategy instead of matching provider ids directly. Added the directory
  README required for the new launch-strategy module.
- 2026-05-11: Moved managed launch target selection into provider behavior.
  `ProviderBehavior` now declares per-mode `ProviderManagedLaunchStrategy`
  values, runtime-profile launch-spec derivation consumes the composed provider
  registry to project those targets into `RuntimeProfileLaunchStrategy`, and
  provider contract serialization tests cover the new launch target wire shape.
  ONNX in-process runtime wiring remains deferred to the managed ONNX lifecycle
  slice.
- 2026-05-11: Moved runtime-profile launch version-manager selection onto
  provider behavior. `ProviderBehavior` now declares the managed runtime app id
  and existing launch failure messages, so the RPC launch handler no longer
  matches Ollama versus llama.cpp to find the version manager or shape those
  errors.
- 2026-05-11: Added frontend runtime provider descriptors in
  `frontend/src/utils/runtimeProviderDescriptors.ts`. Runtime profile settings
  now consume descriptor-owned provider labels, modes, and device modes;
  llama.cpp model-row compatibility and route mutations read the descriptor for
  provider identity and executable formats; and the serve dialog compatibility
  check now uses selected-provider descriptors instead of a hard-coded GGUF
  branch. ONNX UI/provider entries remain deferred to the ONNX app identity
  milestone.
- 2026-05-11: Moved serve-dialog default context size and launch-on-serve
  initial profile fallback onto runtime provider descriptors. Current llama.cpp
  behavior is preserved because it remains the only existing descriptor with
  launch-on-serve support and a default context size.
- 2026-05-11: Extracted managed runtime-profile launch-spec derivation into
  `rust/crates/pumas-core/src/runtime_profiles/launch_specs.rs`. The extracted
  module owns implicit port allocation, runtime directory derivation, existing
  provider launch args/env vars, and provider-behavior launch target projection.
  This reduces `runtime_profiles.rs` before ONNX launch wiring; route
  persistence/migration and frontend provider row decomposition remain open.
- 2026-05-11: Added Milestone 0 backend provider-path verification tests.
  Runtime-profile launch-spec derivation now has an explicit test proving a
  missing composed provider behavior blocks launch specs, and serving validation
  now has an explicit test proving artifact compatibility comes from composed
  provider behavior. Existing provider-scoped route and served-state tests cover
  same-model-id routing/unload behavior for separate providers.
- 2026-05-11: Extracted core serving placement validation into
  `rust/crates/pumas-core/src/serving/placement.rs` and updated the serving
  README module table/design notes. This reduces the serving service entrypoint
  before ONNX placement rules are added; route persistence/migration and
  frontend provider row/view-model decomposition remain open under the
  large-file split task.
- 2026-05-11: Moved core serving placement rule selection onto
  `ProviderBehavior::serving_placement_policy`. Existing Ollama profile-only
  placement behavior and llama.cpp router/dedicated placement behavior are
  preserved, while the validation entrypoint no longer selects placement rules
  by matching `RuntimeProviderId`.
- 2026-05-11: Extracted core serving gateway alias validation into
  `rust/crates/pumas-core/src/serving/gateway_alias.rs` and updated the serving
  README module table/design notes. Effective alias derivation remains exported
  through the serving module, but validation orchestration no longer owns alias
  character, path-segment, or duplicate-alias policy directly.
- 2026-05-11: Moved managed runtime path segments and implicit base ports into
  `ProviderBehavior`, and updated launch-spec derivation to consume those
  provider-owned values. Existing provider-specific env/arg construction still
  remains in launch-spec derivation and is recorded as a remaining Milestone 0
  cleanup before ONNX in-process runtime wiring.
- 2026-05-11: Updated managed launch-spec env/arg derivation to consume
  `RuntimeProfileLaunchStrategy` instead of matching directly on provider ids.
  Existing Ollama and llama.cpp launch output remains unchanged; non-binary
  runtime strategies fail explicitly until a provider lifecycle slice
  implements that target.
- 2026-05-11: Added provider-owned launch-on-serve support and moved stopped
  managed profile acceptance in serving validation onto `ProviderBehavior`.
  Existing Ollama remains rejected for stopped managed serve requests, and
  llama.cpp router/dedicated launch-on-serve behavior is preserved through the
  provider contract.
- 2026-05-11: Updated runtime-profile lifecycle launch preparation to consume
  `RuntimeProfileLaunchStrategy` for llama.cpp router/dedicated preset/model
  prep instead of matching on provider id plus provider mode. ONNX in-process
  runtime preparation remains unwired until the Rust ONNX lifecycle slice.
- 2026-05-11: Extracted runtime-profile route config initialization, one-way
  legacy route migration, and model-route validation into
  `rust/crates/pumas-core/src/runtime_profiles/route_config.rs`. The
  runtime-profile README now documents this persistence boundary. This reduces
  `runtime_profiles.rs` before ONNX route assignment while preserving the
  schema-2 provider-scoped route contract.
- 2026-05-11: Extracted the llama.cpp compatible-model list and row renderers
  into `frontend/src/components/app-panels/sections/LlamaCppModelLibraryList.tsx`
  and `frontend/src/components/app-panels/sections/LlamaCppModelRow.tsx`.
  Quick-serve config/error helpers now live in `llamaCppQuickServe.ts`, while
  `LlamaCppModelLibrarySection.tsx` keeps route persistence and serving
  orchestration. The section, list, and row are all below the component-size
  threshold, and the sections README records the boundaries.
- 2026-05-11: Closed the Milestone 0 large-file split checklist item. The named
  ONNX prerequisite split targets now have focused delegates: provider behavior,
  route config migration, serving adapters, gateway proxy helpers, runtime
  launch strategy/spec derivation, and frontend provider row/view-model
  components. Large unrelated legacy files remain outside this plan's ONNX
  write surface.
- 2026-05-11: Re-plan trigger accepted: ONNX Runtime will be hosted through
  Rust ONNX Runtime bindings rather than a Python sidecar. Removed the aborted
  uncommitted `onnx-server/` skeleton from the worktree and updated plan
  contracts, risks, milestones, and ADR language to target an in-process Rust
  ONNX provider/session manager. Candidate binding is `ort`, pending focused
  Rust dependency review and native-library packaging decision.
- 2026-05-11: Started Milestone 1 with a Rust-only ONNX provider/session
  skeleton in `rust/crates/pumas-core/src/onnx_runtime/`. The slice adds
  validated model-id, model-path, load, embedding request, execution-provider,
  status, response, and typed error contracts; a fake backend for load, unload,
  list/status, and deterministic embeddings; a semaphore-bound session manager;
  README traceability; and unit tests for root escape, extension validation,
  payload validation, fake backend ordering, unloaded embedding rejection, and
  unload cleanup. Real ONNX Runtime dependencies, composition-root wiring,
  gateway error bodies, and full cancellation/shutdown ordering remain open.
  Verification passed: `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`, and
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`.
- 2026-05-11: Added the Rust provider-contract slice for ONNX Runtime.
  `RuntimeProviderId::OnnxRuntime`, `RuntimeProviderMode::OnnxServe`,
  `ExecutableArtifactFormat::Onnx`, `ProviderLaunchKind::InProcessRuntime`,
  and the ONNX provider behavior are now in core contracts. The old reserved
  Python-sidecar launch target was removed from Rust provider/runtime-profile
  contracts. ONNX is declared embedding-only with `.onnx` artifact support,
  `/v1/models` and `/v1/embeddings` capability, session-manager unload policy,
  and in-process runtime launch strategy. RPC serving dispatch now returns
  explicit non-critical "not wired in this slice" errors for ONNX load/unload
  rather than falling through an existing provider path. Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml providers`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles`,
  and `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`.
- 2026-05-11: Synced frontend runtime-provider contracts with the new Rust ONNX
  provider enums. `RuntimeProviderId` now includes `onnx_runtime`,
  `RuntimeProviderMode` includes `onnx_serve`, runtime provider descriptors
  declare ONNX as `.onnx`/CPU embedding-only without llama.cpp placement
  controls, and runtime profile settings exhaustively consume the descriptor
  maps. Verification passed: `npm run -w frontend check:types` and
  `npm run -w frontend test:run -- runtimeProviderDescriptors`.
- 2026-05-11: Started Milestone 4 serving validation for ONNX. Core serving
  validation now accepts ONNX Runtime requests for running ONNX profiles when
  the model's primary executable artifact is `.onnx`, rejects unsupported
  artifacts through provider-owned compatibility, and rejects ONNX per-load
  `gpu_layers`/llama.cpp-style placement controls with generic provider
  messages instead of Ollama-specific wording. Load/unload remain explicitly
  unwired in RPC until the ONNX session-manager adapter slice. Verification
  passed: `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml serving`, and
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`.
- 2026-05-11: Wired the fake ONNX serving adapter through RPC `serve_model` and
  `unserve_model`. `AppState` now owns a bounded Rust ONNX session manager,
  `serving_onnx.rs` resolves validated `.onnx` primary artifacts under the
  model library root, loads/unloads through the fake session manager, and
  records/removes backend served status for `onnx_runtime`. `OnnxModelId` now
  accepts slash-delimited Pumas library model ids while rejecting empty,
  absolute, and traversal-style segments. Real ONNX Runtime execution,
  duplicate load idempotency, status reconciliation before record, and gateway
  embedding dispatch remain open. Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`, and
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`.
- 2026-05-11: Started Milestone 5 gateway routing for ONNX Runtime. Added the
  focused `openai_gateway_onnx.rs` adapter so served `onnx_runtime` models with
  no provider HTTP endpoint execute `/v1/embeddings` through the bounded Rust
  ONNX session manager instead of the proxy path. The adapter validates
  OpenAI-compatible string/string-array input, optional dimensions, and float
  encoding, returns OpenAI-compatible embedding JSON, and maps ONNX validation,
  not-loaded, and backend failures into bounded gateway error bodies. Existing
  gateway tests were moved to `openai_gateway_tests.rs` so the shared gateway
  module stays below the 500-line standards threshold after adding the ONNX
  dispatch point. No Python sidecar or new dependency was introduced, and no
  new re-plan trigger was found. Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`,
  and `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`.
- 2026-05-11: Added the M5 handler-contract test slice for ONNX gateway
  routing. `openai_gateway_tests.rs` now builds an isolated `AppState`, records
  a served ONNX model, loads the fake ONNX session where needed, and calls the
  OpenAI gateway handler for `/v1/embeddings` and `/v1/chat/completions`. The
  tests prove the public gateway handler returns OpenAI-compatible embedding
  JSON for a loaded ONNX session, rejects chat/completion routing for the
  embedding-only ONNX provider through endpoint capabilities, and maps an ONNX
  not-loaded backend failure to a bounded `model_not_found` error body.
  Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` and
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`.
- 2026-05-11: Completed the focused M5 gateway-routing test coverage slice for
  body limits and lookup errors. The public gateway handler tests now prove
  `/v1/embeddings` rejects an oversized body with HTTP 413 before JSON parsing
  or ONNX session dispatch, rejects an unknown model with HTTP 404, and rejects
  duplicate gateway aliases with HTTP 409 plus the duplicate-alias error code.
  Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` and
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`.
- 2026-05-11: Added structured ONNX gateway/provider boundary logging without
  request text or path leakage. The in-process ONNX embedding adapter logs
  provider id, served model id, gateway model, profile id, input count,
  dimensions, and backend error code, but not embedding input text, tokens,
  secrets, or full model paths. Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check` and
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`.
- 2026-05-11: Added an ONNX serving status reconciliation guard before durable
  served-state updates. After the Rust ONNX session manager reports a successful
  load, `serving_onnx.rs` now lists sessions and verifies the requested model is
  present before calling `record_served_model`; a mismatch returns a
  non-critical provider-load failure instead of publishing stale loaded status.
  Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml serving`.
- 2026-05-12: Moved ONNX serving session model-id selection onto provider
  behavior. `serving_onnx.rs` now asks the composed provider registry for the
  ONNX provider-side request model id before loading the Rust session, so an
  explicit gateway alias cannot become the ONNX session name accidentally.
  Focused RPC serving coverage proves the built-in ONNX policy keeps the
  library model id when a public alias is present. Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml serving`.
- 2026-05-12: Added ONNX serving idempotency and stale-status cleanup.
  Duplicate ONNX `serve_model` calls now confirm the provider session is still
  listed and then return the existing loaded status with
  `loaded_models_unchanged: true` without advancing the served-state cursor.
  ONNX unload now removes backend served status even when the session manager
  reports the model was already absent, preventing stale loaded status from
  surviving partial provider cleanup. Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml serving`.
- 2026-05-12: Added structured ONNX serving lifecycle logging for load,
  duplicate-load reuse, load validation failure, load backend failure, load
  status-confirmation failure, unload success, stale-session cleanup, and unload
  failure. Logged fields are limited to provider id, public model id,
  provider-side model id where applicable, profile id, gateway alias,
  embedding dimensions, and structured error codes/fields; logs intentionally
  omit full model paths, request payloads, secrets, and embedding input text.
  The ONNX serving tests were extracted to `serving_onnx_tests.rs` so
  `serving_onnx.rs` remains below the 500-line standards threshold after adding
  observability. ONNX runtime-profile restart is not yet implemented; the
  future lifecycle slice must instrument restart ownership when that workflow
  exists. Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving_onnx`,
  and `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`.
- 2026-05-12: Added ONNX serving boundary values for load and unload. The ONNX
  adapter now resolves and validates the executable ONNX artifact path,
  provider-side session model id, effective gateway alias, unload model id,
  profile id, and unload alias once before provider-session calls or
  served-state reconciliation helpers consume them. Existing Ollama and
  llama.cpp adapters may still need the same pattern where they consume raw
  request strings or ports; that follow-up is tracked separately in Milestone 4
  instead of being hidden inside the ONNX slice. Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving_onnx`,
  and `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`.
- 2026-05-12: Added serve-dialog ONNX route/profile selection. Runtime provider
  descriptors now declare whether implicit serving requires a saved route;
  ONNX Runtime requires one, while Ollama and llama.cpp keep their existing
  fallback behavior. The serve dialog still honors explicit profile choices,
  resolves saved routes by `(provider, model_id)`, selects the saved ONNX
  profile when present, and leaves the target unselected with a clear
  validation message when an ONNX-filtered serve dialog has no saved route.
  ONNX model-row selection remains deferred to the Milestone 6 ONNX panel
  slice, and backend/core default-profile fallback cleanup remains tracked as a
  separate M4 follow-up. Verification passed: `npm run -w frontend
  check:types` and `npm run -w frontend test:run -- ModelServeDialog
  runtimeProviderDescriptors`.
- 2026-05-12: Removed backend/core model endpoint default-profile fallback for
  ONNX Runtime. `ProviderBehavior` now declares whether model endpoint
  resolution may fall back to the global default profile; Ollama and llama.cpp
  preserve the existing fallback, while ONNX requires an explicit profile or a
  provider-scoped saved route. A focused runtime-profile regression test proves
  Ollama still resolves an unrouted model through the default endpoint and ONNX
  returns `runtime profile id is required` instead of selecting the global
  default. Verification passed: `cargo fmt --manifest-path rust/Cargo.toml
  --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml
  runtime_profile_service_does_not_default_onnx_model_endpoint_to_global_profile`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml
  runtime_profiles`, `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml
  providers`, `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml
  serving`, and `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml
  serving`.
- 2026-05-12: Added ONNX load workflow compensation for recoverable post-load
  failures. If the ONNX session manager loads a session but status confirmation
  fails, or if backend served-state recording fails after load, the serving
  adapter now attempts to unload the session and logs the compensation outcome
  with safe provider/model/profile/reason fields. This narrows stale
  session/state divergence for normal error paths; a broader cancellation audit
  across provider adapters remains tracked in Milestone 4. Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving_onnx`,
  and `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`.
- 2026-05-12: Reconciled completed M4 existing-provider adapter work. Earlier
  committed slices extracted Ollama and llama.cpp serving into
  `serving_ollama.rs`, `serving_llama_cpp.rs`, `serving_llama_cpp_router.rs`,
  and `serving_llama_cpp_shared.rs`, and moved load/unload dispatch through
  provider behavior rather than legacy two-provider fallbacks. Current focused
  verification for the surrounding M4 slices passed with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml serving` and `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, so the milestone
  status now records those extraction and existing-provider preservation tasks
  as complete.
- 2026-05-12: Completed the M1 ONNX session-manager shutdown-ordering slice.
  `OnnxSessionManager::shutdown` now marks the manager closed, waits for all
  operation permits with a caller-provided bounded timeout, unloads listed
  sessions while holding those permits, and rejects later load/list/unload/embed
  work with a typed backend error. The ONNX Runtime module README now documents
  this lifecycle contract. Verification passed: `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, and `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml onnx`.
- 2026-05-12: Started M2 dependency review without changing manifests or the
  lockfile. Added `dependency-review.md` with provisional Rust-owned candidates:
  `ort` `2.0.0-rc.12` for ONNX Runtime CPU execution, `tokenizers` `0.23.1`
  for local tokenizer JSON loading, and `ndarray` `0.17.2` only if direct
  post-processing ownership requires it beyond `ort` value handling or checked
  `Vec<f32>` code. The review records in-house alternatives, package strategy,
  native-library/release risks, and the verification required before a manifest
  slice may add dependencies. Verification is documentation/source review only;
  no build command was needed because no code or manifest files changed.
- 2026-05-12: Confirmed dependency metadata with `cargo info` for `ort`
  `2.0.0-rc.12`, `tokenizers` `0.23.1`, and `ndarray` `0.17.2`. The review now
  records `ort`'s Rust 1.88 requirement and default native binary/TLS features,
  `tokenizers` default `onig`/`progressbar`/`esaxx_fast` features, and the
  narrower manifest decisions required before lockfile changes.
- 2026-05-12: Added the first M2 manifest/lockfile dependency slice. Workspace
  dependencies now declare `ort` `2.0.0-rc.12` with explicit CPU-first default
  equivalents (`std`, `ndarray`, `tracing`, `download-binaries`, `copy-dylibs`,
  `tls-native`, `api-24`) and `tokenizers` `0.23.1` with only `onig`;
  `pumas-core` is the only workspace crate that consumes them. `ndarray`
  remains transitive through `ort` rather than direct. Verification passed:
  `rustc --version` (`1.92.0`), `cargo check --manifest-path
  rust/crates/pumas-core/Cargo.toml`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo tree --manifest-path
  rust/crates/pumas-core/Cargo.toml -i ort`, `cargo tree --manifest-path
  rust/crates/pumas-core/Cargo.toml -i tokenizers`, and `cargo tree
  --manifest-path rust/crates/pumas-core/Cargo.toml -i ndarray`. `cargo audit`
  is unavailable because `cargo-audit` is not installed; advisory audit remains
  open before release.
- 2026-05-12: Added an ONNX Runtime module-size guard before continuing M2
  execution work. The existing fake backend and unit tests moved out of
  `onnx_runtime/mod.rs` into focused `fake.rs` and `tests.rs` modules, reducing
  the ONNX contract entrypoint below the 500-line standards threshold before
  tokenizer/session loading code is added. Verification passed: `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml onnx`, and file-size
  evidence: `mod.rs` 489 lines, `fake.rs` 116 lines, `tests.rs` 158 lines.
- 2026-05-12: Added the M2 tokenizer loading/tokenization slice. `OnnxTokenizer`
  now resolves `tokenizer.json` next to the already validated `.onnx` model
  file, canonicalizes it under the configured model root before parsing it with
  the Rust `tokenizers` crate, and returns ordered `i64` input-id and
  attention-mask rows for bounded embedding inputs. Tokenization rejects empty
  and over-limit tokenized inputs before tensor construction and uses checked
  accumulation for total token counts. Verification passed: `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml onnx`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size
  evidence: `mod.rs` 491 lines, `tokenizer.rs` 128 lines, `tests.rs` 248 lines.
- 2026-05-12: Added the M2 real ONNX session-loader boundary. `OnnxRuntimeSession`
  now combines a validated load request, sibling tokenizer loading, explicit
  CPU execution-provider setup, bounded ONNX Runtime session thread options,
  and input/output name introspection without yet running inference or changing
  serving/gateway behavior. Focused tests cover the validated model-directory
  contract and typed backend failure for invalid ONNX bytes. A successful
  real-model smoke remains open until a known ONNX embedding fixture is
  available. Verification passed: `cargo fmt --manifest-path rust/Cargo.toml
  --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size evidence: `mod.rs`
  493 lines, `real.rs` 104 lines, `tokenizer.rs` 128 lines, `tests.rs` 268
  lines.
- 2026-05-12: Added the M2 pure embedding postprocess strategy. The new
  postprocess module covers masked mean pooling, optional layer normalization,
  optional Matryoshka truncation, optional L2 normalization, checked response
  size calculation, one output row per tokenized input row, and deterministic
  tolerance-based numerical tests. Real ONNX output tensor selection remains
  open until inference consumes actual session outputs. Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml onnx`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size
  evidence: `mod.rs` 498 lines, `postprocess.rs` 275 lines, `real.rs` 104
  lines, `tokenizer.rs` 128 lines, `tests.rs` 268 lines.
- 2026-05-12: Confirmed the next M2 slice before implementation: reduce the
  ONNX Runtime entrypoint below the standards threshold before adding real
  inference wiring. The session backend trait and bounded session manager moved
  from `onnx_runtime/mod.rs` into focused `manager.rs`, keeping public exports
  stable while freeing the entrypoint for later contract-only additions.
  Verification passed: `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`, and
  file-size evidence: `mod.rs` 372 lines, `manager.rs` 136 lines.
- 2026-05-12: Discovered the local Nomic embedding ONNX package stores
  `onnx/model_fp16.onnx` below a package root that contains `tokenizer.json`
  and `config.json`. The tokenizer contract now searches from the ONNX file
  directory up through the validated model root for `tokenizer.json`, still
  canonicalizing and rejecting root escapes before parsing. Verification
  passed: `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`, and
  file-size evidence: `mod.rs` 372 lines, `tokenizer.rs` 156 lines,
  `tests.rs` 293 lines.
- 2026-05-12: Added model-package config discovery for real ONNX session
  loading. `OnnxModelConfig` reads `config.json` from the same validated
  package scope as tokenizer discovery, derives source embedding dimensions
  from agreeing `hidden_size`/`n_embd` values, and rejects missing, malformed,
  or conflicting dimension metadata. `OnnxLoadOptions::default()` now delegates
  dimensions to real model config, while the fake backend keeps its deterministic
  8-dimensional fallback. Verification passed: `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size evidence: `mod.rs` 376
  lines, `config.rs` 75 lines, `package.rs` 55 lines, `real.rs` 131 lines,
  `tokenizer.rs` 110 lines, `tests.rs` 361 lines.
- 2026-05-12: Added an opt-in real fixture smoke for
  `OnnxRuntimeSession::load`. The normal focused suite skips the fixture unless
  `PUMAS_ONNX_REAL_MODEL_ROOT` is supplied; the local Nomic package smoke was
  run with absolute root
  `/media/jeremy/OrangeCream/Linux Software/repos/owned/ai-systems/Pumas-Library/shared-resources/models/embedding/nomic_bert/nomic-ai--nomic-embed-text-v1_5__files_0a032b7277be`
  and `PUMAS_ONNX_REAL_MODEL_PATH=onnx/model_fp16.onnx`, validating 768
  dimensions, `input_ids`, `attention_mask`, and non-empty outputs. Verification
  passed: `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml
  real_session_loader_smokes_optional_real_fixture -- --nocapture` with the
  env vars above, and `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml
  onnx`. File-size evidence: `mod.rs` 376 lines, `tests.rs` 392 lines.
- 2026-05-12: Added the FP16 extraction dependency slice before real inference.
  The selected local Nomic ONNX fixture is `model_fp16.onnx`, and `ort`
  requires its `half` feature to extract `f16`/`bf16` output tensors. Workspace
  dependencies now enable `ort/half` and add direct `half` `2.7.1` consumption
  only in `pumas-core`, the Rust ONNX execution owner. Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`, `cargo check
  --manifest-path rust/crates/pumas-core/Cargo.toml`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml onnx`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`, and `cargo tree
  --manifest-path rust/crates/pumas-core/Cargo.toml -i half`. `cargo audit`
  remains unavailable in this environment.
- 2026-05-12: Added the first real ONNX inference backend slice.
  `RealOnnxEmbeddingBackend` now owns loaded `OnnxRuntimeSession` values behind
  the existing bounded session-manager contract. `OnnxRuntimeSession::embed`
  tokenizes requests, pads `input_ids` and `attention_mask`, supplies optional
  `token_type_ids` when the graph declares that input, runs ONNX Runtime,
  selects a named output or first floating tensor, extracts `f32`/`f16`/`bf16`
  hidden states with checked shape/value counts, applies the existing mean-pool
  postprocessor, and returns one embedding row per input. The opt-in local
  Nomic FP16 smoke produced two ordered 256-dimensional finite embeddings from
  real ONNX Runtime inference. Verification passed: `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `PUMAS_ONNX_REAL_MODEL_ROOT=<absolute
  local Nomic package> PUMAS_ONNX_REAL_MODEL_PATH=onnx/model_fp16.onnx cargo
  test --manifest-path rust/crates/pumas-core/Cargo.toml
  real_backend_embeds_optional_real_fixture -- --nocapture`, and `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`. File-size evidence:
  `mod.rs` 378 lines, `real.rs` 452 lines, `real_backend.rs` 71 lines,
  `tests.rs` 435 lines. New risk recorded: split `real.rs` before adding more
  real-inference responsibilities.
- 2026-05-12: Completed the no-behavior real-inference module split required
  by the standards risk above. Tokenized input padding moved to `tensors.rs`,
  and output tensor selection/dtype extraction/shape conversion moved to
  `output.rs`. `real.rs` now owns session load/run orchestration only and is
  reduced from 452 to 219 lines before serving integration begins. Verification
  passed: `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`,
  `PUMAS_ONNX_REAL_MODEL_ROOT=<absolute local Nomic package>
  PUMAS_ONNX_REAL_MODEL_PATH=onnx/model_fp16.onnx cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml real_backend_embeds_optional_real_fixture
  -- --nocapture`, and `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml onnx`. File-size evidence: `mod.rs` 380
  lines, `real.rs` 219 lines, `output.rs` 178 lines, `tensors.rs` 60 lines,
  `real_backend.rs` 71 lines, `tests.rs` 435 lines.
- 2026-05-12: Wired the RPC composition root to the real Rust ONNX backend
  without changing handler contracts. `OnnxEmbeddingBackendKind` now lives in
  `pumas-core` and delegates to either fake or real backends; production RPC
  state constructs `OnnxEmbeddingBackendKind::real()`, while focused RPC tests
  explicitly construct `OnnxEmbeddingBackendKind::fake()` for deterministic
  fake embedding behavior. Verification passed: `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml onnx`, and `PUMAS_ONNX_REAL_MODEL_ROOT=<absolute
  local Nomic package> PUMAS_ONNX_REAL_MODEL_PATH=onnx/model_fp16.onnx cargo
  test --manifest-path rust/crates/pumas-core/Cargo.toml
  real_backend_embeds_optional_real_fixture -- --nocapture`. File-size
  evidence: `real_backend.rs` 124 lines, RPC `server.rs` 344 lines,
  `openai_gateway_tests.rs` 422 lines, `serving_onnx_tests.rs` 161 lines.
- 2026-05-12: Added the real Rust ONNX gateway facade smoke. The focused RPC
  gateway test helper now serializes its test registry override and injects
  either fake or real ONNX backends into isolated `AppState`. The opt-in
  `openai_proxy_smokes_real_onnx_embedding_fixture` test loads the local Nomic
  FP16 package through the real ONNX backend, records backend-owned served
  status, calls the public `/v1/embeddings` gateway handler with the public
  alias, and verifies HTTP 200, OpenAI-compatible JSON, 256 finite embedding
  values, and non-zero token usage. Verification passed:
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`, and
  `PUMAS_ONNX_REAL_MODEL_ROOT=<absolute local Nomic package>
  PUMAS_ONNX_REAL_MODEL_PATH=onnx/model_fp16.onnx cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml
  openai_proxy_smokes_real_onnx_embedding_fixture -- --nocapture`. The focused
  RPC gateway commands require permission to bind PumasApi's local loopback IPC
  listener in this sandbox; a sandboxed run failed with `Operation not
  permitted` before the same test was rerun with that allowance. A broader
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx` run also
  exposed existing `serving_onnx_tests` process-global registry leakage under
  parallel execution (`database is locked`); that helper now uses the same
  shared serialized isolated registry override pattern as gateway tests.
  Follow-up verification passed:
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`.
- 2026-05-12: Completed the M5 gateway timeout/error mapping test slice.
  Gateway handler coverage now proves malformed JSON is rejected before model
  lookup/provider dispatch, upstream provider error status and JSON bodies are
  preserved through the proxy response, and provider timeouts map to bounded
  Pumas-shaped gateway errors. The timeout test uses paused Tokio time to
  advance the existing 120-second gateway endpoint policy without a real-time
  sleep. Verification passed: `cargo fmt --manifest-path rust/Cargo.toml --all
  -- --check` and `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml
  openai_gateway`.

## Commit Cadence Notes

- Commit the Rust ONNX provider skeleton and tests as the first verified slice.
- Commit Rust provider/profile contracts separately from frontend UI when
  feasible.
- Commit gateway routing with Rust tests before release validation.
- Keep code, tests, and documentation together when they describe one completed
  behavior.
- Follow `COMMIT-STANDARDS.md`.

## Optional Parallel Worker Plan

Use only if implementation is parallelized.

Parallel work is allowed only after Milestone 0 freezes the shared contracts and
the integration branch is clean. Shared contracts, persisted schemas, plugin
metadata, route DTOs, lockfiles, launcher behavior, and ADRs are serial
integration files unless one explicit owner is assigned for the current wave.

| Owner/Agent | Primary Write Set | Allowed Adjacent Write Set | Forbidden/Shared Files | Output Contract | Handoff Checkpoint |
| ----------- | ----------------- | -------------------------- | ---------------------- | --------------- | ------------------ |
| Rust ONNX worker | `rust/crates/pumas-core/`, `rust/crates/pumas-rpc/` ONNX provider/session modules | Rust README updates and dependency manifest/lockfile changes when assigned | Frontend components, plugin metadata unless explicitly assigned | Rust ONNX provider/session boundary, fake and real-session tests, gateway adapter contracts | Rust focused tests pass, dependency ownership evidence recorded, endpoint contract documented. |
| Rust contract worker | `rust/crates/pumas-core/`, `rust/crates/pumas-rpc/` | `launcher-data/plugins/onnx-runtime.json`, Rust docs/README updates when assigned | Frontend components, ONNX execution internals unless explicitly assigned | Provider contracts, route migration/cleanup, serving, gateway tests | Rust focused tests pass, serialization/migration evidence recorded, no old route shape active. |
| Frontend worker | `frontend/src/` | Electron bridge/types only when required by the frozen contract | Rust DTOs, ONNX execution internals, plugin metadata unless explicitly assigned | ONNX app icon/panel/profile/model-route UI and tests | Typecheck/build/focused frontend tests pass, no optimistic backend-owned state introduced. |
| Integration owner | Plan, ADR, cross-layer docs, release notes, shared schema/manifest files | Coordination reports and final verification notes | None; this owner serializes cross-cutting edits | Contract sync, docs, release evidence, final verification | Full vertical acceptance path passes and worker outputs match assigned write sets. |

Worker reports must be written under this plan directory if workers are used:
`docs/plans/onnx-runtime-embedding-serving/reports/<worker>-<date>.md`.
Each report must list changed files, tests run, skipped checks, contract
assumptions, and any needed out-of-scope edits. Integrate one worker branch at a
time, verify after each integration, and clean up worker workspaces only after
their commits are reachable from the integration branch and no uncommitted
changes remain.

## Re-Plan Triggers

- The available ONNX model package does not include enough tokenizer/config
  files for local tokenization.
- `nomic-embed-text-v1.5` ONNX exports require model-specific custom ops or
  output handling that cannot be represented by a generic Rust embedding
  provider.
- ONNX Runtime GPU packaging differs enough by platform to require separate CPU
  and GPU plugin/runtime profiles.
- The Pumas gateway cannot safely route embedding-only providers without the new
  provider capability model fully replacing path/provider dispatch.
- The provider capability boundary grows enough to require a separate runtime
  provider registry refactor.
- Any standards compliance gate cannot be satisfied in the current architecture
  without expanding the blast radius beyond the affected systems named in this
  plan.
- A boundary contract cannot name one owner, runtime validator/decoder,
  producer test, consumer test, and persisted compatibility policy.
- New extracted modules, source directories, or provider descriptors cannot
  stay within standards file-size/responsibility thresholds without a broader
  decomposition plan.
- The frontend generic app panel cannot support runtime profiles without
  duplicating provider-specific panel behavior.
- The hard-coded app registry, managed-app decoration, selected-version state,
  or app-panel renderer cannot represent ONNX Runtime cleanly without replacing
  the app registry approach.
- Rust `AppId`, plugin metadata, version-manager registration, and frontend app
  descriptors cannot be kept in sync with focused drift tests.
- Reusable provider clients cannot be injected through the provider registry or
  gateway composition root without a broader RPC server state refactor.
- Provider-scoped model routes reveal a broader route/default-profile redesign
  is required before ONNX route assignment can be implemented cleanly.
- Dependency evaluation finds ONNX Runtime Rust packaging, transitive
  dependency cost, license, or CPU/GPU split is not acceptable for Rust
  ownership.
- Required lifecycle/concurrency guarantees require a broader process manager
  refactor than this feature can safely include.
- Cross-platform launch or path handling cannot be expressed through existing
  launcher/process abstractions without scattering platform checks.
- Required frontend accessibility or event-driven state constraints conflict
  with the current app-panel architecture.
- Runtime-profile schema migration/cleanup cannot remove the old global route
  shape without unacceptable data loss.
- External apps require LAN access or authentication behavior beyond the
  existing loopback-first gateway policy.
- Emily needs a different embedding dimension than the served model provides,
  implying a memory schema migration.

## Recommendations

- Recommendation 1: Keep ONNX Runtime separate from llama.cpp. Both can expose
  OpenAI-compatible endpoints, but they own different artifact formats,
  lifecycle behavior, and dependency footprints.
- Recommendation 2: Prefer pointing external apps at the Pumas gateway instead
  of raw provider endpoints. This keeps aliases, served state, and future auth
  policy in one place.
- Recommendation 3: Keep the first slice embedding-only. Add reranking or other
  ONNX tasks later behind explicit provider capability flags.
- Recommendation 4: Do Milestone 0 before ONNX execution integration. It reduces the
  risk that ONNX support cements current Ollama-vs-llama.cpp assumptions.
- Recommendation 5: Keep the first complete vertical slice managed in-process
  runtime first because the expected UX is setup, profile save, model route
  assignment, and serving from the ONNX app panel.
- Recommendation 6: Treat the ONNX model library panel as a provider-specific
  sibling of `LlamaCppModelLibrarySection`, not as a generic `ModelManager`
  variant. The user workflow is route/profile assignment plus serving, which is
  closer to llama.cpp than to the generic model download/library surface.

## Completion Summary

### Completed

- Initial implementation hygiene check completed. Pre-existing dirty
  model-library implementation files were committed separately before ONNX
  implementation resumed.
- Provider-model documentation setup completed in
  `docs/adr/0001-onnx-runtime-provider-model.md`.
- Backend provider behavior contract and built-in provider registry added under
  `rust/crates/pumas-core/src/providers/`.
- Runtime-profile validation now consumes provider behavior for provider mode
  and management-mode support.
- Runtime profile capability DTOs now project from provider behavior values.
- Provider-scoped runtime route contracts and runtime-profile config migration
  completed for existing providers.
- Serving artifact format validation now derives supported formats from
  provider behavior.
- `unserve_model` unload selection now uses recorded served provider instead of
  falling back to Ollama for every non-llama.cpp status.
- Serving and gateway provider-side request model ids now derive from provider
  behavior policy.
- Gateway proxying now checks typed provider endpoint capabilities before
  forwarding OpenAI-compatible requests.
- OpenAI-compatible gateway proxying now uses a shared timeout-bound HTTP
  client owned by RPC server state.
- Gateway proxy routes now have explicit endpoint-specific body and timeout
  policies before provider forwarding.
- Served-model replace/find/unload identity is provider-scoped, and frontend
  unload requests send the served provider.
- Serving alias defaulting now derives from provider behavior policy.
- `unserve_model` dispatch now derives from provider unload behavior policy.
- `serve_model` dispatch now derives from provider serving adapter kind.
- OpenAI-compatible gateway handlers and proxy helpers now live in a focused
  `handlers/openai_gateway.rs` module instead of the oversized RPC handlers
  module.
- Ollama serving load/unload now lives in a focused
  `handlers/serving_ollama.rs` adapter module.
- llama.cpp serving now lives in focused adapter, router, and shared helper
  modules. Existing-provider serving adapter extraction is complete.
- Serving artifact compatibility now uses typed `ExecutableArtifactFormat`
  values from the API boundary through shared serving validation.
- Handler README traceability now covers the extracted gateway and serving
  adapter modules.
- llama.cpp router catalog compatibility projection now uses typed executable
  artifact values.
- Runtime-profile managed/external validation now derives from provider launch
  kinds, including the previously reserved sidecar kind now superseded for ONNX.
- Backend provider registry usage across existing runtime profiles, serving,
  gateway, compatibility, and launch-kind validation is complete.
- RPC gateway and serving handlers now consume the provider registry from
  `AppState`.
- Core serving validation now consumes the registry owned by `ServingService`.
- Core runtime-profile validation now consumes the registry owned by
  `RuntimeProfileService`, and the primary API builder owns registry
  composition for core services.
- llama.cpp router serving HTTP operations now consume a reusable
  composition-root-owned provider client.
- Existing Ollama and llama.cpp provider serving/app HTTP clients are now owned
  by RPC composition state or factories instead of being built in serving
  request handlers.
- Provider capability DTOs and provider-scoped served-model status fields now
  have focused JSON serialization/round-trip coverage.
- Runtime-profile provider-specific validation now dispatches through composed
  runtime provider adapters instead of a direct provider match.
- Runtime-profile launch specs now carry a typed launch strategy consumed by
  lifecycle launch-config construction for existing managed providers.
- Provider behavior now owns per-mode managed launch targets consumed by
  runtime-profile launch-spec derivation.
- Provider behavior now owns managed runtime app ids used by runtime-profile
  launch version-manager lookup.
- Frontend provider descriptors now centralize existing provider labels, modes,
  device modes, executable-format compatibility, and serve-dialog capability
  flags.
- Serve-dialog provider defaults and launch-on-serve profile fallback now read
  from frontend provider descriptors instead of hard-coded llama.cpp checks.
- Managed runtime-profile launch-spec derivation now lives in
  `runtime_profiles/launch_specs.rs`.
- Backend provider-path verification now covers composed-registry launch-spec
  derivation, provider-declared artifact compatibility, and the existing
  provider-scoped route/served-instance contracts.
- Core serving placement validation now lives in a focused
  `serving/placement.rs` module instead of the serving service entrypoint.
- Core serving placement rule selection now derives from provider behavior
  instead of provider-id dispatch.
- Core serving gateway alias validation and effective-alias derivation now live
  in a focused `serving/gateway_alias.rs` module.
- Provider behavior now owns managed runtime path segments and implicit base
  ports consumed by runtime-profile launch-spec derivation.
- Runtime-profile launch-spec env/arg derivation now consumes
  `RuntimeProfileLaunchStrategy` rather than provider-id dispatch.
- Serving validation now consumes provider-owned launch-on-serve policy instead
  of provider-id dispatch for stopped managed profiles.
- Runtime-profile lifecycle launch preparation now consumes launch strategy
  instead of provider-id/provider-mode dispatch for llama.cpp prep.
- Runtime-profile route config initialization, legacy route migration, and
  route validation now live in `runtime_profiles/route_config.rs`.
- llama.cpp compatible-model list and row rendering now live in
  `LlamaCppModelLibraryList.tsx` and `LlamaCppModelRow.tsx`, leaving each
  component below the size threshold before ONNX adds sibling UI.
- Milestone 0 large-file split work is complete for the named ONNX provider
  prerequisite surfaces.
- Rust ONNX re-plan is complete: the future Python sidecar milestone is
  replaced by a Rust ONNX provider/session skeleton, real ONNX Runtime
  execution moves into Rust dependency review/execution slices, and the aborted
  uncommitted Python sidecar files were removed before this documentation
  update.
- Rust ONNX Milestone 1 skeleton is in progress: `pumas-core` now owns
  validated ONNX provider/session contracts, a fake embedding backend, bounded
  session manager, README, and focused tests. Composition-root integration,
  gateway error shaping, and full shutdown/cancellation ordering remain open.
- Rust ONNX provider-contract work is in progress: core provider/runtime-profile
  enums and behavior now model ONNX as an embedding-only in-process runtime,
  and RPC serving has explicit non-critical ONNX not-yet-wired paths until the
  serving adapter slice connects the session manager.
- Frontend runtime-provider contracts are synced for ONNX provider ids, modes,
  labels, CPU-only device options, `.onnx` compatibility, and descriptor tests.
- Serving validation is in progress for ONNX: `.onnx` artifacts and running
  ONNX profiles validate through provider behavior, while unsupported artifacts
  and per-load placement overrides fail before provider execution.
- Fake ONNX serving adapter is in progress: RPC serving can load/unload ONNX
  served status through the Rust fake session manager, with real ONNX Runtime
  inference and gateway embedding dispatch still pending.

### Deviations

- None.

### Follow-Ups

- Any implementation-time standards deviation must be recorded here with an
  owner, mitigation, and revisit trigger.

### Verification Summary

- Pre-existing model-library dirty slice verified before commit with focused
  migration dry-run tests and Rust workspace formatting. ONNX plan verification
  for the documentation slice is limited to link/file existence and pre-commit
  Markdown hygiene.
- Provider registry slice verified with `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml providers` and `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`.
- Runtime-profile provider-registry validation slice verified with the focused
  runtime profile tests recorded for that slice.
- Runtime profile capability projection slice verified with the focused runtime
  profile tests recorded for that slice.
- Provider-scoped route slice verified with focused Rust runtime profile tests,
  the runtime route contract serialization test, pumas-rpc runtime profile
  build/test filter, frontend typecheck, Electron type validation, and focused
  frontend route/serve dialog tests.
- Provider artifact compatibility slice verified with focused provider and
  serving tests plus Rust formatting.
- Provider-based unload dispatch slice verified with `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml serving` and Rust
  formatting.
- Provider request model-id policy slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml providers`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`, and
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Gateway endpoint capability slice verified with `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml providers`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml openai_gateway`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`.
- Shared gateway HTTP client slice verified with `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml openai_gateway`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml gateway_http_client`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`.
- Gateway endpoint policy slice verified with `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml openai_gateway`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`.
- Provider-scoped served-instance unload slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml serving`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, `npm run -w
  frontend check:types`, `npm run -w frontend test:run -- useModelServingActions`,
  and `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Provider alias policy slice verified with `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml providers`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`.
- Provider unload behavior dispatch slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml providers`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`.
- Provider serving adapter kind dispatch slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml providers`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`.
- Gateway helper extraction slice verified with `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml openai_gateway`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`.
- Ollama serving adapter extraction slice verified with `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml serving` and `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`.
- llama.cpp serving adapter extraction slice verified with `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml serving` and `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`.
- Typed model compatibility slice verified with `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml providers`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml serving`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`.
- Handler README traceability slice is documentation-only; reviewed the updated
  module table and design notes against the extracted files.
- Router catalog compatibility projection slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles` and
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Launch-kind validation contract slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml providers`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles`, and
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.
- RPC provider registry composition slice verified with `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`.
- Core serving provider-registry injection slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml serving`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`.
- Core runtime-profile provider-registry injection slice verified with `cargo
  test --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml serving`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.
- llama.cpp router provider-client slice verified with `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml provider_clients`, `cargo
  test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo
  fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Ollama provider-client reuse slice verified with `cargo test --manifest-path
  rust/crates/pumas-app-manager/Cargo.toml ollama_client`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml ollama`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml provider_clients`, `cargo
  test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo
  fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Provider contract serialization slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles`, `cargo
  test --manifest-path rust/crates/pumas-core/Cargo.toml serving_contract`,
  `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml
  served_model_status_contract`, and `cargo fmt --manifest-path rust/Cargo.toml
  --all -- --check`.
- Runtime-profile provider-adapter dispatch slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles`, `cargo
  test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo
  fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Runtime-profile launch strategy slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml launch_strategy`, `cargo
  test --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Provider-owned managed launch target slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml providers`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml launch_strategy`, `cargo
  test --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles`,
  `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Provider-owned runtime app id slice verified with `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml providers`, and `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml runtime_profiles`.
- Frontend provider descriptor slice verified with `npm run -w frontend
  check:types` and `npm run -w frontend test:run --
  runtimeProviderDescriptors llamaCppLibraryViewModels
  LlamaCppModelLibrarySection RuntimeProfileSettingsSection ModelServeDialog`.
- Frontend serve-dialog descriptor cleanup slice verified with `npm run -w
  frontend check:types` and `npm run -w frontend test:run --
  runtimeProviderDescriptors ModelServeDialog LlamaCppModelLibrarySection
  llamaCppLibraryViewModels`.
- Runtime-profile launch-spec extraction slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles`, `cargo
  test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`, and `cargo
  fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Backend provider-path verification slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml
  runtime_profile_launch_specs_require_composed_provider_behavior`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml
  validation_uses_composed_provider_artifact_compatibility`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles`, `cargo
  test --manifest-path rust/crates/pumas-core/Cargo.toml serving`, and `cargo
  fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Core serving placement extraction slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml serving` and `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`.
- Provider serving placement policy slice verified with `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml providers`, and `cargo
  test --manifest-path rust/crates/pumas-core/Cargo.toml serving`.
- Core serving gateway alias extraction slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml serving` and `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`.
- Provider-owned managed runtime layout slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml providers`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles`, and
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Launch-strategy env/arg derivation slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles` and
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Provider launch-on-serve policy slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml providers`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml serving`, and `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`.
- Runtime-profile launch-preparation strategy slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles` and
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Runtime-profile route config extraction slice verified with `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml runtime_profiles` and
  `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`.
- Frontend llama.cpp row extraction slice verified with `npm run -w frontend
  check:types` and `npm run -w frontend test:run --
  LlamaCppModelLibrarySection llamaCppLibraryViewModels`.
- Large-file split status closure is documentation-only; evidence is the
  committed module extraction history and the focused verification recorded for
  each split slice.
- ONNX Runtime module-size guard verified with `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, and file-size evidence:
  `onnx_runtime/mod.rs` 489 lines, `fake.rs` 116 lines, `tests.rs` 158 lines.
- ONNX tokenizer loading/tokenization slice verified with `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml onnx`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size evidence:
  `onnx_runtime/mod.rs` 491 lines, `tokenizer.rs` 128 lines, `tests.rs` 248
  lines.
- ONNX real session-loader boundary verified with `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size evidence:
  `onnx_runtime/mod.rs` 493 lines, `real.rs` 104 lines, `tokenizer.rs` 128
  lines, `tests.rs` 268 lines.
- ONNX postprocess strategy verified with `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size evidence:
  `onnx_runtime/mod.rs` 498 lines, `postprocess.rs` 275 lines, `real.rs` 104
  lines, `tokenizer.rs` 128 lines, `tests.rs` 268 lines.
- ONNX session-manager extraction verified with `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size evidence:
  `onnx_runtime/mod.rs` 372 lines, `manager.rs` 136 lines.
- ONNX package-root tokenizer discovery verified with `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml onnx`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size
  evidence: `onnx_runtime/mod.rs` 372 lines, `tokenizer.rs` 156 lines,
  `tests.rs` 293 lines.
- ONNX model config discovery verified with `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size evidence:
  `onnx_runtime/mod.rs` 376 lines, `config.rs` 75 lines, `package.rs` 55 lines,
  `real.rs` 131 lines, `tokenizer.rs` 110 lines, `tests.rs` 361 lines.
- ONNX real fixture session-loader smoke verified with `cargo fmt
  --manifest-path rust/Cargo.toml --all -- --check`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml onnx`, `cargo test
  --manifest-path rust/crates/pumas-core/Cargo.toml
  real_session_loader_smokes_optional_real_fixture -- --nocapture` using the
  local Nomic model package env vars recorded above, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size evidence:
  `onnx_runtime/mod.rs` 376 lines, `tests.rs` 392 lines.
- ONNX FP16 dependency slice verified with `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo check --manifest-path
  rust/crates/pumas-core/Cargo.toml`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml onnx`, and `cargo tree --manifest-path
  rust/crates/pumas-core/Cargo.toml -i half`.
- ONNX real inference backend verified with `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `PUMAS_ONNX_REAL_MODEL_ROOT=<absolute
  local Nomic package> PUMAS_ONNX_REAL_MODEL_PATH=onnx/model_fp16.onnx cargo
  test --manifest-path rust/crates/pumas-core/Cargo.toml
  real_backend_embeds_optional_real_fixture -- --nocapture`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size
  evidence: `onnx_runtime/mod.rs` 378 lines, `real.rs` 452 lines,
  `real_backend.rs` 71 lines, `tests.rs` 435 lines.
- ONNX real-inference module split verified with `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `PUMAS_ONNX_REAL_MODEL_ROOT=<absolute
  local Nomic package> PUMAS_ONNX_REAL_MODEL_PATH=onnx/model_fp16.onnx cargo
  test --manifest-path rust/crates/pumas-core/Cargo.toml
  real_backend_embeds_optional_real_fixture -- --nocapture`, `cargo test
  --manifest-path rust/crates/pumas-rpc/Cargo.toml onnx`, and file-size
  evidence: `onnx_runtime/mod.rs` 380 lines, `real.rs` 219 lines,
  `output.rs` 178 lines, `tensors.rs` 60 lines, `real_backend.rs` 71 lines,
  `tests.rs` 435 lines.
- ONNX RPC real-backend composition verified with `cargo fmt --manifest-path
  rust/Cargo.toml --all -- --check`, `cargo test --manifest-path
  rust/crates/pumas-core/Cargo.toml onnx`, `cargo test --manifest-path
  rust/crates/pumas-rpc/Cargo.toml onnx`, `PUMAS_ONNX_REAL_MODEL_ROOT=<absolute
  local Nomic package> PUMAS_ONNX_REAL_MODEL_PATH=onnx/model_fp16.onnx cargo
  test --manifest-path rust/crates/pumas-core/Cargo.toml
  real_backend_embeds_optional_real_fixture -- --nocapture`, and file-size
  evidence: `real_backend.rs` 124 lines, RPC `server.rs` 344 lines,
  `openai_gateway_tests.rs` 422 lines, `serving_onnx_tests.rs` 161 lines.

### Traceability Links

- Module README updated: `rust/crates/pumas-rpc/src/handlers/README.md`.
- Module README updated: `rust/crates/pumas-core/src/providers/README.md`.
- Module README added: `rust/crates/pumas-core/src/runtime_profiles/README.md`.
- Module README updated: `rust/crates/pumas-core/src/serving/README.md`.
- Module README updated: `rust/crates/pumas-core/src/onnx_runtime/README.md`.
- Module README updated: `frontend/src/utils/README.md`.
- Module README updated:
  `frontend/src/components/app-panels/sections/README.md`.
- ADR added/updated: `docs/adr/0001-onnx-runtime-provider-model.md`.
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: pending.
