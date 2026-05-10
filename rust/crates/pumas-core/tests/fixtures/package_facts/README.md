# Package Facts Fixtures

## Purpose

Small JSON fixtures for the versioned `ResolvedModelPackageFacts` producer
contract. These fixtures verify that package facts are serializable without
depending on Pumas SQLite layout, `models.metadata_json`, or runtime backend
selection policy.

## Rules

- Keep fixtures deterministic and minimal.
- Include `package_facts_contract_version`.
- Use stable snake_case field names and stable enum labels.
- Omit optional fields when absent so defaults remain part of the contract.
- Represent backend hints as advisory facts only.
- Treat these fixtures as the producer truth for host consumers. Consumers may
  copy or generate snapshots from them for tests, but any copied fixture must
  preserve field names and enum labels unless it is intentionally testing a
  consumer-owned adapter.
- Do not expose SQLite table names, cache row metadata, `models.metadata_json`,
  local frontend bridge state, scheduler state, runtime registry state, or
  diagnostics-ledger fields in package-facts fixtures.
- `ResolvedModelPackageFactsSummary` payloads are derived from the full
  package-facts shape. Summary tests should verify compatibility with the same
  enum labels, model refs, artifact facts, task evidence, backend hints,
  component status semantics, custom-code state, and diagnostic-code semantics.

## Consumer Handoff

Pantograph and other host consumers should use the fixtures in this directory
as the canonical producer-contract reference. A consumer test fixture may be:

- a generated copy of one of these files,
- a vendored test-data snapshot with a recorded source commit, or
- a consumer-owned projection fixture that explicitly documents the adapter it
  is testing.

Pumas does not own host runtime selection, technical-fit ranking, runtime
candidate derivation, scheduler policy, queue state, warm-process state, or
diagnostics-ledger mapping. Those concerns should be tested in the consuming
repository from Pumas package facts, summaries, and update events.

## Fixture Index

- `hf_transformers_text_generation_package_facts.json`: HF-compatible
  Transformers package with config, tokenizer, generation defaults, custom-code
  evidence, and advisory Transformers/vLLM/MLX hints.
- `gguf_text_generation_package_facts.json`: GGUF text-generation artifact with
  llama.cpp advisory hinting and quantization evidence, without HF tokenizer or
  processor requirements.
- `gguf_embedding_package_facts.json`: GGUF embedding artifact proving embedding
  tasks use the same package-facts contract as generation models.
- `diffusers_sd_text_to_image_package_facts.json`: Diffusers text-to-image
  package with `model_index.json`, component roles, Stable Diffusion family
  evidence, image-generation task facts, and advisory Diffusers backend hinting.
- `unsupported_ollama_hint_package_facts.json`: ecosystem hint preserved as
  unsupported evidence rather than converted into executable support.
- `invalid_generation_config_package_facts.json`: invalid model-provided
  generation defaults with parse diagnostics.
- `missing_tokenizer_package_facts.json`: tokenizer configuration without a
  known tokenizer vocabulary companion.
- `custom_code_required_package_facts.json`: trust-relevant package with
  `auto_map`, custom generation code, and dependency manifest evidence.
- `remote_search_mlx_vllm_hint.json`: Hugging Face search result carrying
  MLX/vLLM discovery hints without installed-model package facts.
- `hf_rerank_package_facts.json`: HF-compatible rerank package with text
  ranking task evidence.
- `hf_multimodal_processor_package_facts.json`: HF-compatible multimodal
  package with processor, image processor, tokenizer, and chat template
  evidence.
- `stale_package_facts.json`: durable package-facts cache row with stale
  contract-version/source-fingerprint semantics and decodable detail payload.
- `invalid_cached_package_facts.json`: durable package-facts cache row whose
  detail payload is valid JSON but not a decodable `ResolvedModelPackageFacts`,
  proving recovery paths can bypass malformed cached detail.
- `model_library_package_facts_modified_event.json`: host cache-invalidation
  event for package-fact detail refresh.

## Canonical Image-Generation Fixtures

The canonical producer fixture for host-side image-generation planning is
`diffusers_sd_text_to_image_package_facts.json`.

Consumers should treat this fixture as a Pumas-owned package-facts contract
sample, not a Pantograph adapter contract. The stable planning facts it exposes
are:

- `artifact.artifact_kind = "diffusers_bundle"`
- `task.task_type_primary = "image_generation"`
- `task.pipeline_tag = "text-to-image"`
- `diffusers.pipeline_class = "StableDiffusionPipeline"`
- `diffusers.family_evidence[].family = "stable_diffusion"`
- `backend_hints.accepted[] = "diffusers"`

The fixture intentionally does not include host runtime registry state,
workflow-node fields, scheduler policy, queue state, diagnostics-ledger payloads,
or any consumer-specific adapter fields.
