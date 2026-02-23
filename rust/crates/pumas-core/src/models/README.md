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
| `model.rs` | Model-related types: `ModelData`, `HuggingFaceModel`, `ModelMetadata`, download/import types |
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

## Dependencies

### Internal
- None (leaf module -- no internal dependencies)

### External
- `serde` / `serde_json` - Serialization and deserialization
- `chrono` - Date/time types in version metadata
