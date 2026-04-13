# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- No changes yet.

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
