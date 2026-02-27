# pumas-core model_library hf

## Purpose
Hugging Face-specific integrations for model discovery, download orchestration, and metadata retrieval used by the model library.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `api.rs` | HF API client calls and typed response handling. |
| `download.rs` | Download planning/progress and file transfer helpers. |
| `metadata.rs` | Metadata lookup and normalization helpers. |
| `types.rs` | HF-specific request/response model types. |

## Design Decisions
- Provider-specific concerns are isolated under `hf/` to keep the main model-library flow provider-agnostic.
- Typed DTOs are used to avoid ad-hoc JSON parsing across the codebase.

## Dependencies
**Internal:** `crate::model_library`, `crate::network`, `crate::models`.
**External:** `reqwest`, `serde`, async utilities.

## Usage Examples
```rust
let models = hf_client.search("llama", Some("text-generation"), 20).await?;
println!("matches={}", models.len());
```
