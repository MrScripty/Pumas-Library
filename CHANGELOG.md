# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- No changes yet.

## [0.5.0] - 2026-05-03

### Added

- Package-facts APIs for summary snapshots, lazy package-facts resolution, model-reference resolution, update events, and persisted update-feed cursors
- Rich package evidence for import and execution review, including tokenizer diagnostics, special tokens, generation defaults, custom code requirements, auto-map and processor metadata, adapter/quantization/shard facts, sibling files, class references, source-repository evidence, and missing declared shards
- Hugging Face search and model-library compatibility hints for MLX, vLLM, unsupported backends, and unresolved/canonicalized Pumas model references
- Electron/RPC contract validation, plugin endpoint bridging, request schemas, empty-parameter handling, and preload drift tests
- Standards adoption, release-artifact, native-bindings, desktop-RPC, workspace ownership, and audit documentation, plus release-version, dependency-ownership, commit-message, README coverage, and file-size checks

### Changed

- Refactored Rust runtime paths to move blocking filesystem and metadata work off async request paths, isolate path validation at API/RPC/UniFFI ingress, own background task handles, and cap RPC in-flight concurrency
- Split the frontend app shell, import workflow, model rows, version controls, mapping previews, metadata modals, install dialogs, download state, and API type contracts into smaller tested modules
- Split UniFFI bindings into focused API and FFI type modules while tightening native binding input validation
- Updated launcher and release flows to run from current build outputs without requiring toolchain dependencies in packaged release launches
- Updated README and workspace documentation for the current build, launcher, Rust, frontend, Electron, scripts, torch-server, and native-binding workflows

### Fixed

- Hardened desktop IPC, RPC CORS, Torch sidecar access, LAN listener policy, path canonicalization, writable target probes, native import paths, migration/recovery targets, and direct file/open operations
- Restored frontend reliability for library download progress rings, inactive download indicators, cleaned metadata fallbacks, link-health rendering, app icon assets, model preview limits, failed shortcut rollback, and native accessible controls
- Corrected package-facts cache invalidation, metadata projection cleanup, library size calculation, proxy link exclusions, and unresolved library-path handling
- Fixed launcher dev-run backend artifact selection, release checksum digest exclusions, ComfyUI temp directory handling, Windows symlink test permissions, Rust/frontend CI failures, and write-target path canonicalization coverage

## [0.4.0] - 2026-04-13

### Added

- Shared cross-platform launcher core with consistent Bash and PowerShell lifecycle flows, including bounded `--release-smoke` verification
- GitHub release checks and download prompts in the desktop updater flow
- UniFFI C# smoke verification and release-packaging scripts for generated bindings and native artifacts
- Packaged-app fallback flow that lets users choose and persist an existing library root when no library is detected automatically

### Changed

- Moved workspace tooling ownership and CI bootstrap flows to the pinned pnpm/Corepack contract across frontend, Electron, and launcher workflows
- Refined desktop shell defaults and header interactions, including default window sizing, drag behavior, and feature-split production frontend bundles
- Updated repo-facing documentation for current cross-platform setup, launcher usage, and contribution workflows

### Fixed

- UniFFI download request drift that broke `pumas-uniffi` builds
- Packaged desktop launcher root detection, shortcut relocation, and release GUI startup behavior
- Linux CI Electron smoke execution under sandboxed GitHub runners
- Reconciliation, registry/test isolation, and canonical path handling across Linux, macOS, and Windows CI
- Tokio example runtime configuration so documented examples compile with the pinned workspace runtime features

## [0.3.0] - 2026-03-09

### Added

- Shared library IPC operations and primary-instance reuse across processes
- External and library-owned diffusers bundle import support, including bundle-aware execution descriptors
- Bundle component previews in import flows and richer model metadata in the frontend
- Partial download staging, resume orchestration, and migration reporting
- Recommended backend hints and canonical reranker model type support

### Changed

- Improved model classification and metadata projection to rely on SQLite-backed library data
- Expanded Hugging Face bundle lookup, hydration, and dependency autobinding flows for imported models

### Fixed

- Release validation and frontend lint/test coverage for packaged builds
- IPC test stability when loopback sockets are restricted by the host environment
- Cross-platform execution path normalization for macOS and Windows CI
- Model type preservation during index rebuilds, partial migration handling, and download recovery

## [0.2.0] - 2026-02-28

### Added

- Core model library with SQLite-backed index and full-text search
- HuggingFace model search, download, and multi-file import pipeline
- In-place import and orphan model recovery at startup
- Download provenance tracking and shard recovery
- TCP IPC (JSON-RPC 2.0, length-prefixed frames) for cross-process library discovery
- Library registry for instance convergence across processes
- Library merger for consolidating duplicate libraries with hash-based dedup
- Model format conversion module with quantization backends (llama.cpp, NVFP4, Sherry)
- Process management for ComfyUI and Torch inference servers
- GPU monitoring via nvidia-smi
- Ollama model management integration
- Torch inference server with Python backend
- App manager with version management for ComfyUI and PyTorch
- JSON-RPC server (`pumas-rpc`) for Electron frontend
- UniFFI bindings for Python, C#, Kotlin, Swift, Ruby, Go
- Rustler NIFs for Elixir/Erlang
- Electron desktop application with React frontend
- Cross-platform support: Linux x86_64, Windows x86_64, macOS ARM
