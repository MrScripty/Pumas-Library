# pumas-app-manager custom_nodes

## Purpose
Implements custom-node operations (list/install/update/remove) for app runtimes that support extension nodes.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `mod.rs` | Public custom-node manager and operation entrypoints. |

## Design Decisions
- Keep custom-node behavior in app-manager so core library stays runtime-agnostic.
- Operations return structured results suitable for RPC and UI consumption.

## Dependencies
**Internal:** version-manager state and process utilities.
**External:** git/process/filesystem tooling.

## Usage Examples
```rust
let nodes = manager.get_custom_nodes("v1.0.0").await?;
println!("nodes={}", nodes.len());
```
