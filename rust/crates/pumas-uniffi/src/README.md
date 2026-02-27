# pumas-uniffi src

## Purpose
UniFFI-based language binding layer for exposing Pumas APIs to non-Rust consumers.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `bindings.rs` | UniFFI-exposed wrapper functions/types. |
| `lib.rs` | Crate entrypoint and UniFFI module wiring. |
| `bin/` | Utility binaries for binding generation workflows. |

## Design Decisions
- Keep FFI-safe wrapper types in one binding module.
- Core logic remains in `pumas-core`; UniFFI crate is an adapter.

## Dependencies
**Internal:** `pumas-library` and shared response/model types.
**External:** `uniffi` tooling and serialization helpers.

## Usage Examples
```text
cargo run -p pumas-uniffi --bin pumas-uniffi-bindgen
```
