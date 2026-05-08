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
| `serving.rs` | User-directed model serving status and validation handlers. |
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
  status storage to backend-owned serving APIs, and perform the current Ollama
  provider orchestration through `pumas-app-manager`.

## Dependencies
**Internal:** `AppState`, `pumas-library` API, helper utilities in `shared.rs`.
**External:** `serde_json` and transport-level error mapping.

## Usage Examples
```rust
let value = handlers::models::validate_file_type(state, params).await?;
```
