# Pumas Library

![License](https://img.shields.io/badge/license-MIT-purple.svg)
![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)
![Electron](https://img.shields.io/badge/electron-38+-blue.svg)
![Platform](https://img.shields.io/badge/platform-Linux%20%7C%20Windows-green.svg)
![Bindings](https://img.shields.io/badge/bindings-Python%20%7C%20C%23%20%7C%20Kotlin%20%7C%20Swift%20%7C%20Ruby%20%7C%20Elixir-violet.svg)

Available as a desktop GUI for end-users, and as a headless Rust crate with language bindings for embeddable API use.

Pumas Library is an easy to use AI model library that downloads, organizes, and serves AI model weights and metadata to other apps. Instead of having models duplicated or scattered across applications, Pumas Library provides a standardized central source that is automatically maintained. When integrated into other software via the Rust crate, it eliminates the need for a slew of file, network, and remote API boilerplate and smart logic.

## Features

### Core Library

- Single portable model library with rich metadata and full-text search (SQLite FTS5)
- HuggingFace integration — search, download with progress tracking, metadata lookup, cached search (24hr TTL)
- Model import with automatic type detection and dual-hash verification (SHA256 + BLAKE3)
- Model mapping — symlink/hardlink models into app directories with health tracking
- Instance convergence — multiple processes share a single primary via local TCP IPC
- Cross-process library discovery via global SQLite registry
- Resilient networking — per-domain circuit breaker, exponential backoff, rate limit handling
- Library merging with hash-based deduplication

**Supported Model Types**: LLM, Diffusion, Embedding, Audio, Vision
**Supported Subtypes**: Checkpoints, LoRAs, VAE, ControlNet, Embeddings, Upscale, CLIP, T5
**Compatible Engines**: Ollama, llama.cpp, Candle, Transformers, Diffusers, ONNX Runtime, TensorRT

### Desktop GUI (Electron)

- Link your apps to your library, no manual setup required
- System and per-app resource monitoring
- Install and run different app versions (ComfyUI, Ollama, OpenWebUI, InvokeAI, KritaDiffusion)
- Smart system shortcuts that don't require the launcher to work
- Plugin system for JSON-based app definitions

## Architecture

### Core Library

The Rust crate (`pumas-library`) operates in one of two transparent modes:

- **Primary** — owns the full state and runs a local IPC server. Holds all subsystems: model library, network manager, process manager, HuggingFace client, model importer, model mapper, IPC server, and registry.
- **Client** — discovers a running primary via the global registry and proxies calls over TCP IPC. The public API is identical in both modes.

Key internals:

- **Registry**: SQLite database at `~/.config/pumas/registry.db` for cross-process library and instance discovery
- **IPC Protocol**: JSON-RPC 2.0 over length-prefixed TCP frames on localhost
- **Search Index**: SQLite FTS5 for model metadata full-text search
- **Best-effort design**: registry and IPC failures never block API initialization
- **Feature flags**: `full` (default), `hf-client`, `process-manager`, `gpu-monitor`, `uniffi`

### Desktop Application

- **Frontend**: React 19 + Vite (rendered in Electron's Chromium)
- **Desktop Shell**: Electron 38+ (with native Wayland support on Linux)
- **Backend**: Rust `pumas-rpc` binary running as a sidecar (Axum HTTP server, JSON-RPC)
- **IPC**: JSON-RPC communication between Electron and the Rust backend

## Quick Start (Rust Crate)

Add the dependency:

```toml
[dependencies]
pumas-library = { path = "rust/crates/pumas-core" }

# Or with specific features only
pumas-library = { path = "rust/crates/pumas-core", features = ["hf-client"] }
```

Basic usage:

```rust
use pumas_library::PumasApi;

#[tokio::main]
async fn main() -> pumas_library::Result<()> {
    // Standard initialization
    let api = PumasApi::new("/path/to/pumas").await?;

    // Or use the builder for more control
    let api = PumasApi::builder("./my-models")
        .auto_create_dirs(true)
        .with_hf_client(false)
        .build()
        .await?;

    // Or discover an existing instance from the global registry
    let api = PumasApi::discover().await?;

    // List models in the library
    let models = api.list_models().await?;
    println!("Found {} models", models.len());

    // Search for models
    let search = api.search_models("llama", 10, 0).await?;
    println!("Search found {} results", search.total_count);

    Ok(())
}
```

## Supported Platforms

| Platform      | Status            | Notes                                                    |
| ------------- | ----------------- | -------------------------------------------------------- |
| Linux (x64)   | Full support      | Debian/Ubuntu recommended, AppImage and .deb packages    |
| Windows (x64) | Full support      | NSIS installer and portable versions                     |
| macOS         | Theoretically Works | Architecture ready, builds available via CI. Not tested. |

## Installation

### System Requirements

#### Linux

- **Operating System**: Linux (Debian/Ubuntu-based distros recommended)
- **Rust**: 1.75+
- **Node.js**: 22+ LTS

#### Windows

- **Operating System**: Windows 11 (x64)
- **Rust**: 1.75+ (install via [rustup](https://rustup.rs/))
- **Node.js**: 22+ LTS (install via [nodejs.org](https://nodejs.org/))
- **Build Tools**: Visual Studio Build Tools with C++ workload

---

## Linux Installation

### Quick Install (Recommended)

Run the automated installation script:

```bash
./install.sh
```

The installer will:

1. Check and install system dependencies (with your permission)
2. Build the Rust backend
3. Install and build the frontend
4. Install and build Electron
5. Create the launcher script

### Manual Installation (Linux)

1. **Install system dependencies** (Debian/Ubuntu):

   ```bash
   sudo apt update
   sudo apt install nodejs npm cargo
   ```

2. **Build Rust backend**:

   ```bash
   cd rust
   cargo build --release
   cd ..
   ```

3. **Install and build frontend**:

   ```bash
   cd frontend
   npm install
   npm run build
   cd ..
   ```

4. **Install Electron dependencies**:

   ```bash
   cd electron
   npm install
   npm run build
   cd ..
   ```

5. **Make launcher executable** (should already be executable):

   ```bash
   chmod +x launcher
   ```

### Optional: Add to PATH (Linux)

For system-wide access:

```bash
ln -s $(pwd)/launcher ~/.local/bin/pumas-library
```

Then run from anywhere:

```bash
pumas-library
```

---

## Windows Installation

### Prerequisites

1. **Install Rust** via [rustup](https://rustup.rs/):

   - Download and run `rustup-init.exe`
   - Follow the prompts to install

2. **Install Node.js** from [nodejs.org](https://nodejs.org/):

   - Download the LTS version (22+)
   - Run the installer

3. **Install Visual Studio Build Tools** (if not already installed):

   - Download from [Visual Studio Downloads](https://visualstudio.microsoft.com/downloads/)
   - Select "Desktop development with C++" workload

### Manual Installation (Windows)

Open PowerShell and run:

1. **Build Rust backend**:

   ```powershell
   cd rust
   cargo build --release
   cd ..
   ```

2. **Install and build frontend**:

   ```powershell
   cd frontend
   npm install
   npm run build
   cd ..
   ```

3. **Install and build Electron**:

   ```powershell
   cd electron
   npm install
   npm run build
   cd ..
   ```

4. **Run the application**:

   ```powershell
   cd electron
   npm start
   ```

### Building Windows Installer

To create a distributable Windows installer:

```powershell
cd electron
npm run package:win
```

This creates:

- NSIS installer (`.exe`) in `electron/release/`
- Portable version (`.exe`) in `electron/release/`

---

## Usage

### Linux Launcher Commands

Run the launcher with different modes:

| Command                                 | Description                                     |
| --------------------------------------- | ----------------------------------------------- |
| `./launcher`                            | Launch the application                          |
| `./launcher dev`                        | Launch with developer tools                     |
| `./launcher build`                      | Build all components (Rust, frontend, Electron) |
| `./launcher build-rust`                 | Build Rust backend only                         |
| `./launcher build-electron`             | Build Electron TypeScript only                  |
| `./launcher package`                    | Package Electron app for distribution           |
| `./launcher electron-install`           | Install Electron dependencies                   |
| `./launcher generate-bindings --python` | Generate Python bindings                        |
| `./launcher generate-bindings --csharp` | Generate C# bindings                            |
| `./launcher generate-bindings --elixir` | Build Elixir Rustler NIF                        |
| `./launcher generate-bindings --all`    | Generate all language bindings                  |
| `./launcher help`                       | Display usage information                       |

### Windows Commands

On Windows, use npm scripts directly:

| Command                              | Description                      |
| ------------------------------------ | -------------------------------- |
| `npm start` (in electron/)           | Launch the application           |
| `npm run dev` (in electron/)         | Launch with developer tools      |
| `npm run package:win` (in electron/) | Package for Windows distribution |

---

## Building from Source

### All Platforms

```bash
# Build Rust backend
cd rust
cargo build --release

# Build frontend
cd ../frontend
npm ci
npm run build

# Build and run Electron
cd ../electron
npm ci
npm run build
npm start
```

### Creating Distribution Packages

| Platform | Command                 | Output                   |
| -------- | ----------------------- | ------------------------ |
| Linux    | `npm run package:linux` | AppImage, .deb           |
| Windows  | `npm run package:win`   | NSIS installer, portable |
| macOS    | `npm run package:mac`   | DMG                      |

---

## Development

### Project Structure

```text
Pumas-Library/
├── rust/                       # Rust workspace
│   └── crates/
│       ├── pumas-core/         # Core headless library (model library, IPC, registry, networking)
│       ├── pumas-app-manager/  # App version and extension management (ComfyUI, Ollama)
│       ├── pumas-rpc/          # Axum JSON-RPC server (Electron backend)
│       ├── pumas-uniffi/       # Python, C#, Kotlin, Swift, Ruby bindings (UniFFI)
│       └── pumas-rustler/      # Elixir/Erlang NIFs (Rustler)
├── frontend/                   # React 19 + Vite frontend
├── electron/                   # Electron 38+ shell
├── bindings/                   # Generated language bindings (not committed)
└── .github/workflows/          # CI/CD
```

### Platform-Specific Code

All platform-specific code is centralized in `rust/crates/pumas-core/src/platform/`:

- `paths.rs` - Platform-specific directories
- `permissions.rs` - File permission handling
- `process.rs` - Process management

### Managed Applications

Process management, version installation, and model mapping are supported for:

ComfyUI, Ollama, OpenWebUI, InvokeAI, and KritaDiffusion.

Additional apps can be defined via the JSON plugin system without code changes.

---

## Language Bindings

Pumas Library's core Rust crate can be used from other languages via cross-language bindings. Two binding systems are available:

- **UniFFI** (Python, C#, Kotlin, Swift, Ruby) — Mozilla's cross-language bindings generator
- **Rustler** (Elixir/Erlang) — Native Implemented Functions for the BEAM VM

### Generating Bindings

Use the launcher to generate bindings:

```bash
# Generate Python bindings
./launcher generate-bindings --python

# Generate C# bindings
./launcher generate-bindings --csharp

# Build Elixir Rustler NIF
./launcher generate-bindings --elixir

# Generate all
./launcher generate-bindings --all
```

Or use the standalone script directly:

```bash
./scripts/generate-bindings.sh python
./scripts/generate-bindings.sh csharp
./scripts/generate-bindings.sh elixir
./scripts/generate-bindings.sh all
```

Generated bindings are written to `bindings/` and are not committed to the repository.

### Prerequisites

| Language | Tool | Install Command |
| -------- | ---- | --------------- |
| Python | uniffi-bindgen | `cargo install uniffi-bindgen-cli` |
| C# | uniffi-bindgen-cs | `cargo install uniffi-bindgen-cs --git https://github.com/NordSecurity/uniffi-bindgen-cs --tag v0.9.0+v0.28.3` |
| Elixir | Rustler | Add `{:rustler, "~> 0.34"}` to `mix.exs` |

### Python

After generating, the bindings are in `bindings/python/`. The native shared library is copied alongside the Python module.

```python
import sys
sys.path.insert(0, "bindings/python")
from pumas_uniffi import version
print(version())
```

### C#

After generating, add the `.cs` files from `bindings/csharp/` to your .NET project and ensure the native library (`libpumas_uniffi.so` / `.dll` / `.dylib`) is in the output directory.

```csharp
using PumasUniFFI;
Console.WriteLine(PumasUniffiMethods.Version());
```

### Elixir

Elixir bindings use Rustler, which compiles the NIF as part of the Mix build rather than generating source files. Add Rustler as a dependency and create a NIF module:

```elixir
# mix.exs
defp deps do
  [{:rustler, "~> 0.34"}]
end
```

```elixir
# lib/pumas/native.ex
defmodule Pumas.Native do
  use Rustler, otp_app: :pumas, crate: "pumas_rustler"

  def version(), do: :erlang.nif_error(:nif_not_loaded)
  def parse_model_type(_type), do: :erlang.nif_error(:nif_not_loaded)
  def validate_json(_json), do: :erlang.nif_error(:nif_not_loaded)
end
```

### UniFFI Feature Flag

The `uniffi` feature on `pumas-core` is optional and only adds derive annotations. It has zero overhead when disabled:

```toml
# Use pumas-core without FFI (default)
pumas-library = { path = "rust/crates/pumas-core" }

# Use pumas-core with UniFFI derives enabled
pumas-library = { path = "rust/crates/pumas-core", features = ["uniffi"] }
```
