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
| `package_facts.rs` | Versioned model package-fact DTOs for artifact, component, task, backend-hint, generation-default, and custom-code evidence |
| `version.rs` | `VersionInfo`, `VersionsMetadata` - Version tracking and metadata persistence types |
| `github.rs` | GitHub-specific types for release and asset data |
| `custom_node.rs` | Custom node metadata: `CompatibilityStatus`, `CustomNodeVersionStatus` |

## Design Decisions

- **`#[serde(flatten)]` on `ApiResponse<T>`**: Eliminates the need for a nested `data` field,
  keeping JSON output flat and compatible with the existing frontend contract.
- **`#[serde(rename_all = "camelCase")]`**: All types use camelCase serialization to match the
  TypeScript/JavaScript frontend conventions.
- **Re-exports via glob (`pub use *`)**: All types are available from `pumas_library::models::`
  without needing to know which submodule defines them.
- **Append-only contract growth**: New external-asset fields are added as optional metadata
  extensions so existing file-based models and consumers remain compatible while the model-library
  contract expands.

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
- Compatibility policy is append-only for milestone one: new optional fields may appear, but
  existing file-based fields and semantics must remain valid.
