# Models

## Purpose

Shared data models and DTOs (Data Transfer Objects) used across the Pumas Library API. These
structs map directly to the Python TypedDict definitions and TypeScript interfaces in the
frontend, ensuring type-compatible serialization across all layers.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, re-exports all types |
| `api_response.rs` | `ApiResponse<T>` - Generic response wrapper with `success`/`error` fields and flattened data |
| `responses.rs` | `BaseResponse` and concrete response types matching frontend TypeScript interfaces |
| `model.rs` | Model-related types: `ModelData`, `HuggingFaceModel`, `ModelMetadata`, external-asset metadata fields, and download/import types |
| `model_library_selector.rs` | Fast model-library selector snapshot DTOs with canonical model refs, selected artifact identity, entry-path state, artifact state, and detail freshness |
| `package_facts.rs` | Versioned model package-fact DTOs for artifact, component, task, backend-hint, generation-default, and custom-code evidence |
| `runtime_profile.rs` | Local runtime profile, provider settings, model-route, status, snapshot, and update-feed DTOs shared with RPC/Electron/frontend consumers. |
| `serving.rs` | User-directed model serving DTOs for explicit placement, served-model status, endpoint mode, update feeds, and non-critical load-error envelopes. |
| `version.rs` | `VersionInfo`, `VersionsMetadata` - Version tracking and metadata persistence types |
| `github.rs` | GitHub-specific types for release and asset data |
| `custom_node.rs` | Custom node metadata: `CompatibilityStatus`, `CustomNodeVersionStatus` |

## Design Decisions

- **`#[serde(flatten)]` on `ApiResponse<T>`**: Eliminates the need for a nested `data` field,
  keeping JSON output flat and compatible with the existing frontend contract.
- **Serde field naming is contract-specific**: legacy response DTOs may use
  camelCase, while newer backend-owned contracts such as runtime profiles,
  package facts, and user-directed serving use snake_case exactly as defined in
  their Rust DTO modules and matching TypeScript types.
- **Re-exports via glob (`pub use *`)**: All types are available from `pumas_library::models::`
  without needing to know which submodule defines them.
- **Append-only contract growth**: New external-asset fields are added as optional metadata
  extensions so existing file-based models and consumers remain compatible while the model-library
  contract expands.
- **Backend-owned runtime routing**: Runtime profile DTOs describe Pumas-owned
  routing and provider settings. They do not claim upstream provider
  capabilities that are not represented by the selected provider adapter.

## Dependencies

### Internal
- None (leaf module -- no internal dependencies)

### External
- `serde` / `serde_json` - Serialization and deserialization
- `chrono` - Date/time types in version metadata

## API Consumer Contract

- `ModelMetadata` remains the shared cross-layer metadata contract for persisted model records.
- External directory-root assets extend `ModelMetadata` with optional fields such as
  `source_path`, `entry_path`, `storage_kind`, `bundle_format`, `pipeline_class`,
  `import_state`, and asset-level validation fields.
- `ModelExecutionDescriptor` is the runtime-facing contract for executable model assets and is
  intended to replace file-centric execution-path assumptions for external bundles.
- `ResolvedModelPackageFacts` is the richer package-evidence contract. It stays separate from
  `ModelExecutionDescriptor` so consumers can inspect compatibility, trust, and package layout
  facts without forcing every execution-summary caller to deserialize the full package contract.
- Package facts have two stability classes:
  - Stable reference facts: contract version, model ref, selected artifact identity, artifact kind,
    storage kind, validation summary, task evidence, backend hint labels, and custom-code trust
    state. These can be persisted or cached when tied to an artifact signature.
  - Volatile inspection facts: selected files, sibling files, component presence, parse diagnostics,
    generation defaults, `auto_map` evidence, and source revision details. These may be regenerated
    from package files when the artifact signature or package-facts contract version changes.
- `ModelLibraryUpdateEvent` is a host-agnostic cache-invalidation contract. It identifies model and
  fact-family changes without prescribing consumer cache shape, runtime selection, or scheduling.
- `ModelLibrarySelectorSnapshot` is the fast list/selector contract. It carries `PumasModelRef`,
  selected artifact identity, entry path readiness, artifact readiness, and compact display facts.
  `indexed_path` is display/debug data only, and `entry_path` is executable only when both
  `entry_path_state` and `artifact_state` are `ready`.
- `RuntimeProfileConfig`, `ModelRuntimeRoute`, `RuntimeProfileStatus`, and
  `RuntimeProfileUpdateFeed` are the host-facing local runtime contract.
  Consumers should treat `profile_id` as the stable route key; raw endpoint
  URLs are compatibility inputs, not durable internal identity.
- `ModelServingConfig`, `ServeModelRequest`, `ServedModelStatus`, and
  `ServingStatusSnapshot` are the host-facing model serving contract. The
  selected provider/profile/device placement is user-authored; Pumas validates
  and attempts it without silently moving, evicting, or unloading unrelated
  models to make the request fit.
- `ServingStatusUpdateFeed` is an in-memory invalidation contract. Consumers
  can use it to decide when to refresh serving snapshots, but must tolerate
  `snapshot_required` instead of durable event replay.
- `ModelServeError` is the safe load-failure envelope. Failed fit or runtime
  load attempts that do not corrupt existing served state use
  `severity = non_critical` and preserve `loaded_models_unchanged = true`.
- Compatibility policy is append-only for milestone one: new optional fields may appear, but
  existing file-based fields and semantics must remain valid.

## Package-Facts Producer Contract

`ResolvedModelPackageFacts` is the canonical producer DTO for host consumers
that need package evidence. Consumers should decode this shape directly or use
an explicit adapter owned by the consumer. They should not define a parallel
`ResolvedModelPackageFacts` contract with renamed fields.

Required wire-shape rules:

- Field names and enum labels use snake_case as defined by `package_facts.rs`.
- `package_facts_contract_version` is required and must be checked by
  consumers before assuming field semantics.
- `model_ref.model_id` is the stable Pumas identity. Legacy paths are inputs to
  resolution only and are not saved consumer identity.
- `model_ref.model_ref_contract_version` is the Pumas model-reference contract version, not a model
  revision. Model revision remains `model_ref.revision`.
- `model_ref.selected_artifact_id` is optional until selected-artifact identity
  is available. Consumers must tolerate it being absent.
- `artifact` carries executable-entry and validation facts; it does not select
  a runtime.
- `backend_hints` are advisory package facts. They are not runtime placement,
  admission, queueing, or scheduler decisions.
- `generation_defaults` are model-provided defaults from package files, not
  Pumas UI/runtime settings.
- Omitted optional fields have serde defaults and are part of the contract.

`ResolvedModelPackageFactsSummary` is the canonical compact row shape for host
list/search/cache population. It is derived from full package facts and carries
only stable summary fields, component status summaries, custom-code state,
backend hints, task evidence, validation state, and diagnostic codes. Summary
payloads must remain decodable without Pumas SQLite layout or
`models.metadata_json`.

`ModelExecutionDescriptor` remains a compact execution-facing summary for
callers that only need entry path, storage kind, validation state, task summary,
backend hints, and dependency state. It must not grow into the full
package-facts contract.

Host responsibilities:

- Pantograph and other consumers own their local projections, diagnostics-ledger
  mappings, runtime-candidate derivation, technical-fit decisions, scheduler
  policy, and runtime registry interpretation.
- Pumas owns the DTO definitions, producer fixtures, omitted-field defaults,
  enum wire labels, and contract-version semantics.
