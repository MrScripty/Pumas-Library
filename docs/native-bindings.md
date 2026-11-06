# Native Bindings

## Status

No host-language binding is supported. `bindings/support-matrix.json` is the
authority: its contract status is `no_supported_host_bindings` with no accepted
host tuple. It lists each surface under `unsupported_surfaces` with a status,
because none has a real host consumer or host-runtime evidence:

- `present_pending_removal`: UniFFI Python, Kotlin, Swift, Ruby and C#, and the
  Rustler/Elixir crate. Their removal is decided but not carried out: the
  `pumas-uniffi` and `pumas_rustler` crates, the `uniffi` feature and derives in
  `pumas-core`, the generators and the local smoke still exist. Nothing may
  depend on them.
- `claim_removed`: Go. It was only ever a documentation claim, now removed.

CI does not generate or package bindings, and binding archives are not release
assets. `pumas_rustler` does not expose the core Pumas library API.
`scripts/release/bindings-support-matrix.test.mjs` keeps the matrix, the
adapter documentation and the generator pins consistent.

## Boundary

```text
host application
  -> generated language binding
    -> pumas_uniffi native library
      -> pumas-uniffi adapter
        -> pumas-core
```

The adapter owns FFI-safe records, conversions, validation, runtime bridging,
and host-visible error projection. Generated sources and native libraries are a
single compatibility cohort and must come from the same build.

## Generate and Check

```bash
./scripts/generate-bindings.sh python
./scripts/generate-bindings.sh csharp
./scripts/check-uniffi-surface.sh
./scripts/check-uniffi-csharp-smoke.sh
```

Generated outputs belong under ignored build/output directories and must not be
hand-edited. Python, Kotlin, Swift and Ruby output comes from the in-tree
`pumas-uniffi-bindgen`, run with `cargo run --locked`, so the generator always
matches the workspace's locked `uniffi` version. When `uniffi-bindgen-cs` is
absent, the script installs it with `--locked` at the commit of tag
`v0.9.0+v0.28.3`, which matches `uniffi` 0.28.3.

The C# smoke compiles generated sources, loads the matching native library, and
calls the synchronous version function. It does not prove async APIs or domain
workflows.

## Local C#/Native Packaging

```bash
./scripts/package-uniffi-csharp-artifacts.sh
```

The script stages archives under `rust/target/bindings-package/artifacts/` and
copies this document into them. Inspect each generated `manifest.json` for the
native filename, platform, profile, and matching package identity.

At runtime, place the matching native library beside the host executable or on
the platform's native-library search path (`PATH`, `LD_LIBRARY_PATH`, or
`DYLD_LIBRARY_PATH`). Never combine generated sources and a native library from
different commits, versions, targets, or profiles.

## Promotion Requirements

A host/language tuple is admitted to the support matrix only with a named real
consumer, an exact host runtime and native target, a supported operation and
lifecycle contract, deterministic cohort generation, and CI or equivalent
release evidence that exercises the exact packaged cohort through:

- native library loading;
- a synchronous call;
- an async call where exported;
- invalid-input and error projection; and
- the documented target/runtime versions.

The [current standards audit](audits/current-standards-2026-09-03/README.md)
tracks the remaining binding and packaging gaps.
