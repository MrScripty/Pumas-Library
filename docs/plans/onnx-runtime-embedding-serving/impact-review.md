# Impact Review

## Standards Pass Findings By Code Area

- Runtime profile contracts currently use global model routes. Implementation
  must replace the contract, persisted JSON shape, Rust APIs, RPC bridge
  payloads, frontend route helpers, and route tests as one boundary slice.
- Serving validation currently mixes provider-specific placement and artifact
  assumptions. Implementation must move those rules behind provider behavior
  before adding ONNX-specific validation.
- Gateway routing currently couples OpenAI-compatible paths to provider-specific
  request model-id logic. Implementation must add endpoint capability checks,
  shared HTTP client ownership, bounded body handling, and error shaping before
  ONNX gateway exposure is accepted.
- Runtime lifecycle code currently has provider-specific launch branches.
  Implementation must add ONNX in-process runtime behavior through the same
  lifecycle owner, with stale state cleanup, health checks, cancellation, and
  managed state isolation.
- Frontend app registration is partly hard-coded. Implementation must update
  app registry, managed app decoration, selected-version/profile state,
  renderer selection, runtime profile settings, model-library view models, and
  serve dialog filters together so the ONNX icon and panel cannot drift from
  the backend provider contract.
- Model-library metadata currently recognizes a narrow format set.
  Implementation must make `.onnx` a first-class executable format through
  shared model metadata and compatibility helpers, not by adding scattered
  string checks.
- Rust ONNX execution is a new runtime boundary inside Pumas. It must start
  with a documented README contract, runtime validation module, explicit
  resource limits, lifecycle ownership, fake-session tests, and then real ONNX
  Runtime execution.

## Current Code Blast Radius Findings

This section records the concrete code effects found during the plan review.
Implementation must either resolve these findings in the named milestone or
record a re-plan trigger before code is merged.

| Code Area | Touched Surface | Finding | Required Design Response |
| --------- | --------------- | ------- | ------------------------ |
| Runtime profile DTOs | `rust/crates/pumas-core/src/models/runtime_profile.rs`, `frontend/src/types/api-runtime-profiles.ts`, Electron bridge types | `ModelRuntimeRoute` is keyed only by `model_id`; snapshots/config files serialize global routes; provider enums/modes only cover Ollama and llama.cpp. | Replace route identity with provider-scoped route records in one contract slice. Add ONNX provider/mode values at the same time. Do not keep the old route shape in active readers, writers, snapshots, or frontend helpers after migration/cleanup. |
| Runtime profile service | `rust/crates/pumas-core/src/runtime_profiles.rs` | The service owns provider capabilities, profile validation, route persistence, endpoint resolution, launch-spec derivation, event journal, port allocation, env args, and tests in one ~2k-line module. It also uses config write locks for read-heavy flows and has provider-specific matches for launch args, env vars, base ports, path segments, and validation. | Split provider behavior/launch policy into focused modules before ONNX is added. Keep route persistence/migration separate from provider behavior. Avoid expanding this file with ONNX branches; use provider behavior objects/registry and narrower config repository helpers. Review read/write locking so high-frequency snapshots/endpoint resolution do not serialize unnecessarily after config initialization. |
| Route resolution | `set_model_route`, `clear_model_route`, `resolve_model_endpoint_detail`, `model_route_auto_load`, frontend route maps | Route save/clear, endpoint lookup, and auto-load lookup all search by model id only. `resolve_model_endpoint_detail` falls back to `default_profile_id`, which is unsafe for ONNX because a model could be served by the wrong provider. | Route lookup, auto-load, clear, and frontend maps must use `(provider, model_id)`. ONNX serving must fail clearly without a saved ONNX route or explicit ONNX profile. Existing default-profile fallback may remain only for provider flows where capability policy explicitly allows it. |
| Serving core | `rust/crates/pumas-core/src/serving/mod.rs` | Validation hard-codes GGUF as the only executable artifact and has provider-specific placement matches. `find_served_model` and served-model identity do not include provider, which can become ambiguous once multiple providers can serve the same model id. | Move artifact compatibility, placement controls, launch-on-serve support, and endpoint support into provider capabilities. Include provider in served-model lookup/unload identity where ambiguity is possible. Add provider-scoped tests for same model id served by different providers. |
| RPC serving handler | `rust/crates/pumas-rpc/src/handlers/serving.rs` | `serve_model` dispatches directly on two providers. `unserve_model` treats any non-llama.cpp served model as Ollama. The file mixes RPC parsing, validation flow, alias derivation, provider HTTP clients, router restart policy, version-manager lookup, provider load/unload calls, GPU-runtime checks, and served-state recording in one ~1.3k-line handler. | Create a provider serving adapter boundary before ONNX. The RPC handler should parse/validate and call a provider adapter for load, unload, provider-side model id, alias defaulting, runtime support checks, and idempotency. Llama.cpp/Ollama behavior should move through the same adapter path so ONNX is not added as a third match branch. |
| Gateway proxy | `rust/crates/pumas-rpc/src/handlers/mod.rs`, `rust/crates/pumas-rpc/src/server.rs` | `/v1/chat/completions`, `/v1/completions`, and `/v1/embeddings` share one blind proxy path. Gateway creates a new `reqwest::Client` per request and rewrites `model` through a two-provider helper. There is no endpoint capability check before proxying or in-process provider execution. | Add provider endpoint capability checks, shared gateway policy, and provider gateway adapters. Unsupported ONNX endpoints must return Pumas-shaped OpenAI-compatible errors without entering ONNX Runtime. Provider-side model id mapping belongs to provider behavior, not a gateway match. |
| Runtime lifecycle and launcher | `rust/crates/pumas-core/src/api/state_runtime_profiles.rs`, `rust/crates/pumas-core/src/process/launcher.rs` | Runtime profile launch currently builds binary launch configs for Ollama/llama.cpp and applies llama.cpp-only preparation in lifecycle code. The launcher has separate binary constructors plus a Torch Python constructor that is not integrated into runtime profiles. ONNX should be an in-process Rust runtime strategy, not a Python sidecar or binary-process branch. | Extract a launch/runtime strategy abstraction that can represent binary processes, in-process runtimes, and external-only profiles through the same lifecycle owner. ONNX managed profiles must use in-process session state with explicit health/status, stale-state cleanup, and no provider-specific preparation in generic lifecycle code. |
| Model library metadata | `rust/crates/pumas-core/src/model_library/*`, `rust/crates/pumas-core/src/models/model.rs`, `frontend/src/utils/libraryModels.ts`, `frontend/src/types/apps.ts` | Backend model metadata already has some ONNX awareness, including compatible engine hints and existing custom ONNX runtime references. Frontend `ModelInfo.primaryFormat` only allows `gguf | safetensors`, and local serving helpers still treat executable serving as GGUF-only. There is also existing `kittentts` ONNX custom-runtime metadata that should not be conflated with the new generic ONNX Runtime embedding provider. | Define a shared executable-format helper/enum that includes ONNX and separates generic ONNX embedding serving from custom ONNX apps such as KittentTS. Update backend projection and frontend mapping together. Avoid scattered extension checks in UI and serving code. |
| Frontend app registry | `frontend/src/config/apps.ts`, `frontend/src/hooks/useManagedApps.ts`, `frontend/src/components/app-panels/AppPanelRenderer.tsx`, `frontend/src/components/AppShellPanels.ts` | App registration, managed app decoration, and panel rendering are hard-coded. Adding ONNX only to plugin metadata would not show a usable sidebar app/panel. | Add ONNX to every hard-coded registry path or replace the registry with a provider/app descriptor approach. Icon state should derive from ONNX runtime-profile state, not duplicate process state. |
| Frontend runtime profile controls | `RuntimeProfileSettingsShared.ts`, `RuntimeProfileSettings*` | Provider mode/device option maps are exhaustive for only two providers and encode provider labels in UI helpers. | Move display labels, provider modes, and placement controls behind typed provider capability/view-model helpers shared by profile settings, app panels, and serve dialog. |
| Frontend model route UI | `LlamaCppModelLibrarySection.tsx`, `llamaCppLibraryViewModels.ts`, `runtimeRouteMutations.ts`, `ModelServeDialog.tsx` | Llama.cpp route UI maps routes by model id, route mutation APIs do not include provider, and `ModelServeDialog` selects initial profile from model-only routes then falls back to running llama.cpp/default profile behavior. The llama.cpp section is 594 lines and mixes row rendering, route persistence, quick serving, searching, modal state, and serve-dialog transitions. | Extract provider-neutral route mutation helpers and provider-specific library view models. Build ONNX as a sibling view using shared route components where they reduce duplication. Update llama.cpp to provider-scoped routes in the same slice. The serve dialog must use provider/format compatibility helpers instead of GGUF/default-profile fallbacks. |
| Frontend state synchronization | `useRuntimeProfiles`, serving dialog, app-panel model lists | Runtime profile updates are event-driven and have stale-response handling; serving dialog fetches serving status once and does not subscribe to serving updates. ONNX panel served-state display may drift if it copies this one-shot pattern. | Reuse existing serving/runtime update subscriptions for ONNX loaded state. If a one-shot fetch remains, document why it is only for initial alias validation and not for backend-owned loaded-state display. |
| File size and maintainability | Large files listed by `wc -l` | `model_library/library.rs` (~12228), `runtime_profiles.rs` (~1984), `serving/mod.rs` (~1274), RPC `serving.rs` (~1279), RPC `handlers/mod.rs` (~1343), `process/launcher.rs` (~986), and `LlamaCppModelLibrarySection.tsx` (~594) already exceed standards thresholds. | Do not add ONNX responsibilities to these files except for narrow delegating calls. Extract provider behavior, gateway proxy helpers, runtime launch strategies, model-library format/compatibility helpers, and frontend row/view-model components first or within the same milestone. |

## Additional Provider-Model Impact Review

This pass was performed after the plan was tightened around composition roots,
executable contracts, and provider descriptors. These findings must be resolved
before implementation treats the provider refactor as complete.

| Code Area | Touched Surface | Finding | Required Design Response |
| --------- | --------------- | ------- | ------------------------ |
| App identity and version-manager composition | `rust/crates/pumas-core/src/config.rs`, `rust/crates/pumas-rpc/src/main.rs`, `launcher-data/plugins/*.json`, frontend app registry | Rust `AppId`, RPC version-manager initialization, launcher/plugin metadata, and frontend `DEFAULT_APPS` are separate hard-coded sources of app identity. Adding ONNX in only one source will produce a visible app without a version manager, or a version manager without usable UI. | Add an app/runtime descriptor ownership decision in Milestone 0 or Milestone 3. Either update every hard-coded source in one slice with tests, or create a descriptor-driven composition root that derives version-manager keys, default URLs, and frontend app metadata from one validated manifest. |
| Provider registry and composition root | `runtime_profiles.rs`, RPC serving/gateway handlers, lifecycle helpers | Existing provider adapters are zero-sized validators called directly from helpers. There is no composed provider registry that owns HTTP clients, launch strategies, endpoint capability policy, model-id rewriting, or serving adapters. | Milestone 0 must introduce a real registry/facade with one lifecycle owner. Feature code should request provider behavior by id; concrete clients and launch strategies are wired once at app/runtime startup. |
| Client construction and panic surface | `pumas-app-manager` clients, RPC gateway, RPC serving handler | Ollama/Torch clients build reqwest clients internally; gateway builds a fresh client per proxied request; serving handler builds short-lived clients for llama.cpp router operations. Some client constructors use `expect` for client-build failures. | Provider adapters should receive fallibly constructed, reusable clients from the composition root. Request handlers must not create clients per operation or rely on infallible client construction. |
| Torch sidecar reference pattern | `torch-server/*`, Torch process/client handlers | Torch is an app-specific Python sidecar and is not the ONNX implementation target. Copying it would add avoidable Python packaging/process lifecycle work when ONNX can run through Rust bindings. | Do not copy Torch's sidecar path. ONNX must use a Rust provider/session manager with explicit inference concurrency, shutdown ordering, cancellation/stale-load handling, Rust dependency checks, and runtime-profile integration through the provider model. |
| Gateway endpoint surface | `rust/crates/pumas-rpc/src/server.rs`, `handlers/mod.rs` | `/v1/chat/completions`, `/v1/completions`, and `/v1/embeddings` are all wired to one JSON proxy with a global 32 MiB body limit and global concurrency limit. Endpoint-specific capability and size policy are not represented. | Gateway helper must validate endpoint capability, method/body shape, endpoint-specific body limit, and provider support before proxying. Embedding requests should not inherit chat/completion assumptions or broader body limits without an explicit reason. |
| Model-library compatibility boundary | `model_library/library.rs`, `models/model.rs`, frontend `libraryModels.ts`, route/model selector helpers | Backend already detects `.onnx` and `onnx-runtime` engine hints, while frontend primary format excludes ONNX. `model_library/library.rs` is very large and owns custom runtime projections such as KittentTS. | Extract or localize executable-format and provider-compatibility projection logic instead of adding more ONNX branches to the large library file. Generic ONNX embedding compatibility must be separate from custom ONNX runtime projections and tested at backend/frontend projection boundaries. |
| Frontend app/provider identity | `types/apps.ts`, `config/apps.ts`, `useManagedApps.ts`, `useSelectedAppVersions.ts`, `AppShellPanels.ts`, `AppPanelRenderer.tsx` | Frontend app ids are plain strings, selected-version hooks enumerate apps manually, and app panel props enumerate each app. Adding ONNX by copying those branches increases drift. | Add a typed app/provider descriptor layer or an explicit decision to update all enumerations in one slice. ONNX status should derive from runtime-profile state if it is runtime-profile managed, not from a duplicate standalone process hook. |
| Frontend serve-dialog state | `ModelServeDialog.tsx`, `modelServeHelpers.ts`, `useServingStatus` | Serve dialog still uses one-shot serving status fetches for alias requirements and GGUF/llama-specific profile fallback logic. Provider filter does not by itself prevent fallback to the wrong provider when no route exists. | Move initial-profile selection, alias requirement, and placement controls behind provider compatibility helpers and serving-status subscriptions. ONNX missing-route behavior must be an explicit validation error, not a fallback to default profile. |

## Simplification Opportunities

- Introduce a single provider behavior contract that answers: supported local
  formats, serving endpoint capabilities, provider-side model id policy, alias
  default policy, placement controls, launch-on-serve behavior, managed launch
  strategy, unload strategy, and validation hooks.
- Keep runtime-profile persistence as a config repository/migration concern and
  keep provider behavior as policy. This makes route schema migration easier to
  reason about and avoids provider rules being hidden inside persistence code.
- Represent route identity, served instance identity, endpoint capability, and
  executable artifact format with typed values instead of repeated raw
  `String`/extension checks.
- Build ONNX and llama.cpp model-library panels from small shared route-row
  primitives only after provider-specific view models produce rows. Avoid a
  generic model manager that needs to understand every provider rule.
- Centralize gateway proxying into a reusable helper with shared client,
  endpoint capability validation, bounded body/error behavior, and provider
  model-id rewriting delegated to provider behavior.
- Separate generic ONNX embedding serving from custom ONNX app bindings
  already represented in model-library metadata. Shared metadata can expose
  `.onnx` format, but provider compatibility decides whether a concrete ONNX
  artifact is usable by the Rust embedding provider.
- Treat app identity, runtime-provider identity, and version-manager identity as
  separate contracts with an explicit mapping. A sidebar app can have install
  metadata without being a runtime-profile provider, and a runtime provider can
  use an in-process runtime without duplicating standalone process UI state.
- Build provider adapters around reused infrastructure clients that are created
  at composition roots. This keeps request handlers free of per-request client
  construction and makes timeout/body/error policy testable in one place.

## Performance And Maintainability Implications

- Gateway throughput will degrade under embedding load if each request builds a
  new HTTP client. The shared client and bounded body/timeout work is required
  before ONNX embeddings are exposed to external apps.
- Runtime profile snapshots and route resolution are read-heavy. The current
  load-or-initialize path takes a write lock even for reads; implementation
  should avoid making ONNX profile polling/serving resolution increase lock
  contention.
- ONNX inference must own bounded concurrency and request limits in the Rust
  provider/session manager. Pumas gateway limits are not a substitute for ONNX
  Runtime queue/thread limits.
- Provider behavior extraction reduces future cost for reranking or other ONNX
  tasks because endpoint capability, artifact compatibility, and placement
  policy become additive data/behavior rather than new handler branches.
- Updating llama.cpp to the provider-scoped route contract in the same slice is
  higher short-term cost, but it prevents parallel route semantics from living
  indefinitely in backend, frontend, and persisted config.
