# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/),
and this project adheres to [Semantic Versioning](https://semver.org/).

## [Unreleased]

## [0.8.0] - 2026-11-08

See the [release notes](docs/release-notes-0.8.0.md) for upgrade steps,
downloads and known limitations. This release also includes the fixes from
the 2026-11-02 audit
([remediation plan](docs/plans/audit-remediation-2026-11/plan.md)).

### Added

- Typed generic model operations and capability discovery, including bounded
  Image-to-Text requests and explicit model/projector roles. Availability still
  depends on the selected model, serving profile and qualified runtime.
- Authenticated local owner discovery, explicit existing-library enumeration,
  owner retention and generation fencing for embedding consumers.
- Digest-bound local imports and conditional S3 single-object/authored-bundle
  acquisition, with immutable selection and retained download custody.
- Pinned inference-enabled headless candidate assembly for Linux x86_64,
  Windows x86_64 and macOS ARM64. The release inventory now requires sixteen
  artifacts; assembly and startup do not qualify pretrained inference.
- `GET /v1/served-models` lists every served model and profile with its
  provider, context size and capabilities, so consumers discover models
  instead of configuring profiles by hand.
- `multimodal_embedding` capability: image and mixed text/image embeddings
  for llama.cpp embedding models with a projector, such as EmbeddingGemma 2.
- `decision` capability: typed choice/score/yes-no questions answered with
  probabilities by llama.cpp native decision models such as Kev, OpenJev and
  Laya.
- `rerank` capability and `POST /v1/rerank` gateway route: llama.cpp
  rerankers score documents against a query, most relevant first, with an
  optional `top_n`.
- Modality primitives (ADR 0005): served models and capability descriptors
  report the input and output modalities their runtime declares
  (`modalities`, `input_modalities`, `output_modalities`). Audio parts (PCM,
  sent to llama.cpp as WAV) are accepted in chat messages, `content_batch`
  items and `decision` input wherever the model declares audio, and refused
  with `unsupported_modality` where it does not. Router profiles serve image
  and audio input as well as dedicated ones. Models without a recorded task
  are described by their runtime's own declaration.
- Models are grouped by what they are used for, with new **Decision and
  evaluation** and **Video generation** categories. Each row shows the inputs
  and outputs the model handles and marks LoRA/PEFT adapters. A "Used as"
  choice in the model's metadata window overrides detection and survives
  re-scans (`set_model_category` RPC).
- `audio_transcription` capability: Cohere ASR transcribes speech through the
  Torch sidecar (`POST /api/audio/transcriptions`, advertised as the
  `audio_transcription` sidecar capability). The `cohere-asr` Torch adapter
  resolves current packages with a Transformers 5.4 floor, and `cohere_asr`
  configs are classified as audio models.

### Changed

- `ModelImporter::import` and `import_with_progress` require the lifecycle-owned
  `ModelLibrary` from `PumasApi` or `PumasLibraryInstance`; a directly
  constructed library returns `PumasError::Config` before staging.
- `ModelImporter::discover_shard_recovery[_async]` returns a typed read-only
  report; `recover_incomplete_shards[_async]` is deprecated and never starts
  an inferred download.
- Library merge refuses copied imports whose publication receipt is bound to
  their current library root.
- A model's category is metadata: re-detection never moves a model or changes
  its ID for a category change (only a family change still moves it).
  Detection prefers file facts and architectures over task tags: a GGUF
  decision head makes a decision model, `...ForASR` architectures are audio,
  video tasks are video, and a disagreeing task tag requests review instead of
  overruling. The library's rule tables are updated in place on open.
- Older download metadata requires an explicit offline schema-7 upgrade with
  stopped-reader/writer confirmation and an exact metadata-only backup.
- Hugging Face detail resolution follows explicit model selection; cached
  discovery and live download authority remain distinguishable.
- Typed operation errors always carry a human-readable `message`, echo the
  caller's `request_id` whenever it is readable, and use an HTTP status that
  matches the error code. Provider failure reasons reach the caller with
  credential-shaped text redacted.
- `/v1/model-operations` has one request grammar: `capability`,
  `semantic_task` and `options` are optional selectors, and every request
  accepted before remains valid.
- Responses may gain fields, capabilities and enum values within contract
  version 1; consumers must ignore what they do not use.
- Limits come from the serving runtime: `max_tokens` is bounded by the served
  context, ONNX batch limits apply only to ONNX, and requested embedding
  dimensions are applied by Matryoshka truncation when a provider ignores them.
- `/v1/models` lists each model ID once, with its serving profiles.
- Desktop Torch install choices, saved probe results, startup trials and
  alternative wheels are declared once by the backend and validated from that
  declaration. The saved probe result carries only the checks the desktop shows.
- Managed Torch runtimes no longer install from a fixed package lock. The
  fixed v2.9.1 preset and its embedded lock are replaced by verified
  combinations (`torch-server/runtime/verified-runtimes.json`), which the
  install dialog offers as shortcuts that resolve current package versions; a
  runtime is reported as verified only when every package the combination
  records is installed at exactly the recorded version (an absent recorded
  package fails the match). Verified is a version comparison, not a hash or
  provenance claim. The FLUX.2 and Nunchaku image adapters name their packages without
  version pins, and Nunchaku is a regular adapter choice, offered and accepted
  only for the selection its wheel supports (Torch v2.9.1, cu130, Python
  3.12). Install options list such adapters in `adapterRequirements`. Runtimes installed
  from the preset keep working. Torch install options replace
  `bundledPresetAvailable` and `preset` with `verified`.
- Desktop update streams (model library, downloads, runtime profiles, serving
  status and status telemetry) are validated against the backend's declaration.
  A rejected desktop payload says which field and rule failed.
- Updated Electron to 43.8.1, Vite to 8.3.5, `lucide-react` to 1.67, undici
  to 6.29.1 and 7.30.1, `rusqlite` to 0.41, `rustls` to 0.23.46, `nix` to
  0.31, `windows-sys` to 0.61, `base64` to 0.23 and `rand` to 0.10.
- Rejections carry reasons: typed RPC refusals name the offending field,
  pre-handler HTTP refusals return `{"error":{"code","message"}}`,
  `request_limit` refusals name the bound, gateway and IPC capacity refusals
  say what failed.
- Additive wire fields: `stop_ollama`/`stop_torch` report `skipped` processes
  with reasons; `health_check` echoes `launch_nonce`; Hugging Face search rows
  carry `observedAt` and `cached`; ONNX embedding descriptors advertise the
  model's `input_tokens` limit and image capabilities their decode bounds;
  `preview_torch_runtime` accepts `verified_versions` to install a verified
  combination's recorded versions.
- The desktop decodes credential, import, delete, runtime-profile, serving,
  HF search and status pull traffic with Rust-exported contracts, restarts a
  backend that fails three health checks, and reports update-stream loss.
- The renderer shows load failures, cache age and recovery reasons instead of
  silent defaults, and backs off installation polling on transport failures.
- The `hf-client` and `gpu-monitor` features gate behavior; ONNX work runs off
  the async executor and derives its token limit from the model config.
- Model index records its schema version and refuses unknown ones;
  versions metadata changes under one write lock; review edits whose
  `metadata.json` write was interrupted are replayed.
- `OllamaClient::new` and `VersionManager::version_path` return `Result`.
- Updated `tokio` to 1.54.1 (with `mio`, `socket2`, `tokio-macros`), `zstd`
  to 0.13.4, `zstd-safe` to 7.4.0 and `ctutils` to 0.4.8, clearing
  RUSTSEC-2026-0385, -0386 and -0391 to -0406.
- The generated desktop contract is one shared workspace package
  (`packages/desktop-contract`) instead of identical copies in the Electron
  and frontend trees; the packaged app still bundles it into the preload.

### Removed

- `network::DownloadManager` and `DownloadProgress`, `NetworkManager::execute`
  and `CacheStrategy`'s `allow_stale_on_offline`/`max_stale_age`.
- `OllamaVersionManager`, `ProcessManagerFactory` and the app-manager
  `process` module.
- Dead renderer modules, the superseded 0.7.0 and 0.8.0-rc.1 attribution
  sets, and the flake8 configuration.

### Fixed

- Interrupted Hugging Face downloads resume. Hugging Face's CDN returns the
  signed URL's parameters in a different order on each redirect, which made
  every resume fail with "HTTP continuation representation or resource
  changed".
- Local imports keep the Hugging Face `repo_id` the caller gives them;
  copied imports other than Diffusers bundles had stored `repo_id: null`.
- Vision-language models published as `image-text-to-text` (Gemma, Qwen VL)
  are available for chat and image-to-text instead of reporting a task mismatch.
- A model directory's primary file is never its multimodal projector, which
  had launched EmbeddingGemma 2's projector as the model and broke llama.cpp
  router catalog generation.
- llama.cpp embedding and reranking models launch in embedding/reranking mode
  in dedicated profiles too, with a micro-batch that fits whole inputs: the
  served context when the profile sets one, otherwise 8,192 tokens, never more
  than the model's trained context. Ollama embedding models load through
  `/api/embed`.
- Non-causal llama.cpp decision models such as Laya get the same whole-input
  micro-batch; inputs over 512 tokens were refused.
- Serving a reranker GGUF that has no ranking head (`cls.output.weight`) is
  refused with a reason instead of returning meaningless scores.
- Saved download state that this version cannot read (for example, fields
  written by a newer Pumas) no longer stops the backend. The library, models
  and inference start, downloads are disabled without rewriting that state,
  and the reason is shown in a status banner.
- A backend that cannot start reports why, and the desktop recovery screen
  shows that reason instead of guessing. The startup diagnostic is read across
  builds, ignoring fields it does not know.
- Offer explicit recovery for moved legacy download files in the running desktop,
  preserving files and failed-attempt history while releasing exact download custody.
  First confirmation upgrades the HF partition and warns older direct readers.
- Close read-only desktop update streams without falsely failing instance shutdown.
- Desktop startup failures keep recovery and metadata upgrade actionable,
  including small windows and failed replacement startup attempts.
- Startup preserves retained downloads whose destination folders are missing,
  without recreating folders or finalizing absent outputs.
- Receipt-bound GGUF selection survives owned library moves, and explicit
  headless legacy-schema preflight avoids creating a new failed owner claim.
- Candidate packaging selects notices matching the current source version.
- A failed Torch startup trial reports why the managed launch failed.
- A desktop update that fails validation is no longer dropped silently, which
  had left views stale; the reason is reported and the view reloads.
- Version managers list stable releases only, and of each major.minor only
  its newest patch (`v1.1.3` hides `v1.1.2`; `v1.2.0` stays listed). Installed
  versions are always listed. A tag with a pre-release part (`v0.21.3-rc0`) is
  a pre-release even when GitHub does not flag it, so it no longer counted as
  the newest patch.
- Opening a version manager after its release cache is an hour old shows the
  cached list at once and refreshes it in the background, instead of waiting
  for GitHub. The new-version indicator compares the newest stable release, not
  whichever release GitHub lists first (for llama.cpp, a pre-release build).
- Orphaned Hugging Face downloads resume under their recorded commit; repo
  IDs are validated on every metadata read; a corrupt tree cache is refetched;
  unbounded retry settings are refused.
- Model and package-facts removals commit with their update events; search
  setup is atomic; corrupt rows and search failures surface as errors.
- The legacy JSON writer syncs its directory and backup.
- The Torch sidecar stays responsive during generation and refuses unsafe work
  with reasons; externally referenced models stay loadable by Torch.
- The local HTTP consumer ships the capability-descriptor schema it pins.

### Security

- Third-party telemetry is disabled in every runtime and conversion process
  Pumas starts (`HF_HUB_DISABLE_TELEMETRY=1` and `DO_NOT_TRACK=1`, which cover
  huggingface_hub, Transformers, Diffusers and sentence-transformers, and
  `ORT_DISABLE_TELEMETRY=1` for ONNX Runtime), even when the user's
  environment enables it.
- Runtime and conversion processes run with `HF_HUB_OFFLINE=1`, so Hugging
  Face libraries read only the files Pumas downloaded into the library and
  never fetch missing ones into their own cache. The Sherry conversion script
  now also loads with `local_files_only=True` and without remote code.
- The desktop keeps every renderer on the bundled app: child windows,
  off-app navigation, webviews and browser permissions are denied, links open
  in the system browser, and every IPC handler checks its sender. Packaged
  builds ignore `--dev`, `--debug` and `PUMAS_RPC_BINARY`, ship no DevTools,
  source maps or declarations, and the shipped CSP drops loopback
  `connect-src`.
- The desktop launches the backend on port 0 with a fresh launch nonce and
  accepts only its own nonce-bound readiness; the backend exits when the
  desktop dies (`--exit-on-stdin-eof`).
- Owned runtime children start from an environment allowlist; `LD_PRELOAD`,
  `PYTHONPATH`, credentials and the inherited `LD_LIBRARY_PATH` are no longer
  passed.
- Legacy runtime stops and running checks act only on processes whose PID
  identity record matches the live process; a command-line sweep and bare or
  reused PIDs are never signalled.
- Ollama installs only from publisher-SHA-256-verified archives; Torch
  installs only from a validated per-install hash lock
  (`--require-hashes --no-deps`); dependency installs never use the `python3`
  on PATH; version tags that escape their directory are refused.
- Managed Torch sidecars require a per-backend token and confine model loads
  to the library's roots. The token is sent only to a sidecar this backend
  launched and is currently running; a caller-named Torch `connection_url` or
  any other server never receives it. Loopback provider traffic never uses a
  proxy and provider clients follow no redirects.
- Credential-less `http://` artifact sources on a network host are fetched
  over `https` first and fall back to the plaintext URL, with a logged
  warning, only when the HTTPS attempt gets no HTTP response. Loopback
  sources are unchanged and credentialed plaintext stays refused.
- Public error redaction handles control characters, labelled values, JSON
  pairs and `Basic` credentials. The HF token is written atomically with mode
  0600 and `registry.db` is owner-only on Unix.
- `pumas-rpc` bounds open connections (30 s request head, 256 connections)
  and serves with no plugins instead of loading them from a shared temp
  directory.
- Release builds scan dependencies (`cargo-deny`, `pnpm audit`, `pip-audit`):
  advisory on pull requests, blocking for tags and candidates.
- Managed runtime ZIP archives are read with `zip` 9.4, which fixes
  GHSA advisories for symlink-target exhaustion, AES HMAC bypass and
  dual-header parser differentials. An Ollama ZIP entry whose path would leave
  the install directory now rejects the archive instead of being written
  outside it.
- The development `torch-server/requirements.txt` floors for torch,
  transformers, fastapi and accelerate are raised past known advisories
  (development input only; not shipped). The Pantograph guard interop
  harness, whose vendored dependency lock carried zip and PyO3 advisories,
  is removed; its Pumas-side checks are covered by core tests.

## [0.7.0] - 2026-09-17

See the [0.7.0 release](https://github.com/MrScripty/Pumas-Library/releases/tag/v0.7.0)
for capabilities, upgrade guidance, and scope.

### Added

- Headless build/run paths independent of the desktop, with independent GUI and inference selections.
- Thirteen-file release distribution: full desktop installers, no-inference
  desktop installers, and headless no-inference RPC archives for Linux,
  macOS, and Windows, all assembled and inventoried by CI.
- Model Intent API, immutable upstream revision resolution, and durable consumer model declarations.
- Backend-owned conversion setup, cancellation, recovery, and FP8/NVFP4 Safetensors output.
- In-process ONNX Runtime embeddings and optional Torch image-generation adapters.
- Pre-tag CI rehearsal, independent headless QA, and exact installer inventory checks.

### Changed

- Strengthened explicit library ownership, download mutation exclusion, cancellation, and restart reconciliation.
- Expanded backend-owned runtime and serving state, gateway routes, and event contracts.
- Bumped the package-facts contract to version 2 for structured model inspection data.
- Aligned release versions at 0.7.0; retained the Node 24.15.0 toolchain pin.
- Updated the desktop runtime to Electron 43.7.0, removing the legacy ZIP extractor.
- Replaced obsolete crate/binding release assembly with desktop candidate artifacts matching the artifact plan.

### Fixed

- Improved managed llama.cpp readiness, router reconciliation, context configuration, and sibling vision-projector discovery.
- Hardened Hugging Face acquisition, destination validation, and recovery.
- Updated vulnerable Rust and JavaScript dependencies, including the Electron runtime and packaging tools.
- Repaired conversion and mutation-authority QA fixtures and canonical path handling in recovery fixtures.

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
