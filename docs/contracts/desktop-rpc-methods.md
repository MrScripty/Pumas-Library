# Desktop RPC Method Contract

## Purpose
This registry records the current method contract between the renderer bridge, Electron main process, and Rust JSON-RPC backend.

## Current Enforcement
`electron/src/ipc-validation.ts` enforces:

- method name must be a string;
- method name must be in the known backend allowlist;
- params must be an object record when present;
- dialog and external URL IPC payloads are sanitized independently from renderer types.

## Current Limitation
This pass intentionally enforces method-level allowlisting, not full per-method request schemas. The next contract pass should promote this registry into generated request/response schema artifacts used by TypeScript and Rust.

## Method Groups
| Group | Representative Methods | Owner |
| --- | --- | --- |
| Status and system | `get_status`, `get_disk_space`, `get_system_resources`, `get_network_status` | `rust/crates/pumas-rpc/src/handlers/status.rs` |
| Version management | `get_available_versions`, `install_version`, `switch_version`, `get_installation_progress` | `rust/crates/pumas-rpc/src/handlers/versions/` |
| Model library | `get_models`, `import_model`, `search_hf_models`, `get_library_model_metadata` | `rust/crates/pumas-rpc/src/handlers/models/` |
| Process control | `launch_comfyui`, `stop_comfyui`, `open_path`, `open_url` | `rust/crates/pumas-rpc/src/handlers/process.rs` |
| App integrations | `ollama_list_models`, `torch_list_slots`, `torch_configure` | `rust/crates/pumas-rpc/src/handlers/ollama.rs`, `torch.rs` |
| Link and mapping | `get_link_health`, `preview_model_mapping`, `sync_with_resolutions` | `rust/crates/pumas-rpc/src/handlers/links.rs` |
| Shortcuts | `get_version_shortcuts`, `toggle_menu`, `create_desktop_shortcut` | `rust/crates/pumas-rpc/src/handlers/shortcuts.rs` |
| Conversion | `start_model_conversion`, `get_conversion_progress`, `setup_quantization_backend` | `rust/crates/pumas-rpc/src/handlers/conversion.rs` |
| Plugins and custom nodes | `get_plugins`, `get_custom_nodes`, `install_custom_node` | `rust/crates/pumas-rpc/src/handlers/plugins.rs`, `custom_nodes.rs` |

## Contract Rules
- New method names must be added to `electron/src/ipc-validation.ts`.
- Renderer-visible methods must be exposed through `electron/src/preload.ts` and typed in `frontend/src/types/api.ts`.
- Backend handlers must parse params at the boundary before calling internal services.
- Destructive and path-taking methods must receive per-method schemas before broader model-library decomposition proceeds.

## Next Schema Targets
Prioritize these methods for typed request schemas:

- `import_model`
- `import_batch`
- `classify_model_import_paths`
- `download_model_from_hf`
- `start_model_download_from_hf`
- `delete_model_with_cascade`
- `open_path`
- `open_url`
- `torch_configure`
- `install_custom_node`
