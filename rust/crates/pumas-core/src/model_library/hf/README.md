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

## Identity Contract
- `repo_id` is upstream repository provenance. It is preserved in download
  requests, progress records, metadata evidence, and migration reports, but it
  must not be used as the uniqueness key for a selected artifact.
- Selected-artifact identity is derived from the concrete selection: repository,
  revision, optional subfolder, selected filename or file group, quantization,
  and any digest needed for stable disambiguation. This selected-artifact key
  distinguishes variants such as `Q4_K_M` and `Q5_K_M` from the same GGUF repo.
- New download destinations use
  `{library_category}/{architecture_family}/{artifact_slug}`. The
  `artifact_slug` should include repository provenance plus the selected
  artifact selector so equivalent filenames from different publishers do not
  collide.
- Download progress consumers should prefer `selected_artifact_id` when it is
  present and treat `repo_id` as display/provenance data. Repo-only progress
  keying is retained only as a legacy fallback.

## Design Decisions
- Provider-specific concerns are isolated under `hf/` to keep the main model-library flow provider-agnostic.
- Typed DTOs are used to avoid ad-hoc JSON parsing across the codebase.
- HF metadata is normalized into a persisted evidence payload before download placement so later
  import/reclassification can reuse the same source facts.
- Download placement separates upstream repository identity from selected-artifact identity. The
  repository remains provenance, while the selected artifact key distinguishes variants such as
  different GGUF quantizations from the same repo.
- Download orchestration emits evidence at two stages: auxiliary-files-complete for partial
  metadata persistence and final completion for full in-place import.
- Background download tasks are tracked by download ID so explicit cancellation, resume, and client
  drop all operate on owned task handles rather than detached spawned work.
- Legacy `family` remains a compatibility projection while migration and older
  records are supported. New HF evidence and path planning should prefer
  `architecture_family`, including punctuation-preserving version tokens such
  as `qwen3_5` and `qwen3_6` instead of compact historical forms like `qwen35`.

## Migration Notes
- Directories created before selected-artifact identity may contain more than
  one artifact from the same repository. Migration dry-runs should report these
  as split-artifact candidates instead of silently treating duplicate
  `repo_id` values as collisions.
- Active or partial downloads must remain tied to their sidecar and selected
  artifact. Migration execution should skip or explicitly relocate those items
  rather than moving a repository-level directory wholesale.
- Legacy compatibility fields are removable only after `.pumas_download`,
  `downloads.json`, `metadata.json`, SQLite projections, RPC/native bindings,
  and frontend progress maps all use selected-artifact identity without
  repo-only fallback.

## Dependencies
**Internal:** `crate::model_library`, `crate::network`, `crate::models`.
**External:** `reqwest`, `serde`, async utilities.

## Usage Examples
```rust
let models = hf_client.search("llama", Some("text-generation"), 20).await?;
println!("matches={}", models.len());
```
