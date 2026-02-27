# pumas-rpc handlers

## Purpose
Domain-specific JSON-RPC handlers that parse request params, call API services, and return serializable response payloads.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `mod.rs` | Dispatcher, shared param helpers, and boundary utilities. |
| `status.rs` | Status/system/network handler methods. |
| `models.rs` | Model-library/search/import/migration handler methods. |
| `links.rs` | Link health/mapping/sync handler methods. |
| `versions.rs` | Version-manager handler methods. |

## Design Decisions
- Param extraction and validation happen at the handler boundary.
- Handler modules are grouped by capability area to keep dispatch predictable.

## Dependencies
**Internal:** `AppState`, `pumas-library` API, helper utilities in `mod.rs`.
**External:** `serde_json` and transport-level error mapping.

## Usage Examples
```rust
let value = handlers::models::validate_file_type(state, params).await?;
```
