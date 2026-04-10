# Local Library Findings - 2026-04-10

## Scope

This report audits the current on-disk library under
`shared-resources/models/` and the SQLite index in
`shared-resources/models/models.db`.

The goal is to identify why incorrectly organized models are still visible in
the local library even after the recent resolver and reconciliation work.

## Snapshot

- Audit date: `2026-04-10`
- Metadata files on disk: `44`
- SQLite rows in `models`: `47`
- Distinct persisted `repo_id` values in SQLite: `39`
- Duplicate `repo_id` groups in SQLite: `2`
- `unknown/...` model ids still present in SQLite: `3`
- Records with a persisted `pipeline_tag` but stale or collapsed
  `task_type_primary`: `12`

## Key Findings

### 1. Duplicate repo entries still exist in persisted state

Two duplicate `repo_id` groups are still present in SQLite:

| Repo ID | Persisted IDs |
| ------- | ------------- |
| `KittenML/kitten-tts-mini-0.8` | `audio/kittenml/kitten-tts-mini-0_8`, `unknown/kittenml/kitten-tts-mini-0_8` |
| `distil-whisper/distil-large-v3` | `audio/distil-whisper/distil-large-v3`, `unknown/distil-whisper/distil-large-v3` |

This is a direct explanation for the residual migration and organization
problems the GUI still shows. The canonical organized entry exists, but the
stale `unknown/...` entry also remains indexed.

### 2. `unknown/...` paths are still indexed even when stronger evidence exists

Current `unknown/...` rows:

| Model ID | Repo ID | Pipeline Tag | Task Type |
| -------- | ------- | ------------ | --------- |
| `unknown/distil-whisper/distil-large-v3` | `distil-whisper/distil-large-v3` | `automatic-speech-recognition` | `unknown` |
| `unknown/kittenml/kitten-tts-mini-0_8` | `KittenML/kitten-tts-mini-0.8` | `null` | `unknown` |
| `unknown/openai/whisper-large-v3-turbo` | `openai/whisper-large-v3-turbo` | `null` | `unknown` |

This indicates the current repair path is not fully converging stale
metadata-less or weakly-classified entries back into their canonical model
type/family locations.

### 3. Task projection backfill is incomplete for existing library entries

Twelve persisted records still have a `pipeline_tag` that should drive a clear
task projection, but the stored `task_type_primary` is missing, `unknown`, or
collapsed to the wrong value.

Representative examples:

| Model ID | Pipeline Tag | Stored Task | Problem |
| -------- | ------------ | ----------- | ------- |
| `vision/idea-research/grounding-dino-base` | `zero-shot-object-detection` | `image-to-text` | old collapsed task mapping still persisted |
| `vision/vit/birefnet_hr` | `image-segmentation` | `unknown` | segmentation backfill never repaired |
| `vision/vit/rmbg-2_0` | `image-segmentation` | `unknown` | segmentation backfill never repaired |
| `embedding/qwen/qwen3-embedding-06b` | `feature-extraction` | `null` | embedding task not reprojected |
| `embedding/qwen3/qwen3-embedding-06b-gguf` | `feature-extraction` | `null` | embedding task not reprojected |
| `embedding/vit/qwen3-vl-embedding-2b` | `feature-extraction` | `null` | embedding task not reprojected |
| `audio/openmoss-team/moss-soundeffect` | `text-to-audio` | `null` | audio task not reprojected |
| `llm/microsoft/florence-2-large` | `image-text-to-text` | `unknown` | multimodal task still stale |

The important distinction is that the live HF audit no longer shows these task
families failing on fresh metadata-only imports. The problem here is persisted
library state, not only live classification.

### 4. Path/family drift still exists in stored metadata

One clear family/path mismatch remains:

| Model ID | Path Family | Stored Metadata Family |
| -------- | ----------- | ---------------------- |
| `llm/vit/qwen-image-2512-heretic` | `vit` | `catplusplus` |

This is evidence that some older moves or family repairs updated only part of
the persisted state. The path, `model_id`, and metadata family are not fully
normalized together.

### 5. SQLite state is ahead of filesystem truth

The local library currently has:

- `44` on-disk `metadata.json` files
- `47` SQLite `models` rows

There are no missing filesystem paths for the current rows, so this is not a
simple broken-path problem. Instead, the index contains multiple persisted rows
for logically duplicated assets, which is why GUI organization can still look
wrong even when the canonical directory already exists.

## Comparison With Live HF Audit

The live HF metadata audit from
`docs/audits/hf-metadata-audit-2026-04-10.{md,json}` sampled `48` random repos
across text, reranking, audio, image, depth, segmentation, detection, and 3D.

That audit found only one remaining recurring live-classification problem:

- `text-to-3d` repos of the `LLaMA-Mesh` shape still resolve as low-confidence
  `llm` even when the task stays correctly stored as `text-to-3d`

Everything else in the sampled HF audit projected cleanly.

This comparison matters because it narrows the main current failure mode:

- Fresh HF-driven classification is mostly improved
- Existing local-library state is not being backfilled and normalized
  aggressively enough

## Root Cause Assessment

### Root Cause A: repair flows are not reprojecting persisted task fields broadly enough

Resolver and task-signature logic improved for new imports and metadata-only
audits, but existing `metadata.json` files with older `pipeline_tag` values are
not being systematically reprojected into:

- `task_type_primary`
- `input_modalities`
- `output_modalities`
- `task_classification_source`
- `task_classification_confidence`

### Root Cause B: duplicate cleanup is not fully removing stale `unknown/...` entries

The presence of both canonical and `unknown/...` rows for the same `repo_id`
shows that cleanup is not always converging to a single authoritative entry.
This likely leaves migration reports and GUI organization with stale collisions.

### Root Cause C: family/path normalization repair is not applied as a full invariant check

The `qwen-image-2512-heretic` row shows that family repair can still leave path
and metadata out of sync. Repair logic should treat these fields as a single
invariant, not as partially independent projections.

### Root Cause D: 3D task disambiguation is still under-specified

The live HF audit shows a remaining non-local-library issue: several
`text-to-3d` repos with strong `LlamaForCausalLM` architecture signals still
resolve to `llm`. That is a real resolver gap, but it is a much smaller problem
than the persisted-library drift above.

## Recommended Next Code Changes

### 1. Add a library-wide metadata backfill pass for persisted evidence

Implement a non-model-specific repair path that walks all existing entries and
reprojects task/type fields from stored:

- `pipeline_tag`
- `huggingface_evidence`
- `repo_id`
- current resolver/task-signature rules

This should update both `metadata.json` and SQLite rows in one pass.

### 2. Make duplicate convergence repo-centric, not path-centric

When two rows share the same normalized `repo_id`, the repair flow should:

- prefer the canonical typed path over `unknown/...`
- merge or preserve the stronger metadata-bearing entry
- delete or de-index the stale duplicate row
- then rebuild/refresh the index entry for the canonical path only

### 3. Enforce path/metadata family consistency during repair

Repair should validate and rewrite these together:

- directory family segment
- `model_id`
- `family`
- `cleaned_name`

If one source disagrees, prefer the strongest evidence source such as
`repo_id`/HF evidence instead of leaving the path and metadata split.

### 4. Add a dedicated 3D disambiguation rule

For the remaining HF issue class, add an explicit low-confidence rule for
`text-to-3d` and `image-to-3d` repos whose task/tags clearly indicate 3D
generation even when the architecture looks like a plain LLM.

### 5. Add regression coverage for persisted-library backfill

Add fixtures and tests for:

- duplicate `unknown/...` plus canonical entries sharing one `repo_id`
- persisted segmentation/object-detection/embedding/audio records with stale
  `task_type_primary`
- family/path drift after reclassification or relocation
- idempotent rerun of the repair flow

## Commands Used

The findings above were derived from:

- live HF audit via `cargo run --manifest-path rust/Cargo.toml -p pumas-library --example hf_metadata_audit`
- direct inspection of `shared-resources/models/**/*.json`
- direct inspection of `shared-resources/models/models.db`

No model weights were downloaded as part of this audit.
