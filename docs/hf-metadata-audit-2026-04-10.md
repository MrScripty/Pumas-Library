# Hugging Face Metadata Audit - 2026-04-10

## Scope

This audit sampled 30 live Hugging Face repositories across:

- `text-generation`
- `text-ranking`
- `text-to-image`
- `image-to-image`
- `text-to-audio`
- `automatic-speech-recognition`
- `image-classification`
- `image-segmentation`
- `depth-estimation`
- `object-detection`
- `text-to-3d`
- `image-to-3d`

The audit intentionally did **not** download model weights. It only fetched HF API metadata and passed that metadata through Pumas classification, metadata projection, and SQLite indexing using a temporary metadata-only library.

The reusable harness for this lives in `rust/crates/pumas-core/examples/hf_metadata_audit.rs`.

## Baseline Findings

Initial 30-model sample result before changes:

- `20/30` projected records were still marked `metadata_needs_review`
- `20/30` projected records resolved to `model_type = unknown`
- `12/30` task labels were collapsed or misclassified
- `2/30` task labels were fully lost as `unknown`

Representative failures:

| Repo | HF task | Baseline Pumas result |
|------|---------|------------------------|
| `CompVis/stable-diffusion-v1-4` | `text-to-image` | `model_type = unknown` |
| `facebook/mask2former-swin-large-coco-instance` | `image-segmentation` | `task_type_primary = unknown`, `model_type = unknown` |
| `qualcomm/Yolo-v5` | `object-detection` | `task_type_primary = image-to-text`, `model_type = unknown` |
| `apple/coreml-depth-anything-v2-small` | `depth-estimation` | `task_type_primary = image-to-image`, `model_type = unknown` |
| `openai/whisper-base` | `automatic-speech-recognition` | `model_type = unknown` due resolver conflict |
| `liuwenhan/RankMistral100` | `text-ranking` | broad type drifted toward `llm` instead of `reranker` |

## Root Causes

### 1. Hint-only model typing was too strict

Remote HF metadata frequently has a reliable `pipeline_tag` but weak or missing architecture/config signals. The resolver only allowed hint-only resolution for a narrow subset of cases, so many otherwise well-labeled HF repos fell through to `unknown`.

Impact:

- diffusion repos with empty `architectures` or `model_type`
- vision repos exposed mainly through task tags
- LLM repos with sparse config metadata
- 3D repos where the task was present but hard classification evidence was absent

### 2. Task semantics were being collapsed through signature mappings

The importer normalized HF task labels into signatures and then mapped those signatures back to a smaller task set. That caused:

- `object-detection` to become `image-to-text`
- `depth-estimation` to become `image-to-image`
- `image-segmentation` to fall through to `unknown`
- `image-classification` to become `image-to-text`

The signature mapping is still useful for modalities, but it should not overwrite authoritative HF task labels when those labels are already present.

### 3. Generic LLM rules were overriding stronger modality/task evidence

Some model families expose generic LM architectures such as `ForConditionalGeneration` or `ForCausalLM` even when the actual HF task is audio or reranking. Without stronger guards:

- Whisper-family models conflicted between audio and LLM rules
- some reranker models using Mistral/Qwen causal architectures stayed in `llm`

### 4. HF tag inference was too narrow for search display fallback

When the search API omitted `pipeline_tag`, Pumas only inferred a small set of tasks from tags. That left valid task tags under-detected in remote search results.

### 5. Existing library metadata is stale

Several already-downloaded models still carry older projected metadata and will not self-heal until they are reclassified or backfilled.

Current examples in this repo:

- `shared-resources/models/vision/vit/birefnet_hr/metadata.json`
  - `pipeline_tag = image-segmentation`
  - `task_type_primary = unknown`
  - `input_modalities/output_modalities = unknown`
- `shared-resources/models/vision/idea-research/grounding-dino-base/metadata.json`
  - `pipeline_tag = zero-shot-object-detection`
  - `task_type_primary = image-to-text`
- `shared-resources/models/vision/apple/depthpro/metadata.json`
  - `pipeline_tag = null`
  - task fields are missing entirely
- `shared-resources/models/llm/qwen3/qwen3-reranker-4b-gguf/metadata.json`
  - `pipeline_tag = null`
  - `task_type_primary = unknown`
  - broad type still stored as `llm`

## Changes Made

### Resolver

Updated `rust/crates/pumas-core/src/model_library/model_type_resolver.rs` to:

- allow single, unambiguous HF-derived medium hints to resolve as low-confidence model types
- strengthen reranker disambiguation so `text-ranking` can override generic causal-LM architecture evidence
- add an audio disambiguation guard for Whisper/audio families that collide with generic generation rules

### Task normalization

Updated `rust/crates/pumas-core/src/model_library/task_signature.rs` and `rust/crates/pumas-core/src/index/model_index.rs` to:

- add `depth` and `bbox` output modalities
- normalize `object-detection` to `image->bbox`
- normalize `depth-estimation` to `image->depth`
- preserve explicit mappings for `image-segmentation`, `depth-estimation`, and `object-detection`

### Import/task projection

Updated `rust/crates/pumas-core/src/model_library/importer.rs` so that when HF already provides a pipeline tag, Pumas:

- keeps the original HF task label as `task_type_primary`
- still computes normalized input/output modalities from the task signature
- no longer collapses authoritative HF task labels into a smaller internal alias

### Remote HF search fallback

Updated `rust/crates/pumas-core/src/model_library/hf/search.rs` to infer more task labels from tags when HF search omits `pipeline_tag`, including:

- vision tasks
- depth tasks
- multimodal image-text tasks
- feature extraction
- video classification

### Shared type mapping and GUI display

Updated:

- `rust/crates/pumas-core/src/model_library/types.rs`
- `frontend/src/components/ModelManager.tsx`
- `frontend/src/components/ModelKindIcon.tsx`

to:

- correct `video-classification` to `vision` instead of `diffusion`
- recognize more vision/mask/depth/detection kind tokens in the GUI
- avoid “unknown icon” behavior for common HF task categories

### Audit and regression coverage

Added:

- `rust/crates/pumas-core/examples/hf_metadata_audit.rs`
- `frontend/src/components/ModelKindIcon.test.tsx`

and extended backend tests for:

- hint-only model typing
- reranker/audio disambiguation
- task normalization for depth/detection
- HF tag inference fallback
- preserving HF pipeline tags during import

## Post-Fix Result

Re-ran the **same** 30-model sample with the same seed after the changes.

Result:

- `0/30` projected records marked `metadata_needs_review`
- `0/30` projected records with `model_type = unknown`
- `0/30` task labels lost or collapsed for the sampled 2D vision/audio/diffusion/reranker cases
- `1/30` remaining mismatch

The remaining mismatch:

| Repo | HF task | Current Pumas result | Why it remains |
|------|---------|----------------------|----------------|
| `alexgusevski/LLaMA-Mesh-q6-mlx` | `text-to-3d` | `model_type = llm`, `task_type_primary = text-to-3d` | The repo advertises a 3D task but exposes a plain `LlamaForCausalLM` architecture. This needs a deliberate multimodal/3D disambiguation rule instead of a blanket pipeline-tag override. |

## Recommendations

### 1. Add a bulk metadata backfill for existing library entries

The code fixes improve all future HF-driven imports and metadata-only audits, but older library entries remain stale. Add a bulk repair path that:

- reloads stored `pipeline_tag` / `huggingface_evidence`
- reprojects `task_type_primary`
- re-normalizes `input_modalities` and `output_modalities`
- reruns model type resolution
- rewrites `metadata.json` and SQLite rows in place

This should be a library-wide maintenance command, not model-specific patching.

### 2. Add a dedicated multimodal/3D disambiguation pass

The remaining residual issue is a class of models where:

- HF task says `text-to-3d`
- architecture still looks like a conventional causal LM

Recommended approach:

- keep `task_type_primary` authoritative from HF
- add a new low-confidence guard for `text-to-3d` / `image-to-3d` when the repo name, tags, or config clearly indicate 3D generation
- avoid globally forcing all 3D pipeline tags to `diffusion` when hard architecture evidence strongly says otherwise

### 3. Use the audit example as a standing regression harness

The new `hf_metadata_audit.rs` example is useful enough to keep as an operational smoke test. Recommended usage:

- rerun before releases that touch HF search, classification, or metadata projection
- capture a fixed-seed sample in issue investigations
- periodically refresh the task coverage by adjusting the search-plan queries if HF taxonomy evolves

## Verification Notes

Commands used during this audit:

```bash
cargo test -p pumas-library model_type_resolver
cargo test -p pumas-library task_signature
cargo test -p pumas-library infer_pipeline_tag_from_tags
cargo test -p pumas-library test_import_in_place_preserves_hf_pipeline_tag_as_task_type

cd frontend
npm run test:run -- ModelKindIcon

cd ../rust
cargo run -p pumas-library --example hf_metadata_audit -- \
  --sample-size 30 \
  --seed 1775854833 \
  --markdown /tmp/pumas-hf-metadata-audit-final.md \
  --json /tmp/pumas-hf-metadata-audit-final.json
```
