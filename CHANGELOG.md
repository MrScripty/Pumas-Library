# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Added

- No changes yet.

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
