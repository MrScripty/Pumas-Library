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
- `model_library_package_facts_modified_event.json`: host cache-invalidation
  event for package-fact detail refresh.
