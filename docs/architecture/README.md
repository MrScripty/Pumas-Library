# Architecture Documentation

Current architecture references for the Rust/Electron implementation.

## Documents

- `SYSTEM_ARCHITECTURE.md` - End-to-end system layout, runtime boundaries, and process/data flow.
- `MODEL_LIBRARY_ARCHITECTURE.md` - Model library internals, indexing/mapping pipeline, and dependency requirement contract.

## Scope

These docs describe the current code paths in:

- `rust/crates/pumas-core`
- `rust/crates/pumas-app-manager`
- `rust/crates/pumas-rpc`
- `electron/src`
- `frontend/src`

Legacy planning documents from the prior Python/PyWebView implementation were removed from this directory.
