# Integrated headless source baseline

This source baseline combines the producer, acquired-model import, discovery and
Linux inference-built HTTP bootstrap through reviewed source
`67ffdcfa5847807c1a6c70be3a45362838bf3a00`, tree
`4d47f6886652ba978fd0362ce2c56f6740541390`. It is one net-delta commit on public
base `5e114f6d8e4559e0a4d67e56000b423120a0fde0`; production code, tests,
fixtures, generated contracts and license inventories preserve that integrated
source's exact blobs. Only documentation is cleaned up. Pin this baseline's
actual commit and tree from its accompanying qualification receipt, rather than
substituting the older integrated commit when identifying a new build.

The package version remains **0.7.0**. This is a source handoff with bounded Linux
control-plane qualification. v0.8 release acceptance is unfinished, and the
strict inference archive producer still refuses the 0.7 source version.

## Build and exercise the bounded consumer

Use the repository-pinned Rust toolchain and normal native build dependencies,
including a C/C++ compiler, CMake and platform TLS development libraries. Keep
`ORT_SKIP_DOWNLOAD=1`. The ONNX dependency uses dynamic loading; compiling it
neither selects nor loads an inference SDK. Official Cargo dependencies may be
obtained separately; `--offline` requires the locked dependency set already in
Cargo's cache. From the pinned clean checkout:

```sh
ORT_SKIP_DOWNLOAD=1 CARGO_BUILD_JOBS=1 cargo build --locked --offline \
  --manifest-path rust/Cargo.toml -p pumas-rpc --all-features --bin pumas-rpc

rust/target/debug/pumas-rpc --build-info
PUMAS_CONSUMER_TEST_BINARY=/absolute/path/to/rust/target/debug/pumas-rpc \
  python3 -m unittest discover -s scripts/consumers -p 'test_*.py' -v
```

This ordinary development-profile build includes the explicitly selected
`test-support` feature as part of `--all-features`; it is not a release-profile
binary or inference shipping package. A separate `CARGO_TARGET_DIR` changes the
binary path. The reference consumer requires Linux and Python 3.11+; this qualification
uses Python 3.12. The full consumer fixture suite additionally requires
`jsonschema`. The native consumer cohort operates on
isolated, already-existing selected library roots and loopback listeners; it
selects no model and needs no native inference runtime.

Use the [reference consumer](contracts/local-http-consumer.md) to authenticate
and retain a selected HTTP owner. Borrowed-owner cleanup leaves that owner
running. An admitted owned bootstrap must observe shutdown; retained stale or
uncertain ownership refuses replacement. Read the [startup contract](
contracts/inference-enabled-bootstrap.md) for constructor custody, startup
cancellation, fallible readiness and shutdown limits. A ready control plane or
compiled schema advertisement is not model readiness. Query fenced capabilities
for the intended model and operation.

## Remaining acceptance

Real ONNX inference needs an operator-selected trusted C API 24 SDK, its complete
hashed native closure, and an explicitly selected compatible model package with
tokenizer/configuration/external tensors and immutable source/content identities.
Vision additionally needs a compatible model and projector. No SDK/model is
selected or downloaded by this handoff. Actual load, inference, numerics and
loaded-session disposal remain unrun.

Production Audio-to-Text remains unavailable. Its shipping policy catalog is
empty; controlled held-reader and synthetic backend tests do not admit an
installed interpreter/dependency/loader/model read closure or qualify real audio
execution. See [installed audio](contracts/installed-audio-constructor.md).

Linux source builds and bounded local lifecycle observations do not qualify
inference release archives, Windows/macOS native behavior, GPU operation,
external consumer adoption, real AWS/independent-provider/MinIO interoperability,
or physical-store crash recovery. Process exit or a free lock cannot reclaim a
retained generation. Historical refusal evidence remains valid for its exact
older source. The [release acceptance checklist](
plans/v0.8-release-acceptance-reconciliation-2026-10-08.md) retains the separate
model, archive, provider, consumer, security and platform gates.
