# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

### Changed

- Updated the repo Node.js toolchain pin and Node typings to 24.15.0 while opting GitHub JavaScript actions into the Node 24 runner runtime.
- Added a Windows CI RAM disk setup for temporary files in Windows frontend/package jobs.

### Fixed

- Made Windows CI-sensitive runtime tests platform-aware for Ollama binary names and llama.cpp models-directory paths.

## [0.6.0] - 2026-05-06

### Added

- Selected artifact identity across download metadata, persisted package facts, frontend download state, migration dry-run reporting, and contract documentation
- Model-library update notification streams from the core update feed through RPC, Electron preload, and frontend subscriptions
- Backend download snapshot publishing and streamed model-download updates through RPC, Electron, and the React model manager
- Local runtime profile contracts, persistence, update streams, guarded profile operations, managed launch/stop commands, and profile-scoped Ollama model actions
- Managed Ollama and llama.cpp runtime profile support, including launch environment derivation, binary override preparation, provider routing, router catalog generation, presets, and dedicated llama.cpp profile launch flows
- Runtime profile settings and model route editors in the frontend, plus profile subscription hooks and route endpoint resolution
- Fast model selector snapshot contracts, index projections, direct API and read-only model-library access, batch selected-model hydration, and IPC/local-client selector APIs
- Local instance endpoint registration, ready-instance discovery, and local model-library update streaming for explicit local clients
- Status telemetry update streams bridged through Electron and consumed by the frontend

### Changed

- Reworked API ownership so local clients are explicit and UniFFI API bindings are owner-only, removing stale utility, model, runtime, and migration client fallback branches
- Replaced several polling paths with pushed subscriptions for model-library updates, download state, runtime profiles, and status telemetry
- Routed Ollama list, load, and model actions through runtime profiles while preserving default singleton behavior
- Scoped active download indicators, local download rings, recovery state, and full-repository progress identity by selected artifact
- Preserved artifact slug separators and exposed planned migration action kinds for clearer migration review

### Performance

- Cached system resource snapshots and app-liveness reads to reduce repeated status work
- Bounded backend runtime worker threads used by RPC runtime paths
- Lightened telemetry stream update payloads and removed the idle telemetry sampler
- Added selector snapshot timing coverage and local selector snapshot latency measurement

### Fixed

- Artifact migration now honors checkpoint move paths, preserves artifact marker identity, validates mixed and partial artifact directories, blocks unsafe moves with model-id references, remaps references during moves, and reports skipped split partials and orphan payload handoffs
- Model-library integrity updates now refresh stale FTS5 triggers, advance update feeds for producer repair paths, refresh models after migration execution, and emit selector-visible summary updates
- Partial downloads are now separated from complete GGUF variants, routed to incomplete rows for bundle resume state, ignored for duplicate repository warnings, and protected against duplicate partial artifact creation
- Hugging Face GGUF handling now preserves same-quant variants
- Download recovery now preserves artifact scope, keeps full-repository progress identity stable, and aligns the download update notification contract
- RPC now populates in-place import download requests, preserves refresh-model-index responses, and streams model-library updates from the core bus
- Runtime and UI fixes cover release normalization, unsupported profile update stream throttling, Ollama install binary validation, aggregated runtime download status, and refined install version lists
- Stopped model-library watcher access from creating a feedback loop
- CI now satisfies Rust 1.92 verification and current frontend lint rules

### Documentation

- Added and reconciled implementation plans for artifact identity migration, model-library integrity refresh, fast selector snapshots, backend telemetry streams, idle CPU remediation, and local runtime profiles
- Documented selected artifact identity contracts, local runtime profile contracts, explicit client role inventory, selector snapshot fixtures, and updated architecture/API README coverage

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
