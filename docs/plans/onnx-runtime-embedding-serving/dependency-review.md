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
| ONNX Runtime binding | `ort` | `2.0.0-rc.12` | Provisional CPU-first candidate. Add only after a manifest slice that verifies Rust toolchain compatibility, native-library packaging, and lockfile impact. |
| Tokenizer loading/execution | `tokenizers` | `0.23.1` | Provisional local-tokenizer candidate. Use local files only; do not enable HTTP/Hub download features in the first implementation. |
| FP16/BF16 tensor extraction | `half` | `2.7.1` | Required for Rust extraction of ONNX Runtime `f16` outputs from FP16 embedding exports through `ort`'s `half` feature. |
| Tensor/numeric helper | `ndarray` | `0.17.2` | Prefer the `ort` ndarray integration initially. Add a direct dependency only if post-processing code needs owned array operations not available through `ort` values or checked `Vec<f32>` code. |

## Manifest Slice

The first manifest slice added:

```toml
half = "2.7.1"
ort = { version = "2.0.0-rc.12", default-features = false, features = ["std", "ndarray", "tracing", "download-binaries", "copy-dylibs", "tls-native", "api-24", "half"] }
tokenizers = { version = "0.23.1", default-features = false, features = ["onig"] }
```

These dependencies are declared in the workspace and consumed only by
`pumas-core`. `half` is enabled because the local Nomic ONNX fixture is the
FP16 export and `ort` exposes f16/bf16 tensor element extraction through this
feature. `ndarray` is intentionally not a direct dependency; it is present
through `ort`.

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

`half` is the narrow numeric support dependency for FP16/BF16 tensor extraction.
In-house bit conversion would add avoidable correctness risk around IEEE 754
half-precision conversion, NaN/Inf handling, and ONNX Runtime output casting.
It is added only to the Rust crate that owns ONNX execution.

`ndarray` is not selected as a direct first dependency. The `ort` default
feature set already includes ndarray integration, and the initial postprocess
slice should use checked, shape-aware conversion paths before adding broader
numeric dependencies.

## Package Strategy

- First supported ONNX Runtime package target is CPU execution.
- GPU execution-provider features such as CUDA, DirectML, CoreML, TensorRT,
  ROCm, OpenVINO, or XNNPACK are not enabled in the first dependency slice.
- `ort` 2.0.0-rc.12 reports `rust-version = 1.88`; the manifest slice must
  verify the repository toolchain before adding it.
- `ort` default features are `std`, `ndarray`, `tracing`, `download-binaries`,
  `tls-native`, `copy-dylibs`, and `api-24`. The manifest slice must make an
  explicit default-features decision instead of accepting those implicitly.
- Native ONNX Runtime binary handling must be validated in launcher/release
  smoke before Milestone 8 can close.
- `tokenizers` must load local tokenizer files from the validated model
  directory. HTTP/Hub download features remain disabled for the first slice.
- `half` is enabled only for tensor extraction from real ONNX Runtime outputs;
  it does not change the CPU execution-provider package strategy.
- `tokenizers` default features are `progressbar`, `onig`, and `esaxx_fast`.
  The manifest slice uses only `onig` so local tokenizer JSON regex support is
  available without HTTP/Hub, progress bar, or `esaxx_fast` training-oriented
  features.
- `ndarray` default feature is `std`; do not enable BLAS, Rayon, serde, or
  approximation features unless a later post-processing slice needs them.

## Risks And Re-Plan Triggers

- `ort` 2.x is currently an RC release and requires Rust 1.88. If the repo
  toolchain, packaging, API stability, or native-library behavior blocks release
  validation, re-plan before adding more execution code.
- `ort` default features include native binary download/copy behavior plus
  native TLS. The manifest slice must decide whether to keep those defaults,
  use dynamic loading, or vendor binaries through launcher packaging.
- `tokenizers` feature selection may introduce native regex dependencies. If
  local tokenizer JSON fixtures require features that complicate packaging,
  record the transitive dependency and release impact before enabling them.
- Direct `ndarray` usage must be justified by post-processing complexity, not
  added by default.
- If FP16 extraction introduces unacceptable package, audit, or release impact,
  re-plan before supporting FP16 ONNX exports through real inference.

## Verification To Run In Manifest Slice

- `cargo check --manifest-path rust/crates/pumas-core/Cargo.toml`
- `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`
- `cargo tree --manifest-path rust/crates/pumas-core/Cargo.toml -i ort`
- `cargo tree --manifest-path rust/crates/pumas-core/Cargo.toml -i tokenizers`
- `cargo tree --manifest-path rust/crates/pumas-core/Cargo.toml -i half`
- Repository audit/license/package-size checks required by the dependency and
  release standards.

## Manifest Slice Evidence

- `rustc --version`: `rustc 1.92.0`, satisfying `ort`'s Rust 1.88 requirement.
- `cargo check --manifest-path rust/crates/pumas-core/Cargo.toml`: passed.
- `cargo test --manifest-path rust/crates/pumas-core/Cargo.toml onnx`: passed.
- `cargo fmt --manifest-path rust/Cargo.toml --all -- --check`: passed.
- `cargo tree --manifest-path rust/crates/pumas-core/Cargo.toml -i ort`: `ort`
  is pulled only by `pumas-library`.
- `cargo tree --manifest-path rust/crates/pumas-core/Cargo.toml -i
  tokenizers`: `tokenizers` is pulled only by `pumas-library`.
- `cargo tree --manifest-path rust/crates/pumas-core/Cargo.toml -i ndarray`:
  `ndarray` is pulled through `ort`, then `pumas-library`.
- `cargo tree --manifest-path rust/crates/pumas-core/Cargo.toml -i half`:
  `half` is pulled directly by `pumas-library` and through `ort`, then
  `pumas-library`.
- `cargo audit --version`: unavailable in this environment (`cargo-audit` is
  not installed). Security advisory audit remains open before release.

## Sources Checked

- `cargo info ort@2.0.0-rc.12`, crates.io/docs.rs metadata.
- `cargo info tokenizers@0.23.1`, crates.io/docs.rs metadata.
- `cargo info ndarray@0.17.2`, crates.io/docs.rs metadata.
