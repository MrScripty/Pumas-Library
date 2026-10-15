# Pumas Library

Share one local model library across your applications, without keeping duplicate copies of the same weights.

Pumas Library combines a reusable Rust backend with an optional desktop app. Search Hugging Face, resume downloads, import existing files, and keep model metadata and runtime requirements alongside the files they describe. Applications can embed the Rust API or connect to the local HTTP/JSON-RPC server.

![Pumas Library](https://github.com/user-attachments/assets/be18cffc-b4fe-418b-a3b4-034ee0b35060)

## Run the desktop app

Use Node.js 24.15.0, Rust 1.92.0 and Corepack, plus CMake and your platform's C/C++ build tools for native dependencies. The workspace pins pnpm in [package.json](package.json). From a checkout:

```bash
git clone https://github.com/MrScripty/Pumas-Library.git
cd Pumas-Library
./launcher.sh --install
./launcher.sh --build
./launcher.sh --run
```

On Windows, use `launcher.ps1` with the same flags. For a release build, use `--build-release` followed by `--run-release`. The launcher installs workspace packages; Rust, Node and Corepack must already be available. See [development setup](docs/DEVELOPMENT.md#setup) for platform details.

Your library root contains `launcher-data/` and `shared-resources/`; models live in `shared-resources/models/`. Set `PUMAS_LAUNCHER_ROOT` to choose a root. Read [the upgrade guide](docs/library-upgrade.md) before opening an older library.

## Run just the backend

Use Rust 1.92.0, CMake and your platform's C/C++ build tools. No Node, frontend assets or Electron are needed:

```bash
cargo build --manifest-path rust/Cargo.toml -p pumas-rpc --release
./rust/target/release/pumas-rpc --launcher-root /path/to/pumas --port 8080
```

The listener is loopback-only. Add `--no-default-features` to the build to omit inference-plugin integration. Inference runtimes and their native libraries have separate setup requirements; a working model catalog does not require every runtime.

## Integrate it

Use `pumas-library` from [pumas-core](rust/crates/pumas-core) for an embedded library, or [pumas-rpc](rust/crates/pumas-rpc/README.md) for a separate process. One `PumasApi` owns a library root; use `PumasLocalClient` to connect to an existing owner. Native bindings are experimental: check their [support status](docs/native-bindings.md) before choosing one.

Linux x64 is the primary runtime target. Windows x64 and macOS arm64 build artifacts exist, with narrower runtime qualification. See [release guidance](RELEASING.md).

## Go deeper

- [Documentation](docs/README.md): APIs, runtime integrations, optional S3 import and contracts
- [Architecture](docs/ARCHITECTURE.md)
- [Development and verification](docs/DEVELOPMENT.md)
- [Contributing](CONTRIBUTING.md) · [Security](docs/SECURITY.md)

[MIT License](LICENSE).

