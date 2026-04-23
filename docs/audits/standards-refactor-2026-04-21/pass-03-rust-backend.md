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
Status: partially remediated

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
- Remaining: audit `pumas-core/src/ipc/server.rs` nested connection tasks plus model download and conversion manager background tasks for bounded ownership and cancellation.

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

### R06 - Unsafe Rust Is Not Governed by Workspace Policy
Status: partially remediated

Unsafe usages exist in:

- `pumas-core/src/process/manager.rs`
- `pumas-core/src/process/launcher.rs`
- `pumas-core/src/conversion/manager.rs`
- `pumas-core/src/metadata/atomic.rs`
- `pumas-core/src/model_library/library/projection.rs`
- `pumas-core/src/platform/process.rs`
- tests manipulating process-global state

The workspace now inherits `unsafe_op_in_unsafe_fn = "deny"` as the first lint ratchet, but direct
unsafe usage remains allowed while OS/FFI boundaries are isolated.

Rectification:
- Add workspace lint policy with `unsafe_code = "deny"` by default.
- Move OS/FFI unsafe into thin modules that explicitly relax to `warn`.
- Add `SAFETY:` comments to every unsafe block.
- Add Miri/sanitizer plan for pure unsafe and OS FFI wrappers where practical.

Implementation notes:
- Completed: inherited `unsafe_op_in_unsafe_fn = "deny"` across workspace crates.
- Completed: documented current platform process probes, metadata fsync, and Windows long-path FFI
  with explicit `SAFETY:` comments.
- Completed: isolated launcher process detachment behind `platform::configure_detached_command` so
  launcher flows no longer own direct `pre_exec` unsafe blocks.
- Completed: replaced a direct `libc::kill(pid, 0)` call in process resource aggregation with the
  centralized `platform::is_process_alive` wrapper.
- Completed: Unix metadata writes now return fsync failures instead of ignoring them before rename.
- Remaining: isolate conversion-manager raw pointer lifetime bridges and Windows long-path expansion
  into smaller governed modules before ratcheting `unsafe_code`.

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
