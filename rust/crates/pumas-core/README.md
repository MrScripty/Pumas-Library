# pumas-library

`pumas-library` is the headless Rust API for the Pumas model library. It owns
launcher-root lifecycle, model packages, metadata, indexing, downloads,
imports, integrity reconciliation, runtime profiles, and serving state.

## Optional inference dependencies

Default builds retain ONNX execution through the `full` feature. Model-management
consumers can use `default-features = false` (and `features = ["hf-client"]` for
Hugging Face access) without compiling ONNX Runtime, tokenizers or half-precision
tensor support. Enable `onnx-runtime` explicitly to expose `onnx_runtime` and its
re-exported execution types. Provider descriptions and model metadata remain
available without that feature. RPC enables it through `inference-plugins`.

## Choose the Correct Access Role

| API | Use when |
| --- | --- |
| `PumasApi` / `PumasLibraryInstance` | This process owns the launcher root and may mutate it |
| `PumasLocalClient` | Another process owns the root and exposes authenticated local IPC |
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

## Local Intent Interface

`PumasApi::intent()` exposes structured local model requirements through
`query_models`, `get_model`, and `get_model_status`. The existing
`PumasApi::get_model(&str)` retains its operational lookup behavior.

An intent requirement selects an existing local model reference or a Hugging
Face repository, with optional revision, artifact, format, and quantization
constraints. `query_models` and `get_model_status` are read-only local
observations: they do not contact an upstream service, generate package facts,
or reconcile the library. `get_model` also remains observational under
`AcquisitionPolicy::LocalOnly`. With `AllowUpstream`, it can join or admit one
managed Hugging Face acquisition when no matching local artifact is ready.

Upstream branch and tag selectors are resolved to a validated immutable commit
before admission. An `Acquiring` result includes a pinned local
`resolved_requirement`; use that requirement with `get_model_status` to poll
without resolving the moving selector again. The download hint is advisory, and
the managed download lifecycle remains the authority. Existing paused, failed,
or unresolved durable custody is reported explicitly rather than starting a
second writer. If several pinned artifacts satisfy a request,
`UpstreamAmbiguous` returns distinct candidates and selected artifact IDs so the
caller can choose one and retry.

Initial upstream acquisition supports a single GGUF, ONNX, or bare Safetensors
file whose pinned repository tree supplies trusted LFS size and SHA-256
evidence. Known Hugging Face directory, sharded, adapter, and Diffusers bundle
layouts are reported as unsupported before admission. In particular, a
Safetensors repository with directory metadata such as `config.json` is not
treated as a coherent single-file artifact. I8 directory packages remain
unsupported through this path; the interface does not fabricate a file handle
for a directory load target. Filename quantization is only a selection hint:
local package facts must still prove the requested quantization before the
result becomes `Available`.

Public operational `DownloadRequest` values retain their existing default-main
behavior. Intent acquisition carries its immutable revision through metadata,
auxiliary files, retries, persistence, resume, integrity verification, and
import. Download persistence uses schema 5 with validated schema-4 upgrade;
older readers reject the new version. `PersistedDownload` gains a
`revision: Option<String>` field, so Rust callers constructing that lower-level
record must initialize it. Pinned ticket recovery is unavailable; pinned
execution resumes through its persisted download record.

`ensure_model(&EnsureModelRequest)` durably records a consumer's requirement
before attempting convergence. `Accepted` includes the declaration reference and
an observed state; acceptance alone does not mean the artifact is available.
Repeated requests from the same consumer reuse the declaration. Different
consumers retain separate declarations for the same model. Upstream targets bind
to an immutable commit and canonical artifact before download admission.

`get_ensure_status(&ModelEnsureRef)` and `list_declarations()` read durable state.
`release_model(&ModelEnsureRef)` removes only that declaration, without deleting
files or cancelling downloads. The reference's generation prevents an old
release from removing a replacement declaration. Initial durable requirements
support local references with `LocalOnly` and repositories with `AllowUpstream`.

The primary instance reconciles declarations at startup and on existing library
events. After restart it can resume an exact retained download whose durable
state shows an interrupted queue or transfer. Deliberate pauses and failed
verification remain blocked; recovery does not enqueue a replacement writer. Transient upstream work has at most three automatic retry wakes per
unresolved episode (30 seconds, 2 minutes, 10 minutes); later events, explicit
ensure, or restart can trigger another observation. `shutdown_intent()` closes
local admission, drains admitted local effects, then drains downloads, even if
the requesting waiter is dropped. `shutdown_downloads()` retains its narrower
meaning. Neither operation stops inference runtimes.

The index contains the versioned declaration authority for
ensure/release. Its additive migration is transactional and writer connections
use SQLite FULL synchronization. Declaration and deletion-claim rows survive
catalog clearing; incompatible or corrupt declaration state fails closed.
Destructive operations consult durable declarations, deletion claims, and the
existing download inventory, including paused work when HF is disabled. Failed
partial deletion retains its claim for explicit recovery. Standalone libraries
without composed mutation authority refuse destructive operations; path-only
merge is also refused. Cross-filesystem relocation and partial duplicate merging
preserve the source rather than using an unprotected copy/delete fallback.
Historical binaries ignore the new tables, so downgrading with live declarations
or claims is unsupported.

Availability requires matching evidence and a current local artifact. Missing
files, stale or incomplete facts, mismatched constraints, ambiguous selection,
and invalid requirements remain typed outcomes. A returned handle is an
observation of local availability, not a file lease, an integrity certificate,
or proof that an inference runtime can execute the model. Re-resolve when a
fresh access decision is needed. Inspect readiness and match evidence when
consuming query candidates; being listed is not a promise of availability.
GGUF quantization constraints require header evidence; filename-derived guesses
remain insufficient, and canonical header labels such as `MOSTLY_Q4_K_M` match
`Q4_K_M` without changing the recorded evidence.

Some existing local Hugging Face directory packages expose a primary-file entry
path with a directory load-target kind. The intent interface reports
`Incomplete` for that inconsistency rather than returning a handle with the
wrong path kind. Correcting that existing package/load-target contract is
separate work.

See the native [local intent example](examples/intent_model.rs) and [managed
acquisition example](examples/intent_acquire.rs) for the public call shapes.
The [local client example](examples/intent_local_client.rs) uses the same domain
requirements through `PumasLocalClient::intent()` and existing authenticated IPC.
These types and policies belong to core independently of any transport; no new
service or network exposure is needed.

## Intent Through Existing Local Transports

`PumasLocalClient::intent()` provides the seven intent methods with the same
request and outcome types as `PumasApi::intent()`. Discover a ready owner and
connect through the existing registry; its connection token remains required.
The loopback JSON-RPC adapter exposes these methods with an `intent_` prefix;
see its [wire contract](../pumas-rpc/README.md#local-intent-rpc).

Both adapters call the owning core service. Dropping a consumer connection does
not release a declaration or cancel admitted work. Reconnect and query status
using the pinned requirement or declaration reference. For durable retention,
keep the `ModelEnsureRef` returned by ensure and release that exact generation
when it is no longer needed. Release removes the declaration only.

Operational lookup/download methods remain available. UniFFI and the desktop
bridge continue to expose their existing operational contracts; adopting the
intent methods through those bindings is separate work. Nodes, fleet management,
remote discovery and additional network features are deferred until after the
next Pumas release.

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

Built-in conversion and quantization setup is supervised independently of its caller.
Standalone callers use `start_backend_setup(backend, previous_id)` and
`get_backend_setup(backend)` on `PumasApi` or `ConversionManager` for
`PythonConversion`, `LlamaCpp`, `Nvfp4`, `Fp8`, and `Sherry`. The backend selects its
existing installer owner, shared with the corresponding ensure method; it does
not create a second installation. Reads are memory-only, do not probe or install,
and remain available after shutdown. Keep the selected backend alongside its
snapshot: IDs cannot retry another backend's operation. Malformed IDs and retry
tokens without a selected owner-local record return `InvalidParams`.

`Fp8` provides `SafetensorsToFp8` for supported local Transformers language-model
packages. The existing desktop conversion dialog offers **Safetensors (FP8)**.
Conversion runs on CPU and stores E4M3FN linear weights with float32 scales per
128x128 block; embedding/output weights remain BF16. The output is a reloadable
Transformers Safetensors package, with tokenizer/config files and conversion
provenance. Sources remain intact. Quantized inputs, incomplete packages,
non-finite weights and incompatible matrix dimensions fail explicitly. GPU
compatibility is a separate requirement when loading the result for inference.

NVFP4 is available alongside FP8 in the model conversion dialog. It reuses
`SafetensorsToNvfp4` and the managed `Nvfp4` backend. Model Optimizer 0.40.0
calibrates an unquantized local Transformers language model and exports packed
E2M1 weights, block/global scales, tokenizer files and ModelOpt quantization
configuration as Safetensors. CUDA is required for calibration. Its large dependency installation has a
45-minute command budget, with cancellation and cleanup retained by the shared
setup owner; other backend budgets are unchanged. An optional
calibration text file can be supplied through the conversion API; the dialog
uses a small offline fallback corpus. Output is checked for packed weights and
scales before publication. An NVFP4-compatible consumer is required; the qualified
FLUX runtime continues to select its working FP8 encoder. Conversion support does
not imply native NVFP4 encoder serving support. The export format follows the
[Model Optimizer Hugging Face exporter](https://github.com/NVIDIA/Model-Optimizer/blob/0.40.0/modelopt/torch/export/unified_export_hf.py).

For base Python conversion setup,
use `start_conversion_setup(None)` for prompt admission/attachment and
`get_conversion_setup()` for a memory-only latest snapshot. The snapshot contains
a canonical UUID and `in_progress`, `completed`, `failed` or `cancelled` state;
terminal state follows owned cleanup. `None` means this manager has no recorded
setup, not that Python is ready. Check readiness separately.

`is_conversion_environment_ready()` shares an active base Python import probe
between overlapping callers; later reads probe again. Missing interpreter or
normal nonzero import exit returns `false`. Spawn, signal, deadline and cleanup
failures return `ConversionFailed`; cancellation/closed admission returns
`ConversionCancelled`. These are inspection failures, not authoritative
not-ready answers. This replaces the former false-on-timeout/signal and internal
I/O-error behavior. The five-second command budget does not bound cleanup time.
Dropped readers leave work retained until observation or setup shutdown drains
it. The synchronous manager boolean remains conservative on error and its
caller must finish the invocation before shutdown. Neither read installs or
acquires setup exclusion. Base setup uses the same import specification and
command runner within its existing installer worker, not the public read owner.

Starting without a previous ID returns retained work/results without retrying.
To explicitly retry, pass the last observed terminal operation ID. Only that
matching terminal record can be replaced; replaying the same retry request
returns the current operation instead of installing again. A valid old token on a fresh
owner fails without setup. Identity is latest-only and ends with the manager/
process lifetime: it is not historical lookup, durable restart recovery or
discovery of another manager's setup. Existing blocking
`ensure_conversion_environment()` still waits and permits explicit retry.

Concurrent setup requests on one manager share the active result; another manager or
process must acquire the same physical `launcher-data/conversion-setup.lock`
before deploying scripts or installing packages. Managed conversions acquire
the same exclusive lease before execution and retain it through native cleanup,
output publication and indexing. This also excludes other managed conversions
at the same root because base conversions deploy shared scripts; independent
roots remain independent. Contention fails the operation explicitly: inspect
setup status or conversion progress for the terminal failure. There is no queue
or automatic retry. Terminal progress follows observed lease release.
Keep that advisory lock file and its directory stable while either operation is active;
this is not protection against hostile root replacement or abrupt host death.

Managed quantization admission and direct calls to all built-in backends require
an exact target/backend match in that backend's advertised catalog. Unsupported
targets return `InvalidParams` before readiness probes or execution effects;
direct checks also precede source-file inspection. No aliases, normalization or
fallback targets are applied.
Catalog membership does not certify installed-tool or hardware support.
After target validation, managed and built-in direct calls reject
`force_imatrix=true` outside llama.cpp with `InvalidParams`; NVFP4 and Sherry do
not silently ignore it. False remains accepted. llama.cpp retains its IQ/forced
calibration requirements and supplied-file preflight.

Conversion source discovery accepts only regular files with exact, lowercase
format extensions, including symlinks to regular files. Matching directories
and special files are excluded; enumeration and matching-entry inspection
failures remain contextual `Io` errors, including dangling matching symlinks.
llama.cpp now reports missing/uninspectable source directories as `Io`, selects
the first sorted GGUF before staging, and prefers GGUF when both formats exist.
This is file classification, not content validation or immutable input custody.

Managed quantization admission and direct llama.cpp calls validate every supplied
calibration path, even when optional: it must name a nonempty regular file that
can be opened and yield a byte. Missing/nonfile/empty inputs are `InvalidParams`;
other inspection/open/read failures retain contextual `Io` errors. Validation
precedes imports, staging and native execution. Symlinks to valid files remain
supported. This bounded probe does not validate text/content quality or retain
an immutable input: keep paths and contents stable/readable during preflight and
execution. Later native reads can still fail after successful preflight.

Call `PumasApi::shutdown_conversion_setup()` before stopping the hosting runtime.
It closes setup admission and waits for owned cleanup; abandoning a setup or
shutdown waiter does not release the environment lease. Expected cancellation
with completed cleanup is a successful drain; retained setup failures
remain errors. This also closes and drains all built-in quantization installers
and async readiness probes, but does not shut down conversion jobs. Finish
caller-owned synchronous readiness calls first. The RPC server includes these
owners in its shutdown drain. Closed setup admission returns `InstallationCancelled`.
Direct backend execution, independently requested readiness probes and external
tools do not participate in managed execution exclusion. Callers must exclude
these from setup: each admitted llama.cpp setup recipe reconfigures and
clean-builds both native targets, even when existing binaries look usable.
The current checkout after clone/update supplies the source; a normal failed
optional pull warns and uses local source, not necessarily latest upstream.
No revision cache is trusted: Git HEAD alone misses local edits and build inputs.
CUDA configuration explicitly follows the compiler check instead of inheriting
a previous CMake ON value. Generated output directories/symlinks refuse cleaning;
source and Python environments are not reset. Retained operation observation
does not run the recipe again.

Native setup creates `launcher-data/llama-cpp/setup-incomplete` before changing
source/build/Python state under the root lease. A failed or cancelled recipe
leaves that marker in place across process restart. All llama.cpp readiness
checks, `has_imatrix`, and direct/managed quantization refuse it before import,
staging or native execution. Explicit successful setup verifies the environment
and removes the marker; status reads never repair it. The normal marker is a
zero-byte regular file. Unexpected occupied entries block use and refuse setup
without deletion; inspection errors remain errors on fallible surfaces.
Do not manually remove the marker to bypass repair. A retained setup receipt is
historical operation state, not current installation validity.

This protects process-exit/reopen on stable local paths, not power-loss ordering,
hostile edits, manual deletion or overlapping older binaries. No marker means no
recorded incomplete recipe, not certified provenance of preexisting tools or
later external edits. Managed execution retains root exclusion; direct execution
and independent probes still need caller coordination with setup. A release
failure after verified marker removal is a lease-cleanup failure, not invalid
recipe contents.
A completed setup is not proof that a later conversion's route, hardware or
inputs are ready.
Each setup operation uses one host blocking worker without nested filesystem work in that pool;
it also supports a current-thread Tokio runtime with one blocking thread.
Managed conversions use brief owned blocking calls for lease acquisition and
release, leaving that pool available during asynchronous execution.

Linux setup cleanup controls the installer process group and checks that no
live members remain before lease release. Installers must remain in that group.
On other targets, cancellation drains the foreground command naturally before
releasing custody, so shutdown can wait for it. Full process-tree evidence is
Linux-only. If Linux cleanup cannot establish quiescence, it retains custody
rather than report a completed shutdown. RPC and desktop currently expose base
Python and backend-specific setup observation with redacted failures. Rust
callers remain independent of RPC and the optional desktop bridge; the bridge
does not own setup state or retry admission.

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

The default `full` feature enables `hf-client`, `process-manager`, `gpu-monitor`,
and `onnx-runtime`. The first three retain their existing marker behavior.
`onnx-runtime` gates the ONNX execution module and its native/runtime dependencies;
`uniffi` gates the optional UniFFI dependency.

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
