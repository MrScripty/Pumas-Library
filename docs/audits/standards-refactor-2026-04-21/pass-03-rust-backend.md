# Pass 03 - Rust Backend, API, Async, Bindings

## Standards Consulted
- `languages/rust/RUST-STANDARDS.md`
- `languages/rust/RUST-API-STANDARDS.md`
- `languages/rust/RUST-ASYNC-STANDARDS.md`
- `languages/rust/RUST-SECURITY-STANDARDS.md`
- `languages/rust/RUST-CROSS-PLATFORM-STANDARDS.md`
- `languages/rust/RUST-DEPENDENCY-STANDARDS.md`
- `languages/rust/RUST-TOOLING-STANDARDS.md`
- `languages/rust/RUST-UNSAFE-STANDARDS.md`
- `languages/rust/RUST-LANGUAGE-BINDINGS-STANDARDS.md`
- `INTEROP-STANDARDS.md`

## Positive Baseline
- Workspace roles are mostly recognizable: `pumas-core`, `pumas-app-manager`, `pumas-rpc`, `pumas-uniffi`, and `pumas-rustler`.
- `pumas-rustler` is excluded from default Cargo tests because it needs the BEAM runtime, matching binding verification guidance.
- Many dependencies are centralized in `[workspace.dependencies]`.
- Platform-specific code is partly centralized under `pumas-core/src/platform/`.
- The RPC server binds to `127.0.0.1` by default.
- There are public crate docs in `pumas-core/src/lib.rs`.

## Findings

### R01 - Core Crate Combines Domain, Infrastructure, Runtime, IPC, and Binding Concerns
Status: architectural non-compliance

`pumas-core` currently includes:

- domain/model library logic;
- SQLite index/cache/registry;
- network clients/downloads;
- process launching and detection;
- IPC client/server protocol;
- launcher updater;
- system/GPU utilities;
- optional UniFFI annotations in core model types.

This is more than a single core/domain role. It makes dependency direction hard to enforce and causes large files to accumulate multiple responsibilities.

Rectification:
- Define crate roles explicitly:
  - `pumas-contracts`: RPC DTOs, persisted schema DTOs, executable schemas where practical;
  - `pumas-core`: pure domain, validated types, state machines;
  - `pumas-infra` or focused modules: SQLite, filesystem, network, process;
  - `pumas-rpc`: composition root plus transport;
  - `pumas-uniffi` and `pumas-rustler`: thin wrappers only.
- If new crates are too disruptive initially, enforce the same roles as module boundaries inside `pumas-core`.

### R02 - Massive Rust Modules Block Reviewability and Ownership
Status: non-compliant

Top decomposition blockers:

```text
8533 rust/crates/pumas-core/src/model_library/library.rs
2107 rust/crates/pumas-core/src/model_library/importer.rs
1710 rust/crates/pumas-core/src/model_library/hf/download.rs
1554 rust/crates/pumas-core/src/index/model_index.rs
1537 rust/crates/pumas-core/src/api/reconciliation.rs
1531 rust/crates/pumas-core/src/model_library/dependencies.rs
1377 rust/crates/pumas-core/src/model_library/model_type_resolver.rs
1348 rust/crates/pumas-core/src/api/hf.rs
1295 rust/crates/pumas-app-manager/src/version_manager/installer.rs
1252 rust/crates/pumas-core/src/api/state.rs
```

`library.rs` is the highest-risk file because it appears to contain production logic, migrations, projections, fixtures, and many tests in one 8,533-line unit.

Rectification order:
- Extract tests and fixture builders from production files where private access is not required.
- Extract validated request/command types before splitting implementation.
- Split model library into ownership modules: catalog/query, metadata persistence, downloads, migration, projection, integrity, dependency resolution, import/recovery.
- Keep public `ModelLibrary` facade stable during extraction.

### R03 - JSON-RPC Uses Stringly Typed Dispatch and `serde_json::Value` Too Deep
Status: non-compliant with parse-once boundary standards

`rust/crates/pumas-rpc/src/handlers/mod.rs` dispatches many string method names and passes `serde_json::Value` into domain handler modules. `rust/crates/pumas-core/src/api/state.rs` also uses `params["field"].as_*()` patterns in IPC dispatch.

This violates:

- parse once at boundary;
- executable boundary contracts;
- prefer enums/newtypes over raw strings for mode/action/state;
- avoid unchecked raw values through internal APIs.

Rectification:
- Introduce typed request structs per method or grouped by domain.
- Parse/validate at the RPC boundary, then call domain/app services with typed commands.
- Generate or test TypeScript and Rust shapes from the same contract artifact.
- Keep JSON-RPC envelope generic, but method payloads typed.

Implementation notes:
- Completed: `pumas-rpc` handler futures now clone owned `VersionManager` values out of the shared
  manager map before awaiting, and the shared size calculator now uses a Tokio mutex in server
  state so release handlers no longer hold async read/write guards across awaited work. This
  restores `pumas-rpc` compilation by satisfying Axum's `Handler` bound for `handle_rpc`.

### R04 - Background Task Ownership Is Incomplete
Status: compliant

Examples:

- `pumas-core/src/api/builder.rs` started recovery tasks with `tokio::spawn` and discarded handles.
- `pumas-rpc/src/server.rs` started Axum serving with `tokio::spawn` and no lifecycle owner.
- `pumas-core/src/ipc/server.rs` has a server handle for the accept loop, but nested connection tasks need review for bounded ownership and shutdown.
- `pumas-core/src/model_library/hf/download.rs` and `conversion/manager.rs` spawn background tasks that need handle/cancellation audit.

Rectification:
- Add a `TaskSupervisor` or `RuntimeTasks` owner with `JoinSet` or `TaskTracker`.
- Store every spawned handle, propagate cancellation, await/abort during shutdown.
- Convert server startup to return a handle with explicit shutdown.
- Add tests for shutdown idempotency and task panic handling.

Implementation notes:
- Completed: `pumas-rpc/src/server.rs` returns an owned `ServerHandle`, logs server task errors, and aborts the task during explicit or drop-based shutdown.
- Completed: `pumas-core/src/api/runtime_tasks.rs` owns builder-started background task handles and aborts them during `PumasApi` shutdown.
- Completed: `PumasApiBuilder` routes initial connectivity checks, orphan adoption, download completion callbacks, and startup download/shard recovery tasks through `RuntimeTasks`.
- Completed: `ConversionManager` stores conversion/quantization task handles by conversion ID,
  prunes finished handles, and aborts tracked tasks during explicit cancellation or manager
  shutdown.
- Completed: `pumas-core/src/ipc/server.rs` tracks nested connection task handles, prunes finished
  handles when new connections arrive, and aborts remaining connection tasks when the server handle
  is dropped.
- Completed: `pumas-core/src/model_library/hf/download.rs` tracks download task handles by
  download ID, aborts tracked tasks during explicit cancellation and client drop, and covers the
  cancel path with a focused task-ownership test.
- Completed: `pumas-core/src/network/manager.rs` stores the connectivity monitoring task handle and
  aborts it during explicit stop or manager drop.
- Completed: `pumas-core/src/api/reconciliation.rs` routes watcher-triggered and scheduled
  reconciliation tasks through `PrimaryState`-owned `RuntimeTasks`, and `RuntimeTasks` now prunes
  finished handles before tracking new work so repeated reconcile bursts do not accumulate stale
  join handles.
- Completed: `pumas-core/src/api/runtime_tasks.rs` now captures a Tokio runtime handle at
  construction and uses that handle for later spawns, so download callbacks and other non-runtime
  threads can enqueue owned background tasks without panicking during API startup and runtime task
  supervision.

### R05 - Blocking Work in Async Paths Needs Audit
Status: partially compliant

Positive evidence: some blocking process operations are wrapped in `spawn_blocking`.

Risks:

- synchronous filesystem and process work appears in async API flows;
- `std::thread::sleep` appears in production modules such as process launching, registry tests/helpers, and model library wait paths;
- `.wait()` on child processes appears in conversion paths.

Rectification:
- Classify each blocking operation as production async path, sync service path, test, or explicit background worker.
- Replace blocking calls in request/lifecycle paths with async equivalents or `spawn_blocking`.
- Add clippy `await_holding_lock`, `blocking_in_async` review where available, or targeted custom checks.

Implementation notes:
- Completed: `scripts/rust/check.sh blocking-audit` prints blocking-work candidates across
  `pumas-core`, `pumas-app-manager`, and `pumas-rpc` source roots for classification.
- Completed: `pumas-core/src/api/migration.rs` now uses `tokio::fs` for partial-download
  relocation marker reads/writes, directory creation/removal, and rename operations so the
  checkpointed migration execute path no longer performs those filesystem calls directly on the
  async request path.
- Completed: `pumas-core/src/api/reconciliation.rs` now uses `tokio::fs` for partial-download
  marker reads and async existence checks in reconciliation staging/model-scope flows so watcher
  and on-demand reconcile paths no longer rely on synchronous metadata probes for those checks.
- Completed: `pumas-core/src/api/builder.rs`, `api/mapping.rs`, and `api/state.rs` now use
  `tokio::fs` for launcher-root/model-mapping directory existence checks and creation so startup
  and mapping apply/sync request paths do not perform those directory operations synchronously on
  async runtime threads.
- Completed: `pumas-core/src/api/process.rs`, `api/state_process.rs`, and
  `pumas-app-manager/src/version_manager/dependencies.rs` now use `tokio::fs` for async
  version/venv/requirements path probes so launch and dependency-management entry points no longer
  perform synchronous existence checks before handing work to blocking launch/install helpers.
- Completed: `pumas-app-manager/src/version_manager/dependencies.rs` now reads
  `requirements.txt` with `tokio::fs` and uses async venv checks in `install_dependencies`, so
  the dependency status/install flows no longer mix synchronous requirement-file reads into those
  async entry points.
- Completed: `pumas-core/src/api/links.rs` and the mirrored link-health/cleanup IPC dispatch in
  `api/state.rs` now use async existence/symlink checks and async file removal so those link
  registry health/cleanup paths no longer perform synchronous metadata probes or unlinks on async
  runtime threads.
- Completed: `pumas-core/src/api/models.rs` and the mirrored model metadata/mapping-preview IPC
  helpers in `api/state.rs` now use async model-directory and mapping-path existence checks so
  inference-settings, notes, and mapping-preview entry points no longer perform synchronous path
  probes on async runtime threads.
- Completed: `pumas-app-manager/src/version_manager/launcher.rs` now uses `tokio::fs` for async
  version/venv/main-script/binary/pid path probes, PID-file reads, log-directory creation, and
  PID cleanup so the launcher status and stop flows no longer perform those filesystem operations
  synchronously on async runtime threads before delegating into process control.
- Completed: `pumas-app-manager/src/version_manager/mod.rs` now uses `tokio::fs` for async
  launcher-root validation and installed-version directory removal so the async constructor and
  remove flow no longer perform synchronous existence checks or recursive deletion on runtime
  threads.
- Completed: `pumas-app-manager/src/version_manager/ollama.rs` now uses `tokio::fs` for async
  version-directory creation during install and recursive directory removal during uninstall so
  those async Ollama lifecycle entry points no longer perform those filesystem operations directly
  on runtime threads.
- Completed: `pumas-app-manager/src/version_manager/constraints.rs` now uses `tokio::fs` for the
  async constraints build/write path, including async constraints-cache persistence, so constraint
  generation no longer performs direct directory creation or file writes on runtime threads.
- Completed: `pumas-app-manager/src/version_manager/size_calculator.rs` now uses `tokio::fs` for
  async release-size cache persistence so size calculation no longer writes its cache file through
  blocking filesystem calls on runtime threads.
- Completed: `pumas-app-manager/src/version_manager/size_calculator.rs` now provides an async
  cache-loading constructor, and `pumas-rpc` startup uses it, so RPC bootstrap no longer performs
  synchronous release-size cache reads on async runtime threads.
- Completed: `pumas-core/src/network/github.rs` now uses `tokio::fs` for async release-cache disk
  reads and writes in the GitHub release fetch path so cache-backed network entry points no longer
  perform blocking cache I/O on runtime threads before or after fetches.
- Completed: `pumas-app-manager/src/version_manager/state.rs` now uses `tokio::fs` for async
  active-version file reads/writes/removal and async version-directory scans so version-state
  initialization, validation, activation, and uninstall refresh paths no longer perform that
  filesystem work synchronously on runtime threads.
- Completed: `pumas-app-manager/src/version_manager/launcher.rs` now uses `tokio::fs::File` and
  async PID-file writes in the launch path so ComfyUI and Ollama startup no longer create launch
  logs or persist launch PIDs with blocking filesystem calls on runtime threads.
- Completed: `pumas-app-manager/src/version_manager/installer.rs` now uses `tokio::fs` for log,
  temp, cache-download, version, and extract-directory creation, async cache/archive cleanup,
  async cached-download metadata checks, async archive writes, and async requirements/venv path
  probes so the installer request path no longer performs those filesystem operations directly on
  runtime threads before extraction and finalize steps.
- Completed: `pumas-app-manager/src/version_manager/installer.rs` now uses `tokio::fs` for async
  extract-directory scans, versions-root creation, existing-version removal, rename fallback, and
  recursive copy operations in `move_to_final_location`, so the async install handoff from extract
  to final version placement no longer performs that directory work through blocking std fs calls.
- Completed: `pumas-app-manager/src/version_manager/installer.rs` now runs archive extraction and
  Ollama binary finalization through `tokio::task::spawn_blocking`, so zip, tar.gz, tar.zst, and
  binary-permission work no longer execute directly on async runtime threads during install flows.
- Completed: `pumas-app-manager/src/version_manager/constraints.rs` now loads cached constraints
  and materializes cached constraint files through async `tokio::fs` helpers in the dependency
  install path, so `ConstraintsManager` construction and cached-file reuse no longer perform those
  reads and writes synchronously on runtime threads.
- Completed: `pumas-app-manager/src/version_manager/progress.rs` now uses async stale-state
  cleanup during manager initialization and async completed-state file removal in the delayed
  cleanup task, so those installation-progress lifecycle edges no longer perform synchronous
  progress-file reads or removals on runtime threads.
- Completed: `pumas-app-manager/src/version_manager/dependencies.rs` now creates the pip cache
  directory with `tokio::fs` inside `install_with_progress`, so that dependency-install request
  path no longer performs that cache-directory creation through blocking std fs calls.
- Completed: `pumas-app-manager/src/custom_nodes/mod.rs` now uses `tokio::fs` for async custom
  node install/update path existence checks, requirements detection, and `custom_nodes`
  directory creation so those async lifecycle entry points no longer perform synchronous metadata
  probes or directory setup on runtime threads before invoking git operations.
- Completed: `pumas-app-manager/src/process/factory.rs` now uses `tokio::fs` for async
  version/log/pid path checks, async log reads, async pid-file removal, and async log-directory
  creation so binary and Python process manager lifecycle methods no longer perform synchronous
  filesystem probes or file reads on runtime threads.
- Completed: `pumas-app-manager/src/version_manager/ollama.rs` now uses async file creation and
  writes for streamed downloads plus async archive cleanup and direct-binary rename, so those
  async install steps no longer perform synchronous file output or archive relocation on runtime
  threads before extraction/finalization.
- Completed: `pumas-app-manager/src/version_manager/ollama.rs` now routes archive extraction and
  binary finalization through `tokio::task::spawn_blocking`, so zip, tarball, recursive binary
  discovery, and permission-fix work no longer execute inline on async runtime threads during
  Ollama install flows.
- Completed: `pumas-app-manager/src/version_manager/progress.rs` now snapshots state and dispatches
  progress persistence through the current Tokio runtime when available, falling back to blocking
  writes only outside async contexts, so installer progress updates no longer write state files
  inline on async runtime threads.
- Completed: `pumas-app-manager/src/version_manager/state.rs` now uses async binary-path probing in
  Ollama installation validation so version-state refresh no longer performs that completeness
  check with a synchronous metadata probe on async runtime threads.
- Completed: `pumas-rpc/src/handlers/shared.rs`, `handlers/versions/deps.rs`, and
  `handlers/process.rs` now use shared `tokio::fs` helpers for async requirements-file reads and
  install-directory existence checks, so those RPC handler request paths no longer perform
  synchronous file reads or metadata probes on async runtime threads before dispatching UI-facing
  responses.
- Completed: `pumas-rpc/src/handlers/links.rs` now uses async existence and metadata probes for
  hard-link counting and writable-path checks, so those RPC link-management request paths no longer
  perform synchronous filesystem metadata reads on async runtime threads when inspecting files the
  UI wants to link or validate.
- Completed: `pumas-app-manager/src/custom_nodes/mod.rs` and
  `pumas-rpc/src/handlers/custom_nodes.rs` now list and remove custom nodes through async
  directory scans, async metadata probes, async git command execution, and async directory removal,
  so those custom-node RPC request paths no longer perform blocking directory walks or git/process
  inspection on async runtime threads.
- Completed: `pumas-rpc/src/shortcut/manager.rs`, `handlers/shortcuts.rs`, and
  `handlers/status.rs` now use async shortcut-state probes for menu and desktop entries, so RPC
  shortcut state and status request paths no longer perform synchronous existence checks on async
  runtime threads when reporting UI shortcut state.
- Completed: `pumas-rpc/src/handlers/shortcuts.rs` now clones shortcut manager state out of the
  shared lock before awaiting async probes, and routes shortcut toggle mutations through
  `tokio::task::spawn_blocking`, so shortcut create/remove request paths no longer execute their
  synchronous filesystem and script/icon generation work inline on async runtime threads.
- Completed: `pumas-core/src/network/download.rs` now uses `tokio::fs` and async file writes for
  destination directory setup, temp-file creation, streamed chunk writes, flush, atomic rename, and
  temp-file cleanup, so core download request paths no longer perform blocking filesystem work on
  async runtime threads while transferring or finalizing downloaded files.
- Completed: `pumas-core/src/launcher/updater.rs` now uses async cache reads and writes for
  launcher update checks, so the async GitHub release polling path no longer performs synchronous
  cache-file reads, directory creation, or cache writes on runtime threads before returning cached
  or fresh launcher update metadata.
- Completed: `pumas-core/src/api/process.rs`, `api/system.rs`, `api/state_process.rs`, and
  `api/state_runtime.rs` now route process detection and resource aggregation through
  `tokio::task::spawn_blocking`, so direct API calls and mirrored IPC status/process request paths
  no longer execute synchronous process scans, PID-file inspection, sysinfo refreshes, or GPU
  resource queries inline on async runtime threads.
- Completed: `pumas-core/src/api/system.rs`, `api/state_runtime.rs`, and `api/state.rs` now route
  disk-space enumeration through `tokio::task::spawn_blocking`, so direct API calls and mirrored
  IPC disk-space request paths no longer run synchronous `sysinfo::Disks` refresh/enumeration
  inline on async runtime threads.
- Completed: `pumas-rpc/src/handlers/shared.rs` and `handlers/models/imports.rs` now load
  safetensors embedded metadata through async file reads, so RPC model import and metadata request
  paths no longer perform synchronous header reads inline on async runtime threads.
- Completed: `pumas-core/src/api/models.rs` and `pumas-rpc/src/handlers/models/imports.rs` now
  route model file-type detection through `tokio::task::spawn_blocking`, so RPC file validation no
  longer performs synchronous path probes and model header inspection inline on async runtime
  threads.
- Completed: `pumas-core/src/api/system.rs` and `pumas-rpc/src/handlers/process.rs` now route
  open-path, open-url, and open-directory requests through `tokio::task::spawn_blocking`, so RPC
  system utility requests no longer run synchronous path validation or platform shell launches
  inline on async runtime threads.
- Completed: `pumas-core/src/api/system.rs` now probes `open_directory` targets with `tokio::fs`
  before entering the blocking shell-launch closure, so the active-install open request surface no
  longer mixes directory existence checks into its blocking launcher handoff.
- Completed: `pumas-core/src/network/github.rs`, `pumas-app-manager/src/version_manager/mod.rs`,
  and `pumas-rpc/src/handlers/versions/release.rs` now load GitHub release cache status through
  async disk reads, so the RPC cache-status request path no longer performs synchronous cache-file
  inspection inline on async runtime threads.
- Completed: `pumas-core/src/api/system.rs`, `api/state.rs`, and `pumas-rpc/src/handlers/status.rs`
  now route launcher-version requests through `tokio::task::spawn_blocking`, so direct API, mirrored
  IPC, and RPC launcher-version checks no longer run synchronous git/path inspection inline on
  async runtime threads.
- Completed: `pumas-core/src/api/system.rs`, `pumas-rpc/src/handlers/status.rs`, and
  `handlers/versions/patch.rs` now route patch-status and patch-toggle requests through
  `tokio::task::spawn_blocking`, so RPC status polling and patch management no longer run
  synchronous `main.py` inspection, backup writes, or git/curl fallback work inline on async
  runtime threads.
- Completed: `pumas-core/src/api/system.rs` and `pumas-rpc/src/handlers/status.rs` now route
  git, Brave, and setproctitle system-check requests through `tokio::task::spawn_blocking`, so
  RPC system-check endpoints no longer run synchronous command execution and path probes inline on
  async runtime threads.
- Completed: `pumas-core/src/api/system.rs` and `pumas-rpc/src/handlers/status.rs` now route the
  launcher restart request through `tokio::task::spawn_blocking`, so RPC restart requests no longer
  run synchronous launcher-script existence checks and process spawning inline on async runtime
  threads.
- Completed: `pumas-rpc/src/handlers/shared.rs`, `handlers/status.rs`, and `handlers/mod.rs` now
  load sandbox status through async path probes, so the RPC sandbox-info request path no longer
  performs synchronous filesystem checks inline on async runtime threads.
- Completed: `pumas-core/src/launcher/updater.rs` now gathers update-check git metadata through a
  blocking task boundary, so launcher update polling no longer performs synchronous repository/path
  inspection inline on async runtime threads before the async GitHub/cache flow begins.
- Completed: `pumas-core/src/launcher/updater.rs` now dispatches the launcher apply-update
  workflow through `tokio::task::spawn_blocking`, so git pull, pip install, pnpm build, and
  rollback subprocess orchestration no longer execute inline on async runtime threads.
- Completed: `pumas-core/src/model_library/hf/mod.rs` now dispatches HuggingFace token save/clear
  persistence through `tokio::task::spawn_blocking`, so RPC auth-token set/clear requests no
  longer perform synchronous config-file writes inline on async runtime threads.
- Completed: `pumas-core/src/model_library/hf/download.rs` now uses `tokio::fs` for destination
  directory creation, download marker writes, and completed-file existence probes in the async
  download start and run paths, so HuggingFace download lifecycle requests no longer perform those
  filesystem operations inline on runtime threads.
- Completed: `pumas-core/src/model_library/hf/download.rs` now routes persisted-download restore
  scanning through `tokio::task::spawn_blocking`, so startup recovery no longer performs
  persistence loads, stale-entry cleanup, or on-disk byte counting inline on async runtime
  threads before restoring in-memory download state.
- Completed: `pumas-core/src/api/hf.rs` and `api/state_hf.rs` now validate partial-download
  destination directories with `tokio::fs::metadata`, so direct API and mirrored IPC recovery
  requests no longer perform synchronous destination `is_dir` probes inline on async runtime
  threads before deciding whether to recover or resume a partial download.
- Completed: `pumas-core/src/api/migration.rs` and `api/state.rs` now route migration report
  generation, rewrite, listing, deletion, and pruning through `tokio::task::spawn_blocking`, so
  direct API calls and mirrored IPC migration-report requests no longer perform synchronous report
  artifact I/O or index maintenance inline on async runtime threads.
- Completed: `pumas-core/src/api/models.rs` and `api/state.rs` now route library model-count reads
  through `tokio::task::spawn_blocking`, so rebuild-index and library-status request paths no
  longer perform synchronous SQLite count queries inline on async runtime threads.
- Completed: `pumas-core/src/model_library/library.rs` and
  `model_library/dependencies.rs` now route review-queue scans and dependency pin-compliance
  audits through `tokio::task::spawn_blocking` after the async model listing step, so those
  request-facing review and governance endpoints no longer perform per-model effective-metadata
  loads and dependency-binding index scans inline on async runtime threads.
- Completed: `pumas-core/src/api/builder.rs` now initializes the HuggingFace search cache and
  `HuggingFaceClient` through `tokio::task::spawn_blocking`, so API startup no longer performs
  synchronous cache-database setup, HF cache directory creation, or token-file resolution inline
  on async runtime threads before attaching persistence and restoring downloads.
- Completed: `pumas-core/src/api/mapping.rs` and `pumas-rpc/src/handlers/links.rs` now route the
  cross-filesystem warning request through `tokio::task::spawn_blocking`, so RPC link-warning
  checks no longer perform synchronous filesystem metadata inspection inline on async runtime
  threads.
- Completed: `pumas-core/src/api/models.rs` and `pumas-rpc/src/handlers/models/imports.rs` now
  route import-path classification through `tokio::task::spawn_blocking`, so RPC import-path
  analysis no longer performs synchronous directory walking and model/header inspection inline on
  async runtime threads.
- Completed: `pumas-core/src/api/hf.rs`, `api/state.rs`, and
  `pumas-rpc/src/handlers/models/downloads.rs` now route interrupted-download discovery through
  `tokio::task::spawn_blocking`, so RPC and mirrored IPC download-recovery listing no longer
  perform synchronous persistence reads and library tree scans inline on async runtime threads.
- Completed: `pumas-core/src/model_library/importer/recovery.rs` now routes orphan-directory
  discovery through `tokio::task::spawn_blocking` before the async adoption loop begins, so the
  orphan-adoption request path no longer performs its initial library tree scan inline on async
  runtime threads.
- Completed: `pumas-rpc/src/handlers/models/imports.rs` now routes GGUF embedded-metadata
  extraction through `tokio::task::spawn_blocking`, so RPC metadata inspection no longer performs
  synchronous GGUF file reads inline on async runtime threads.
- Completed: `pumas-core/src/api/hf.rs` and `api/state_hf.rs` now route diffusers bundle lookup
  hint extraction through `tokio::task::spawn_blocking`, so direct API and mirrored IPC metadata
  lookup no longer perform synchronous `model_index.json` reads inline on async runtime threads.
- Completed: `pumas-core/src/api/hf.rs` and `api/state_hf.rs` now route HuggingFace metadata
  refresh snapshot loads through `tokio::task::spawn_blocking`, so direct API and mirrored IPC
  metadata refresh requests no longer perform synchronous `metadata.json` reads or primary-file
  discovery inline on async runtime threads before or after the network lookup.
- Completed: `pumas-rpc/src/handlers/models/imports.rs` now routes library metadata snapshot reads
  and diffusers component-manifest extraction through `tokio::task::spawn_blocking`, so the RPC
  model-details request path no longer performs synchronous metadata reads, primary-file discovery,
  or bundle manifest scans inline on async runtime threads.
- Completed: `pumas-core/src/api/models.rs` and `api/state.rs` now route inference-settings,
  model-notes, and effective-metadata loads through `tokio::task::spawn_blocking`, so direct API
  calls and mirrored IPC metadata request paths no longer perform synchronous `metadata.json`
  reads or primary-file discovery inline on async runtime threads before resolving or persisting
  model metadata updates.
- Completed: `pumas-rpc/src/handlers/ollama.rs` now routes primary model-file discovery through
  `tokio::task::spawn_blocking`, so the Ollama create-model request path no longer performs
  synchronous library file discovery inline on async runtime threads before validating GGUF input.
- Completed: `pumas-rpc/src/handlers/models/imports.rs` now routes shard-set detection through
  `tokio::task::spawn_blocking`, so the RPC shard-analysis request path no longer performs
  synchronous shard grouping inline on async runtime threads.
- Completed: `pumas-core/src/model_library/hf/metadata.rs` now routes local fast-hash computation
  through `tokio::task::spawn_blocking`, so HuggingFace file metadata lookup no longer performs
  synchronous file hashing inline on async runtime threads before candidate verification.
- Completed: `pumas-core/src/model_library/importer.rs` now routes external diffusers bundle
  validation through `tokio::task::spawn_blocking` and uses async target-directory existence and
  creation checks, so the external diffusers import request path no longer performs synchronous
  bundle validation or directory setup inline on async runtime threads.
- Completed: `pumas-core/src/model_library/importer.rs` now routes in-place diffusers bundle
  validation through `tokio::task::spawn_blocking`, so download finalization and orphan-adoption
  import paths no longer perform synchronous bundle validation inline on async runtime threads
  before deciding between bundle and file-model import flows.
- Completed: `pumas-core/src/model_library/importer.rs` now routes in-place primary model-file
  discovery through `tokio::task::spawn_blocking`, so download finalization and orphan-adoption
  import paths no longer perform synchronous directory walks inline on async runtime threads when
  selecting the canonical file for type detection and hashing.
- Completed: `pumas-core/src/model_library/importer.rs` now routes in-place primary file-type
  detection through `tokio::task::spawn_blocking`, so download finalization and orphan-adoption
  import paths no longer perform synchronous file header inspection inline on async runtime threads
  after selecting the canonical model file.
- Completed: `pumas-core/src/model_library/importer.rs` now routes in-place file enumeration
  through `tokio::task::spawn_blocking`, so download finalization and orphan-adoption import paths
  no longer perform synchronous directory walks inline on async runtime threads when collecting the
  imported file manifest for metadata and shard validation.
- Completed: `pumas-core/src/model_library/importer.rs` now routes in-place dLLM subtype
  detection through `tokio::task::spawn_blocking`, so download finalization and orphan-adoption
  import paths no longer perform synchronous `config.json` reads inline on async runtime threads
  when refining LLM subtype classification.
- Completed: `pumas-core/src/model_library/importer.rs` now routes in-place primary-file hashing
  through `tokio::task::spawn_blocking`, so download finalization and orphan-adoption import paths
  no longer perform synchronous dual-hash computation inline on async runtime threads when
  `compute_hashes` is enabled.
- Completed: `pumas-core/src/model_library/importer.rs` now routes legacy import-path and temp
  import hashing through `tokio::task::spawn_blocking`, so file-copy import flows no longer
  perform synchronous dual-hash computation inline on async runtime threads while finalizing copied
  model directories.
- Completed: `pumas-core/src/model_library/importer.rs` now routes copy/import type detection
  through `tokio::task::spawn_blocking`, so the primary import and progress-reporting import flows
  no longer perform synchronous file or directory inspection inline on async runtime threads before
  security checks and model-type routing.
- Completed: `pumas-core/src/model_library/importer.rs` now uses async directory creation and
  rename for the progress-reporting import finalize handoff, so that temp-to-final model placement
  no longer performs blocking std fs calls inline on async runtime threads.
- Completed: `pumas-core/src/model_library/importer.rs` now uses async source/target path probes in
  `import` and `import_with_progress`, and routes top-level diffusers bundle validation in
  `import` through `tokio::task::spawn_blocking`, so those import entry points no longer perform
  synchronous existence checks or bundle validation inline on async runtime threads before routing
  into bundle or file-copy flows.
- Completed: `pumas-core/src/model_library/importer.rs` now uses async temp-directory creation for
  atomic imports, so import, progress-reporting import, and copied diffusers import flows no
  longer perform blocking temp-root setup inline on async runtime threads before copying files.
- Completed: `pumas-core/src/model_library/importer.rs` now uses async cleanup, directory
  creation, and rename for copied diffusers finalize handoff, so copied bundle import flows no
  longer perform blocking std fs cleanup or temp-to-final placement inline on async runtime
  threads.
- Completed: `pumas-core/src/model_library/importer.rs` now routes file-copy traversal through
  `tokio::task::spawn_blocking` in copied diffusers import, temp import, and progress-reporting
  import flows, so those async importer paths no longer perform synchronous recursive copy work
  inline on async runtime threads.
- Completed: `pumas-core/src/model_library/importer.rs` now routes copied diffusers expected-file
  collection through `tokio::task::spawn_blocking`, so that bundle import follow-up no longer
  performs a synchronous directory walk inline on async runtime threads when preparing the in-place
  import spec.
- Completed: `pumas-core/src/model_library/importer.rs` now uses async cleanup, directory
  creation, and rename for the plain import finalize path, so file-copy import failure cleanup and
  temp-to-final placement no longer perform blocking std fs calls inline on async runtime threads.
- Completed: `pumas-core/src/model_library/importer.rs` now routes progress-import and temp-import
  primary model-file selection through `tokio::task::spawn_blocking`, so those async importer
  paths no longer perform synchronous directory walks inline on async runtime threads before hash
  computation.
- Completed: `pumas-core/src/model_library/library.rs` now routes external asset revalidation and
  execution-descriptor primary-file discovery through `tokio::task::spawn_blocking`, and uses an
  async existence probe in `resolve_model_execution_descriptor`, so model execution resolution no
  longer performs synchronous bundle revalidation, directory walks, or path existence checks inline
  on async runtime threads.
- Completed: `pumas-core/src/model_library/library.rs` now routes redetect/reclassify primary-file
  inspection and dLLM subtype detection through `tokio::task::spawn_blocking`, and uses async
  existence probes for those entry points, so model reclassification paths no longer perform
  synchronous directory walks, file header inspection, `config.json` reads, or path existence
  checks inline on async runtime threads.
- Completed: `pumas-core/src/conversion/pipeline.rs` now uses `tokio::fs` for async output-dir
  collision probes and temp-to-final rename handoff, and the async conversion flows in
  `conversion/manager.rs`, `conversion/llama_cpp.rs`, `conversion/nvfp4.rs`, and
  `conversion/sherry.rs` now await that helper, so conversion finalize paths no longer perform
  synchronous existence checks or rename operations inline on async runtime threads.
- Completed: `pumas-core/src/conversion/pipeline.rs` now provides async temp-dir prepare and
  cleanup helpers, and the async conversion flows in `conversion/manager.rs`,
  `conversion/llama_cpp.rs`, `conversion/nvfp4.rs`, and `conversion/sherry.rs` now use them, so
  stale-temp cleanup and temp-dir creation no longer perform synchronous remove/create operations
  inline on async runtime threads.
- Completed: `pumas-core/src/conversion/manager.rs` and `conversion/llama_cpp.rs` now use
  `tokio::fs` for conversion-environment/config probes plus intermediate-output cleanup, so those
  async conversion and llama.cpp quantization flows no longer perform synchronous existence checks
  or file removals inline on async runtime threads.
- Completed: `pumas-core/src/conversion/pipeline.rs` now provides async extension-based file
  discovery, and `conversion/manager.rs`, `conversion/llama_cpp.rs`, `conversion/nvfp4.rs`, and
  `conversion/sherry.rs` now use that helper, so conversion source enumeration and quantization
  input validation no longer perform synchronous directory walks inline on async runtime threads.
- Completed: `pumas-core/src/conversion/scripts.rs`, `conversion/manager.rs`,
  `conversion/llama_cpp.rs`, `conversion/nvfp4.rs`, and `conversion/sherry.rs` now use
  `tokio::fs` for script deployment plus backend environment path probes and setup directory/file
  creation, so async conversion-environment setup no longer performs those filesystem operations
  inline on runtime threads.
- Remaining: classify the current audit output and replace blocking work in confirmed async
  request/lifecycle paths with async equivalents or `spawn_blocking`.

### R06 - Unsafe Rust Is Not Governed by Workspace Policy
Status: compliant

Unsafe usages exist in:

- `pumas-core/src/process/manager.rs`
- `pumas-core/src/process/launcher.rs`
- `pumas-core/src/conversion/manager.rs`
- `pumas-core/src/metadata/atomic.rs`
- `pumas-core/src/model_library/library/projection.rs`
- `pumas-core/src/platform/process.rs`
- tests manipulating process-global state

The workspace now denies direct unsafe by default and requires intentional boundary modules to opt
down locally while documenting every unsafe block.

Rectification:
- Add workspace lint policy with `unsafe_code = "deny"` by default.
- Move OS/FFI unsafe into thin modules that explicitly relax to `warn`.
- Add `SAFETY:` comments to every unsafe block.
- Add Miri/sanitizer plan for pure unsafe and OS FFI wrappers where practical.

Implementation notes:
- Completed: inherited `unsafe_code = "deny"` and `unsafe_op_in_unsafe_fn = "deny"` across
  workspace crates.
- Completed: `platform::process`, `platform::paths`, and `metadata::atomic` explicitly opt down to
  `warn(unsafe_code)` as the current intentional unsafe boundary modules.
- Completed: guarded API integration-test environment mutation opts down only on the serialized
  registry override helper.
- Completed: documented current platform process probes, metadata fsync, and Windows long-path FFI
  with explicit `SAFETY:` comments.
- Completed: isolated launcher process detachment behind `platform::configure_detached_command` so
  launcher flows no longer own direct `pre_exec` unsafe blocks.
- Completed: isolated Windows display-path long-path expansion behind
  `platform::platform_display_path` so model library projection no longer owns Windows FFI.
- Completed: replaced a direct `libc::kill(pid, 0)` call in process resource aggregation with the
  centralized `platform::is_process_alive` wrapper.
- Completed: Unix metadata writes now return fsync failures instead of ignoring them before rename.
- Completed: replaced conversion-manager raw pointer lifetime bridges with owned `Arc` handles for
  progress tracking and quantization backends.
- Completed: Miri/sanitizer coverage decision recorded: current unsafe is OS FFI rather than pure
  memory manipulation, so focused platform/metadata tests cover the present surface; future pure
  unsafe must add a Miri or sanitizer target before relaxing the lint.

### R07 - Path Validation Is Not Centralized Around Validated Types
Status: partially compliant

The codebase has many path-oriented APIs and some canonicalization utilities, but external path entrypoints still pass raw `PathBuf`/`String` through multiple layers:

- `pumas-rpc/src/handlers/models/imports.rs` converts incoming file strings to `PathBuf`;
- `pumas-uniffi/src/bindings.rs` exposes string paths;
- Electron dialog/open-path flows pass selected filesystem paths across IPC;
- library root and model paths are used by many services.

Rectification:
- Add validated path newtypes such as `LauncherRoot`, `LibraryRoot`, `ModelPath`, `ExternalImportPath`, and `SafeOpenPath`.
- Canonicalize and validate at ingress.
- Keep raw strings only at FFI/IPC serialization boundaries.
- Add canonical identity tests for symlinked roots, spaces in paths, and platform-managed temp aliases.

### R08 - Network Listener Policy Needs Explicit Enforcement
Status: partially compliant

Positive:

- RPC default host is `127.0.0.1`.
- IPC server uses `127.0.0.1:0`.
- RPC server CORS is restricted to loopback browser origins and `GET`/`POST` with
  `Content-Type`.

Risks:

- `pumas-rpc` now requires explicit `--allow-lan` opt-in for non-loopback `--host`, but the LAN
  mode threat model still needs explicit product documentation and review.
- Torch LAN access can set host to `0.0.0.0`; this may be product-intended but needs documented security policy and authentication/authorization review.
- RPC request concurrency is now capped in transport, but broader listener/backpressure policy for
  Torch LAN mode still needs explicit product documentation and review.

Rectification:
- Validate host binding policy at CLI/config boundary.
- Restrict CORS to renderer/dev origins where possible.
- Add max connection limits or request concurrency limits.
- Document LAN mode threat model for Torch server.

Implementation notes:
- Completed: `pumas-rpc/src/main.rs` now validates `--host` as an IP address and rejects
  non-loopback binds unless `--allow-lan` is passed, so the CLI boundary enforces the local-only
  RPC default instead of accepting arbitrary listener addresses.
- Completed: `pumas-rpc/README.md` now documents the loopback default plus explicit LAN opt-in for
  the RPC binary.
- Completed: `pumas-rpc/src/server.rs` now applies a transport-level concurrency cap of 64
  in-flight HTTP requests, and the crate README records that limit as part of the RPC trust and
  availability policy.
- Completed: `pumas-app-manager/src/torch_client.rs` now validates `TorchServerConfig` so
  non-loopback bind hosts require explicit `lan_access`, enforcing the local-only default at the
  Torch configuration boundary before LAN-exposed settings are sent to the inference server.

### R09 - Language Binding Boundary Is Too Entangled With Core Types
Status: partially compliant

`pumas-core` uses `#[cfg_attr(feature = "uniffi", derive(...))]` on many model types. Standards allow core annotations for simple FFI-safe types, but the scope is broad and needs policy review. `pumas-uniffi/src/bindings.rs` is 1,891 lines and likely owns too much generated/wrapper surface in one file.

Rectification:
- Classify binding surface as `supported`, `experimental`, or `internal-only`.
- Move non-FFI-safe transformations into `Ffi*` wrapper modules.
- Split `pumas-uniffi/src/bindings.rs` by domain.
- Keep generated C# artifacts out of hand-edited paths and verify package/native compatibility in CI.

### R10 - Rust Tooling Baseline Is Missing
Status: partially remediated

Required baseline checks from standards are not visibly encoded in CI/hook files:

- `cargo fmt --all -- --check`
- `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- `cargo test --workspace`
- `cargo test --workspace --doc`
- `cargo check --workspace --all-features`
- `cargo check --workspace --no-default-features` for public feature contracts

Rectification:
- Add `scripts/rust/check.sh` or launcher `--test` extension coverage for these commands.
- Add CI matrix for required platform targets.
- Add workspace lints and member opt-ins.

Implementation notes:
- Completed: `scripts/rust/check.sh` owns standards-aligned fmt, check, clippy, test, doc-test, and no-default-feature commands for the Rust workspace excluding BEAM-dependent `pumas_rustler`.
- Completed: `scripts/rust/check.sh test-isolation` repeatedly exercises the guarded pumas-core API test surfaces with multiple test threads to support D09 process-global state audits.
- Remaining: add dedicated BEAM-aware Rustler CI and continue expanding feature/platform matrix coverage.

## Pass 03 Refactor Inputs
- Rust crate role map.
- Typed RPC/request contract extraction.
- Task lifecycle owner.
- Path newtype/validation layer.
- Unsafe isolation and lint policy.
- Binding surface split and verification matrix.
