# Desktop RPC Method Contract

## Purpose
This registry records the current method contract between the renderer bridge, Electron main process, and Rust JSON-RPC backend.

## Current Enforcement
`electron/src/rpc-method-registry.ts` owns the executable method registry and `electron/src/ipc-validation.ts` enforces:

- method name must be a string;
- method name must be in the known backend allowlist;
- params must be an object record when present;
- dialog and external URL IPC payloads are sanitized independently from renderer types.

## Current Limitation
This pass intentionally enforces method-level allowlisting, not full per-method request schemas. The executable registry records request and response schemas as `deferred` until the next contract pass promotes them into generated request/response schema artifacts used by TypeScript and Rust.

## Method Groups
| Group | Representative Methods | Owner |
| --- | --- | --- |
| Status and system | `get_status`, `get_disk_space`, `get_system_resources`, `get_network_status` | `rust/crates/pumas-rpc/src/handlers/status.rs` |
| Local runtime profiles | `get_runtime_profiles_snapshot`, `list_runtime_profile_updates_since`, `upsert_runtime_profile`, `set_model_runtime_route` | `rust/crates/pumas-rpc/src/handlers/runtime_profiles.rs` |
| User-directed serving | `get_serving_status`, `list_serving_status_updates_since`, `validate_model_serving_config`, `serve_model`, `unserve_model` | `rust/crates/pumas-rpc/src/handlers/serving.rs` |
| Version management | `get_available_versions`, `install_version`, `switch_version`, `get_installation_progress` | `rust/crates/pumas-rpc/src/handlers/versions/` |
| Model library | `get_models`, `import_model`, `search_hf_models`, `get_library_model_metadata` | `rust/crates/pumas-rpc/src/handlers/models/` |
| Process control | `launch_comfyui`, `stop_comfyui`, `open_path`, `open_url` | `rust/crates/pumas-rpc/src/handlers/process.rs` |
| App integrations | `ollama_list_models`, `ollama_list_models_for_profile`, `ollama_create_model_for_profile`, `ollama_load_model_for_profile`, `ollama_unload_model_for_profile`, `ollama_delete_model_for_profile`, `torch_list_slots`, `torch_configure` | `rust/crates/pumas-rpc/src/handlers/ollama.rs`, `torch.rs` |
| Link and mapping | `get_link_health`, `preview_model_mapping`, `sync_with_resolutions` | `rust/crates/pumas-rpc/src/handlers/links.rs` |
| Shortcuts | `get_version_shortcuts`, `toggle_menu`, `create_desktop_shortcut` | `rust/crates/pumas-rpc/src/handlers/shortcuts.rs` |
| Conversion | `start_model_conversion`, `get_conversion_progress`, `setup_quantization_backend` | `rust/crates/pumas-rpc/src/handlers/conversion.rs` |
| Plugins and custom nodes | `get_plugins`, `get_custom_nodes`, `install_custom_node` | `rust/crates/pumas-rpc/src/handlers/plugins.rs`, `custom_nodes.rs` |

## Event Channels
| Channel | Backend SSE Route | Electron Channel | Preload Method |
| --- | --- | --- | --- |
| Model library updates | `/events/model-library-updates` | `model-library:update` | `onModelLibraryUpdate` |
| Runtime profile updates | `/events/runtime-profile-updates` | `runtime-profile:update` | `onRuntimeProfileUpdate` |
| Serving status updates | `/events/serving-status-updates` | `serving-status:update` / `serving-status:error` | `onServingStatusUpdate` |
| Status telemetry updates | `/events/status-telemetry-updates` | `status-telemetry:update` | status telemetry store subscription |

## Serving Gateway
- The RPC server exposes a local OpenAI-compatible serving gateway at
  `/v1/models`, `/v1/chat/completions`, `/v1/completions`, and `/v1/embeddings`.
- `/v1/models` is backed by `ServingStatusSnapshot.served_models`.
- When one or more models are loaded, aggregate serving status reports
  `endpoint_mode = pumas_gateway`; each `ServedModelStatus.endpoint_url`
  remains the provider endpoint used internally by the gateway.
- Proxy routes resolve the request `model` by unique gateway alias first, then
  by base `model_id` only when that id is unambiguous. Ambiguous base-model
  requests return a deterministic conflict instead of selecting the first
  loaded instance.
- Gateway aliases are globally unique across loaded models. The backend derives
  one effective gateway alias before validation and before recording
  `ServedModelStatus`.
- If a provider-facing alias exists, the forwarded `model` field is rewritten
  to that alias.
- The gateway is available only on the RPC server bind address. The binary
  already rejects non-loopback binds unless `--allow-lan` is supplied.

## Local Runtime Profile Rules
- `profile_id` is the canonical internal address for local runtime operations.
- `connection_url` inputs remain available only on legacy Ollama methods and
  must be validated at the RPC boundary before provider clients are created.
- Runtime profile mutations, model routes, lifecycle state, and update cursors
  are backend-owned. Renderer code should refresh a snapshot or consume
  `onRuntimeProfileUpdate`; it should not infer persisted state from a local
  optimistic edit.
- `launch_runtime_profile(profile_id, tag?, model_id?)` is provider-neutral at
  the bridge boundary. Provider-specific launch arguments, llama.cpp router
  presets, and dedicated `llama-server -m` model paths are derived in the
  backend service.

## User-Directed Serving Rules
- Serving requests are user-authored commands from a model row or model modal.
  Runtime routes may prefill defaults, but the `serve_model` command must carry
  the explicit provider/profile/device placement that will be attempted.
- Loaded-model state, endpoint mode, and last load errors are backend-owned.
  Renderer code should not mark a model as served until a backend response or
  serving snapshot confirms it.
- Serving status is pushed to the renderer through `onServingStatusUpdate`.
  Interactive serving UI may call `get_serving_status` for initial state and
  pushed snapshot refreshes, but must not start renderer polling.
- `list_serving_status_updates_since(cursor?)` remains a lightweight serving
  update feed for explicit RPC callers. Renderer serving UI does not use it as
  a fallback path.
- A failed fit or provider load is a domain response, not a renderer crash.
  Valid requests that cannot be loaded should return a `ModelServeError` with
  `severity = non_critical` and should preserve already-served models unless a
  user explicitly unloads them.
- `serve_model` wires Ollama orchestration and llama.cpp orchestration for the
  runtime profile selected by the user. llama.cpp requests may load through a
  managed dedicated server or through a managed router profile, depending on
  the saved profile mode.
- Same-model multi-profile serving requires distinct gateway aliases. Frontend
  prompts are usability aids; backend validation owns duplicate-alias rejection
  and gateway ambiguity errors.
- Endpoint status must report `not_configured`, `provider_endpoint`, or
  `pumas_gateway` truthfully. When models are served through the Pumas gateway,
  `/v1/models` lists the backend-confirmed aliases that clients should use.

## Contract Rules
- New method names must be added to `electron/src/rpc-method-registry.ts`.
- Renderer-visible methods must be exposed through `electron/src/preload.ts` and typed in `frontend/src/types/api.ts`.
- Backend handlers must parse params at the boundary before calling internal services.
- Destructive and path-taking methods must receive per-method schemas before broader model-library decomposition proceeds.
- `electron/tests/ipc-validation.test.mjs` must keep enforcing registry uniqueness and representative runtime allowlisting.

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
- `upsert_runtime_profile`
- `set_model_runtime_route`
- `install_custom_node`
