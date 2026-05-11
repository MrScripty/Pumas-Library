# pumas-core providers

## Purpose

Own backend runtime-provider behavior contracts for profile validation,
serving compatibility, gateway endpoint support, model-id policy, unload
behavior, launch-on-serve support, placement policy, and launch-strategy
selection.

## Contents

| File | Description |
| ---- | ----------- |
| `mod.rs` | Typed provider behavior values, built-in provider registry, and contract tests. |

## Problem

Runtime provider policy is currently spread across runtime profiles, serving,
gateway handlers, launcher code, and frontend helpers. ONNX Runtime cannot be
added safely while those systems infer provider behavior from provider-specific
match blocks.

## Constraints

- Provider behavior is backend-owned policy.
- Runtime profile persistence and migration stay separate from provider policy.
- Gateway handlers consume endpoint capabilities; they do not decide support by
  matching individual providers.
- ONNX Runtime must use this provider model before load, unload, or gateway
  routing is wired.

## Decision

Start with a typed provider behavior registry for existing Ollama and llama.cpp
providers. Runtime profile validation consumes the registry for provider-mode
and managed/external support checks. Serving and gateway request model-id
rewriting now consume the provider model-id policy, and gateway proxying checks
provider endpoint capabilities before forwarding through a shared gateway HTTP
client. Serving alias defaulting consumes the provider gateway-alias policy.
Serving load dispatch consumes the provider serving adapter kind. Runtime
profile launch-spec derivation consumes managed launch strategies declared by
provider behavior, including provider-owned runtime directory segments and
implicit managed base ports.
Runtime profile launch handlers consume provider-owned managed runtime app ids
for version-manager lookup, with existing provider-specific launch failure
messages kept in the provider contract so the handler does not dispatch on
provider ids.
Serving validation consumes provider launch-on-serve policy instead of accepting
stopped managed profiles by matching provider ids.
Serving placement validation consumes provider placement policy instead of
selecting rules by matching provider ids. Existing Ollama requests use the
profile-only placement policy, while llama.cpp requests use the llama.cpp
runtime policy for router/dedicated placement behavior.
ONNX Runtime is represented as an embedding-only provider with `.onnx`
artifact support, an in-process managed runtime target, and a session-manager
unload policy. Real ONNX execution and gateway routing are owned by later ONNX
provider/session slices.
Later slices migrate frontend bridge contracts onto this registry.

## Alternatives Rejected

- Add provider data directly to `runtime_profiles.rs`: rejected because that
  file already owns profile persistence, lifecycle events, route lookup, and
  launch details.
- Put endpoint support in `pumas-rpc`: rejected because provider capability is
  domain policy consumed by more than one transport surface.

## Invariants

- Every built-in provider id has exactly one registry entry.
- Provider modes are declared by provider behavior before profile validation
  consumes them.
- Runtime profile management-mode support is derived from provider launch kinds:
  `binary_process` and `in_process_runtime` support managed profiles, and
  `external_only` supports external profiles.
- OpenAI-compatible endpoint support is explicit per provider.
- Local executable artifact compatibility is separate from gateway endpoint
  support.
- Provider-side request model ids are derived from the declared model-id
  policy, not from transport-layer provider matches.
- Managed runtime profile launch targets are declared per provider mode by
  provider behavior, then projected into runtime-profile launch specs.
- Managed runtime app identity is declared separately from provider id and
  runtime directory layout so launch handlers do not use provider ids as hidden
  app/version-manager keys.
- Managed runtime profile path segments and implicit base ports are declared by
  provider behavior so launch-spec derivation does not infer launch layout from
  provider ids.
- Launch-on-serve support is provider-owned policy. Serving validation may
  accept a stopped managed profile only when provider behavior declares support
  for that provider mode.
- Serving placement rule selection is provider-owned policy. Serving validation
  asks provider behavior which placement policy to apply before checking
  per-model placement fields.
- Executable artifact formats are parsed into `ExecutableArtifactFormat` at
  boundaries before serving validation consumes them.

## Revisit Triggers

- Provider behavior needs infrastructure clients or async lifecycle ownership.
- Runtime profile migration requires persisted capability versioning.
- A plugin-defined provider needs to register behavior outside the built-in
  registry.

## Dependencies

**Internal:** `crate::models` runtime provider, mode, and device DTOs.

**External:** None.

## Related ADRs

- `docs/adr/0001-onnx-runtime-provider-model.md` records the provider model and
  route-contract decision.

## Usage Examples

```rust
let registry = pumas_library::ProviderRegistry::builtin();
let behavior = registry
    .get(pumas_library::models::RuntimeProviderId::LlamaCpp)
    .expect("llama.cpp provider is built in");
assert!(behavior.supports_openai_endpoint(
    pumas_library::OpenAiGatewayEndpoint::Embeddings
));
```

## API Consumer Contract

- Consumers request provider behavior by `RuntimeProviderId`.
- Missing providers return `None`; callers decide whether that is a validation
  error or a re-plan trigger.
- Capability lists are deterministic for built-in providers.
- Registry values are immutable after construction in this slice.
- `ProviderBehavior::provider_request_model_id` applies the provider's
  model-id policy to a library model id and optional gateway alias.
- `ProviderBehavior::managed_launch_target` returns the provider-owned managed
  launch target for a validated provider mode.
- `managed_runtime_path_segment` and `managed_runtime_base_port` are consumed
  by launch-spec derivation for managed runtime directories and implicit port
  allocation.
- `ProviderBehavior::supports_launch_on_serve` is the shared policy check for
  serving validation of stopped managed profiles.
- `ExecutableArtifactFormat::from_path` is the shared boundary parser for
  local executable model artifact paths.
- `ProviderBehavior::supports_management_mode` is the shared policy check for
  runtime-profile managed/external support.

## Structured Producer Contract

- Stable producer fields are provider id, provider modes, device modes, local
  artifact formats, serving tasks, OpenAI endpoints, launch strategies,
  managed runtime app/version messages, managed runtime layout, provider
  model-id policy, gateway alias policy, serving adapter kind, serving
  placement policy, unload behavior, and launch-on-serve support.
- Enum values serialize with snake_case when they cross a boundary.
- ONNX Runtime consumers depend on the existing provider behavior entry and
  matching contract tests before wiring load, unload, or gateway behavior.
