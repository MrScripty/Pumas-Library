# pumas-app-manager src

## Purpose
Application-manager layer for installed app versions, dependency checks, process adapters, and app-specific operations (ComfyUI/Ollama/Torch).

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `version_manager/` | Version install/remove/switch/progress and dependency checks. |
| `custom_nodes/` | Custom node lifecycle for supported app runtimes. |
| `ollama_client/` | Focused Ollama client helper modules (for example naming logic). |
| `process/` | Process factory/wrappers used by app managers. |
| `ollama_client.rs` | Ollama RPC/HTTP client integrations. |
| `torch_client.rs` | Torch-related environment and runtime checks. |

## Design Decisions
- App/version-specific orchestration is separated from core model-library logic.
- Reusable clients and managers are composed by higher transport layers.

## Dependencies
**Internal:** `pumas-library` (`pumas-core`) types/services.
**External:** filesystem/network/process crates used by installers and runtime probes.

## Usage Examples
```rust
let installed = version_manager.get_installed_versions().await?;
println!("installed versions: {}", installed.len());
```
