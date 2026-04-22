# Pumas Library

![License](https://img.shields.io/badge/license-MIT-purple.svg)
![Rust](https://img.shields.io/badge/rust-1.92.0-orange.svg)
![Electron](https://img.shields.io/badge/electron-38+-blue.svg)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Windows%20%7C%20macOS-green.svg)
![Bindings](https://img.shields.io/badge/bindings-Python%20%7C%20C%23%20%7C%20Kotlin%20%7C%20Swift%20%7C%20Ruby%20%7C%20Elixir-violet.svg)

![banner](https://github.com/user-attachments/assets/be18cffc-b4fe-418b-a3b4-034ee0b35060)

Pumas Library is a shared AI asset library for people and applications that need one dependable place to store model files, track metadata, and manage downloads. It is available both as a desktop application and as a headless Rust library with cross-language bindings.

The core idea is simple: stop treating model storage as app-by-app glue code. Pumas gives you a single library that can be searched, repaired, linked into downstream tools, and embedded into other software without rebuilding the same filesystem, networking, and metadata logic over and over.

## Why Pumas

- Centralize model weights and metadata instead of duplicating them across tools
- Keep downloads resumable and library state repairable after interruptions
- Search and inspect a large local library with structured metadata and full-text indexing
- Embed the same core behavior into desktop apps, services, and language bindings
- Package a desktop experience and a reusable backend from the same codebase

## What This Repository Contains

- `rust/crates/pumas-core`: the core headless library
- `rust/crates/pumas-rpc`: the Rust sidecar/backend used by the desktop shell
- `rust/crates/pumas-uniffi`: UniFFI bindings surface
- `rust/crates/pumas-rustler`: Rustler bindings for Elixir/Erlang
- `frontend/`: the React UI
- `electron/`: the desktop shell and packaging configuration
- `bindings/`: generated binding artifacts and packaging outputs

Standards adoption is tracked in [docs/STANDARDS_ADOPTION.md](docs/STANDARDS_ADOPTION.md).

## Core Capabilities

- Shared model library with SQLite-backed metadata and FTS5 search
- Hugging Face search, metadata fetch, and resumable download support
- Model import with hashing and type detection
- Library reconciliation and repair flows for drifted on-disk state
- Link and mapping support so consumer tools can reference a central library
- Cross-process discovery and primary/client coordination over local IPC
- Network resilience with caching, retries, and circuit breaking
- Cross-language access through Rust, Python, C#, Kotlin, Swift, Ruby, and Elixir/Erlang

Supported model families include text, diffusion, embedding, audio, and vision workloads, with metadata and indexing designed for mixed libraries rather than a single runtime.

## Architecture At A Glance

The Rust API runs in one of two transparent modes:

- `Primary`: owns the local state, runs the IPC server, and manages the library directly
- `Client`: discovers an existing primary instance and proxies requests to it

That design lets multiple processes share one library safely while presenting the same public API either way.

Key implementation pieces:

- SQLite for metadata storage and full-text search
- Local JSON-RPC over TCP for cross-process API access
- A global registry for instance and library discovery
- Best-effort startup behavior so registry or IPC failures do not block initialization

## Quick Start

### Desktop App

The desktop launcher has one shared CLI contract with thin platform wrappers:

```bash
# Linux / macOS
./launcher.sh --install
./launcher.sh --build-release
./launcher.sh --run
```

```powershell
# Windows PowerShell
./launcher.ps1 --install
./launcher.ps1 --build-release
./launcher.ps1 --run
```

If PowerShell blocks local scripts on Windows, use:

```powershell
powershell -ExecutionPolicy Bypass -File .\launcher.ps1 --help
```

Use the same lifecycle flags on both wrappers:

| Flag | Purpose |
| ---- | ------- |
| `--install` | Install workspace dependencies |
| `--build` | Build debug artifacts |
| `--build-release` | Build release artifacts |
| `--run` | Run the desktop app in development mode |
| `--run-release` | Run the built desktop runtime |
| `--test` | Run the canonical launcher-facing verification flow |
| `--release-smoke` | Launch the release runtime briefly and fail if startup is not healthy |
| `--help` | Show launcher usage |

Note: `--run` expects the debug backend binary from `--build`. Use
`--build-release` before `--run-release` or `--release-smoke`.

Packaged desktop builds try to reuse an existing launcher root by walking up
from the packaged binary location. If you need to pin a specific existing
library root, set `PUMAS_LAUNCHER_ROOT=/path/to/root` before launching the app.

### Rust Crate

Add the core crate:

```toml
[dependencies]
pumas-library = { path = "rust/crates/pumas-core" }
```

Minimal example:

```rust
use pumas_library::PumasApi;

#[tokio::main]
async fn main() -> pumas_library::Result<()> {
    let api = PumasApi::new("/path/to/pumas").await?;

    let models = api.list_models().await?;
    println!("Found {} models", models.len());

    let search = api.search_models("llama", 10, 0).await?;
    println!("Search found {} results", search.total_count);

    Ok(())
}
```

Alternative initialization styles are also available through `PumasApi::builder(...)` and `PumasApi::discover()`.

## Repairing a Drifted Library

When the filesystem and SQLite metadata drift apart, run the integrity repair example:

```bash
cd rust
cargo run --package pumas-library --example repair_library_integrity -- /path/to/shared-resources/models
```

This flow is intended for recovery scenarios such as interrupted downloads, stale index rows, and partial content that needs to be reconciled back into canonical library state.

## Language Bindings

Two binding paths are supported:

- `UniFFI` for Python, C#, Kotlin, Swift, and Ruby
- `Rustler` for Elixir/Erlang

Generate bindings with:

```bash
./scripts/generate-bindings.sh python
./scripts/generate-bindings.sh csharp
./scripts/generate-bindings.sh elixir
./scripts/generate-bindings.sh all
```

Useful binding validation and packaging helpers:

```bash
./scripts/check-uniffi-csharp-smoke.sh
./scripts/package-uniffi-csharp-artifacts.sh
```

Generated outputs are written under `bindings/`.

## Build and Package

### Build From Source

```bash
corepack pnpm install --frozen-lockfile

cd rust
cargo build --release

cd ..
npm run -w frontend build
npm run -w electron build
```

### Package Desktop Releases

From `electron/`:

| Command | Output |
| ------- | ------ |
| `npm run package:linux` | AppImage and `.deb` |
| `npm run package:win` | Windows installer and portable executable |
| `npm run package:mac` | DMG |

## Supported Platforms

| Platform | Status | Notes |
| -------- | ------ | ----- |
| Linux (x64) | Full support | Primary packaging target |
| Windows (x64) | Full support | Installer and portable outputs |
| macOS (ARM) | Best-effort | Build support exists, regular testing is lighter |

## Release Validation

Before cutting a release, run:

```bash
corepack pnpm install --frozen-lockfile
./launcher.sh --test
./launcher.sh --release-smoke

cd rust
cargo test --workspace --exclude pumas_rustler
cargo clippy --workspace --exclude pumas_rustler -- -D warnings
cargo build --workspace --exclude pumas_rustler

cd ..
npm run -w frontend test:run
npm run -w frontend check:types
npm run -w frontend build
npm run -w electron validate
npm run -w electron build
```

Use the same launcher flags with `./launcher.ps1` on Windows. The underlying
`cargo` and `npm` validation commands are the same there.

For `pumas_rustler`, run its checks on a machine with Erlang/OTP installed.

## Development Notes

- Rust, Node, and the workspace package manager are pinned in `rust-toolchain.toml`, `.node-version`, and the root `package.json`
- The desktop app is built from the React frontend plus the Electron shell plus the Rust `pumas-rpc` sidecar
- The canonical desktop workflow is the shared launcher contract exposed by
  `launcher.sh` on Unix and `launcher.ps1` on Windows
- The repository contains both reusable library code and end-user application packaging

## Repository Layout

```text
Pumas-Library/
├── rust/
│   └── crates/
│       ├── pumas-core/
│       ├── pumas-rpc/
│       ├── pumas-uniffi/
│       └── pumas-rustler/
├── frontend/
├── electron/
├── bindings/
├── scripts/
└── .github/workflows/
```

## License

MIT
