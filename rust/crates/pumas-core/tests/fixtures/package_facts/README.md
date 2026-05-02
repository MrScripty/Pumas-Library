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
