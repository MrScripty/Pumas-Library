# Pumas → Pantograph guard interoperability

This standalone harness calls the unchanged `SelectedAudioLoad::validate` from
Pantograph commit `038dacaaa98ebd007c32e13d4608726ca5ccf63a`. It compiles against
that checkout's actual inference types and the current local Pumas library. It
also compiles the pinned host's unchanged entry-path privacy-normalization functions.
The runner checks the commit, guard blob, and tracked source cleanliness.

The fixture registers a synthetic managed Cohere package, resolves package facts
and an owner-fresh target through production Pumas APIs, serializes their actual
wire output, and adapts it to Pantograph's contracts. The positive admission,
primary-weight path regression, absent/blank fingerprint, model/artifact/path/
revision/contract/storage mismatch, and changed-package observation tests all
call the real consumer guard. The synthetic weight bytes cannot load a model.
No inference backend, model download, model loading, or credentials are used.

With an existing clean public checkout at the pinned commit and Rust source
dependencies cached:

```sh
ORT_SKIP_DOWNLOAD=1 python3 rust/crates/pumas-core/tests/pantograph_guard_interop/run.py \
  --pantograph /path/to/Pantograph
```

The default execution is `cargo test --offline --locked --lib`. The separate
`--resolve-source-dependencies` option permits public Rust source dependency
resolution for initial setup; it keeps all inference backends and Pumas's ONNX
runtime disabled. A retained `--work-dir` exposes the generated manifest, lock,
and module-path adapter for review. This harness tests admission and package
observation, not the consumer's private resident cache or real Cohere inference.
