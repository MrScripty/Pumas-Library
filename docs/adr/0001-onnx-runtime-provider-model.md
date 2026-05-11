# ADR 0001: ONNX Runtime Provider Model

## Status

Accepted.

## Date

2026-05-11.

## Context

The ONNX Runtime embedding serving plan needs Pumas to serve local `.onnx`
embedding models through the existing OpenAI-compatible `/v1` gateway. Current
runtime profile, serving, gateway, launcher, and frontend paths are shaped
around Ollama and llama.cpp. Adding ONNX as another branch would preserve the
existing two-provider assumptions and make route identity, endpoint capability,
and ONNX runtime/session lifecycle behavior harder to reason about.

This ADR records the Milestone 0 provider-model decision required by
`docs/plans/onnx-runtime-embedding-serving/`.

## Decision

Introduce ONNX through a cleaned-up provider model before wiring ONNX load,
unload, gateway, or frontend flows.

The provider model separates these contracts:

- App/plugin identity: sidebar identity, plugin manifest, default URL, install
  metadata, and panel entry.
- Runtime provider behavior: profile validation, provider modes, model
  compatibility, endpoint capabilities, placement controls, alias policy,
  provider-side model id policy, launch-on-serve behavior, and unload behavior.
- Runtime profile: persisted managed or external runtime configuration for one
  provider.
- Launch/runtime strategy: binary process, in-process Rust runtime, or
  external-only lifecycle plan selected by provider behavior.
- Model route: provider-scoped saved route keyed by `(provider, model_id)`.
- Served instance: backend-owned loaded model status with enough provider
  identity to disambiguate the same model id served by multiple providers.
- Serving adapter: provider-specific load/unload/status implementation behind a
  shared serving boundary.
- Gateway endpoint capability: per-provider support for `/v1/models`,
  `/v1/embeddings`, `/v1/chat/completions`, and `/v1/completions`.
- Model compatibility: executable artifact format and serving task, separated
  from custom app metadata such as KittentTS ONNX metadata.
- Frontend provider descriptor: view-model data consumed by profile settings,
  model route rows, and serve dialog filtering.

Provider-scoped routes replace model-only routes. The old global route shape is
not retained as a parallel active reader after migration and cleanup. Existing
Ollama and llama.cpp behavior moves through the provider behavior/adapter path
before ONNX serving is accepted.

## Existing Shared Systems Inventory

| System | Current Owner | ONNX Treatment |
| ------ | ------------- | -------------- |
| App/plugin registry | Plugin JSON, Rust `AppId`, RPC version-manager composition, frontend app registry | Refactor or update as one app identity slice. ONNX extends this only after drift tests or a descriptor owner exists. |
| Version/process management | `pumas-app-manager`, `pumas-rpc` composition, `pumas-core` process helpers | Refactor launch/runtime strategy first. ONNX uses an in-process Rust ONNX Runtime session manager, not Ollama/llama.cpp binary constructors or a Python sidecar. |
| Runtime profiles | `pumas-core` runtime profile service and DTOs | Refactor to provider behavior and provider-scoped routes before ONNX routes. |
| Model library | `pumas-core` model library and frontend projection helpers | Extend executable format/compatibility helpers; keep generic ONNX embedding compatibility separate from custom ONNX app metadata. |
| Serving state | `pumas-core` serving contracts and `pumas-rpc` serving handler | Refactor to provider serving adapters and provider-aware served identity before ONNX load/unload. |
| OpenAI gateway | `pumas-rpc` gateway handlers and Axum routes | Refactor endpoint capability checks, shared HTTP client, body limits, and provider model-id rewriting before ONNX gateway routing. |
| Frontend runtime/profile UI | Frontend app panels, runtime profile sections, route rows, serve dialog | Refactor to provider descriptors and provider-scoped route helpers before ONNX panel and route assignment. |
| Torch sidecar | `torch-server/` and Torch-specific process/client integration | Keep as unrelated Torch architecture. It is not the ONNX Runtime implementation target. |

## App And Runtime Descriptor Strategy

Milestone 0 records the strategy as a serial contract decision:

1. Keep app/plugin identity separate from runtime provider identity.
2. For the ONNX implementation, either replace hard-coded app identity lists
   with a validated descriptor-driven composition root, or update every
   hard-coded source in one slice with drift tests.
3. Do not add ONNX to only plugin metadata, only Rust `AppId`, or only frontend
   registry code.

The descriptor-driven composition root is preferred if it can be introduced
without widening the first provider-model slice beyond runtime profile,
version-manager, plugin metadata, and frontend registry ownership. If that
replacement proves too large, the fallback is an explicit hard-coded identity
slice that updates plugin metadata, Rust registration, RPC composition,
frontend registry, selected-version state, managed-app decoration, and panel
renderer together with tests.

## Capability And Route Contract Ownership

Provider capability and route contracts are owned by backend runtime-profile
and serving contract modules, with TypeScript bridge types updated in the same
boundary slice. Provider behavior owns:

- supported artifact formats and serving tasks
- supported runtime modes
- OpenAI endpoint capabilities
- route alias and provider-side model id policy
- placement controls
- launch strategy selection
- load/unload idempotency expectations

The route repository owns persisted route migration and cleanup. It may call
provider validators, but it must not hide provider behavior inside persistence
code. The gateway consumes provider endpoint capabilities and provider-side
model-id rewriting; it does not match providers directly to decide endpoint
support.

## First Vertical Acceptance Path

The first complete public-contract acceptance path is:

1. Create or load a managed ONNX Runtime profile.
2. Save a provider-scoped route for an ONNX-compatible model.
3. Call `serve_model` without an explicit profile and resolve the saved ONNX
   route.
4. Load through a fake or fixture Rust ONNX provider adapter.
5. Record backend-owned `ServedModelStatus`.
6. Confirm `GET /v1/models` lists the public alias.
7. Confirm `POST /v1/embeddings` proxies through the Pumas gateway.
8. Confirm chat and completion endpoints fail before proxying for the ONNX
   embedding-only provider.

The failing-first acceptance test should be added at the earliest slice where
the gateway can exercise provider-scoped served state against a fake or fixture
provider endpoint.

## Decomposition Review

The implementation must not add ONNX responsibilities directly to already-large
mixed-responsibility files except as narrow delegating calls. Current review
targets include:

| File | Approximate Size | Required Response |
| ---- | ---------------- | ----------------- |
| `rust/crates/pumas-core/src/model_library/library.rs` | 12k+ lines | Extract/localize executable-format and provider-compatibility projection helpers before adding generic ONNX embedding compatibility. |
| `rust/crates/pumas-core/src/runtime_profiles.rs` | 1.9k+ lines | Extract provider behavior, route persistence/migration, and launch strategy concerns. |
| `rust/crates/pumas-core/src/serving/mod.rs` | 1.2k+ lines | Move provider compatibility and served identity policy behind provider contracts. |
| `rust/crates/pumas-rpc/src/handlers/serving.rs` | 1.2k+ lines | Move load/unload provider behavior to serving adapters. |
| `rust/crates/pumas-rpc/src/handlers/mod.rs` | 1.3k+ lines | Extract gateway proxy helper and endpoint capability checks. |
| `rust/crates/pumas-core/src/process/launcher.rs` | 900+ lines | Introduce typed launch strategies instead of ONNX-specific launcher branches. |
| Frontend llama.cpp model library section | 500+ lines in current section module | Extract provider route view models and shared route primitives before ONNX model rows. |

## Consequences

- ONNX work starts slower because provider-scoped routes, provider behavior, and
  adapter boundaries must land before ONNX load/unload.
- Gateway and serving tests become clearer because unsupported endpoint,
  duplicate alias, and same-model-id-across-provider cases are explicit.
- Persisted runtime profile routes need a one-way migration/cleanup. Keeping
  dual old/new active readers is not accepted.
- Frontend route and serve-dialog code must stop assuming model id alone is
  enough to select a runtime profile.

## Alternatives Rejected

- Add `onnx_runtime` branches beside Ollama and llama.cpp matches: rejected
  because it keeps the unsafe non-llama.cpp-implies-Ollama fallback and spreads
  endpoint support policy across handlers.
- Keep model-only runtime routes: rejected because the same model id can be
  served by multiple providers, and ONNX missing-route behavior must not fall
  back to a llama.cpp or default profile.
- Add a Python ONNX sidecar: rejected because ONNX Runtime can be hosted through
  Rust bindings, and adding a second Python sidecar would expand packaging,
  process lifecycle, and cross-language contracts without a clear need.
- Expose a raw ONNX provider endpoint as the supported external app contract:
  rejected because Pumas owns aliases, served state, future auth policy, and the
  existing `/v1` facade.
- Copy the Torch sidecar integration path as-is: rejected because Torch is app
  specific and does not provide the runtime-profile provider contract or Rust
  ONNX session lifecycle required here.

## Invariants

- Pumas `/v1` remains the supported external facade.
- Runtime profile and served-model state remain backend-owned.
- ONNX Runtime supports embeddings only until a later provider capability says
  otherwise.
- Provider-scoped route identity is `(provider, model_id)`.
- Gateway endpoint capability checks happen before proxying.
- Generic ONNX embedding compatibility remains separate from custom ONNX app
  metadata.
- ONNX dependencies remain owned by the Rust crate/module that executes ONNX
  Runtime.

## Revisit Triggers

- Provider behavior cannot be injected through a composition root without a
  broader RPC/server state refactor.
- Runtime-profile route migration cannot remove the old global route shape
  without unacceptable data loss.
- A descriptor-driven app identity root is feasible and materially simpler than
  synchronizing hard-coded Rust, plugin, and frontend lists.
- ONNX Runtime GPU packaging requires separate CPU and GPU provider profiles.
- The first ONNX model package lacks tokenizer/config files needed for local
  tokenization.
- External app requirements force LAN/auth behavior beyond the existing
  loopback-first Pumas gateway policy.

## Related Plan

- `docs/plans/onnx-runtime-embedding-serving/plan.md`
- `docs/plans/onnx-runtime-embedding-serving/provider-model-and-contracts.md`
- `docs/plans/onnx-runtime-embedding-serving/milestones.md`
