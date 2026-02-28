# Pumas Resolve-Only Hard Cutover Plan

## Status
Proposed

## Scope
Hard cutover to a resolve-only dependency contract for Pumas.

## Non-Goals
- No backward-compatibility layer.
- No deprecation timeline mechanics.
- No install/readiness execution semantics in Pumas.

## Required Standards
- Commit workflow and message format must follow:
  `/media/jeremy/OrangeCream/Linux Software/Coding-Standards/COMMIT-STANDARDS.md`.
- Every step below ends with a required commit gate.

## Frozen Boundary
1. Pumas resolves dependency requirements only.
2. Pumas does not perform runtime readiness checks.
3. Pumas does not execute installs.
4. Consumer owns environment creation, install, readiness checks, and lifecycle.

## API Cutover (Atomic)
1. Keep only: `resolve_model_dependency_requirements`.
2. Remove:
   - `get_model_dependency_profiles`
   - `resolve_model_dependency_plan`
   - `check_model_dependencies`
   - `install_model_dependencies`
3. Apply cutover across core API, JSON-RPC handlers/method names, bindings, docs, and tests in one migration unit.

## Contract Version Gate
1. Resolver response includes `dependency_contract_version: 1`.
2. Consumers must enforce exact match (`== 1`) and fail fast on mismatch.

## Resolver Contract (Normative)

### Top-Level Shape
```json
{
  "model_id": "string",
  "platform_key": "string",
  "backend_key": "string|null",
  "dependency_contract_version": 1,
  "validation_state": "resolved|unknown_profile|invalid_profile|profile_conflict",
  "validation_errors": [
    {
      "code": "string",
      "scope": "top_level|binding",
      "binding_id": "string|null",
      "field": "string|null",
      "message": "string"
    }
  ],
  "bindings": []
}
```

### Per-Binding Shape
```json
{
  "binding_id": "string",
  "profile_id": "string",
  "profile_version": 1,
  "profile_hash": "string|null",
  "backend_key": "string|null",
  "platform_selector": "string|null",
  "environment_kind": "string|null",
  "env_id": "string|null",
  "validation_state": "resolved|unknown_profile|invalid_profile|profile_conflict",
  "validation_errors": [],
  "requirements": []
}
```

### Requirement Entry Shape (v1)
```json
{
  "kind": "python_package",
  "name": "string",
  "exact_pin": "==x.y.z",
  "index_url": "string (optional)",
  "extra_index_urls": ["string (optional)"],
  "markers": "string (optional)",
  "python_requires": "string (optional)",
  "platform_constraints": ["string (optional)"],
  "hashes": ["sha256:<hex> (optional)"],
  "source": "string (optional, metadata only)"
}
```

## Nullability and Omission Policy
1. Always present and non-null (top-level):
   - `model_id`, `platform_key`, `dependency_contract_version`, `validation_state`, `validation_errors`, `bindings`
2. Always present and nullable (top-level):
   - `backend_key`
3. Always present and non-null (per-binding):
   - `binding_id`, `profile_id`, `profile_version`, `validation_state`, `validation_errors`, `requirements`
4. Always present and nullable (per-binding):
   - `profile_hash`, `backend_key`, `platform_selector`, `environment_kind`, `env_id`
5. Requirement optional fields:
   - `index_url`, `extra_index_urls`, `markers`, `python_requires`, `platform_constraints`, `hashes`, `source`
   - Omit when absent (do not emit `null`).

## Validation Semantics
1. Per-binding `validation_state` values:
   - `resolved`
   - `unknown_profile`
   - `invalid_profile`
   - `profile_conflict`
2. Aggregate top-level `validation_state` precedence:
   - `profile_conflict > invalid_profile > unknown_profile > resolved`
3. `validation_errors` schema:
   - fields: `code`, `scope`, `binding_id?`, `field?`, `message`
4. Deterministic `validation_errors` ordering:
   - `(binding_id or ""), code, (field or ""), message`
5. Aggregate top-level errors:
   - deterministic union of top-level + per-binding errors.

## Empty-Bindings Semantics
1. If `bindings=[]` and model/context has no declared dependency bindings:
   - top-level `validation_state=resolved`, `validation_errors=[]`
2. If `bindings=[]` but model/context declares dependency bindings and none resolve:
   - top-level `validation_state=unknown_profile`
   - include top-level error code such as `declared_bindings_unresolved`.

## Determinism, Normalization, and Duplicate Rules
1. Binding canonical sort key:
   - `(binding_kind, backend_key, platform_selector, profile_id, profile_version, priority, binding_id)` with null treated as empty string.
2. Requirement canonical sort key:
   - `(kind, name, exact_pin)`
3. Normalize lowercase:
   - `name`, `kind`, `backend_key`, `environment_kind`
4. URL normalization for `index_url` and `extra_index_urls`:
   - trim surrounding whitespace
   - lowercase scheme and host
   - remove trailing `/`
   - preserve path and query
5. Hash ordering:
   - lexicographically sorted.
6. Duplicate requirements keyed by `(kind, name)`:
   - same key + different `exact_pin` => `invalid_profile`
   - identical duplicates => dedupe deterministically.

## env_id Contract and Conflict Rule
1. `env_id` is public and frozen:
   - `{environment_kind}:{profile_id}:{profile_version}:{profile_hash}:{platform_key}:{backend_key_or_any}`
2. Conflict rule:
   - same `env_id` and different `profile_hash` => `profile_conflict`
3. Missing hash rule:
   - if profile hash is missing/unknown/invalid, binding must be `unknown_profile` or `invalid_profile` and `env_id=null`.

## Projection Coherence (`dependency_bindings`)
1. SQLite dependency tables are authoritative.
2. `list_models` and `search_models` must include active `dependency_bindings`.
3. Projection refresh triggers:
   - writes to `model_dependency_bindings`
   - writes to `dependency_profiles`
   - model import/reindex paths
4. Projection refresh must be transactional with dependency writes.
5. Read path must repair from SQLite when projection is stale or missing.

## v1 Requirement Scope
1. Requirement kinds supported in v1:
   - Python only (`python_package`)
2. Contract is extensible for future non-Python kinds without changing v1 semantics.
3. No install command hints in resolver output.

## Implementation Steps and Commit Gates

### Step 1: Freeze Contract and Add Spec Types
- Add/replace Rust contract types for resolver response, validation errors, and requirement entries.
- Remove lifecycle/install state from public dependency contract.
- Define serde nullability and omission behavior exactly as specified above.

Commit gate (required before next step):
1. Stage only files for this step.
2. Run relevant tests/checks.
3. Commit using Conventional Commits.
4. Include `BREAKING CHANGE:` footer if public contract changed.
5. Suggested message:
   - `feat(model-library): add resolve-only dependency requirements contract`

### Step 2: API Surface Cutover
- Implement `resolve_model_dependency_requirements` in core and API facade.
- Remove old methods and call sites from core API.
- Update RPC routing and handlers to new method only.

Commit gate (required before next step):
1. Stage only Step 2 files.
2. Run affected tests.
3. Commit per commit standards.
4. Include `BREAKING CHANGE:` footer.
5. Suggested message:
   - `refactor(rpc): replace dependency lifecycle APIs with resolver-only method`

### Step 3: Determinism, Validation, and env_id Rules
- Implement normalization, canonical ordering, dedupe, and conflict detection rules.
- Implement empty-bindings semantics and validation aggregation precedence.
- Enforce `dependency_contract_version: 1`.

Commit gate (required before next step):
1. Stage only Step 3 files.
2. Run affected tests.
3. Commit per commit standards.
4. Suggested message:
   - `fix(model-library): enforce deterministic dependency requirements resolution`

### Step 4: Projection Coherence
- Ensure `list_models`/`search_models` projection coherence for `dependency_bindings`.
- Add transactional projection refresh and read-repair behavior.

Commit gate (required before next step):
1. Stage only Step 4 files.
2. Run affected tests.
3. Commit per commit standards.
4. Suggested message:
   - `fix(index): keep dependency_bindings projection coherent with sqlite`

### Step 5: Docs and Consumer Contract Update
- Update migration docs and remove references to removed lifecycle/check/install APIs.
- Document consumer responsibility boundary.
- Document nullability/omission and empty-bindings behavior.

Commit gate (required before next step):
1. Stage only Step 5 files.
2. Run doc lint/checks if configured.
3. Commit per commit standards.
4. Suggested message:
   - `docs(api): document resolve-only dependency requirements hard cutover`

### Step 6: Fixtures and Tests
- Add golden fixtures:
  - `resolved`
  - `unknown_profile`
  - `invalid_profile`
  - `profile_conflict`
- Add tests for:
  - exact pin enforcement (`==`)
  - deterministic ordering
  - URL/hash canonicalization stability
  - duplicate conflict behavior
  - env_id determinism and conflict semantics
  - empty-bindings behavior
  - stable-audio includes `stable-audio-tools`

Commit gate (required before next step):
1. Stage only Step 6 files.
2. Run full affected test suite.
3. Commit per commit standards.
4. Suggested message:
   - `test(model-library): add resolve-only dependency contract fixtures and coverage`

### Step 7: Final Contract Validation
- Run end-to-end contract verification for core + RPC + bindings.
- Confirm old methods are absent from public API and docs.
- Confirm consumer mismatch fails fast on `dependency_contract_version`.

Commit gate (required to complete plan):
1. Stage only final validation artifacts/adjustments.
2. Run final test suite and checks.
3. Commit per commit standards.
4. Suggested message:
   - `chore(contract): finalize resolve-only dependency cutover validation`

## Acceptance Criteria
1. No public Pumas API implies runtime check/install execution.
2. Resolver response is fully declarative and sufficient for deterministic external install tooling.
3. Public contract contains no readiness/install lifecycle state.
4. `dependency_contract_version=1` is emitted and enforced by consumers.
5. Validation state/error semantics and empty-bindings semantics are deterministic and documented.
6. `list_models`/`search_models` reliably include coherent active `dependency_bindings`.
7. Golden fixtures exist for `resolved`, `unknown_profile`, `invalid_profile`, `profile_conflict`.
8. Stable-audio requirements include pinned `stable-audio-tools`.
