# pumas-core providers

## Purpose

Own backend runtime-provider behavior contracts for profile validation,
serving compatibility, gateway endpoint support, model-id policy, unload
behavior, and launch-strategy selection.

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
and managed/external support checks. Later slices migrate serving adapters,
launcher strategies, gateway proxying, and frontend bridge contracts onto this
registry.

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
- OpenAI-compatible endpoint support is explicit per provider.
- Local executable artifact compatibility is separate from gateway endpoint
  support.

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

## Structured Producer Contract

- Stable producer fields are provider id, provider modes, device modes, local
  artifact formats, serving tasks, OpenAI endpoints, launch strategies,
  provider model-id policy, and unload behavior.
- Enum values serialize with snake_case when they cross a boundary.
- Adding ONNX Runtime requires adding one provider behavior entry and matching
  contract tests before consumers depend on it.
