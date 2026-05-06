# pumas-uniffi src

## Purpose
UniFFI wrapper surface for exposing Pumas model-library and runtime management
APIs to foreign-language consumers without leaking core-library implementation
types across the FFI boundary.

## Contents
| File/Folder | Description |
| ----------- | ----------- |
| `bindings.rs` | FFI-safe records, error mapping, and the exported `FfiPumasApi` object that adapts `pumas-core` APIs for UniFFI. |
| `lib.rs` | Crate entrypoint that gates the binding implementation behind the `bindings` feature and re-exports the adapter surface. |
| `bin/uniffi_bindgen.rs` | Thin bindgen entrypoint used to print UniFFI metadata and generate host-language bindings from the compiled library. |

## Problem
`pumas-core` uses idiomatic Rust types and async APIs that are not directly safe
or convenient for Python, C#, Kotlin, Swift, or Ruby consumers. This crate
defines the stable boundary where those APIs become FFI-safe records, enums,
objects, and flattened error types.

## Constraints
- `pumas-core` must remain usable without depending on the UniFFI wrapper crate.
- The wrapper must map non-FFI-safe types into stable foreign-language shapes.
- Generated bindings are derived artifacts and must not become the maintained
  source of truth.
- Contract drift between `pumas-core` and this crate must be caught quickly.

## Decision
Keep the UniFFI surface in a dedicated adapter crate that owns FFI-safe types,
lossy error conversion, and foreign-language constructors while delegating all
domain behavior to `pumas-core`. Validate the compiled UniFFI metadata with a
dedicated script so exported-surface regressions are detectable even before
host-language smoke coverage runs.

## Alternatives Rejected
- Annotate and export every core type directly from `pumas-core`: rejected
  because FFI constraints would leak into core architecture and many types are
  not boundary-safe.
- Treat generated host-language code as the contract source of truth: rejected
  because generated output should be disposable and regenerated from the native
  library.

## Invariants
- `pumas-core` remains the authoritative implementation layer.
- `pumas-uniffi` owns FFI-safe wrapper records, objects, and error translation.
- Generated bindings are produced from the compiled native library and are never
  hand-edited.
- Exported UniFFI metadata must continue to include the documented API object
  and core binding records.

## Revisit Triggers
- The foreign-language API needs streaming callbacks, event sinks, or lifecycle
  behavior not well served by the current object surface.
- Product-facing artifact naming is changed away from `pumas_uniffi`.
- More host-language smoke/package flows are added and this README no longer
  covers the relevant workflow boundaries clearly.

## Dependencies
**Internal:** `pumas-core`, IPC client types, and shared model/response types.
**External:** `uniffi`, `tokio`, `serde`, and `thiserror`.

## Related ADRs
- None identified as of 2026-04-12.
- Reason: The adapter layering follows repo architecture and language-binding
  standards but has not been split into a separate ADR.
- Revisit trigger: The foreign-language facade or artifact model changes in a
  way that affects multiple repos or release channels.

## Usage Examples
```bash
cargo build --manifest-path rust/Cargo.toml -p pumas-uniffi
./scripts/check-uniffi-surface.sh
cargo run --manifest-path rust/Cargo.toml -p pumas-uniffi --bin pumas-uniffi-bindgen --features cli -- \
  generate --library --language python --out-dir bindings/python rust/target/debug/libpumas_uniffi.so
```

## API Consumer Contract
- `FfiPumasApi` is the primary foreign-language object surface.
- Constructors currently return either a primary in-process API or an
  IPC-backed client depending on launcher-root state. This is transitional
  compatibility behavior inherited from the legacy `PumasApi` facade.
- Future binding work should expose explicit owner, same-device local-client,
  and read-only roles instead of requiring foreign-language callers to infer
  ownership mode from one constructor.
- Async UniFFI methods return flattened `FfiError` variants rather than rich
  Rust error chains.
- Foreign-language consumers must keep generated bindings matched to the native
  library they were generated from.

## Structured Producer Contract
- This crate produces UniFFI metadata that drives generated host-language
  bindings.
- Key exported names such as `FfiPumasApi`, `FfiApiConfig`,
  `FfiDownloadRequest`, `FfiModelRecord`, and core methods like
  `list_models`/`start_hf_download` are treated as stable verification targets.
- Metadata verification output is written to transient build directories under
  `rust/target/`.
- When exported record/object names or required fields change, generated
  bindings, smoke scripts, packaging docs, and any persisted examples must be
  updated in the same change.
