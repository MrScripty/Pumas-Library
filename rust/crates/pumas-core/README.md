# pumas-library

`pumas-library` is the headless Rust API for the Pumas model library. It owns
launcher-root lifecycle, model packages, metadata, indexing, downloads,
imports, integrity reconciliation, runtime profiles, and serving state.

## Choose the Correct Access Role

| API | Use when |
| --- | --- |
| `PumasApi` / `PumasLibraryInstance` | This process owns the launcher root and may mutate it |
| `PumasLocalClient` | Another process owns the root and exposes the local RPC service |
| `PumasReadOnlyLibrary` | The caller needs indexed reads without lifecycle ownership |

Owner construction fails when another live owner has claimed the same root.
That result must remain distinct from connection, read-only, and recovery
outcomes.

```rust
use pumas_library::{PumasApi, Result};

#[tokio::main]
async fn main() -> Result<()> {
    let api = PumasApi::builder("/path/to/pumas")
        .auto_create_dirs(true)
        .build()
        .await?;

    for model in api.list_models().await? {
        println!("{}", model.official_name);
    }

    Ok(())
}
```

## Model and Storage Rules

The launcher root contains the canonical model filesystem and SQLite index.
Model identity includes repository/source and artifact information; repository
name alone is insufficient because a repository can contain several files or
quantizations. Equivalent content published in a different repository remains
a separate source model.

Import, resumable download, finalization, repair, migration, and deletion must
coordinate filesystem state, metadata, index rows, and update events. A
recoverable partial file, an automatically finalized download, and a complete
model are different transitions even when the final UI presentation is simple.

For managed HF downloads, Completed requires successful metadata publication
and indexing before the durable download admission is settled. Startup
finalization uses the same importer. An import failure retains the downloaded
files and recovery admission rather than reporting completion; the existing
resume or restart path can retry after the failure is resolved. Verified
recovery-ticket work retains its separately constrained mutation contract.

`PumasApi::shutdown_downloads()` closes download admission permanently and waits
for owned effects, including download-owned importer work and notifications.
Repeated callers share the result; cancelling a waiter does not cancel the
drain. This is not shutdown of unrelated imports, search, inference plugins,
or the application's runtime. Public completion notifications run after
terminal settlement and destination release; their failure does not undo a
completed download.

## Public Boundary

Base Python conversion setup is supervised independently of its caller.
Use `start_conversion_setup(None)` for prompt admission/attachment and
`get_conversion_setup()` for a memory-only latest snapshot. The snapshot contains
a canonical UUID and `in_progress`, `completed`, `failed` or `cancelled` state;
terminal state follows owned cleanup. `None` means this manager has no recorded
setup, not that Python is ready. Check readiness separately.

Starting without a previous ID returns retained work/results without retrying.
To explicitly retry, pass the last observed terminal operation ID. Only that
matching terminal record can be replaced; replaying the same retry request
returns the current operation instead of installing again. A valid old token on a fresh
owner fails without setup. Identity is latest-only and ends with the manager/
process lifetime: it is not historical lookup, durable restart recovery or
discovery of another manager's setup. Existing blocking
`ensure_conversion_environment()` still waits and permits explicit retry.

Concurrent requests on one manager share the active result; another manager or
process must acquire the same physical `launcher-data/conversion-setup.lock`
before deploying scripts or installing packages. Contention fails explicitly.
Keep that advisory lock file and its directory stable while setup is active;
this is not protection against hostile root replacement or abrupt host death.

Call `PumasApi::shutdown_conversion_setup()` before stopping the hosting runtime.
It closes setup admission and waits for owned cleanup; abandoning a setup or
shutdown waiter does not release the environment lease. Expected cancellation
with completed cleanup is a successful drain; retained setup failures
remain errors. This does not shut down conversion jobs or quantization-backend
installation. The RPC server includes this owner in its shutdown drain.
Setup uses one host blocking worker without nested filesystem work in that pool;
it also supports a current-thread Tokio runtime with one blocking thread.

Linux setup cleanup controls the installer process group and checks that no
live members remain before lease release. Installers must remain in that group.
On other targets, cancellation drains the foreground command naturally before
releasing custody, so shutdown can wait for it. Full process-tree evidence is
Linux-only. If Linux cleanup cannot establish quiescence, it retains custody
rather than report a completed shutdown. RPC and desktop expose the same setup
observation contract with redacted failures; dialog integration remains a
separate consumer follow-up.

The crate builds and runs independently of the optional GUI and RPC process.
For registered-link inspection, `PumasApi::get_link_health(None)` exposes the
owner's registry report. `model_library::LinkRegistry::health()` also supports
an independently loaded registry without constructing an application owner.
Both use the same read-only scan and return inspection failures as `Result`
errors, not an empty healthy report. This checks registered entries only: it
does not discover orphaned files, and the facade's version argument currently
does not filter the registry. Mutation remains with the separately owned link
operations; reading health never deletes or repairs files.

Prefer the facade and typed domain records exported by `src/lib.rs`. Internal
modules own persistence, network, process, provider, and conversion details.
Do not make a new internal module public to avoid designing a stable operation.

Untrusted JSON, paths, URLs, metadata, and persisted rows must be decoded and
validated at entry. Preserve invalid, absent, unsupported, stale, partial, and
failed outcomes instead of collapsing them into defaults.

## Features

The default `full` feature enables the named `hf-client`, `process-manager`, and
`gpu-monitor` markers. Those markers currently do not remove their dependencies
or module surfaces when disabled. The `uniffi` feature does gate the optional
UniFFI dependency.

## Verification

From the repository root:

```bash
cargo test --manifest-path rust/Cargo.toml -p pumas-library
cargo check --manifest-path rust/Cargo.toml -p pumas-library --all-targets --all-features
cargo clippy --manifest-path rust/Cargo.toml -p pumas-library --all-targets --all-features -- -D warnings
cargo doc --manifest-path rust/Cargo.toml -p pumas-library --no-deps
```

Use `./scripts/rust/check.sh` for the workspace evidence set. Tests must use
temporary roots and must not discover or mutate a developer's real library.

See the workspace [Rust guide](../../README.md) and
[architecture](../../../docs/ARCHITECTURE.md).
