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

Risks:

- `pumas-rpc` accepts arbitrary `--host`, and server CORS is configured as `Any` for all origins/methods/headers.
- Torch LAN access can set host to `0.0.0.0`; this may be product-intended but needs documented security policy and authentication/authorization review.
- Listener concurrent connection limits are not visible in `pumas-rpc/src/server.rs`.

Rectification:
- Validate host binding policy at CLI/config boundary.
- Restrict CORS to renderer/dev origins where possible.
- Add max connection limits or request concurrency limits.
- Document LAN mode threat model for Torch server.

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
