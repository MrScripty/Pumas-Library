# Dependency Pinning Implementation Plan

## Status
Draft (ready for implementation)

## Scope
This plan defines how Pumas enforces dependency version pinning for model dependency profiles and bindings.

## Hard Constraints
1. Do not version the dependency API contract.
2. Keep API fields additive only.
3. Existing field names are stable and must not be renamed.
4. Consumers must ignore unknown keys.

## Frozen Decisions

### 1. Canonical Pin Schema (`dependency_profiles.spec_json`)
Dependency binding rows remain references only (`profile_id`, `profile_version`).
Pinning lives in `dependency_profiles.spec_json`.

Canonical required shape (unknown extra fields allowed):

```json
{
  "python_packages": [
    {
      "name": "torch",
      "version": "==2.5.1+cu121",
      "index": "https://download.pytorch.org/whl/cu121",
      "markers": "python_version >= '3.10'"
    }
  ],
  "probes": [
    { "kind": "command", "program": "python", "args": ["-c", "import torch"] }
  ],
  "install": {
    "commands": [
      {
        "program": "pip",
        "args": ["install", "torch==2.5.1+cu121"],
        "source_url": "https://pytorch.org/get-started/locally/",
        "source_ref": "pytorch install matrix"
      }
    ]
  },
  "pin_policy": {
    "required_packages": [
      {
        "name": "xformers"
      }
    ]
  },
  "binding_modality_overrides": {
    "binding.trado.pytorch.core.linux_x86_64": {
      "input_modalities": ["text"],
      "output_modalities": ["text"]
    }
  }
}
```

### 2. Exact Version Syntax
Pinned entries must use exact PEP 440 form only:
1. Allowed: `==x.y.z`
2. Allowed: exact local build tag, e.g. `==2.5.1+cu121`
3. Allowed: exact pre/post/dev forms, e.g. `==2.6.0rc1`
4. Rejected: ranges/wildcards (`>=`, `<=`, `~=`, `!=`, `*`, comma lists)

### 3. Backend Required Pins
For `backend_key = pytorch`:
1. Always required: `torch`
2. Required when image modality is used: `torchvision`
3. Required when audio modality is used: `torchaudio`

### 4. Modality Source Precedence
When deciding modality-required pins:
1. Binding-level modality override (if present) from `spec_json.binding_modality_overrides[<binding_id>]`
2. Model metadata (`input_modalities`, `output_modalities`)
3. Task fallback map

Binding-level modality override shape:
1. `spec_json.binding_modality_overrides` is an object keyed by exact `binding_id`.
2. Each override value is:
   - `input_modalities: string[]`
   - `output_modalities: string[]`
3. Allowed modality tokens are canonical lowercase modality tags already used by metadata v2 (`text`, `image`, `audio`, `video`, `document`, `mask`, `keypoints`, `action`, `3d`, `embedding`, `tabular`, `timeseries`, `rl-state`, `any`, `unknown`).

If required-modality resolution is still unknown/ambiguous:
1. Return `manual_intervention_required`
2. Use deterministic code `modality_resolution_unknown`
3. Do not infer required pins by guesswork

### 5. `missing_pins` Semantics
Top-level `missing_pins` is:
1. The union of required missing pins only
2. Deduped by normalized package name (lowercase)
3. Excludes optional missing pins

### 6. Required Pin Provenance
Each required pin must include one or more reasons:
1. `backend_required`
2. `modality_required`
3. `profile_policy_required`

Profile policy requirement source:
1. `spec_json.pin_policy.required_packages[]`
2. Each entry shape:
   - `name: string` (normalized to lowercase)
3. If a package is required by this section, it contributes `profile_policy_required` to that pin's reason set.

### 7. Profile Immutability
For `(profile_id, profile_version)`:
1. Same canonical content hash: idempotent no-op allowed
2. Different canonical content hash: hard fail with `dependency_profile_version_immutable`

### 8. Canonicalization and Hashing
Canonical hash input rules:
1. Normalize package names to lowercase
2. Trim version strings
3. Sort JSON object keys recursively
4. For `python_packages`, sort by `(name, version)`
5. Preserve order of arrays not declared unordered by schema
6. Serialize canonical JSON bytes and hash those bytes only

## Frozen Dependency Response Schema Snippet
This snippet applies to `resolve_model_dependency_plan`, `check_model_dependencies`, and `install_model_dependencies` responses.

```json
{
  "state": "ready | missing | failed | unknown_profile | manual_intervention_required | profile_conflict",
  "error_code": "string | null",
  "message": "string | null",
  "missing_pins": [
    "torch",
    "torchvision"
  ],
  "bindings": [
    {
      "binding_id": "string",
      "model_id": "string",
      "profile_id": "string",
      "profile_version": 2,
      "binding_kind": "required_core | required_custom | optional_feature | optional_accel",
      "backend_key": "string | null",
      "platform_selector": "string | null",
      "state": "ready | missing | failed | unknown_profile | manual_intervention_required | profile_conflict",
      "error_code": "string | null",
      "message": "string | null",
      "pin_summary": {
        "pinned": true,
        "required_count": 2,
        "pinned_count": 2,
        "missing_count": 0
      },
      "required_pins": [
        {
          "name": "torch",
          "reasons": ["backend_required", "profile_policy_required"]
        },
        {
          "name": "torchvision",
          "reasons": ["modality_required"]
        }
      ],
      "missing_pins": [
        "torchvision"
      ]
    }
  ]
}
```

Field rules:
1. `pin_summary`, `required_pins`, and `missing_pins` are per-binding and always present.
2. Top-level `missing_pins` is always present and follows required-only semantics.
3. `pin_summary.pinned` means all required pins for that binding are present and exactly pinned (`==...`), regardless of optional packages.
4. `required_pins[].reasons` is a deduped array (not a single value) so one pin can carry multiple requirement sources.
5. Ordering is deterministic:
   - `required_pins` sorted lexicographically by normalized `name`
   - `required_pins[].reasons` sorted lexicographically
   - per-binding `missing_pins` sorted lexicographically by normalized package name
   - top-level `missing_pins` sorted lexicographically by normalized package name
6. New fields may be added, existing fields are not renamed.

## Frozen Error-Code Table

| Code | Surfaces | State | Trigger | Notes |
|---|---|---|---|---|
| `unpinned_dependency` | resolve/check/install (binding-level) | `manual_intervention_required` | Required pin absent or non-exact pin syntax | Deterministic across all three surfaces |
| `modality_resolution_unknown` | resolve/check/install (binding-level) | `manual_intervention_required` | Cannot resolve modality using precedence rules | Distinct from unpinned syntax/content issues |
| `unknown_profile` | resolve/check/install | `unknown_profile` | Binding references profile not resolvable/incomplete in SQLite | Existing behavior retained |
| `profile_conflict` | resolve/check/install | `profile_conflict` | Conflicting profiles resolve to same deterministic environment target | Existing behavior retained |
| `required_binding_omitted` | check/install | `failed` | Caller-provided `selected_binding_ids` omits required binding | Existing behavior retained |
| `dependency_profile_version_immutable` | profile write path | write rejected | Same `(profile_id, profile_version)` with different canonical hash | Hard fail; no upsert mutation |
| `invalid_dependency_pin` | profile write path | write rejected | Invalid exact-version syntax or invalid required pin shape | Validation error with field path |

Top-level aggregation rule:
1. If any required binding has `unpinned_dependency` or `modality_resolution_unknown`, top-level state is `manual_intervention_required`.

Install execution guardrail:
1. For bindings in `manual_intervention_required` with `error_code` equal to `unpinned_dependency` or `modality_resolution_unknown`, no install command execution is allowed.
2. These bindings are reported as skipped in install results with the same deterministic error code retained.

## Rollout Phases

### Phase 0: Spec + Contract Freeze
1. Land this plan and schema/error code freeze.
2. Align Pantograph and other consumers on field names and semantics.

### Phase 1: Audit/Report-Only
1. Add dependency audit command to scan profiles/bindings.
2. Report unpinned profiles and required pin gaps.
3. Emit backfill patch suggestions.
4. No hard write/runtime blocking yet.

### Phase 2: Soft Enforcement (Flagged)
1. Enable write-time validation and runtime guardrails under feature flag.
2. Default enabled in dev/test.
3. Collect migration and CI signal.

### Phase 3: Hard Enforcement (Default)
1. Make write-time validation mandatory.
2. Keep runtime guardrails always on.
3. Remove temporary soft-enforcement flag.

## Implementation Work Breakdown
1. `rust/crates/pumas-core/src/index/model_index.rs`
   - Add canonicalization + hash + immutability enforcement in profile writes.
2. `rust/crates/pumas-core/src/model_library/dependencies.rs`
   - Add required pin resolution, modality precedence logic, and deterministic binding/top-level state mapping.
3. `rust/crates/pumas-core/src/model_library/metadata_v2.rs`
   - Extend dependency validation to include pin compliance when `dependency_bindings` exists.
4. `rust/crates/pumas-rpc/src/handlers/models/dependencies.rs`
   - Expose additive pin fields in existing dependency APIs.
5. `rust/crates/pumas-core/src/model_library/library.rs` (or migration runner module)
   - Add audit/report command and optional backfill patch generation.

## Test Matrix (must-pass)
1. Exact syntax accept/reject tests for pin versions.
2. Backend-required pin tests (`torch` always; modality-driven `torchvision`/`torchaudio`).
3. Modality precedence tests (binding override > metadata > fallback).
4. `missing_pins` required-only top-level union semantics.
5. Required pin provenance (`reasons[]`) correctness, including multi-reason cases.
6. Deterministic ordering tests for `required_pins`, `required_pins[].reasons`, and `missing_pins`.
7. Immutability hard-fail for changed content on same `(id, version)`.
8. Runtime deterministic error/state mapping for resolve/check/install.
9. Install no-op execution tests for `unpinned_dependency` and `modality_resolution_unknown`.
10. Unknown-field tolerance tests for forward compatibility.
11. Cross-platform canonical hash stability test (Linux/macOS/Windows matrix).

## Acceptance Criteria
1. No required dependency profile can enter storage without exact pins.
2. No required binding with unpinned deps can resolve as `ready`.
3. Dependency APIs expose stable additive pin details without contract versioning.
4. Existing libraries can be audited and migrated before hard enforcement.
