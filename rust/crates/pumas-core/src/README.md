# pumas-core src

## Purpose
Core domain and infrastructure library for Pumas. This crate owns model-library logic, indexing, networking, process control abstractions, and API surfaces consumed by RPC/bindings.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `api/` | High-level application API methods exposed to adapters. |
| `model_library/` | Model import, metadata, mapping, dependency, and filesystem logic. |
| `index/` | Indexed model catalog and search data structures. |
| `models/` | Public DTOs and response contracts shared across adapters. |
| `network/` | Connectivity checks, HTTP integrations, and circuit-breaker state. |
| `process/` | Process management utilities used by higher-level integrations. |

## Design Decisions
- Domain behavior is implemented here so adapters (`pumas-rpc`, bindings) stay thin.
- APIs return structured result/response types to stabilize cross-language contracts.

## Dependencies
**Internal:** `pumas-app-manager` (for launcher/version integration at higher layers), internal modules in this crate.
**External:** async runtime (`tokio`), serialization (`serde`), storage/network utilities (`rusqlite`, `reqwest`).

## Usage Examples
```rust
let status = api.get_library_status().await?;
if status.success {
    println!("models={}", status.model_count);
}
```
