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

## Explicit Hugging Face file selections

`DownloadRequest::filenames` selects both regular repository files and LFS files
from one resolved commit. Every distinct requested path must exist in that pinned
tree before admission. LFS selections retain the tree's size and SHA-256;
regular files retain unknown size/digest until shared acquisition verifies their
actual bytes. A mixed set has no selected-set size denominator while any file
size is unknown. Explicitly selected config/tokenizer files are fetched once,
alongside the existing automatic auxiliaries. Completion still waits for model
import and its consumer receipt; selecting a file does not authorize code execution.

Counted shard sets must be complete. Selected SafeTensors/PyTorch shards include
only their matching `*.safetensors.index.json` / `*.bin.index.json`, and missing
indexes fail before admission. Acquired indexes must map tensors to the exact
selected weight payloads in the index's format (SafeTensors, or PyTorch
`.bin`/`.pt`/`.pth`), including every selected member of their shard family.
Auxiliary/index documents cannot serve as weight targets; invalid,
empty, duplicate-key or out-of-selection maps cannot reach final import/completion. Existing partial metadata and recovery
markers remain available after such a failure.
Index validation reads verified local descriptors once without a second payload
fetch and uses the existing 16 MiB package JSON input ceiling.

A whole Diffusers request (without a file or quant selector) includes both regular
and LFS files from the pinned tree, including component configs and tokenizer
assets. Classification preserves explicit selectors; an incomplete explicit
component set fails rather than becoming a whole-repository download. Acquired
`model_index.json` must name a supported pipeline with its declared non-optional
components in the selected set, using existing component/path semantics. An
explicit bundle format skips the preliminary classification read; automatic
classification retains its existing metadata observation before the one acquired
model-index payload. Public request types, signatures and receipt identity are
unchanged. This is package selection/import support, not inference qualification.

## Shared HTTP source-wait budgets

`AcquisitionConsumer::acquire_http` accepts an exact manifest/file-source set and
holds verified inputs through consumer publication. For each selected file,
positive `AcquisitionRetryPolicy.elapsed` starts one source-wait deadline before
local file preparation. The deadline bounds response headers, body streaming,
all attempts and backoff; retries do not reset it, and another file receives its
own budget. The clock range is checked before public worker/store admission.
HTTP `elapsed = Duration::ZERO` preserves the existing opt-out. Attempt limits,
source identity, verification requirements, receipts and public shapes are unchanged.

Expiry prevents a new source/body effect from starting, but already registered
filesystem effects are still drained before retry or return. Partial inputs keep
exact Transferring custody and cannot reach consumer preparation/publication or
receipt settlement on timeout. This is a source-wait budget, not a hard bound on
filesystem drainage, verification, import or child cleanup. The existing positive
HF configuration is enforced; explicit native zero-budget behavior is preserved.

## Optional S3 protocol reader

Enable `s3` explicitly to use `acquisition::{S3Reader, S3ReaderConfig}`. This
reader supports anonymous or explicitly authenticated access to configured
versioned objects. It does not discover credentials or endpoints from the environment. Configure
an HTTP(S) origin, region, bucket, addressing style, and positive operation
budget. Virtual-hosted endpoints must already identify the bucket; HTTP requires
explicit opt-in. Redirects, ambient proxies, SDK retries, and automatic HTTP
protocol retries are disabled.

`S3Reader::new(config)` retains anonymous behavior. For authentication, construct
`S3Credentials::new(access_key_id, secret_access_key, optional_session_token)`
and pass the owned value to `S3Reader::new_authenticated(config, credentials)`.
Credentials must be nonempty printable ASCII without whitespace; access-key IDs
also exclude `/`, `,`, and `=`. Invalid input returns `S3ReaderError::Configuration`
without echoing its value. Authenticated construction requires HTTPS, even with
`allow_http: true`; it uses normal peer verification. The public `test-support`
feature does not enable plaintext credential transport.

The optional reader uses pinned `aws-sdk-s3` 1.137.0 as its sole protocol and
credential owner, without `aws-config`, a default AWS HTTP transport, or a storage
backend fallback. Requests use the existing reqwest transport pool, constrained
to the configured origin and GET/HEAD. SDK construction, complete request futures
and body polls use scoped diagnostics to prevent credential-header tracing while
preserving outer acquisition status events. Error bodies are bounded to 1 MiB;
provider diagnostics are never propagated. Prefix listing and its reviewed XML
validation guard remain separate planned work.

The maintained SDK signs HEAD and conditional range GET with SigV4 and includes
the optional session token. Credentials remain in the in-memory reader/selection
capability until its last owner drops; they have no serialization or discovery
API and are never part of manifests, receipts, checkpoint identity, or persisted
state. Their `Debug` output redacts all fields. Remote protocol diagnostics are
replaced with bounded safe messages, including for anonymous readers, because
response errors can echo access material. Existing error variants are retained.
Selections can outlive their reader. Callers should scope those selections to
their acquisition; expiration or revocation fails the selected request and does
not refresh credentials, retry anonymously, or select a different object.

`select(key, version_id, logical_path, sha256)` validates the local path and
exact remote key, resolves HEAD for that VersionId, and refuses mutable `null`
versions or missing/different version evidence. The returned selection exposes
the existing `ArtifactManifest`; its source identity binds endpoint, bucket,
addressing style, key, and revision. `read_range(start..end, &mut staging)` streams
exact selected bytes using VersionId and If-Match, with metadata checks before
writes. ETag is a conditional validator, never a digest. Dropping the read future
stops polling it; timeout, error, or cancellation can leave partial staging bytes
under the caller's custody.

The reader owns no acquisition store, tasks, retry policy, verifier, or
publication. Default and headless builds omit the SDK.

### Native explicit S3 model workflow

With `s3` enabled, `PumasApi::import_s3_model(request, control)` composes explicit
source selection, shared acquisition/verification and the existing model importer.
`S3ModelImportRequest` supplies endpoint/region/bucket/addressing/timeout facts,
`Vec<S3ManifestEntry>` with exact key/VersionId/logical path/SHA-256 evidence,
`ModelImportSpec` naming the selected primary GGUF, a caller-reserved
`AcquisitionWorkspace`, positive finite `AcquisitionRetryPolicy` budgets and a
caller-retained `operation_id: Uuid`. One GGUF and optional explicit data/text
auxiliaries use the existing importer eligibility and receipt rules.

Set `credentials: None` for anonymous access, or move explicit `S3Credentials`
into `Some(...)` for HTTPS authentication. The request has no Debug or serde
implementation. Credentials never become the importer payload, manifest,
receipt, progress or model metadata; there is no account discovery or saved
source/credential configuration. Production authentication requires HTTPS even
when the source config has `allow_http: true`.

Create one `S3ModelImportControl::new()` per operation and call `subscribe()`
before starting work to observe coalesced phase and current-file byte progress.
The receiver contains no URLs, keys, credentials or errors; bytes can reset
between files/retries and do not prove verification. `cancel()` returns true
only when cancellation wins before finalization. Keep awaiting the result to
observe owned drainage. Once finalization starts, control cancellation is
refused; the existing importer/receipt pipeline owns publication and settlement.
Disconnecting a progress receiver does not cancel the operation.

Success returns the existing `ModelImportResult` after receipt settlement.
`S3ModelImportError` distinguishes source selection, operation and scope drainage
failures. A drainage error preserves an already published result or original
operation failure. Errors/cancellation can leave retained staging or Using custody;
there is no blanket cleanup or automatic reimport. Keep the same operation UUID
for the same logical request, and reconcile retained work through the existing
`model.s3.workflow` acquisition consumer and exact model-output proof pipeline.
A reused control is refused; never use a new UUID to replay uncertain publication.
Dropping the result waiter reports `Interrupted`, not completion; shared
`shutdown_acquisition()` drains registered effects. RPC/desktop source entry,
credential refresh and real-provider acceptance remain separate.

### Shared S3 acquisition and one-file GGUF import

`AcquisitionConsumer::acquire_s3` accepts an `AcquisitionS3Request` containing
the exact selection, demand, reserved workspace, and positive finite attempt and
elapsed retry budgets. It uses the same transfer, live-prefix checkpoints,
verification, task supervision and consumer-receipt settlement as HTTP.
Network/body deadlines include destination writes; an interrupted registered
filesystem effect must drain before retry or release. Admission and consumer
publication are separate lifecycle phases, not a hard wall-clock guarantee.

For a single GGUF, keep the acquired use in the prepare result and perform model
publication in the publish callback. Its receipt payload must be the serialized
`ModelImportSpec`, whose `path` is the exact selected logical path:

```rust,ignore
let prepared_spec = spec.clone();
let result = consumer.acquire_s3(request, host,
    move |inputs| async move {
        Ok((inputs, serde_json::to_value(prepared_spec)?))
    },
    move |inputs, receipt| async move {
        importer.import_acquired_gguf(&inputs, &receipt, &spec).await
    },
).await?;
```

Use the lifecycle-owned `ModelLibrary` supplied by `PumasApi` or
`PumasLibraryInstance`. Model publication requires the exact currently issued
acquisition receipt and reads the verified descriptor. Copying checks the
output digest against the acquisition receipt before the existing model
publisher can expose Ready. A refused import returns an error and retains
consumer custody. A retained `Using` intent receipt without proven model output
requires explicit owner reconciliation; automatic reimport is unavailable.

For a confirmed model whose acquisition acknowledgement was interrupted, use
`ModelImporter::reconcile_acquired_gguf` as the output-validation callback of
`AcquisitionConsumer::reconcile`. Supply the retained demand, exact manifest,
fresh held workspace, import spec and candidate model ID. The candidate ID is
only a lookup: recovery requires the exact acquisition receipt in the model's
confirmed publication record, acknowledged Ready index, canonical primary
metadata and unchanged physical payload with the verified input digest. The
observer performs no download, import, index repair, Pending promotion or
cleanup. Successful proof lets the existing consumer settle the same use;
repeated adopted proof is observational and idempotent.

Acquired recovery requires an explicit canonical publication identity with its
matching indexed projection before Confirmed/root/payload proof. Missing/null
identity retains acquisition uncertainty even if a matching v2 receipt survives.
Ordinary legacy readiness remains supported separately.

New consumer-facade operations admit at most 4 MiB of actual pretty-serialized
manifest JSON and a 2 MiB payload namespace proof reserve before transfer or
store admission. The reserve charges the encoded file names and unique parent
prefixes plus 512 bytes per physical proof entry. New completion bindings must
also fit 4 MiB before issuance or the publication callback; callback refusal
retains the verified input and unreceipted use. These budgets leave room inside
the copied-output receipt's 16 MiB observation limit. Existing manifest decoding,
retained receipt reconciliation and ordinary copied-import formats are unchanged.

Acquired copied imports now emit publication receipt version 2 with their exact
issued acquisition binding. Ordinary copied imports retain version 1. Existing
version-1 copied models remain readable, but an unbound version-1 model cannot
prove an interrupted acquisition generation. Unsupported versions and missing
bindings are refused and retained without automatic migration. Model metadata's
existing publication identity remains version 1; it is a separate contract.

Explicit file sets use `S3Reader::select_manifest(Vec<S3ManifestEntry>)` followed
by `AcquisitionConsumer::acquire_s3_manifest(AcquisitionS3ManifestRequest, ...)`.
Each entry declares an exact key, immutable VersionId, logical path and SHA256.
Resolution validates the complete namespace before HEAD and returns a selection
only after every declared version resolves. The manifest revision retains every
original per-object identity in canonical logical-path order. Its existing
16-KiB revision bound limits the encoded pin set; operation/retry budgets remain
per object. This is an explicit set, not a snapshot obtained from prefix listing.

For one primary GGUF with selected data/text auxiliaries, use
`ModelImporter::import_acquired_gguf_bundle` in the commit callback and
`reconcile_acquired_gguf_bundle` in the existing consumer reconciliation callback.
The exact serialized import spec names the primary logical GGUF path. Auxiliary
paths are preserved, every held input is copied and digest-verified, and the
existing publisher confirms the complete model before acquisition acknowledgement.
Cold proof requires the exact current receipt and every selected output path,
size and digest; omitted or changed members cannot settle the use. Allowed
auxiliary extensions are json, txt, md, model, tiktoken, vocab and merges. Another
weight file, executable auxiliary and reserved root metadata name are refused.
Output receipt versions and ordinary/single-object behavior remain unchanged.

Authenticated stores, prefix discovery, sharded/multifile weight formats,
desktop source selection, and live AWS/non-AWS/MinIO qualification
remain open. Local synthetic GGUF fixtures prove model-library publication,
not inference execution, packaged consumers, or the AQ-S3 gate. Hosted S3
commands explicitly enable `s3`; ordinary feature graphs omit the reader.

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

## Full Package Facts Through Local IPC

`PumasLocalClient::resolve_model_package_facts(&model_id)` returns the owner's
full `ResolvedModelPackageFacts` version 3, including the nested model reference,
artifact evidence and diagnostics. It uses the existing authenticated framed
`resolve_model_package_facts` IPC operation and the owner's model-library resolver.
Model IDs must be nonempty relative identities of at most 4,096 UTF-8 bytes;
accepted identities are forwarded without rewriting. Registered external assets
continue to resolve through their relative library model IDs.

Invalid parameters and missing or wrong connection tokens retain JSON-RPC code
`-32602` and become `PumasError::InvalidParams` in the local client. Other server
errors retain their wire codes and use the existing `PumasError::Other` client
mapping; missing models retain wire code `-32002`. Transport loss remains
`PumasError::SharedInstanceLost`. This method does not select or launch inference.

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

An orderly owner restart must first await successful `shutdown_instance()`;
subsystem shutdown and synchronous `Drop` are not owner-release receipts.
Failed construction, failed composed shutdown, and abrupt process exit retain
the registry generation and block another primary. Acknowledged declarations,
releases, and replacement generations can survive process loss and be inspected
read-only without authorizing owner takeover. Automatic primary crash recovery
awaits a qualified physical-store lifetime mechanism. Persistence after process
loss and generation/ABA protection across orderly restart are separate claims.

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

Managed Hugging Face directory packages use their canonical package directory
in execution descriptors, package facts, and artifact load targets. Persisted
upstream revisions survive the facts and target projections, including literal
legacy `main` revisions without claiming immutable provenance.

`resolve_model_artifact_load_target` in `OwnerFresh` mode reobserves managed
packages through the existing package-facts producer. Missing selected or
expected members prevent a ready HF target. `ReadOnlyIndexed` returns indexed
snapshot evidence without inspecting package files or repairing cache rows.
Legacy HF observations require owner reinspection; indexed resolution refuses
their stale tokens, file-shaped paths, and dropped revisions.
HF load targets currently require library-owned package roots. External-reference
HF metadata and cache labels do not qualify a directory root or its read set, so
the shared resolver returns a typed invalid-artifact response. External Diffusers
directory support follows its separate existing validation contract.

Owner and read-only views capture the existing canonical library root and its
platform display spelling at startup/open. Indexed HF path checks then compare
against that stored display root without probing package directories. Relative
and symlink root aliases select the same index; Windows verbatim drive and UNC
prefixes follow the existing display contract. Portable relative model IDs are
required before joining paths. Physical canonical roots remain the custody basis.

Target `content_fingerprint` projects the cache's exact
`pumas-package-observation-v1:sha256:<digest>` token. The protocol version is part
of the observation hash domain. It covers canonical metadata, descriptor,
dependencies, manifest paths, file sizes and mtimes. This is a resident-cache
invalidation observation, not a whole-package content digest: equal-length
byte changes that restore mtime may leave it unchanged. Primary-file hashes and
managed acquisition verification remain separate evidence.

The synthetic producer tests and the pinned Pantograph guard interoperability
runner at `tests/pantograph_guard_interop/run.py` exercise this handoff without
loading model tensors. Real Cohere processor-file completeness requires a
separate pinned member-manifest audit, and runtime compatibility is not claimed.

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

For calls on a retained `PumasLocalClient`, dropping the caller future skips
requests still waiting for transport admission. Once admitted, the client owns
the complete write/read exchange and validates its original request ID before
allowing another request onto that connection, even if the caller disappears.
Valid RPC errors leave the connection reusable. Partial IO, malformed responses,
or invalid correlation close it; later calls return `SharedInstanceLost`.
Uncertain operations are never retried automatically.

There is no response-drain deadline. A hung peer can block subsequent calls
while the client remains alive. Dropping the final client owner requests worker
abortion and socket closure; synchronous disposal cannot await that closure,
which requires the Tokio runtime to poll the abort. Disposal does not prove
that server-side work stopped, and it does not drain a hung exchange to completion.

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
