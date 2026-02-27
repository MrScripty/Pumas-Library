# Pumas Library Metadata v2 + Dependency System Master Plan

## Status
Draft

## Audience
Pumas Library team and maintainers of local apps/language bindings that consume Pumas APIs.

## Scope and Cutover Policy
1. Metadata v2 and model-level dependencies are breaking changes.
2. Legacy metadata/dependency methods are removed immediately when new systems ship.
3. There is no backward-compatibility window.
4. Migration impact is local consumers only (apps and language bindings using local Pumas APIs).

## Implementation Procedure (Actionable)
Execution model:
1. Land this work in small, compilable slices with tests at each slice.
2. Keep storage migrations additive until final cutover switch; then remove legacy code paths in one release window.
3. Ship core + bindings together only after contract tests pass.

Milestone 1: Metadata v2 schema foundation
1. Extend `ModelMetadata` with v2 task/model-type provenance fields, review fields, and dependency binding references.
2. Keep serde defaults permissive so existing `metadata.json` files still deserialize.
3. Add validators for bounded confidence scores and unknown-value review requirements.
4. Add unit tests for deserialize/serialize roundtrips on old + new metadata payloads.

Milestone 2: Task signature normalization engine
1. Implement canonical signature parser/normalizer (`from->to`, modality aliases, deterministic ordering).
2. Emit normalization status/warnings and enforce idempotency.
3. Add test vectors for separators, aliases, unknown-token handling, and round-trip stability.

Milestone 3: SQLite rule registries + migrations
1. Add schema creation/migration for:
   - `task_signature_mappings`
   - `model_type_arch_rules`
   - `model_type_config_rules`
   - metadata provenance tables (`model_metadata_baselines`, overlays/history)
   - dependency tables (`dependency_profiles`, `model_dependency_bindings`, `dependency_binding_history`)
2. Seed initial mapping/rule rows idempotently.
3. Backfill metadata baselines from existing `models.metadata_json`.
4. Add migration tests asserting table presence, unique indexes, and re-run idempotency.

Milestone 4: Classification and resolver integration
1. Replace direct pipeline-tag-to-model-type shortcut in importer path with:
   - task signature normalization + mapping lookup
   - separate model-type resolver using hard signals (architectures/config model_type)
2. On signature miss, upsert pending mapping row and set review flags.
3. On resolver miss/conflict, set `model_type=unknown`, mark review, continue import/download.
4. Add unit tests for precedence, conflicts, and unknown behavior.

Milestone 5: Dependency profile/binding APIs
1. Implement profile + multi-binding persistence and deterministic resolution ordering.
2. Add `selected_binding_ids` required-closure checks and deterministic error codes (`required_binding_omitted`, `profile_conflict`).
3. Expose pluralized model dependency APIs in core + bindings.
4. Add integration tests for multi-row bindings per model and conflict/manual states.

Milestone 6: Metadata edit overlays and audit trail
1. Implement baseline + active overlay read path (`effective = baseline + overlay`).
2. Implement overlay write transaction (supersede old active, create new active, append history events).
3. Implement reset-to-original path (revert active overlay + history event).
4. Add integration tests for overlay lifecycle and deterministic history output.

Milestone 7: Migration + reorganization runner
1. Add dry-run report generation (classification, target path, move plan, dependency/license findings).
2. Add checkpointed move + rewrite flow with resume safety.
3. Rebuild index and validate referential integrity post-migration.
4. Emit machine-readable and human-readable migration reports.

Milestone 8: API/binding cutover
1. Remove legacy metadata/dependency methods and fields from implementation/docs.
2. Update affected language bindings and contract tests in same cutover.
3. Publish migration guide for local consumers with old->new API mapping and examples.
4. Release only when acceptance checklist in this plan is fully green.

## Goals
1. Enforce deterministic model taxonomy and canonical directory placement.
2. Add explicit model-level dependency contracts and reusable dependency profiles.
3. Add model dependency APIs in `pumas-core`.
4. Implement deterministic, source-first classification with review flags for ambiguous/unknown cases.
5. Migrate existing model library contents safely with auditable, rerunnable reports.

## Non-Goals
1. Preserve old metadata/dependency API behavior.
2. Block import/download when licenses are unresolved.
3. Hardcode every future Hugging Face task signature in source code.

## Canonical Metadata v2 Contract
Required fields:
1. `schema_version`
2. `task_type_primary`
3. `input_modalities`
4. `output_modalities`
5. `task_classification_source`
6. `task_classification_confidence`
7. `model_type_resolution_source`
8. `model_type_resolution_confidence`

Key optional/extended fields:
1. `task_type_secondary`
2. `runtime_engine_hints`
3. `dependency_bindings` (`Vec<DependencyBindingRef>`)
4. `requires_custom_code`
5. `custom_code_sources`
6. `metadata_needs_review`
7. `review_reasons` (`Vec<String>`)
8. `review_status`
9. `reviewed_by`
10. `reviewed_at`
11. model card and license artifact metadata (path/source/hash/status)

Validation requirements:
1. `model_type` must be a canonical enum value, including `unknown`.
2. `task_type_primary` must be a canonical task tag, including `unknown`.
3. Non-empty input/output modalities.
4. `task_classification_confidence` and `model_type_resolution_confidence` must be bounded (`0.0..1.0`).
5. Custom code sources required when `requires_custom_code=true`.
6. Every referenced dependency binding target (`profile_id`, `profile_version`) must exist.
7. If `task_type_primary=unknown`, then:
   - `metadata_needs_review=true`
   - `review_reasons` must be non-empty
   - `task_classification_confidence=0.0`
8. If `model_type=unknown`, then:
   - `metadata_needs_review=true`
   - `review_reasons` must be non-empty
   - `model_type_resolution_confidence=0.0`
9. `review_reasons` normalization:
   - values must be deduplicated
   - values must be lowercase
   - storage order must be lexicographically sorted for deterministic output
10. Canonical `review_reasons` values (initial):
   - `invalid-task-signature`
   - `unknown-task-signature`
   - `model-type-unresolved`
   - `model-type-conflict`
   - `model-type-low-confidence`
   - `missing-license`
   - `unknown-profile`
   - `manual-intervention-required`

## Classification Policy (Task Signature Driven)
Classification is based on source task signature (`from -> to`), including multimodal signatures like `text+image->image`.

Source precedence (required):
1. Source-provided type/task from model origin (currently Hugging Face repo metadata) is first.
2. If missing, use source config/tag signals from the same origin.
3. Only then use local heuristics for task semantics extraction.

Heuristic scope boundaries:
1. Allowed heuristics:
   - task signature normalization/parsing (for example converting source task strings into canonical `from->to` signatures)
   - modality extraction/normalization for `input_modalities` and `output_modalities`
2. Disallowed heuristics:
   - any heuristic that directly sets or overrides `model_type`
   - repo-name/family-text heuristics for `model_type`
3. `model_type` must be resolved only by the model-type resolver policy and rule tables in this plan.

### Task Signature Normalization Specification
Purpose:
1. Convert heterogeneous source task labels into a deterministic canonical signature key.
2. Produce stable `input_modalities` and `output_modalities` for metadata and mapping lookup.

Canonical signature format:
1. `signature_key = <inputs>-><outputs>`
2. `<inputs>` and `<outputs>` are `+`-joined canonical modality tokens.
3. Modalities are deduplicated and sorted by fixed order:
   - `text`, `image`, `audio`, `video`, `document`, `mask`, `keypoints`, `action`, `3d`, `embedding`, `tabular`, `timeseries`, `rl-state`, `any`, `unknown`

Canonical modality tokens:
1. `text`
2. `image`
3. `audio`
4. `video`
5. `document`
6. `mask`
7. `keypoints`
8. `action`
9. `3d`
10. `embedding`
11. `tabular`
12. `timeseries`
13. `rl-state`
14. `any`
15. `unknown` (only when parse cannot confidently resolve one side)

Coverage note:
1. This token set is designed to cover current Hugging Face task families including multimodal/any-to-any and reinforcement-learning style tasks.
2. New tokens may be added via versioned schema updates when Hugging Face introduces new modality concepts.

Accepted raw separators:
1. Direction separators: `->`, `=>`, `to`, `2`, `→`
2. Multi-modality separators: `+`, `,`, `&`, `and`, `/`

Normalization algorithm (deterministic):
1. Lowercase, trim, collapse repeated whitespace.
2. Normalize unicode arrows and variant direction separators to `->`.
3. Split into exactly two sides (`lhs`, `rhs`) using the first directional separator found.
4. Tokenize each side using multi-modality separators.
5. Normalize each token via alias table (below).
6. Remove empty tokens, dedupe tokens, and sort by canonical modality order.
7. If one side resolves empty, set that side to `unknown` and mark parse warning.
8. Emit:
   - `signature_key`
   - `input_modalities`
   - `output_modalities`
   - `normalization_status` (`ok` | `warning` | `error`)
   - `normalization_warnings[]`

Alias table (initial):
1. `txt`, `natural-language`, `language`, `nlp` -> `text`
2. `img`, `images`, `vision-image`, `picture`, `pictures` -> `image`
3. `speech`, `voice`, `sound`, `music` -> `audio`
4. `vid`, `movie`, `movies`, `clip` -> `video`
5. `doc`, `docs`, `document-image`, `pdf` -> `document`
6. `segmentation-mask`, `binary-mask`, `instance-mask` -> `mask`
7. `pose`, `skeleton`, `landmarks` -> `keypoints`
8. `action-label`, `activity` -> `action`
9. `mesh`, `pointcloud`, `point-cloud` -> `3d`
10. `embeddings`, `vector`, `vectors`, `feature`, `features` -> `embedding`
11. `table`, `tables`, `csv`, `structured` -> `tabular`
12. `time-series`, `series`, `temporal` -> `timeseries`
13. `reinforcement-learning`, `rl`, `rl-state` -> `rl-state`
14. `any-to-any`, `multi-any` -> `any`

Invalid/ambiguous input handling:
1. If no directional separator is found:
   - `signature_key = unknown->unknown`
   - `normalization_status = error`
   - set `task_type_primary=unknown`, `metadata_needs_review=true`, `review_reasons` includes `invalid-task-signature`
2. If unrecognized tokens remain on either side:
   - keep recognized modalities
   - set `normalization_status = warning`
   - append warning entries with unresolved tokens
3. Unknown or warning status never blocks import/download.

Round-trip determinism rules:
1. Re-normalizing an already canonical signature must return identical output.
2. Signature generation must be stable across platforms/locales.
3. Matching must be case-insensitive at input, case-stable at output (always lowercase canonical).

Examples:
1. `Text to Image` -> `text->image`
2. `text,image -> image` -> `text+image->image`
3. `speech2text` -> `audio->text`
4. `video and text to text` -> `text+video->text`
5. `unknownformat` -> `unknown->unknown` (`error`)

Test vectors (minimum required):
1. Direction separator variants (`to`, `->`, `=>`, unicode arrow).
2. Multi-modality separator variants (`+`, `,`, `and`, `/`, `&`).
3. Alias expansion for each canonical modality.
4. Duplicate modality collapse (`text+text->image`).
5. Mixed known/unknown tokens with warning behavior.
6. Idempotency test (`normalize(normalize(x)) == normalize(x)`).
7. HF-style any-to-any inputs (`any-to-any`, `any->any`).
8. RL-style inputs (`rl-state->action`, `state->action` after aliasing).

Decision behavior:
1. If signature mapping exists, set task semantics fields (`task_type_primary`, `input_modalities`, `output_modalities`) from mapping and set `task_classification_source` + `task_classification_confidence`.
2. If signature mapping does not exist, set `task_type_primary=unknown`, set `task_classification_source=runtime-discovered-signature`, set `task_classification_confidence=0.0`, set `metadata_needs_review=true`, set `review_reasons` includes `unknown-task-signature`, and preserve parsed modalities when available.
3. Resolve `model_type` in a separate model-specific step (not from signature mapping).
4. If `model_type` cannot be resolved, set `model_type=unknown`, set `model_type_resolution_source=unresolved`, set `model_type_resolution_confidence=0.0`, set `metadata_needs_review=true`, and set `review_reasons` includes `model-type-unresolved`.
5. Unknown/ambiguous classification never blocks download/import.

## SQLite Mutable Mapping Registry
Mapping rules are stored in library SQLite to support runtime extension as Hugging Face introduces new task signatures.

Proposed table: `task_signature_mappings`

```sql
CREATE TABLE IF NOT EXISTS task_signature_mappings (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  signature_key TEXT NOT NULL,
  mapping_version INTEGER NOT NULL,
  input_modalities_json TEXT NOT NULL,
  output_modalities_json TEXT NOT NULL,
  task_type_primary TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 100,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
  source TEXT NOT NULL DEFAULT 'system',
  supersedes_id INTEGER,
  change_reason TEXT,
  notes TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  FOREIGN KEY (supersedes_id) REFERENCES task_signature_mappings(id),
  UNIQUE(signature_key, mapping_version)
);

CREATE INDEX IF NOT EXISTS idx_task_signature_mappings_lookup
  ON task_signature_mappings(status, signature_key, priority, mapping_version DESC);

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_signature_mappings_one_active
  ON task_signature_mappings(signature_key)
  WHERE status = 'active';

CREATE UNIQUE INDEX IF NOT EXISTS idx_task_signature_mappings_one_pending
  ON task_signature_mappings(signature_key)
  WHERE status = 'pending';
```

Initial mapping table draft:
| signature_key | task_type_primary |
|---|---|
| `text->text` | `text-generation` |
| `text->image` | `text-to-image` |
| `image->image` | `image-to-image` |
| `text+image->image` | `text-image-to-image` |
| `text->audio` | `text-to-audio` |
| `audio->audio` | `audio-to-audio` |
| `audio->text` | `audio-to-text` |
| `text->audio+text` | `text-to-audio-text` |
| `text->embedding` | `text-embedding` |
| `image->embedding` | `image-embedding` |
| `audio->embedding` | `audio-embedding` |
| `image->text` | `image-to-text` |
| `video->text` | `video-to-text` |
| `text+image->text` | `visual-question-answering` |
| `text+video->text` | `video-question-answering` |
| `text->video` | `text-to-video` |
| `text->3d` | `text-to-3d` |
| `image->3d` | `image-to-3d` |

Runtime mutability rules:
1. Normalize source signature into `signature_key`.
2. Resolve mapping using the active row for that `signature_key` (if present).
3. If no active row exists, upsert one pending row for that `signature_key`:
   - If pending exists, update it in place (`updated_at`, optional notes/counters).
   - If pending does not exist, insert a new pending row with `mapping_version = COALESCE(MAX(mapping_version), 0) + 1`, `source='runtime-discovered'`, and `task_type_primary='unknown'`.
4. Mark model metadata for review with `review_reasons` includes `unknown-task-signature`.
5. Continue import/download without blocking.
6. Promote pending mapping via explicit state transition transaction:
   - Set current active row to `deprecated` (if present).
   - Set pending row to `active`.
   - Preserve `supersedes_id` linkage for auditability.
7. Keep historical rows for the same `signature_key`; do not delete prior versions.

## Model Type Resolution Policy (Model-Specific)
`model_type` is resolved from model-specific/source metadata and must not be inferred from task signature alone.

Resolution precedence:
1. Source architecture/runtime metadata from model origin (currently Hugging Face repo/config).
2. Source tags and config signals combined with runtime engine hints.
3. Curated architecture/engine rules maintained in library logic.
4. Fallback to `model_type=unknown` with review-required metadata.

Disallowed signal class:
1. Do not use soft repo name/family string heuristics for model type resolution.

Confidence scoring:
1. Start at `0.0`.
2. Primary hard-signal match (architecture/runtime): `+0.70`.
3. Additional agreeing hard signal: `+0.20`.
4. Agreeing medium signal (tags/config): `+0.10`.
5. Conflicting medium signal: `-0.20`.
6. Conflicting hard signals force `model_type=unknown`, `model_type_resolution_confidence=0.0`, and review-required metadata.
7. Clamp score to `0.0..1.0`.

Decision thresholds:
1. `>=0.85`: auto-accept resolved `model_type`.
2. `0.60..0.84`: accept but mark review-required when routing-critical policy applies.
3. `<0.60`: set `model_type=unknown`, set `model_type_resolution_confidence=0.0`, and mark review-required.

Initial hard-signal mapping table (architectures/config classes -> `model_type`):

Architecture/class patterns (`config.architectures[]`, exact or suffix match):
| Hard signal | Match style | model_type |
|---|---|---|
| `*ForCausalLM` | suffix | `llm` |
| `*ForMaskedLM` | suffix | `llm` |
| `*ForConditionalGeneration` | suffix | `llm` |
| `*ForSequenceClassification` | suffix | `llm` |
| `*ForTokenClassification` | suffix | `llm` |
| `*ForQuestionAnswering` | suffix | `llm` |
| `*ForSpeechSeq2Seq` | suffix | `audio` |
| `*ForAudioClassification` | suffix | `audio` |
| `Whisper*` | prefix | `audio` |
| `Encodec*` | prefix | `audio` |
| `*ForImageClassification` | suffix | `vision` |
| `*ForObjectDetection` | suffix | `vision` |
| `*ForSemanticSegmentation` | suffix | `vision` |
| `*ForImageSegmentation` | suffix | `vision` |
| `CLIPVisionModel*` | prefix | `vision` |
| `UNet2DConditionModel` | exact | `diffusion` |
| `UNet2DModel` | exact | `diffusion` |
| `AutoencoderKL` | exact | `diffusion` |
| `VQModel` | exact | `diffusion` |
| `StableDiffusion*Pipeline` | wildcard | `diffusion` |
| `DiffusionPipeline` | exact | `diffusion` |

`config.model_type` exact values (hard when provided by source config):
| Hard signal (`config.model_type`) | model_type |
|---|---|
| `llama`, `mistral`, `mixtral`, `gpt2`, `gpt_neo`, `gpt_neox`, `gptj`, `phi`, `phi3`, `qwen2`, `qwen3`, `gemma`, `gemma2`, `gemma3`, `deepseek_v2`, `deepseek_v3`, `falcon`, `mpt`, `bloom`, `opt`, `codegen`, `starcoder2`, `rwkv`, `rwkv5`, `rwkv6`, `mamba`, `mamba2`, `jamba`, `dbrx`, `stablelm` | `llm` |
| `stable_diffusion`, `sdxl`, `kandinsky`, `pixart` | `diffusion` |
| `whisper`, `wav2vec2`, `hubert`, `wavlm`, `seamless_m4t`, `bark`, `musicgen`, `encodec`, `speecht5`, `mms` | `audio` |
| `vit`, `swin`, `convnext`, `deit`, `beit`, `dinov2`, `clip`, `siglip`, `blip`, `blip2` | `vision` |
| `sentence-transformers`, `bge`, `e5`, `gte`, `jina-embeddings` | `embedding` |

Resolver notes:
1. If architecture/class and `config.model_type` disagree, treat as hard conflict (`model_type=unknown`, review required).
2. These are initial allowlist mappings and should be extended over time via curated rule updates.
3. Unmatched values are not guessed; they resolve to `unknown` pending review.

SQLite resolver rule tables (exact):

```sql
CREATE TABLE IF NOT EXISTS model_type_arch_rules (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  pattern TEXT NOT NULL,
  match_style TEXT NOT NULL CHECK (match_style IN ('exact', 'prefix', 'suffix', 'wildcard')),
  model_type TEXT NOT NULL CHECK (model_type IN ('llm', 'diffusion', 'audio', 'vision', 'embedding', 'unknown')),
  priority INTEGER NOT NULL DEFAULT 100,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
  source TEXT NOT NULL DEFAULT 'system',
  notes TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_model_type_arch_rules_active_unique
  ON model_type_arch_rules(pattern, match_style)
  WHERE status = 'active';

CREATE INDEX IF NOT EXISTS idx_model_type_arch_rules_lookup
  ON model_type_arch_rules(status, priority, pattern, match_style);

CREATE TABLE IF NOT EXISTS model_type_config_rules (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  config_model_type TEXT NOT NULL,
  model_type TEXT NOT NULL CHECK (model_type IN ('llm', 'diffusion', 'audio', 'vision', 'embedding', 'unknown')),
  priority INTEGER NOT NULL DEFAULT 100,
  status TEXT NOT NULL DEFAULT 'active' CHECK (status IN ('active', 'pending', 'deprecated')),
  source TEXT NOT NULL DEFAULT 'system',
  notes TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_model_type_config_rules_active_unique
  ON model_type_config_rules(config_model_type)
  WHERE status = 'active';

CREATE INDEX IF NOT EXISTS idx_model_type_config_rules_lookup
  ON model_type_config_rules(status, priority, config_model_type);
```

Initial seed SQL (normalize all source values to lowercase before matching):

```sql
-- Architecture/class rules
INSERT OR IGNORE INTO model_type_arch_rules (pattern, match_style, model_type, priority, status, source) VALUES
  ('ForCausalLM', 'suffix', 'llm', 100, 'active', 'system'),
  ('ForMaskedLM', 'suffix', 'llm', 100, 'active', 'system'),
  ('ForConditionalGeneration', 'suffix', 'llm', 100, 'active', 'system'),
  ('ForSequenceClassification', 'suffix', 'llm', 100, 'active', 'system'),
  ('ForTokenClassification', 'suffix', 'llm', 100, 'active', 'system'),
  ('ForQuestionAnswering', 'suffix', 'llm', 100, 'active', 'system'),
  ('ForSpeechSeq2Seq', 'suffix', 'audio', 100, 'active', 'system'),
  ('ForAudioClassification', 'suffix', 'audio', 100, 'active', 'system'),
  ('Whisper', 'prefix', 'audio', 100, 'active', 'system'),
  ('Encodec', 'prefix', 'audio', 100, 'active', 'system'),
  ('ForImageClassification', 'suffix', 'vision', 100, 'active', 'system'),
  ('ForObjectDetection', 'suffix', 'vision', 100, 'active', 'system'),
  ('ForSemanticSegmentation', 'suffix', 'vision', 100, 'active', 'system'),
  ('ForImageSegmentation', 'suffix', 'vision', 100, 'active', 'system'),
  ('CLIPVisionModel', 'prefix', 'vision', 100, 'active', 'system'),
  ('UNet2DConditionModel', 'exact', 'diffusion', 100, 'active', 'system'),
  ('UNet2DModel', 'exact', 'diffusion', 100, 'active', 'system'),
  ('AutoencoderKL', 'exact', 'diffusion', 100, 'active', 'system'),
  ('VQModel', 'exact', 'diffusion', 100, 'active', 'system'),
  ('StableDiffusion*Pipeline', 'wildcard', 'diffusion', 100, 'active', 'system'),
  ('DiffusionPipeline', 'exact', 'diffusion', 100, 'active', 'system');

-- config.model_type rules
INSERT OR IGNORE INTO model_type_config_rules (config_model_type, model_type, priority, status, source) VALUES
  ('llama', 'llm', 100, 'active', 'system'),
  ('mistral', 'llm', 100, 'active', 'system'),
  ('mixtral', 'llm', 100, 'active', 'system'),
  ('gpt2', 'llm', 100, 'active', 'system'),
  ('gpt_neo', 'llm', 100, 'active', 'system'),
  ('gpt_neox', 'llm', 100, 'active', 'system'),
  ('gptj', 'llm', 100, 'active', 'system'),
  ('phi', 'llm', 100, 'active', 'system'),
  ('phi3', 'llm', 100, 'active', 'system'),
  ('qwen2', 'llm', 100, 'active', 'system'),
  ('qwen3', 'llm', 100, 'active', 'system'),
  ('gemma', 'llm', 100, 'active', 'system'),
  ('gemma2', 'llm', 100, 'active', 'system'),
  ('gemma3', 'llm', 100, 'active', 'system'),
  ('deepseek_v2', 'llm', 100, 'active', 'system'),
  ('deepseek_v3', 'llm', 100, 'active', 'system'),
  ('falcon', 'llm', 100, 'active', 'system'),
  ('mpt', 'llm', 100, 'active', 'system'),
  ('bloom', 'llm', 100, 'active', 'system'),
  ('opt', 'llm', 100, 'active', 'system'),
  ('codegen', 'llm', 100, 'active', 'system'),
  ('starcoder2', 'llm', 100, 'active', 'system'),
  ('rwkv', 'llm', 100, 'active', 'system'),
  ('rwkv5', 'llm', 100, 'active', 'system'),
  ('rwkv6', 'llm', 100, 'active', 'system'),
  ('mamba', 'llm', 100, 'active', 'system'),
  ('mamba2', 'llm', 100, 'active', 'system'),
  ('jamba', 'llm', 100, 'active', 'system'),
  ('dbrx', 'llm', 100, 'active', 'system'),
  ('stablelm', 'llm', 100, 'active', 'system'),
  ('stable_diffusion', 'diffusion', 100, 'active', 'system'),
  ('sdxl', 'diffusion', 100, 'active', 'system'),
  ('kandinsky', 'diffusion', 100, 'active', 'system'),
  ('pixart', 'diffusion', 100, 'active', 'system'),
  ('whisper', 'audio', 100, 'active', 'system'),
  ('wav2vec2', 'audio', 100, 'active', 'system'),
  ('hubert', 'audio', 100, 'active', 'system'),
  ('wavlm', 'audio', 100, 'active', 'system'),
  ('seamless_m4t', 'audio', 100, 'active', 'system'),
  ('bark', 'audio', 100, 'active', 'system'),
  ('musicgen', 'audio', 100, 'active', 'system'),
  ('encodec', 'audio', 100, 'active', 'system'),
  ('speecht5', 'audio', 100, 'active', 'system'),
  ('mms', 'audio', 100, 'active', 'system'),
  ('vit', 'vision', 100, 'active', 'system'),
  ('swin', 'vision', 100, 'active', 'system'),
  ('convnext', 'vision', 100, 'active', 'system'),
  ('deit', 'vision', 100, 'active', 'system'),
  ('beit', 'vision', 100, 'active', 'system'),
  ('dinov2', 'vision', 100, 'active', 'system'),
  ('clip', 'vision', 100, 'active', 'system'),
  ('siglip', 'vision', 100, 'active', 'system'),
  ('blip', 'vision', 100, 'active', 'system'),
  ('blip2', 'vision', 100, 'active', 'system'),
  ('sentence-transformers', 'embedding', 100, 'active', 'system'),
  ('bge', 'embedding', 100, 'active', 'system'),
  ('e5', 'embedding', 100, 'active', 'system'),
  ('gte', 'embedding', 100, 'active', 'system'),
  ('jina-embeddings', 'embedding', 100, 'active', 'system');
```

Conflict handling:
1. If source signals conflict, set `metadata_needs_review=true` and `review_reasons` includes `model-type-conflict`.
2. Do not block download/import on unresolved conflicts.

Example:
1. `text->text` can resolve to `llm` or `diffusion` depending on model-specific source signals.

## Canonical Library Reorganization
Canonical path format remains:
1. `model_type/family/name`

Reorganization rules:
1. Generate move plan from current path to canonical v2 path.
2. Resolve `model_type` using model-specific resolution before computing target path.
3. Apply idempotent move operations with checkpoints.
4. Rewrite metadata to v2 at destination.
5. Rebuild SQLite index after successful move+rewrite.
6. Emit old-to-new path mappings in migration report.

Examples:
1. If resolved `model_type=diffusion`: `<diffusion>/<family>/<name>`
2. If resolved `model_type=audio`: `<audio>/<family>/<name>`
3. If resolved `model_type=llm`: `<llm>/<family>/<name>`

## Dependency Architecture
Dependency profiles:
1. Use reusable model-level profiles rather than duplicating per model.
2. Profile fields include environment kind, requirement sources, platform constraints, install policy, validation probes, notes.

Model dependency bindings (multi-row per model by design):
1. A model may have multiple active dependency bindings simultaneously.
2. Bindings cover backend/runtime requirements and custom-code requirements (for example transformers/candle/llamacpp/pytorch/custom modules).
3. Suggested binding fields:
   - `binding_id`
   - `model_id`
   - `profile_id`
   - `profile_version`
   - `binding_kind` (`required_core`, `required_custom`, `optional_feature`, `optional_accel`)
   - `backend_key` (for example `transformers`, `candle`, `llamacpp`, `pytorch`, `custom`)
   - `platform_selector`
   - `status` (`active`, `deprecated`)
   - `priority`
   - `attached_by`, `attached_at`
4. Multiple rows for one `model_id` are expected and valid.

Dependency source-of-truth rule:
1. SQLite dependency tables are authoritative (`dependency_profiles`, `model_dependency_bindings`).
2. Metadata field `dependency_bindings` is a denormalized projection for read/UI convenience.
3. On mismatch, resolver and APIs must trust SQLite tables.
4. Projection refresh occurs after binding writes and during reindex/rebuild operations.

Canonical dependency persistence (SQLite):
1. `dependency_profiles(profile_id, profile_version, profile_hash, environment_kind, spec_json, created_at, PRIMARY KEY(profile_id, profile_version))`
2. `model_dependency_bindings(binding_id, model_id, profile_id, profile_version, binding_kind, backend_key, platform_selector, status, priority, attached_by, attached_at, FOREIGN KEY(profile_id, profile_version) REFERENCES dependency_profiles(profile_id, profile_version))`
3. `dependency_binding_history(event_id, binding_id, model_id, actor, action, old_value_json, new_value_json, reason, created_at)`
4. `profile_hash` is informational and must NOT be globally unique.
5. Add lookup index for duplicate-content awareness:
   - `CREATE INDEX IF NOT EXISTS idx_dependency_profiles_hash ON dependency_profiles(profile_hash);`

Duplicate profile awareness (UX optimization):
1. Detect duplicate-content profiles by grouping on `profile_hash`.
2. Expose duplicate groups in tooling/UI so consumers can reuse compatible environments instead of provisioning multiple equivalent environments.
3. Duplicate detection is advisory only; it does not block profile creation or binding.

Resolution behavior:
1. Resolver first applies explicit context (`backend_key`, `platform_context`) when provided.
2. Resolver builds a candidate binding set for the model.
3. Resolver output must be deterministic: binding plan ordering uses stable key sort, not `binding_id`.
4. Stable sort key:
   - (`binding_kind`, `backend_key`, `platform_selector`, `profile_id`, `profile_version`, `priority`, `binding_id`)
5. If context still matches multiple incompatible required bindings, do not guess; return `manual_intervention_required` and add `dependency-binding-conflict` to `review_reasons`.
6. If no profile can be resolved, return `unknown_profile`.
7. Best-effort discovery is allowed; unresolved dependency details do not block download/import.

Model-level dependency APIs:
1. `get_model_dependency_profiles(model_id, platform_context, backend_key?)`
2. `resolve_model_dependency_plan(model_id, platform_context, backend_key?)`
3. `check_model_dependencies(model_id, platform_context, backend_key?, selected_binding_ids?)`
4. `install_model_dependencies(model_id, platform_context, backend_key?, selected_binding_ids?)`
5. `list_models_needing_review(filter)`
6. `submit_model_review(model_id, patch, reviewer)`

Dependency API response contract:
1. Return per-binding results (not aggregate-only), including `binding_id`, `profile_id`, `profile_version`, `profile_hash`, `env_id`.
2. Deterministic environment key format per binding:
   - `{environment_kind}:{profile_id}:{profile_version}:{profile_hash}:{platform_key}:{backend_key}`
3. If same `env_id` is requested with different `profile_hash`, return `profile_conflict`.
4. If `selected_binding_ids` is provided, it must include all required bindings from the resolved plan.
5. Missing required bindings in caller selection return deterministic error code `required_binding_omitted` and no install action.

Dependency check/install states:
1. `ready`
2. `missing`
3. `failed`
4. `unknown_profile`
5. `manual_intervention_required`
6. `profile_conflict`

Operational rules:
1. Public internet sources are allowed (including GitHub and Hugging Face).
2. Dependency install attempts must log source URL/ref and result.
3. Failures must return structured actionable errors.
4. Offline mode fails fast if required dependencies are absent.
5. Install operations should be idempotent and safe to retry.

Manual intervention workflow:
1. Run best-effort dependency extraction from source metadata/config and any available README/model-card hints.
2. If extraction is incomplete/ambiguous, set review-required metadata and dependency state `manual_intervention_required`.
3. User resolves/edits dependency bindings through existing local UI metadata/edit workflow.
4. All manual dependency edits are audited in `dependency_binding_history` and remain resettable to baseline-derived values.

## License Governance Policy
1. Fetch and retain license artifact locally for each model during import or migration.
2. Record license artifact path, source URL, and hash in metadata.
3. If no license is found, set metadata license status to `license_unknown`.
4. `license_unknown` must never block download/import or normal local library operations.
5. Keep license artifacts alongside model metadata for local auditability.

## Metadata Editability and Provenance (GUI)
Metadata shown in the GUI (for example from Ctrl+click model details) may be user-editable for correction workflows.

Rules:
1. Preserve the original Pumas-authored metadata as immutable baseline.
2. User edits are stored as overlay patches, not in-place destructive replacement of baseline.
3. Effective metadata presented to consumers is `baseline + overlays`.
4. Provide explicit `reset-to-original` action that removes overlays and restores baseline values.
5. All edits must be audited with actor, timestamp, changed fields, old value, new value, and reason.

SQLite DDL (exact):

```sql
PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS model_metadata_baselines (
  model_id TEXT PRIMARY KEY,
  schema_version INTEGER NOT NULL,
  baseline_json TEXT NOT NULL CHECK (json_valid(baseline_json)),
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  created_by TEXT NOT NULL DEFAULT 'pumas-library',
  FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
);

-- Baseline is immutable once inserted.
CREATE TRIGGER IF NOT EXISTS trg_model_metadata_baselines_no_update
BEFORE UPDATE ON model_metadata_baselines
FOR EACH ROW
BEGIN
  SELECT RAISE(ABORT, 'model_metadata_baselines is immutable');
END;

CREATE TABLE IF NOT EXISTS model_metadata_overlays (
  overlay_id TEXT PRIMARY KEY, -- UUID/ULID generated by app layer
  model_id TEXT NOT NULL,
  overlay_json TEXT NOT NULL CHECK (json_valid(overlay_json)), -- JSON Merge Patch document
  status TEXT NOT NULL DEFAULT 'active'
    CHECK (status IN ('active', 'superseded', 'reverted')),
  reason TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  created_by TEXT NOT NULL,
  FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_model_metadata_overlays_model
  ON model_metadata_overlays(model_id, created_at);

-- Exactly one active overlay per model.
CREATE UNIQUE INDEX IF NOT EXISTS idx_model_metadata_overlays_one_active
  ON model_metadata_overlays(model_id)
  WHERE status = 'active';

CREATE TABLE IF NOT EXISTS model_metadata_history (
  event_id INTEGER PRIMARY KEY AUTOINCREMENT,
  model_id TEXT NOT NULL,
  overlay_id TEXT,
  actor TEXT NOT NULL,
  action TEXT NOT NULL
    CHECK (action IN (
      'baseline_created',
      'overlay_created',
      'overlay_superseded',
      'overlay_reverted',
      'reset_to_original',
      'field_updated'
    )),
  field_path TEXT,
  old_value_json TEXT,
  new_value_json TEXT,
  reason TEXT,
  created_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ','now')),
  FOREIGN KEY (model_id) REFERENCES models(id) ON DELETE CASCADE,
  FOREIGN KEY (overlay_id) REFERENCES model_metadata_overlays(overlay_id) ON DELETE SET NULL
);

CREATE INDEX IF NOT EXISTS idx_model_metadata_history_model
  ON model_metadata_history(model_id, created_at);
```

Effective metadata read rule:
1. Load baseline row by `model_id`.
2. Load active overlay row (if present).
3. Effective metadata = `json_patch(baseline_json, overlay_json)` (or equivalent merge-patch logic in app layer).
4. Reset-to-original = mark active overlay as `reverted` and remove active overlay for that model.

Small migration sequence (existing library):

```sql
-- 001_create_metadata_provenance_tables.sql
BEGIN IMMEDIATE;
PRAGMA foreign_keys = ON;

-- Create tables/indexes/triggers (DDL above)

-- Backfill immutable baselines from existing models metadata
INSERT OR IGNORE INTO model_metadata_baselines (model_id, schema_version, baseline_json, created_at, created_by)
SELECT
  m.id,
  COALESCE(CAST(json_extract(m.metadata_json, '$.schema_version') AS INTEGER), 1) AS schema_version,
  m.metadata_json,
  m.updated_at,
  'pumas-library'
FROM models m;

-- Record baseline creation events for audit trail
INSERT INTO model_metadata_history (
  model_id, overlay_id, actor, action, field_path, old_value_json, new_value_json, reason, created_at
)
SELECT
  b.model_id,
  NULL,
  'pumas-library',
  'baseline_created',
  NULL,
  NULL,
  b.baseline_json,
  'migration-backfill',
  b.created_at
FROM model_metadata_baselines b
WHERE NOT EXISTS (
  SELECT 1
  FROM model_metadata_history h
  WHERE h.model_id = b.model_id AND h.action = 'baseline_created'
);

COMMIT;
```

Overlay write sequence (application transaction):
1. `BEGIN IMMEDIATE`.
2. Mark current active overlay (if any) as `superseded`.
3. Insert new active overlay.
4. Insert history rows (`overlay_superseded`, `overlay_created`, plus optional `field_updated` events).
5. `COMMIT`.

## Migration Plan (Library Data)
Phase A: Preflight
1. Snapshot index and metadata.
2. Verify disk space and permissions.
3. Produce dry-run classification/move report.

Phase B: Classification
1. Apply source-first signature mapping for task semantics.
2. Resolve `model_type` using model-specific source signals.
3. Compute canonical target path from resolved `model_type`.
4. Mark unknown/ambiguous models for review.

Phase C: Relocation + Metadata Rewrite
1. Move model directory to canonical path.
2. Rewrite metadata to v2.
3. Persist old->new path mapping in report.

Phase D: Reindex + Validation
1. Rebuild index.
2. Validate uniqueness and referential integrity.
3. Validate dependency profile links.

Phase E: Dependency Attach
1. Attach known dependency profiles automatically.
2. Mark unresolved profile matches for review.

Safety requirements:
1. Idempotent operations.
2. Crash-safe checkpoints for resume.
3. Machine-readable and human-readable reports.

## Consumer Migration Plan (Apps and Bindings)
Phase 0: Inventory
1. Enumerate consumers and all legacy field/endpoint usage.
2. Record baseline behavior tests/scripts.

Phase 1: Contract Updates
1. Replace legacy metadata parsing with v2 fields.
2. Replace legacy dependency calls with model-level APIs.
3. Treat `metadata_needs_review=true` as non-ready for automatic routing unless explicitly overridden.
4. Treat dependency states `unknown_profile`, `manual_intervention_required`, and `profile_conflict` as non-ready and surface remediation actions.

Phase 2: Path/ID Refactor
1. Remove assumptions about stable absolute model paths.
2. Resolve by model ID + index lookup.

Phase 3: Dependency Flow Update
1. Pre-execution call: `check_model_dependencies`.
2. If missing and policy allows: call `install_model_dependencies`.
3. Surface structured errors and logs on failure.

Phase 4: Validation + Cutover
1. Validate all consumers against migrated staging library.
2. Remove legacy adapters.
3. Release core + bindings in one cutover window.

## API and Binding Synchronization Rules
1. Core API and language bindings are one atomic migration unit.
2. Maintain migration matrix for endpoint/field changes and binding impacts.
3. Block release until all affected bindings pass contract tests.
4. Publish binding changelog listing removed legacy methods and replacements.

## Post-Cutover Required Behavior
1. Reject unknown `schema_version`.
2. Handle `metadata_needs_review` explicitly.
3. Do not silently fallback to deprecated classification behavior.
4. Use model dependency APIs before inference where dependency profiles exist.
5. Never call removed legacy endpoints or parse removed legacy fields.
6. Never auto-route models when dependency state is `unknown_profile`, `manual_intervention_required`, or `profile_conflict` unless policy explicitly allows it.

## Rollback Strategy
Because legacy methods are removed, rollback is snapshot/artifact based:
1. Keep pre-migration model metadata/index snapshot.
2. Keep pre-cutover app/binding artifacts.
3. If cutover fails, restore snapshot and redeploy pre-cutover build.

## Testing and Acceptance Criteria
Testing:
1. Unit tests for task-signature mapper, model-type resolver, validators, and dependency profile resolution.
2. Integration tests for model dependency APIs.
3. Migration tests on mixed realistic libraries.
4. Recovery tests for interrupted migration.
5. Contract tests for every affected binding.

Acceptance checklist:
1. All local consumers are migrated to Metadata v2 and model-level dependency APIs.
2. No runtime code path references removed legacy methods.
3. Directory reorganization report is generated and validated.
4. Unknown signatures are captured in SQLite pending mappings without blocking operations.
5. License artifacts are retained; unresolved license is represented as `license_unknown`.
6. Dependency binding edits and approvals are persisted in `dependency_binding_history`.
7. Binding contract tests and core tests pass in cutover build.

## Current Reference Docs
1. `/media/jeremy/OrangeCream/Linux Software/Pantograph/PROPOSAL-pumas-library-metadata-dependency-v2.md`
2. `/media/jeremy/OrangeCream/Linux Software/Pumas-Library/docs/plans/PUMAS_LIBRARY_METADATA_V2_CONSUMER_MIGRATION.md`
