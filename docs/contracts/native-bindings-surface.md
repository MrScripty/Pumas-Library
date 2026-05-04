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
| `FfiPumasApi::new` | Stable | `pumas-uniffi` | Validates `launcher_root` before primary or IPC-backed construction. |
| `FfiPumasApi::with_config` | Stable | `pumas-uniffi` | Validates `FfiApiConfig.launcher_root`; other flags are configuration booleans. |
| `list_models`, `get_model`, `search_models` | Stable | `pumas-core` via adapter | Read-only model catalog surface. |
| `import_model`, `import_models_batch` | Preview | `pumas-core` via adapter | Validates import path, family, and official name in the adapter. |
| `delete_model` | Preview | `pumas-core` via adapter | Destructive operation; keep compatibility notes when changing semantics. |
| `rebuild_model_index`, `reclassify_model`, `reclassify_all_models` | Preview | `pumas-core` via adapter | Long-running catalog maintenance operations. |
| `get_inference_settings`, `update_inference_settings` | Preview | `pumas-core` via adapter | Schema shape may grow as backend support expands. |
| `search_hf_models`, `start_hf_download`, `get_hf_download_progress`, `cancel_hf_download` | Preview | `pumas-core` via adapter | Validates required download request strings in the adapter. Download progress includes optional `selected_artifact_id` so host consumers can distinguish variants from one repo while retaining `repo_id` for compatibility. |
| `list_interrupted_downloads`, `recover_download`, `lookup_hf_metadata_for_file`, `get_hf_repo_files` | Preview | `pumas-core` via adapter | Recovery and metadata helpers; path-bearing methods must keep adapter validation. |
| `is_online`, `get_disk_space`, `get_status`, `get_system_resources` | Stable | `pumas-core` via adapter | Read-only status and diagnostics. |
| `is_torch_running`, `torch_stop` | Preview | `pumas-core` via adapter | Runtime management surface. |
| FFI records and enums | Stable only when referenced by a stable method | `pumas-uniffi` | Records used only by preview methods inherit the preview tier. |

## Validation Rules
- `launcher_root` must be non-empty and must not contain NUL bytes.
- `FfiModelImportSpec.path` must be non-empty and must not contain NUL bytes.
- `FfiModelImportSpec.family` and `official_name` must be non-empty.
- `FfiDownloadRequest.repo_id`, `family`, and `official_name` must be non-empty.
- Generated host-language bindings must keep these adapter validations; callers should not rely on core services to sanitize invalid host strings.

## Revisit Triggers
- Adding a new exported method, record, or enum.
- Promoting a preview method to stable.
- Changing generated package layout or native module identity.
- Adding a second host-language packaging workflow beyond C# smoke artifacts.
