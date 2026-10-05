# ADR 0001: ONNX Runtime Provider Model

- Status: Accepted and implemented
- Date: 2026-05-11

## Context

Pumas originally expressed serving behavior through Ollama/llama.cpp-specific
branches. Adding ONNX Runtime that way would have coupled app identity, process
lifecycle, route identity, model compatibility, and gateway capabilities even
more tightly.

ONNX embedding execution also differs materially: it is hosted in the Rust
process rather than an external binary or Python sidecar.

## Decision

Inference integrations use a provider model with separate contracts for:

- app/plugin identity and presentation metadata;
- provider behavior and endpoint capabilities;
- persisted runtime profiles;
- launch strategy (external process, in-process, or external-only);
- provider-scoped model routes keyed by `(provider, model_id)`;
- backend-owned served-instance identity;
- provider-specific serving adapters; and
- frontend provider descriptors.

The Pumas `/v1` gateway remains the supported external facade. Provider
capability checks happen before dispatch. ONNX Runtime initially supports
embeddings only and owns an in-process session manager.

Existing Ollama and llama.cpp behavior uses the same provider abstractions.
Legacy model-only routes are migrated only when their provider is unambiguous;
the old route shape is not retained as a parallel reader.

## Ownership

| Concern | Current owner |
| --- | --- |
| Provider behavior | `rust/crates/pumas-core/src/providers/` |
| Runtime profiles and route persistence | `rust/crates/pumas-core/src/runtime_profiles/` |
| Serving contracts | `rust/crates/pumas-core/src/serving/` |
| ONNX sessions and execution | `rust/crates/pumas-core/src/onnx_runtime/` |
| RPC serving and gateway adapters | `rust/crates/pumas-rpc/src/handlers/` |
| Frontend descriptors | `frontend/src/utils/runtimeProviderDescriptors.ts` |
| Source plugin metadata | `launcher-data/plugins/*.json` |

The ONNX manifest uses `installationType = "in-process"`; it must not enter
binary, Python, Docker, or version-manager installation flows.

## Consequences

- Route and served-instance identity can distinguish the same model used by
  multiple providers.
- Gateway support and provider-side model-name rewriting are explicit.
- Adding a provider requires coordinated core, RPC, frontend, and manifest
  contract work rather than one fallback branch.
- Persisted runtime-profile routes require migration when identity changes.
- ONNX Runtime packaging remains part of the Rust native dependency closure.

The no-download correction keeps execution in-process but provisions native
ONNX Runtime separately. `ort` uses `load-dynamic` with its download capability
disabled across supported Cargo graphs. Cargo never fetches runtime binaries;
the explicit installed library or release-packaged adjacent library is selected
before session creation. Build/setup source choice is not ordinary app settings
and does not enter app-manager's version installation flows. See
[development setup](../DEVELOPMENT.md#rust) for the compatibility change and
verified-distribution requirements.

## Rejected Alternatives

- Add ONNX-specific branches beside Ollama and llama.cpp: this preserves hidden
  provider fallbacks and scattered policy.
- Keep model-only routes: model ID alone cannot identify a provider-specific
  route or served instance.
- Add a Python ONNX sidecar: the Rust binding already supplies the required
  execution boundary without another process and packaging surface.
- Expose the raw ONNX session endpoint: Pumas owns aliases, served state, and
  the external gateway contract.

## Revisit When

- provider behavior cannot be composed without direct provider matches;
- GPU execution requires separate provider/package profiles;
- a descriptor-driven identity source can replace synchronized hard-coded
  registries; or
- the gateway's authentication or external compatibility contract changes.
