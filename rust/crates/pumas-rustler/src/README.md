# pumas-rustler src

## Purpose
Rustler NIF bridge exposing selected Pumas capabilities to the BEAM/Elixir runtime.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `lib.rs` | NIF entrypoints and type conversions for Rustler integration. |

## Design Decisions
- Keep binding glue isolated so core crate remains runtime-independent.
- Marshal data through explicit conversion boundaries.

## Dependencies
**Internal:** `pumas-library` APIs/types.
**External:** `rustler` and BEAM interop primitives.

## Usage Examples
```text
# Loaded by Elixir NIF module; Rust entrypoints are exported from lib.rs.
```
