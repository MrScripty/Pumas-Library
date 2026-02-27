# pumas-uniffi bin

## Purpose
Houses helper binaries used for UniFFI binding generation and maintenance tasks.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `pumas-uniffi-bindgen.rs` | CLI entrypoint for generating UniFFI language bindings. |

## Design Decisions
- Keep codegen tooling separate from runtime FFI glue.
- Provide a reproducible entrypoint for binding regeneration.

## Dependencies
**Internal:** `pumas-uniffi` crate exports and UDL configuration.
**External:** `uniffi_bindgen`/related UniFFI tooling.

## Usage Examples
```text
cargo run -p pumas-uniffi --bin pumas-uniffi-bindgen -- --help
```
