# System Architecture

## Purpose

Describe the current runtime architecture for the desktop app and shared Rust library.

## Top-Level Components

1. `frontend` (React + Vite renderer)
2. `electron` (main process + preload bridge)
3. `pumas-rpc` (Rust JSON-RPC server sidecar)
4. `pumas-library` / `pumas-core` (model/system API)
5. `pumas-app-manager` (app version + dependency + launcher management)

## Runtime Boundary

Renderer code does not access Node APIs directly.

- Renderer calls methods on `window.electronAPI` (exposed by `electron/src/preload.ts`).
- Electron main process handles those calls and forwards backend RPC operations through `api:call`.
- `PythonBridge` (name retained for compatibility) launches the Rust `pumas-rpc` binary and sends JSON-RPC requests to it over HTTP on localhost.
- `pumas-rpc` routes requests to `pumas-core` and `pumas-app-manager` services.

## Process Model

### Electron Main Process

- Starts the Rust sidecar binary (`pumas-rpc` or `pumas-rpc.exe`).
- Chooses launcher root based on environment:
  - AppImage portable path when running as AppImage
  - standard Electron user data for packaged builds
  - project root in local dev
- Hosts browser window lifecycle, shell/dialog window controls, and IPC handlers.

### Rust Sidecar (`pumas-rpc`)

- Axum server with:
  - `GET /health`
  - `POST /rpc`
- Initializes:
  - `PumasApi` (`auto_create_dirs(true)`)
  - per-app `VersionManager` instances (currently `comfyui`, `ollama`, `torch`)
  - `CustomNodesManager`, `SizeCalculator`, and `PluginLoader`

### Core API (`pumas-core`)

`PumasApi` runs in two transparent modes:

- `Primary`: owns full state and services
- `Client`: discovers an existing primary through registry + IPC and proxies calls

Registry and IPC are intentionally best-effort to keep startup resilient.

## Storage Layout (Launcher Root)

Key paths used by the current implementation:

- `launcher-data/metadata/` - persisted metadata
- `launcher-data/cache/` - runtime cache and download persistence
- `launcher-data/mapping-configs/` - mapping configuration
- `launcher-data/plugins/` - plugin app descriptors
- `shared-resources/models/` - canonical model library root
- `shared-resources/models/models.db` - SQLite model index
- `shared-resources/cache/search.sqlite` - HuggingFace search cache
- `<app>-versions/` directories for managed app version installs (`comfyui-versions`, `ollama-versions`, `torch-versions`)

## Managed Applications

The `AppId` enum supports multiple app identifiers, while the current sidecar initialization path actively starts managers for:

- ComfyUI
- Ollama
- Torch

Plugin descriptors under `launcher-data/plugins/*.json` drive UI capability surfaces and app-specific behavior.

## API Surfaces

### Renderer API

- Strongly typed methods exposed by preload and consumed from `frontend/src/api/adapter.ts`.
- Includes RPC passthrough and Electron-specific window/file/shell helpers.

### Backend RPC

- JSON-RPC methods implemented in `rust/crates/pumas-rpc/src/handlers`.
- Wrappers over:
  - model library
  - version management
  - dependency requirement resolution
  - mapping/import/download flows
  - utility and migration/report operations

## Build and Packaging Architecture

- Rust crates built in `rust` workspace.
- Electron packaging bundles renderer assets and the `pumas-rpc` binary as extra resources.
- CI runs cross-platform build/test/package jobs for Linux, Windows, and macOS runners.

## Non-Goals of This Document

- Detailed endpoint-by-endpoint API contracts
- UI component-level design guidelines
- migration plans for old pre-0.2.0 code paths

Use docs under `docs/` and crate-local READMEs for those details.
