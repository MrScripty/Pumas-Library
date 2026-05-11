# Provider Model And Contracts

## Cleaner Provider Model Design

ONNX must be implemented as the first new provider on a cleaned-up provider
model, not as a third pile of special cases next to Ollama and llama.cpp. The
implementation must make the following ownership boundaries explicit.

| Concept | Owner | Required Shape |
| ------- | ----- | -------------- |
| App/plugin identity | Plugin metadata, app registry, frontend shell | Describes sidebar identity, install/version metadata, default URL, icon, and panel entry. This is related to a runtime provider but not the same contract. Torch can remain an app/sidecar without becoming a runtime-profile provider in this plan. |
| Runtime provider behavior | Backend provider registry | One behavior contract per provider for profile validation, supported modes, supported artifact formats, supported serving tasks, endpoint capabilities, alias/model-id policy, launch-on-serve policy, placement controls, and provider-specific unload behavior. Ollama and llama.cpp must be migrated onto this contract before ONNX serving lands. |
| Runtime profile | Runtime profile service and DTOs | Saved managed/external configuration for a provider. Profiles must reference provider behavior instead of containing scattered provider matches. |
| Launch strategy | Runtime lifecycle/launcher modules | Provider behavior selects a typed launch strategy such as binary process, Python sidecar, or external-only. ONNX uses Python sidecar; Ollama and llama.cpp use binary; Torch's existing sidecar is a reference pattern, not the runtime-profile contract. |
| Model route | Runtime profile route repository and RPC/frontend contracts | Route identity is `(provider, model_id)`. Save, clear, auto-load, endpoint resolution, snapshots, and frontend maps use this key. No old model-only route reader remains after migration/cleanup. |
| Served instance | Serving state and gateway lookup | Served-model identity includes provider where ambiguity is possible. Same `model_id` can be served by different providers/profiles without unload or gateway lookup crossing providers. |
| Serving adapter | Provider-specific serving modules | RPC serving handlers parse/validate boundary input, then call provider adapters for load, unload, status/list, provider-side model id, alias defaulting, idempotency, and runtime support checks. |
| Gateway endpoint capability | Gateway proxy helper and provider behavior | `/v1/*` routing checks whether the selected provider supports the requested OpenAI-compatible endpoint before proxying. ONNX supports `/v1/models` and `/v1/embeddings` in the first slice only. |
| Model compatibility | Model library projection plus provider behavior | Shared executable artifact format includes `onnx`, but provider behavior decides task/runtime compatibility. Generic ONNX embedding serving must not consume custom ONNX runtime metadata such as KittentTS by accident. |
| Frontend provider descriptor | Frontend provider/app view-model helpers | App panel, runtime profile settings, model route rows, serve dialog filtering, endpoint/task compatibility, and route mutations consume typed provider descriptors instead of llama.cpp-specific or GGUF-only helpers. |

## Contract Ownership Matrix

Each boundary contract must have one owner, one runtime validator, and at least
one producer/consumer test. Implementation must update this matrix if code
finds a better owner.

| Contract | Owner | Validator / Decoder | Required Tests |
| -------- | ----- | ------------------- | -------------- |
| Runtime provider ids, modes, capabilities | Rust runtime-profile contract module plus generated/hand-maintained TypeScript bridge types | Rust serde/newtype parsing and frontend runtime shape guards where payloads cross IPC | Rust serialization round trips, TypeScript type/fixture tests, bridge casing tests |
| App/runtime descriptor mapping | Plugin/app descriptor owner plus Rust `AppId`/version-manager composition root and frontend app registry | Manifest/parser validation for app id, runtime provider id, version-manager key, default URL, icon/panel metadata | App descriptor fixture tests, version-manager registration tests, frontend app registry/renderer tests |
| Provider-scoped runtime routes | Runtime profile route repository/contract module | Boundary parser for provider id, model id, profile id, and auto-load policy | Migration/cleanup tests, snapshot serialization tests, frontend route mutation tests |
| Served-model status and served identity | Serving core contract module | Serve/unserve request validators and served-status serializer | Same model id across providers, unload disambiguation, gateway lookup tests |
| Gateway OpenAI-compatible payloads | RPC gateway facade | JSON body parser with endpoint capability and body-size checks | `/v1/models`, `/v1/embeddings`, unsupported endpoint, malformed JSON, timeout/error mapping tests |
| ONNX sidecar control payloads | `onnx-server/` API models | Pydantic/request model validators plus shared validation helpers | Invalid path/root escape, invalid model id, bad bind host, load/unload idempotency, status shape tests |
| ONNX sidecar embedding payloads | `onnx-server/` OpenAI API models | Request models for model, input, dimensions, batch/token limits | Response shape, dimension, batch ordering, invalid dimensions, oversized request tests |
| Plugin/app metadata | Plugin metadata owner and frontend app registry/descriptor layer | Manifest loader/schema validation or documented parser rules | Manifest fixture tests, frontend registry/descriptor tests, structured producer contract docs |
