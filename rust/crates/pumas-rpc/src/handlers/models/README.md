# pumas-rpc handlers models

## Purpose
Organizes model-domain RPC handlers into focused submodules while preserving the public `handlers::models::*` API surface.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `catalog.rs` | Model list/index/mapping refresh and shared-storage scan handlers. |
| `downloads.rs` | Hugging Face download lifecycle handlers. |
| `search.rs` | Model search handlers (HF and local FTS). |
| `imports.rs` | Import, file-type, and metadata extraction handlers. |
| `auth.rs` | Hugging Face token/auth status handlers. |
| `inference.rs` | Inference settings handlers. |
| `dependencies.rs` | Dependency and review workflow handlers. |
| `migration.rs` | Migration report and prune handlers. |

## Design Decisions
- Keep modules under ~500 lines and grouped by behavior.
- Re-export all handlers from `models.rs` so dispatcher call sites remain unchanged.

## Dependencies
**Internal:** `crate::handlers` parameter helpers, `AppState`, `pumas-library` APIs.
**External:** `serde_json`.

## Usage Examples
```rust
let value = crate::handlers::models::search_models_fts(state, params).await?;
```
