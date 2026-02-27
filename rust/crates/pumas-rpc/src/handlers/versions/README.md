# pumas-rpc handlers versions

## Purpose
Splits version-domain RPC handlers into focused submodules while preserving `handlers::versions::*` function exports.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `lifecycle.rs` | Install/remove/switch/default/active and install-progress handlers. |
| `release.rs` | Available versions, version status/info, release sizing, and cache status handlers. |
| `deps.rs` | Version dependency check/install and requirements extraction handlers. |
| `patch.rs` | Patch status and patch toggle handlers. |

## Design Decisions
- Keep module size below the file-size target and organize by behavior area.
- Re-export all functions from `versions.rs` so dispatcher code remains unchanged.

## Dependencies
**Internal:** handler param helpers, `AppState`, version-manager interfaces.
**External:** `serde_json`, `tracing` (release lifecycle warnings).

## Usage Examples
```rust
let value = crate::handlers::versions::get_available_versions(state, params).await?;
```
