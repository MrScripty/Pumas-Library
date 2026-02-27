# pumas-core api

## Purpose
Defines the primary API façade (`PumasApi`) methods that orchestrate core subsystems without embedding transport-specific behavior.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `builder.rs` | API builder and initialization wiring. |
| `models.rs` | Model-library and link-management API methods. |
| `network.rs` | Connectivity and network-status API methods. |
| `process.rs` | Process lifecycle/status API methods. |
| `system.rs` | System and environment utility API methods. |

## Design Decisions
- API modules group methods by domain area for discoverability.
- Transport layers call these methods directly and convert only request/response shapes.

## Dependencies
**Internal:** `crate::model_library`, `crate::network`, `crate::process`, `crate::models`.
**External:** standard library path/collections and async primitives.

## Usage Examples
```rust
let net = api.get_network_status_response().await;
println!("offline={}", net.is_offline);
```
