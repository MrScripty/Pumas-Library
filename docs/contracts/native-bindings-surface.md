# Native Bindings Surface Contract

## Purpose
This contract classifies the current UniFFI export surface by support tier and documents the validation rules applied before host-language input reaches core services.

## Support Tiers
| Tier | Meaning | Change Policy |
| --- | --- | --- |
| Stable | Intended for generated package consumers in this release line. | Additive changes only unless a changelog migration note is included. |
| Preview | Usable for smoke checks and early consumers, but may change shape. | Breaking changes require a contract note and regenerated examples. |
| Internal | Exported only because current adapters need it. | May change with the owning adapter and should not be promoted without review. |

## Current Export Classification
| Export | Tier | Owner | Notes |
| --- | --- | --- | --- |
| `version` | Stable | `pumas-uniffi` | Binding/native version identity. |
| `FfiPumasApi::new` | Preview | `pumas-uniffi` | Validates `launcher_root` before owner construction. Returns an error when another process already owns the launcher root; future bindings should expose explicit local-client/read-only roles as separate objects. |
| `FfiPumasApi::with_config` | Preview | `pumas-uniffi` | Validates `FfiApiConfig.launcher_root`; other flags are configuration booleans. This constructor is owner-only and must not hide local-client transport behind the same object. |
| `list_models`, `get_model`, `search_models` | Stable | `pumas-core` via adapter | Read-only model catalog surface. |
| `import_model`, `import_models_batch` | Preview | `pumas-core` via adapter | Validates import path, family, and official name in the adapter. |
| `delete_model` | Preview | `pumas-core` via adapter | Destructive operation; keep compatibility notes when changing semantics. |
| `rebuild_model_index`, `reclassify_model`, `reclassify_all_models` | Preview | `pumas-core` via adapter | Long-running catalog maintenance operations. |
| `get_inference_settings`, `update_inference_settings` | Preview | `pumas-core` via adapter | Schema shape may grow as backend support expands. |
| `search_hf_models`, `start_hf_download`, `get_hf_download_progress`, `cancel_hf_download` | Preview | `pumas-core` via adapter | Validates required download request strings in the adapter. Download requests carry repository provenance plus artifact-selection inputs, and progress carries optional `selected_artifact_id` so host consumers can distinguish variants from one repo while retaining `repo_id` for compatibility. |
| `list_interrupted_downloads`, `recover_download`, `lookup_hf_metadata_for_file`, `get_hf_repo_files` | Preview | `pumas-core` via adapter | Recovery and metadata helpers; path-bearing methods must keep adapter validation. |
| `is_online`, `get_disk_space`, `get_status`, `get_system_resources` | Stable | `pumas-core` via adapter | Read-only status and diagnostics. |
| `is_torch_running`, `torch_stop` | Preview | `pumas-core` via adapter | Runtime management surface. |
| FFI records and enums | Stable only when referenced by a stable method | `pumas-uniffi` | Records used only by preview methods inherit the preview tier. |

## Validation Rules
- `launcher_root` must be non-empty and must not contain NUL bytes.
- `FfiModelImportSpec.path` must be non-empty and must not contain NUL bytes.
- `FfiModelImportSpec.family` and `official_name` must be non-empty.
- `FfiDownloadRequest.repo_id`, `family`, and `official_name` must be non-empty.
- `FfiDownloadRequest.quant`, `filename`, and `filenames` are artifact-selection inputs when present. They are optional for compatibility, but new HF download flows should provide the most specific selector available so core can derive a stable selected-artifact identity.
- Generated host-language bindings must keep these adapter validations; callers should not rely on core services to sanitize invalid host strings.

## Selected-Artifact Identity
- `repo_id` remains the upstream Hugging Face repository identity and provenance field. It is not unique enough to identify an active download or library artifact because one repository can expose multiple GGUF quantizations, file groups, or full-repo selections.
- `selected_artifact_id` is the backend-derived identity for the selected artifact. Host-language consumers should use it as the preferred download progress key when present and fall back to `repo_id` only for legacy progress records.
- RPC and frontend JSON also expose a camelCase compatibility alias, `artifactId`, for the same selected-artifact value. Native records use `selected_artifact_id`.
- `family` remains accepted as a legacy compatibility input. New path, migration, and identity logic should prefer normalized `architecture_family` when it is available in metadata or report records.

## Migration Compatibility
- Historical compact family tokens such as `qwen35` are migration inputs, not canonical output. Migration planning should normalize unambiguous version-family tokens to punctuation-preserving underscore forms such as `qwen3_5`; future version evidence such as `qwen3.6` should similarly project to `qwen3_6`.
- Compatibility fields can be retired only after native bindings, RPC/JSON consumers, frontend state, persisted download sidecars, and metadata migration no longer need repo-only or legacy-family fallbacks.

## Revisit Triggers
- Adding a new exported method, record, or enum.
- Promoting a preview method to stable.
- Changing generated package layout or native module identity.
- Adding a second host-language packaging workflow beyond C# smoke artifacts.
