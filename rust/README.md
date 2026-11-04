# Rust Workspace

The Rust workspace owns the model library, local RPC service, optional runtime
integration, and experimental host-language adapters.

## Crates

| Crate | Responsibility |
| --- | --- |
| `pumas-library` (`crates/pumas-core`) | Library state, metadata, SQLite index, imports, downloads, reconciliation, runtime profiles, and serving contracts |
| `pumas-app-manager` | External runtime installation, version selection, processes, and service clients |
| `pumas-rpc` | Desktop sidecar, HTTP/JSON-RPC dispatch, events, and `/v1` gateway |
| `pumas-uniffi` | Experimental UniFFI adapter; no supported host binding ([status](../docs/native-bindings.md)) |
| `pumas-rustler` | Experimental Rustler/NIF conversion surface; no supported host binding |

`pumas-rustler` is excluded from the default workspace test set because it
requires an Erlang/OTP host. It does not currently expose the core library API.

## Ownership Model

One `PumasApi`/`PumasLibraryInstance` owns a launcher root and its lifecycle.
`PumasLocalClient` connects to an existing owner, while
`PumasReadOnlyLibrary` provides indexed read-only access. Do not silently fall
back from owner construction to client or read-only behavior.

The core crate owns durable state. RPC owns transport and decoding. App manager
owns external tools and processes. Binding crates own conversion at their host
boundary rather than duplicating core policy.

## Provider Model

Inference behavior is provider-scoped. Ollama and llama.cpp use managed
external processes, ONNX Runtime runs in the Rust process, and Torch uses a
Python sidecar. Runtime profiles, model routes, served-instance identity, and
gateway capability checks remain separate contracts. See
[ADR 0001](../docs/adr/0001-onnx-runtime-provider-model.md).

## Features and Variants

This section owns the Cargo feature facts that other guides link to.

`pumas-rpc` enables `inference-plugins` by default (`pumas-app-manager` plus the
core `onnx-runtime`, `process-manager` and `gpu-monitor` features). Its verified
inference-disabled variant is:

```bash
cargo build --manifest-path rust/Cargo.toml -p pumas-rpc --no-default-features
```

`s3` (S3 import RPCs), `export-contract` (desktop contract export) and
`test-support` are opt-in.

`pumas-library` enables `full` by default: `hf-client`, `process-manager`,
`gpu-monitor` and `onnx-runtime`. Only `onnx-runtime`, `s3`, `contract-schema`
and `uniffi` gate optional dependencies. `hf-client` and `gpu-monitor` change
only the advertised build-info feature list, and `process-manager` only whether
the builder may enable the process manager; none of the three removes a
dependency or public module. `default-features = false` therefore drops ONNX
Runtime, tokenizers and half-precision support, but is not otherwise a minimal
build. Do not describe it as minimal until those three features gate their code.

## Verification

From the repository root:

```bash
./scripts/rust/check.sh
cargo test --manifest-path rust/Cargo.toml -p pumas-library <test-filter>
cargo test --manifest-path rust/Cargo.toml -p pumas-rpc <test-filter>
```

The aggregate check runs formatting, all-target/all-feature compilation,
Clippy, default-member tests and docs, and no-default compilation. Keep tests
isolated from the user's launcher root, database, processes, ports, and
environment.

Workspace lint policy denies unsafe code by default. A module that owns an OS
or FFI boundary may opt down locally, but every unsafe block requires a
boundary-specific safety argument.

See [Architecture](../docs/ARCHITECTURE.md),
[Development](../docs/DEVELOPMENT.md), and the
[current standards audit](../docs/audits/current-standards-2026-09-03/README.md).
