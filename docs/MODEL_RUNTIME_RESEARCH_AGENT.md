# Model Runtime Research Agent Instruction

## Objective

Provide a repeatable workflow for researching and persisting model-specific runtime dependencies and custom inference settings so Pumas API clients can run newly added models without manual guesswork.

## Scope

In scope:
- Researching runtime requirements from Hugging Face, GitHub, and local model files.
- Creating deterministic dependency profile + binding records.
- Creating model-specific `inference_settings` schemas.
- Persisting data through canonical Pumas pathways so SQLite/API outputs are correct.
- Verifying client-visible contract fields and dependency resolution responses.

Out of scope:
- Rewriting generic importer/type-classification architecture.
- Shipping upstream runtime engines themselves.
- Ad-hoc one-off local hacks without durable code/tests.

## Source Priority Order

Use sources in this exact order and record evidence for each final field:

1. Hugging Face model card + repo tree
- Read: model card text, files list, tags, `pipeline_tag`, usage snippets.
- Capture: inference parameters, required packages, runtime backend hints, custom code requirements.

2. Upstream GitHub repository
- Read: README, inference examples, `requirements*.txt`, `pyproject.toml`, `setup.py`, release notes, inference scripts.
- Capture: exact package names/versions, required optional deps, supported settings and constraints.

3. Local downloaded model directory
- Read: `config.json`, tokenizer/config files, model-specific `.py` scripts, sidecar metadata.
- Capture: enum options (for example voices), defaults, local file requirements, variant-specific behavior.

4. Existing Pumas metadata/index rows
- Read: current `metadata.json` + indexed `models.metadata_json`.
- Capture: existing bindings/settings to avoid drift or duplicate incompatible profiles.

## Required Deliverables Per Model Family

1. Runtime dependency profile (`dependency_profiles.spec_json`) with exact pins.
2. Active model binding (`model_dependency_bindings`) pointing at that profile.
3. Metadata fields for runtime behavior:
- `recommended_backend`
- `runtime_engine_hints`
- `requires_custom_code`
- `custom_code_sources`
- `dependency_bindings` (metadata projection reference)
4. Model-specific `inference_settings` schema (`InferenceParamSchema[]`).
5. Verification evidence that API consumers receive all required fields.

## Dependency Authoring Rules

Use the `dependency_profiles` JSON contract enforced by `parse_and_canonicalize_profile_spec`:
- `python_packages[].version` must be exact pin syntax (`==...`).
- Package names must be normalized and unique.
- Python environments must include at least one pinned package.
- `pin_policy.required_packages` must reference packages present in `python_packages`.
- Include `source` URLs for non-PyPI or critical provenance.
- Use a new `profile_version` when the package set changes.

Template:

```json
{
  "python_packages": [
    {
      "name": "package-name",
      "version": "==1.2.3",
      "python_requires": ">=3.10,<3.13",
      "source": "https://github.com/org/repo/releases/download/v1.2.3/pkg.whl"
    }
  ],
  "pin_policy": {
    "required_packages": [
      { "name": "package-name" }
    ]
  }
}
```

Binding requirements:
- `binding_kind`: usually `required_core` unless a narrower contract is justified.
- `backend_key`: canonical token (for example `onnx-runtime`, `pytorch`, `transformers`).
- `status`: `active` for usable bindings.
- `priority`: deterministic ordering; use `100` unless precedence is required.

## Inference Settings Authoring Rules

Map upstream parameters into `InferenceParamSchema` entries:
- `key`: stable snake_case identifier (client-facing).
- `label`: human-readable display name.
- `param_type`: one of `Number`, `Integer`, `String`, `Boolean`.
- `default`: must match type and upstream default.
- `description`: short operational meaning.
- `constraints`:
  - numeric range via `min`/`max`
  - enum options via `allowed_values`
  - when UI label differs from runtime token, use objects shaped as `{ "label": "...", "value": "..." }`

Rules:
- Include only runtime-tunable parameters clients should set.
- Exclude internal-only implementation knobs.
- If parameter options come from local files (for example voice aliases), derive them from the file at runtime.
- Apply the same schema logic across model variants (including quantized variants like INT8) unless upstream behavior differs and is documented.

## Canonical Persistence Path (Preferred)

Persist through `ModelLibrary` + `ModelIndex` code paths, not manual DB edits:

1. Add/extend model-family projection logic in `rust/crates/pumas-core/src/model_library/library.rs`.
2. Ensure metadata projection sets runtime fields and `inference_settings`.
3. Upsert dependency profile via `ModelIndex::upsert_dependency_profile`.
4. Upsert binding via `ModelIndex::upsert_model_dependency_binding`.
5. Save metadata and re-index (`save_metadata` then `index_model_dir`/`rebuild_index` flow).

Reason: this preserves validation, normalization, history semantics, and avoids schema drift.

## SQLite Verification Queries

Run these checks against `shared-resources/models/models.db` (or active library DB):

```sql
SELECT profile_id, profile_version, environment_kind, profile_hash
FROM dependency_profiles
WHERE profile_id = '<profile_id>';
```

```sql
SELECT binding_id, model_id, profile_id, profile_version, binding_kind, backend_key, status, priority
FROM model_dependency_bindings
WHERE model_id = '<model_id>' AND status = 'active';
```

```sql
SELECT json_extract(metadata_json, '$.inference_settings') AS inference_settings,
       json_extract(metadata_json, '$.recommended_backend') AS recommended_backend,
       json_extract(metadata_json, '$.requires_custom_code') AS requires_custom_code
FROM models
WHERE id = '<model_id>';
```

```sql
PRAGMA foreign_key_check;
```

## API Contract Verification

Verify all consumer-facing surfaces:

1. `get_model` / `list_models` / `search_models`
- Metadata includes runtime fields and projected dependency binding refs.

2. `get_inference_settings`
- Returns expected model-specific schema.

3. `resolve_model_dependency_requirements`
- Returns resolved bindings with required pins for the target backend/platform.
- No `unknown_profile`, `invalid_profile`, or `profile_conflict` for supported contexts.

## Test Requirements

Add or update tests in `rust/crates/pumas-core/src/model_library/library.rs` and related dependency modules:
- projection applies metadata runtime fields
- profile + binding are present and valid
- inference setting keys/defaults/constraints are correct
- dependency resolution returns required packages
- variant coverage (for example fp16 + int8) when family has multiple runtime-identical artifacts

## Commit Requirements

Keep commits atomic and conventional:
- `docs(model-library): add runtime research agent instruction`
- If code is changed later, separate docs from behavior changes unless tightly coupled.

Before committing:
- confirm staged diff is scoped
- run affected tests for changed behavior
- validate no regression/fix pair remains in unpushed history

## Completion Checklist

- Evidence collected from HF, GitHub, and local files.
- Dependency profile validates and is bound to target model(s).
- Inference settings schema is complete and client-usable.
- SQLite rows and API responses match expected contract.
- Tests pass for new behavior.
- Changes committed with clear scope.
