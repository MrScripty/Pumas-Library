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

## Fixture Index

- `hf_transformers_text_generation_package_facts.json`: HF-compatible
  Transformers package with config, tokenizer, generation defaults, custom-code
  evidence, and advisory Transformers/vLLM/MLX hints.
- `gguf_text_generation_package_facts.json`: GGUF text-generation artifact with
  llama.cpp advisory hinting and quantization evidence, without HF tokenizer or
  processor requirements.
- `gguf_embedding_package_facts.json`: GGUF embedding artifact proving embedding
  tasks use the same package-facts contract as generation models.
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
- `model_library_package_facts_modified_event.json`: host cache-invalidation
  event for package-fact detail refresh.
