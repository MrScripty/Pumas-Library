# Pumas Library

![License](https://img.shields.io/badge/license-MIT-purple.svg)
![Rust](https://img.shields.io/badge/rust-1.92.0-orange.svg)
![Electron](https://img.shields.io/badge/electron-39-blue.svg)

![Pumas Library](https://github.com/user-attachments/assets/be18cffc-b4fe-418b-a3b4-034ee0b35060)

Pumas Library is a reusable Rust backend with an optional desktop GUI for keeping AI model files,
metadata, downloads, and local-runtime configuration in one place.

Its main capabilities are:

- an SQLite-indexed local model library with full-text search;
- Hugging Face search, metadata lookup, and resumable downloads;
- local import, integrity reconciliation, and repair;
- typed resolution of model artifacts and runtime requirements; and
- optional local inference integrations for Ollama, llama.cpp, ONNX Runtime,
  and Torch.

## Repository Layout

| Path | Responsibility |
| --- | --- |
| `rust/crates/pumas-core` | Model library, persistence, downloads, runtime profiles, and public Rust API |
| `rust/crates/pumas-rpc` | Standalone local HTTP/JSON-RPC server, also used by the desktop app |
| `rust/crates/pumas-app-manager` | Optional runtime installation and process integration |
| `rust/crates/pumas-uniffi` | Experimental UniFFI adapter and generators |
| `frontend` | React renderer |
| `electron` | Desktop main process, preload boundary, and packaging |
| `torch-server` | Optional Python Torch inference sidecar |

See [Architecture](docs/ARCHITECTURE.md) for process and ownership details.

## Desktop Quick Start

The root launchers require Node and delegate every action to one shared
implementation, so Bash and PowerShell use the same parsing, environment, and
exit-code contract.

```bash
./launcher.sh --install
./launcher.sh --build
./launcher.sh --run
```

On Windows:

```powershell
.\launcher.ps1 --install
.\launcher.ps1 --build
.\launcher.ps1 --run
```

Release-mode local builds use `--build-release` followed by `--run-release`.
Run either launcher with `--help` for the complete command and exit-code
contract. Unsupported operating systems are rejected explicitly. The
`--release-smoke` action owns its child process tree and treats a missed
maximum/grace/force deadline as failure rather than leaving the smoke running.

Inference integrations are included in the default desktop build. Build the
model-library-only variant with:

```bash
PUMAS_INFERENCE_PLUGINS=false ./launcher.sh --build-release
```

`PUMAS_LAUNCHER_ROOT=/path/to/root` selects a specific library root. A launcher
root contains `launcher-data/` and `shared-resources/`; the model library itself
lives under `shared-resources/models/`.

## Standalone Backend

Build and run without Node, Corepack, frontend assets or Electron:

```bash
cargo build --manifest-path rust/Cargo.toml -p pumas-rpc --release
./rust/target/release/pumas-rpc --launcher-root /path/to/pumas --port 8080
```

Use `--no-default-features` on the Cargo build to omit RPC inference-plugin
integration. GUI selection does not disable backend model-library operations.
The RPC listener accepts loopback addresses only; standalone does not imply
remote-network exposure. Its port and root can be selected with `--help`.

The optional Node launcher exposes the same backend-only selection:

```bash
PUMAS_GUI=false ./launcher.sh --build-release
PUMAS_GUI=false ./launcher.sh --run-release -- --launcher-root /path/to/pumas
```

`PUMAS_GUI` defaults to `true` and accepts only `true` or `false`. Select it
for each launcher invocation. With `false`, build/install/test omit GUI packages
and run requires only the selected existing backend binary; it never starts
Electron or builds missing artifacts. `PUMAS_INFERENCE_PLUGINS` is independent
and controls the backend build configuration. `--release-smoke` remains a
GUI-only check and is explicitly unsupported in headless mode.

## Rust Usage

The backend does not depend on React, Electron, or a running GUI. Applications
can embed the Rust crate directly; separate processes can use the standalone
RPC server. Native file dialogs and window controls belong only to the GUI,
not to the reusable model-library contract.

`PumasApi` is the owning API. Construction fails when another process already
owns the same launcher root. Use `PumasLocalClient` to connect to a running
owner, or `PumasReadOnlyLibrary` for indexed read-only access.

```rust
use pumas_library::{PumasApi, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let api = PumasApi::builder("/path/to/pumas")
        .auto_create_dirs(true)
        .build()
        .await?;

    for model in api.list_models().await? {
        println!("{}", model.official_name);
    }

    Ok(())
}
```

The crate is currently consumed from this workspace:

```toml
[dependencies]
pumas-library = { path = "rust/crates/pumas-core" }
```

Download mutations also acquire advisory exclusion on the physical model-library
root. Independent HF clients contend even when using different destinations;
one client's active downloads share the grant. Idle and paused clients release
it. Contention returns `DownloadRootBusy` without automatic retry, including when
startup requires download restoration. Read-only runtime progress remains
available but may be stale. This does not lock unrelated imports or external
writers; native root-exclusion behavior is verified on Linux only.

## Verification

```bash
./scripts/rust/check.sh
npm run -w frontend lint
npm run -w frontend check:types
npm run -w frontend test:run
npm run -w electron lint
npm run -w electron test
npm run test:launcher
python3 -m unittest discover -s torch-server/tests
```

Use `./launcher.sh --test` for the launcher-owned aggregate flow. A passing
build or startup smoke is not evidence for every runtime or release contract;
see [Development](docs/DEVELOPMENT.md) and [Releasing](RELEASING.md).

## Platform and Binding Status

Linux x64 is the primary development and runtime target. CI also compiles and
packages Windows x64 and macOS arm64 artifacts, but runtime evidence on those
platforms is narrower. Launcher process-tree behavior is verified locally on
Linux; equivalent required-real Windows and macOS evidence remains pending.

Binding generators exist for Python, Kotlin, Swift, Ruby, and C#, with a local
C# smoke harness. These surfaces are not yet backed by a complete host/runtime
support matrix. The Rustler crate is experimental and does not currently expose
the core library API. See [Native bindings](docs/native-bindings.md).

## Documentation

- [Documentation index](docs/README.md)
- [Contributing](CONTRIBUTING.md)
- [Releasing](RELEASING.md)
- [Security](docs/SECURITY.md)
- [Current standards audit](docs/audits/current-standards-2026-09-03/README.md)

## License

Pumas Library is licensed under the [MIT License](LICENSE).
