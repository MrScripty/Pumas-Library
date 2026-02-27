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
| `versions.rs` | Re-export surface for version handlers. |
| `versions/` | Focused version-domain handler submodules. |

## Design Decisions
- Param extraction and validation happen at the handler boundary.
- Handler modules are grouped by capability area to keep dispatch predictable.

## Dependencies
**Internal:** `AppState`, `pumas-library` API, helper utilities in `shared.rs`.
**External:** `serde_json` and transport-level error mapping.

## Usage Examples
```rust
let value = handlers::models::validate_file_type(state, params).await?;
```
