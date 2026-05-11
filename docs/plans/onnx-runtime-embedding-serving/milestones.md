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
      sidecar reference.
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
      `/v1/models` and `/v1/embeddings` against a fake or fixture sidecar. Add
      the failing-first acceptance test at the earliest slice where the public
      gateway contract can be exercised.
- [x] Define provider capabilities/behavior for supported artifact formats,
      OpenAI gateway endpoints, provider modes, device/placement support,
      launch-on-serve support, unload behavior, and provider-side model id
      policy.
- [x] Create a backend provider registry used by runtime profiles, serving
      adapters, gateway routing, and launch strategy selection. The registry is
      the extension point for Ollama, llama.cpp, and ONNX Runtime.
- [ ] Establish the provider registry composition root and lifecycle owner.
      Feature modules may request provider behavior from the registry, but must
      not construct HTTP clients, process launchers, sidecar clients, or
      concrete provider implementations ad hoc.
- [ ] Build reusable provider HTTP clients and gateway clients at composition
      roots with explicit timeout/body/error policy. Provider serving adapters
      consume those clients; request handlers must not build clients directly.
- [ ] Include alias defaulting, served-instance identity, route identity,
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
- [ ] Add a typed managed launch strategy abstraction for binary process,
      Python sidecar, and external-only profiles. Use it for existing Ollama and
      llama.cpp launch behavior before adding ONNX managed launch behavior.
- [ ] Add frontend provider descriptors consumed by profile settings, compatible
      model lists, route mutations, and serve dialog selection. Move
      llama.cpp-specific route-row behavior behind provider-specific view models
      that feed shared route primitives.
- [ ] Split already-large files through narrow delegating modules before adding
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
- [ ] Add backend tests that prove Ollama and llama.cpp behavior now flows
      through the new provider behavior and provider-scoped route contracts,
      while unsupported provider/path combinations fail cleanly.
- [ ] Add contract serialization tests for provider capabilities and any new
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
ONNX sidecar supplies a narrower endpoint contract.
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
a `python_sidecar` launch kind for the future ONNX managed sidecar path, but
the process-launch abstraction itself remains pending.
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

### Milestone 1: ONNX Sidecar Skeleton

**Goal:** Add a standalone sidecar with validated control and OpenAI-compatible
embedding endpoint shape before wiring Pumas serving.

**Tasks:**
- [ ] Create `onnx-server/` with README, requirements, app factory, control API,
      OpenAI-compatible API, model manager, validation module, and tests.
- [ ] Keep the sidecar app factory as the composition root for Python service
      wiring. Route modules may read managers from request/app state, but must
      not create global model managers, ONNX sessions, tokenizers, or
      long-lived clients at import time.
- [ ] Document `onnx-server/README.md` with purpose, lifecycle, API consumer
      contract, structured producer contract, errors, timeout/retry behavior,
      and compatibility notes before expanding sidecar internals.
- [ ] Keep the sidecar source layout small and documented. Every non-obvious
      source directory under the sidecar gets a README using the standards
      template or an explicit `None` statement for inapplicable sections.
- [ ] Implement `GET /health`, `GET /api/status`, `POST /api/load`,
      `POST /api/unload`, `GET /v1/models`, and `POST /v1/embeddings`.
- [ ] Add centralized path, model-name, bind-host, port, batch, and token-limit
      validation.
- [ ] Parse incoming HTTP payloads into validated request objects before model
      manager/session code receives them. Do not duplicate validation regexes or
      path containment checks in handlers.
- [ ] Add loopback-only bind defaults, explicit LAN opt-in policy, optional
      token auth, read/request timeouts, and maximum request body/item limits.
- [ ] Implement a session abstraction that can be unit-tested with a fake ONNX
      backend before real ONNX Runtime integration.
- [ ] Add bounded inference concurrency so ONNX Runtime threading and Python
      request handling cannot create unbounded work under embedding load.
- [ ] Define sidecar shutdown ordering: stop accepting new load/inference work,
      cancel or drain queued work with bounded timeout, unload sessions, and
      report cleanup failures through logs/status rather than process exit only.
- [ ] Ensure sidecar startup/shutdown owns model sessions, queue/semaphore
      cleanup, and stale-load cancellation without relying on process exit as
      the only cleanup path.
- [ ] Return OpenAI-compatible error bodies for embedding endpoint failures.

**Verification:**
- `python -m unittest discover -s onnx-server/tests`
- `ruff check onnx-server`
- `ruff format --check onnx-server`
- Sidecar tests cover invalid path/root escape, invalid bind host, invalid
  payload shape, request-size limits, and shutdown cleanup.
- Tests use isolated temp roots, unique ports where applicable, and no shared
  mutable process-global state unless explicitly serialized.
- Sidecar tests run from the sidecar ownership boundary or package-scoped
  command and prove dependencies are declared by `onnx-server/`, not supplied
  incidentally by unrelated root tooling.
- Manual smoke with fake/session-backed model manager if real ONNX fixture is
  not yet available.

**Status:** Not started.

### Milestone 2: ONNX Embedding Execution

**Goal:** Run real ONNX embedding inference with explicit, configurable
post-processing semantics.

**Tasks:**
- [ ] Add sidecar-local dependencies for `onnxruntime`, `numpy`, and
      `transformers`.
- [ ] Record dependency justification for `onnxruntime`, `numpy`, and
      `transformers`: in-house alternative, maintenance/license, transitive
      cost, CPU/GPU package choice, and sidecar-local ownership.
- [ ] Pin sidecar dependencies in the owning sidecar manifest/lock strategy
      used by the repo, and verify sidecar-local install/test commands do not
      depend on unrelated root dependencies.
- [ ] Record dependency tree, license, security-audit, and package-size impact.
      If ONNX Runtime introduces separate CPU/GPU packages, document the chosen
      default and the re-plan trigger for GPU support.
- [ ] Load tokenizer and ONNX session from a validated model directory.
- [ ] Keep ONNX Runtime native-library/provider selection explicit in sidecar
      configuration or startup logs so CPU/GPU package behavior is observable
      and does not silently vary by platform.
- [ ] Tokenize string or string-array `input`.
- [ ] Run ONNX Runtime inference with bounded batch size and token length.
- [ ] Implement a configurable embedding postprocess strategy covering output
      tensor selection, pooling, optional layer normalization, optional
      Matryoshka truncation, and optional L2 normalization.
- [ ] Default conservatively from model metadata/config when possible and fail
      with explicit configuration errors when the output contract is ambiguous.
- [ ] Support optional `dimensions` only when the loaded model/postprocess
      strategy can produce a compatible vector length.
- [ ] Use checked arithmetic before tensor allocation, vector truncation, or
      response-size calculations derived from request payloads.
- [ ] Return one embedding row per input item.
- [ ] Add tests for response shape, vector dimensions, batch ordering, and
      rejected invalid dimensions.
- [ ] Add deterministic numerical tests with tolerances for fake/session-backed
      post-processing and shape tests for real ONNX fixtures. Do not make
      broad performance or quality claims without benchmark evidence.
- [ ] Add a throughput/resource-limit check for representative batch sizes if
      ONNX inference becomes a hot path or any performance claim is made.

**Verification:**
- `python -m unittest discover -s onnx-server/tests`
- Dependency tree/audit output recorded in Execution Notes or PR notes.
- Package-local dependency install/check command recorded in Execution Notes or
  PR notes.
- A local smoke call against a known ONNX embedding fixture:
  `POST /v1/embeddings` returns HTTP 200 and expected vector length.
- Resource-limit tests prove oversized batches/tokens/dimensions fail before
  unbounded allocation.

**Status:** Not started.

### Milestone 3: Plugin And Runtime Profile Contracts

**Goal:** Make ONNX Runtime a typed Pumas runtime provider without affecting
Ollama or llama.cpp profiles.

**Tasks:**
- [ ] Add `launcher-data/plugins/onnx-runtime.json`.
- [ ] Add `RuntimeProviderId::OnnxRuntime` and
      `RuntimeProviderMode::OnnxServe`.
- [ ] Add ONNX Runtime to the frontend app registry with a sidebar icon,
      display name, description, default connection URL/port, and status
      defaults.
- [ ] Update Rust `AppId`, version-manager registration, plugin metadata,
      frontend app registry, selected-version hooks, managed-app decoration,
      app-shell panel props, and panel renderer in one app identity slice unless
      Milestone 0 replaced them with a descriptor-driven composition root.
- [ ] Extend managed app decoration/state so the ONNX icon reflects installed,
      offline, running, starting, stopping, and error states from runtime
      profile or sidecar state.
- [ ] Extend selected app version/process state hooks only as far as the
      selected lifecycle slice requires. If ONNX uses runtime profiles instead
      of standalone process hooks, keep the icon state derived from profile
      statuses rather than adding duplicate process state.
- [ ] Register ONNX Runtime capabilities in the provider capability/behavior
      boundary created in Milestone 0.
- [ ] Remove assumptions that provider enums and runtime-profile DTOs are
      append-only. Replace route DTOs and provider behavior contracts cleanly
      where the old shape is wrong for ONNX.
- [ ] Update Rust and TypeScript contracts in the same logical slice and verify
      serde/JSON casing for every new enum value and field.
- [ ] Add or update executable schema/fixture tests for runtime profile
      snapshots, provider capabilities, route mutations, and plugin metadata
      before frontend or RPC consumers depend on the new fields.
- [ ] Update runtime profile validation, default profile creation policy,
      endpoint resolution, status snapshots, and provider-mode compatibility
      rules.
- [ ] Add managed launch specs for ONNX sidecar process lifecycle, PID/log
      paths, health URL, and environment variables.
- [ ] Extract runtime-profile launch strategy so managed profiles can launch
      either binary runtimes or Python sidecars without forcing ONNX through
      Ollama/llama.cpp binary constructors or generic lifecycle branches.
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

**Status:** Not started.

### Milestone 4: Serving Validation And Load/Unload

**Goal:** Let users serve ONNX embedding models through backend-owned serving
state.

**Tasks:**
- [ ] Extend serving validation so ONNX Runtime profiles accept primary `.onnx`
      embedding artifacts.
- [ ] Use provider behavior to decide whether a `.onnx` artifact is compatible
      with generic embedding serving; do not infer that every ONNX/custom ONNX
      app artifact belongs to the ONNX Runtime embedding provider.
- [ ] Reject non-ONNX artifacts and unsupported model types with non-critical
      domain errors.
- [ ] Validate ONNX placement through provider capabilities: reject llama.cpp
      specific `gpu_layers`, `tensor_split`, and `context_size` controls unless
      ONNX gains an explicit equivalent later.
- [ ] Use the provider-side model id policy from Milestone 0 so gateway aliases
      are not overloaded as sidecar session names accidentally.
- [ ] Parse raw serving requests into validated boundary types before provider
      adapters consume them. Internal load/unload code should not re-validate
      raw strings, ports, dimensions, or paths.
- [ ] Ensure load/unload operations do not split durable state updates across
      cancellation points unless the step is transactional, idempotent, or has
      explicit compensation.
- [ ] Instrument load/unload/restart workflows with tracing spans or equivalent
      structured logs at lifecycle owners so partial failures and cancellations
      are observable without reading sidecar internals.
- [ ] Resolve the effective ONNX serving profile from the saved runtime route
      when a model row or serve dialog does not provide an explicit profile.
      Explicit profile choices must override the saved route for that request.
- [ ] Resolve saved routes by `(provider, model_id)`, not by model id alone.
      Remove any default-profile fallback that would silently serve ONNX with
      the wrong provider when no ONNX route exists.
- [ ] Return a clear validation error when an ONNX model has no saved route and
      no explicit ONNX profile selection.
- [ ] Add ONNX provider adapter calls from `serve_model` to sidecar `/api/load`.
- [ ] Move existing Ollama and llama.cpp serving paths behind provider serving
      adapters before adding ONNX load/unload so the RPC handler only performs
      boundary parsing, validation orchestration, and response shaping.
- [ ] Confirm sidecar `/v1/models` includes the model before recording loaded
      status.
- [ ] Add unload support through sidecar `/api/unload` and served status
      removal.
- [ ] Make load and unload idempotent where possible: duplicate load returns
      the existing loaded state, duplicate unload returns an unchanged snapshot,
      and partial sidecar failures do not leave stale loaded status.
- [ ] Preserve user-visible Ollama and llama.cpp outcomes through the new
      provider-scoped route and provider behavior paths. Do not preserve their
      legacy internal dispatch or global-route implementation.

**Verification:**
- `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml serving`
- `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml serving`
- Tests include duplicate load/unload, sidecar load failure, profile restart,
  stale endpoint, invalid alias, missing ONNX route, explicit profile override,
  provider-scoped route resolution, absence of default-profile fallback for
  ONNX, and invalid artifact cases.
- Replay/recovery tests cover persisted provider-scoped routes and served-state
  cleanup after process failure or app restart.
- Affected integration tests are run with normal parallelism enabled and repeated
  at least once to detect temp root, port, environment, or persisted-state
  leakage.
- Sidecar load/unload smoke test against a real or fixture ONNX embedding model.

**Status:** Not started.

### Milestone 5: Pumas Gateway Routing

**Goal:** Expose served ONNX models to external applications through the
existing Pumas `/v1` gateway.

**Tasks:**
- [ ] Update gateway provider routing for `onnx_runtime`.
- [ ] Ensure provider request model id uses the sidecar model name/alias needed
      by `/v1/embeddings`.
- [ ] Move provider request model-id rewriting out of the gateway helper and
      into provider behavior. The gateway should not match on individual
      providers to decide how to rewrite `model`.
- [ ] Proxy `/v1/embeddings` to ONNX sidecar endpoint with bounded request body
      behavior preserved.
- [ ] Validate OpenAI-compatible request JSON at the gateway boundary before
      dispatch, including model field shape, endpoint support, body limit, and
      provider capability.
- [ ] Add endpoint-specific body-limit tests so `/v1/embeddings` rejects
      oversized embedding payloads before contacting the sidecar.
- [ ] Use provider endpoint capabilities to keep `/v1/chat/completions` and
      `/v1/completions` unavailable for ONNX embedding-only models unless a
      future provider capability says otherwise.
- [ ] Reuse the shared gateway HTTP client from Milestone 0.
- [ ] Ensure gateway request handling uses bounded body reads, connection
      limits/timeouts, and no per-request client construction.
- [ ] Preserve timeout and error mapping semantics so provider failures return
      bounded OpenAI-compatible error bodies and do not hang external callers.
- [ ] Add request correlation or structured logging at the gateway/provider
      boundary without logging embedding input text, tokens, secrets, or full
      model paths.
- [ ] Add gateway tests for success, unknown model, ambiguous alias, and
      provider error pass-through.

**Verification:**
- `cargo test --manifest-path rust/crates/pumas-rpc/Cargo.toml openai_gateway`
- Gateway tests include unsupported endpoint, provider timeout, malformed JSON,
  body too large, duplicate alias, and provider error pass-through.
- Gateway tests prove request handlers use the shared provider/gateway client
  path and do not construct per-request HTTP clients.
- Gateway acceptance test exercises the real Pumas `/v1` facade instead of the
  raw ONNX sidecar endpoint.
- Manual curl:
  `GET /v1/models` includes the ONNX model alias.
- Manual curl:
  `POST /v1/embeddings` returns OpenAI-compatible embedding JSON.

**Status:** Not started.

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
- [ ] Add `onnx-server/README.md` with sidecar lifecycle, endpoints, limits,
      and troubleshooting.
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
      sidecar, for normal usage.

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
- [ ] Run focused Python, Rust, and frontend checks from prior milestones.
- [ ] Run package-local sidecar lint/format/test/install checks from the
      sidecar owner, not only from root convenience scripts.
- [ ] Build the release app.
- [ ] Launch the app and serve an ONNX embedding model through an ONNX Runtime
      profile.
- [ ] Verify external gateway calls from a separate process.
- [ ] Verify unload removes the model from `/v1/models`.
- [ ] Verify launcher-compatible install/build/release-smoke paths package or
      locate ONNX sidecar dependencies without mutating normal user state.
- [ ] Record dependency audit, license review, package size/transitive cost,
      and CPU/GPU packaging decision.
- [ ] Record release artifact impact, checksum/SBOM expectations, and whether
      the ONNX sidecar changes installer/package size or platform support.
- [ ] Update changelog or release notes for the user-visible ONNX serving
      feature.
- [ ] Record results in Execution Notes and Completion Summary.

**Verification:**
- `bash launcher.sh --build-release`
- Existing launcher release smoke command, if available for this repo.
- Release app smoke: ONNX model loaded, `/v1/models` lists alias,
  `/v1/embeddings` returns expected dimension, unload removes alias.

**Status:** Not started.
