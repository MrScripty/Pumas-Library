# pumas-rpc handlers

## Purpose
Domain-specific JSON-RPC handlers that parse request params, call API services, and return serializable response payloads.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `mod.rs` | Dispatcher and JSON-RPC entrypoint utilities. |
| `shared.rs` | Shared parameter extraction and cross-handler helper functions. |
| `status.rs` | Status/system/network handler methods. |
| `models.rs` | Re-export surface for model handlers. |
| `models/` | Focused model-domain handler submodules. |
| `links.rs` | Link health/mapping/sync handler methods. |
| `ollama.rs` | Legacy endpoint and profile-aware Ollama model operation handlers. |
| `runtime_profiles.rs` | Runtime profile snapshot, update-feed, mutation, model-route, launch, and stop handlers. |
| `serving.rs` | User-directed model serving status, update-feed, validation, serve, and unload handlers. |
| `serving_ollama.rs` | Ollama-specific serving load/unload adapter implementation. |
| `serving_llama_cpp.rs` | llama.cpp serving adapter entry points for dedicated and router profiles. |
| `serving_llama_cpp_router.rs` | llama.cpp router serving lifecycle and router HTTP helpers. |
| `serving_llama_cpp_shared.rs` | Shared llama.cpp serving compatibility and runtime-version helpers. |
| `serving_onnx.rs` | ONNX Runtime serving adapter entry points backed by the Rust ONNX session manager. |
| `openai_gateway.rs` | OpenAI-compatible gateway model listing, routing, proxy, and response helpers. |
| `process.rs` | Legacy singleton process launch/stop and filesystem/window process handlers. |
| `torch.rs` | Torch server status, slot, and configuration handlers. |
| `versions.rs` | Re-export surface for version handlers. |
| `versions/` | Focused version-domain handler submodules. |

## Design Decisions
- Param extraction and validation happen at the handler boundary.
- Handler modules are grouped by capability area to keep dispatch predictable.
- Runtime profile handlers accept `profile_id` as the canonical route key and
  keep raw endpoint URLs confined to legacy Ollama compatibility methods.
- Launch/stop profile commands delegate to backend runtime-profile ownership;
  RPC handlers do not derive provider-specific process arguments themselves.
- Serving handlers parse model-row/modal requests, delegate validation and
  status/update-feed storage to backend-owned serving APIs, and dispatch
  provider load/unload through focused adapter modules. Adapter modules own
  provider API calls, router load/unload requests, runtime-version checks, and
  managed profile launch calls until the launch-strategy abstraction lands.
- OpenAI-compatible gateway handlers own HTTP gateway request policy, served
  model lookup, provider endpoint capability checks, and proxy response
  shaping. They consume provider behavior rather than matching individual
  providers in the gateway boundary.
- Serving handlers return non-critical domain errors for valid requests that
  cannot load, preserving existing served models and avoiding renderer crash
  behavior.

## Dependencies
**Internal:** `AppState`, `pumas-library` API, helper utilities in `shared.rs`.
**External:** `serde_json` and transport-level error mapping.

## Usage Examples
```rust
let value = handlers::models::validate_file_type(state, params).await?;
```
