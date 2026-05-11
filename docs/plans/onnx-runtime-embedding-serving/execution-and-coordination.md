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
  narrower ONNX sidecar endpoint contract lands.
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

## Commit Cadence Notes

- Commit the sidecar skeleton and tests as the first verified slice.
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
| Sidecar worker | `onnx-server/` | Sidecar README and sidecar-local dependency manifest/lock files | Rust DTOs, frontend types, root/workspace dependency manifests unless explicitly assigned | Python sidecar, validation, fake and real-session tests, README | Sidecar tests pass, dependency ownership evidence recorded, endpoint contract documented. |
| Rust worker | `rust/crates/pumas-core/`, `rust/crates/pumas-rpc/` | `launcher-data/plugins/onnx-runtime.json`, Rust docs/README updates when assigned | Frontend components, Python sidecar internals, lockfiles not owned by Rust slice | Provider contracts, route migration/cleanup, serving, gateway tests | Rust focused tests pass, serialization/migration evidence recorded, no old route shape active. |
| Frontend worker | `frontend/src/` | Electron bridge/types only when required by the frozen contract | Rust DTOs, sidecar internals, plugin metadata unless explicitly assigned | ONNX app icon/panel/profile/model-route UI and tests | Typecheck/build/focused frontend tests pass, no optimistic backend-owned state introduced. |
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
  output handling that cannot be represented by a generic embedding sidecar.
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
- Dependency evaluation finds ONNX Runtime packaging, transitive dependency
  cost, license, or CPU/GPU split is not acceptable for sidecar-local ownership.
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
- Recommendation 4: Do Milestone 0 before sidecar integration. It reduces the
  risk that ONNX support cements current Ollama-vs-llama.cpp assumptions.
- Recommendation 5: Keep the first complete vertical slice managed-sidecar
  first because the expected UX is setup, profile save, model route assignment,
  and serving from the ONNX app panel.
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

### Traceability Links

- Module README updated: `rust/crates/pumas-rpc/src/handlers/README.md`.
- Module README updated: `rust/crates/pumas-core/src/providers/README.md`.
- ADR added/updated: `docs/adr/0001-onnx-runtime-provider-model.md`.
- PR notes completed per `templates/PULL_REQUEST_TEMPLATE.md`: pending.
