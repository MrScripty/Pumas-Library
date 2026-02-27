# pumas-rpc src

## Purpose
JSON-RPC transport adapter for Pumas. Converts incoming RPC calls into core/app-manager API calls and wraps responses for frontend contract compatibility.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `handlers/` | Method dispatch handlers grouped by domain. |
| `server.rs` | RPC server initialization and state wiring. |
| `wrapper.rs` | Frontend response-shape compatibility wrapper logic. |
| `main.rs` | RPC service entrypoint. |

## Design Decisions
- Handlers should bridge transport concerns only; domain behavior remains in `pumas-core`.
- Response wrapping is centralized in one module to keep contract changes explicit.

## Dependencies
**Internal:** `pumas-library`, `pumas-app-manager`.
**External:** `axum`, `serde_json`, `tokio`, tracing/logging crates.

## Usage Examples
```text
POST /rpc
{"jsonrpc":"2.0","method":"get_library_status","params":{},"id":1}
```
