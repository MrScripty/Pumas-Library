# Metadata

## Purpose

Atomic JSON metadata persistence for version tracking, model information, and application
configuration. Provides crash-safe file writes (write-to-temp then rename) and a high-level
`MetadataManager` for structured access to the launcher's metadata directory.

## Contents

| File | Description |
|------|-------------|
| `mod.rs` | Module root, re-exports public API |
| `atomic.rs` | `atomic_read_json` / `atomic_write_json` - Crash-safe JSON file I/O via temp file + rename |
| `manager.rs` | `MetadataManager` - Structured access to versions, models, custom nodes, and config metadata |

## Design Decisions

- **Atomic writes via rename**: Writing to a temporary file then renaming ensures that a crash
  mid-write never leaves a corrupted metadata file. The rename operation is atomic on all
  supported filesystems.
- **Single `MetadataManager` instance**: All metadata access goes through one manager to
  ensure consistent directory structure and avoid path duplication across callers.

## Dependencies

### Internal
- `crate::error` - `PumasError` / `Result`

### External
- `serde` / `serde_json` - JSON serialization
- `tempfile` - Temporary file creation for atomic writes
