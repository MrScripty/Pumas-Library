# ONNX Runtime Dependency Review

## Scope

This review covers the first Rust-only ONNX embedding execution dependency
slice. It records the provisional dependency choices before any manifest or
lockfile changes are made.

## Decision

Use Rust-owned dependencies in `pumas-core`, the crate that owns
`rust/crates/pumas-core/src/onnx_runtime/`.

| Need | Candidate | Version Observed | Decision |
| ---- | --------- | ---------------- | -------- |
| ONNX Runtime binding | `ort` | `2.0.0-rc.12` | Provisional CPU-first candidate. Add only after a manifest slice that verifies native-library packaging and lockfile impact. |
| Tokenizer loading/execution | `tokenizers` | `0.23.1` | Provisional local-tokenizer candidate. Use local files only; do not enable HTTP/Hub download features in the first implementation. |
| Tensor/numeric helper | `ndarray` | `0.17.2` | Prefer the `ort` ndarray integration initially. Add a direct dependency only if post-processing code needs owned array operations not available through `ort` values or checked `Vec<f32>` code. |

## Justification

`ort` is the narrow Rust ONNX Runtime binding candidate because it exposes ONNX
Runtime sessions, values, execution providers, and binary-loading features
inside Rust. In-house ONNX Runtime FFI would add unsafe native ABI ownership,
platform loading, tensor marshaling, and error mapping work that is outside
this feature's scope.

`tokenizers` is the narrow tokenizer candidate because it is the Rust tokenizer
implementation used by Hugging Face tokenizer JSON files and supports loading
local tokenizer assets. In-house tokenization would be model-family specific
and would make Nomic/Hugging Face tokenizer compatibility a separate parsing
project.

`ndarray` is not selected as a direct first dependency. The `ort` default
feature set already includes ndarray integration, and the initial postprocess
slice should use checked, shape-aware conversion paths before adding broader
numeric dependencies.

## Package Strategy

- First supported ONNX Runtime package target is CPU execution.
- GPU execution-provider features such as CUDA, DirectML, CoreML, TensorRT,
  ROCm, OpenVINO, or XNNPACK are not enabled in the first dependency slice.
- Native ONNX Runtime binary handling must be validated in launcher/release
  smoke before Milestone 8 can close.
- `tokenizers` must load local tokenizer files from the validated model
  directory. HTTP/Hub download features remain disabled for the first slice.

## Risks And Re-Plan Triggers

- `ort` 2.x is currently an RC release. If packaging, API instability, or
  native-library behavior blocks release validation, re-plan before adding more
  execution code.
- `ort` default features include native binary download/copy behavior. The
  manifest slice must decide whether to keep those defaults, use dynamic
  loading, or vendor binaries through launcher packaging.
- `tokenizers` feature selection may introduce native regex dependencies. If
  local tokenizer JSON fixtures require features that complicate packaging,
  record the transitive dependency and release impact before enabling them.
- Direct `ndarray` usage must be justified by post-processing complexity, not
  added by default.

## Verification To Run In Manifest Slice

- `cargo check --manifest-path rust/crates/pumas-core/Cargo.toml`
- `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`
- `cargo tree --manifest-path rust/crates/pumas-core/Cargo.toml -i ort`
- `cargo tree --manifest-path rust/crates/pumas-core/Cargo.toml -i tokenizers`
- Repository audit/license/package-size checks required by the dependency and
  release standards.

## Sources Checked

- `ort` docs.rs crate page and feature flags, observed as `2.0.0-rc.12`.
- `tokenizers` docs.rs crate page and feature flags, observed as `0.23.1`.
- `ndarray` docs.rs crate page and feature flags, observed as `0.17.2`.
