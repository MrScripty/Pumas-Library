# Architecture

## System Shape

Pumas separates durable model-library behavior from desktop presentation and
optional inference runtimes.

The Rust crate is independently embeddable and the RPC binary independently
executable. Neither requires the GUI or its JavaScript dependencies. The
launcher selects the optional GUI with `PUMAS_GUI`; inference-plugin selection
is independent. The GUI adapts backend operations and owns native dialogs and
window controls, rather than defining the backend's available functionality.

```text
React renderer
  -> sandboxed Electron preload/main boundary
    -> local HTTP/JSON-RPC
      -> pumas-rpc
        -> pumas-core
        -> optional runtime providers and pumas-app-manager
```

| Component | Owns | Does not own |
| --- | --- | --- |
| `frontend` | UI drafts, presentation, renderer subscriptions | Durable model/runtime truth |
| `electron` | Desktop privileges, sidecar lifecycle, IPC validation, windows and dialogs | Model-library domain rules |
| `pumas-rpc` | HTTP transport, JSON-RPC dispatch, SSE, `/v1` gateway | Durable model storage |
| `pumas-core` | Library state, SQLite index, downloads, imports, reconciliation, runtime profiles, serving state | Desktop presentation |
| `pumas-app-manager` | External runtime versions, installers, processes, service clients | Model catalog authority |
| `torch-server` | Torch model slots and Torch HTTP inference | General Pumas library state |

## Library Ownership

A launcher root has one owning `PumasApi`/`PumasLibraryInstance`. Construction
claims the root and refuses an existing owner row, including unresolved
ownership after failed startup/shutdown or process loss. Orderly replacement
requires successful composed `shutdown_instance()`; PID liveness and subsystem
drains do not authorize reclaim.
`PumasLocalClient` explicitly connects to an existing ready owner. The owner's
loopback IPC server admits a bounded number of connections; a connection over
the limit receives an error frame naming the limit before it is closed, and a
client that starts a frame without finishing it within 30 s is closed with a
reason. Idle connections between frames stay open, because local clients hold
one connection for their lifetime. The registry database that carries the IPC
connection tokens is owner-only (0600, directory 0700) on Unix.
`PumasReadOnlyLibrary` reads indexed state without taking lifecycle ownership.
An explicit `InstanceProfile::CatalogQuery` owner opens an existing index read-only
through the same builder/API and restricted local IPC dispatcher. It supports
indexed list/get/literal-search observations and same-boot, exact-checkpoint cold
reopen using the existing physical lease. It admits no acquisition, child/service
launch or external HTTP owner. Full/legacy/unknown histories remain refused; see
the [restricted custody contract](contracts/restricted-owner-custody.md).

On Linux/macOS the existing owner also holds the physical launcher directory
across independent rendezvous registries and symlink aliases. The lifetime is
retained by escaped model/index/link and acquisition handles and their admitted
blocking effects. After `shutdown_instance`, drop the API and those handles
before starting a new owner. This strengthens live exclusion only: unknown or
failed registry generations are still refused, and a free lock does not qualify
recovery from crashed owners or external runtime children. See the
[local discovery contract](contracts/local-discovery-bootstrap.md#slice-3-durable-identity-and-qualified-lifetime-custody)
for the remaining physical-identity and child-custody gates.

An embedding application or standalone RPC process constructs the owner; in
desktop use, Electron supervises that RPC process. Renderer state is a projection of
backend responses and update events; local optimistic state must not redefine
whether a model, download, route, or runtime is authoritative.

## Local Intent Domain

The core intent service accepts transport-independent model requirements and
returns typed observations. Its resolver, immutable acquisition plan, durable
consumer declarations and reconciliation share the existing library owner.
`PumasApi::intent()` and `PumasLocalClient::intent()` provide the native and local
IPC views; seven `intent_` JSON-RPC methods project the same service through the
existing loopback server. Transport handlers decode and project outcomes without
owning another download or declaration lifecycle.

Get may admit acquisition when explicitly allowed. Ensure commits retention
before convergence; release removes one declaration generation without deleting
files or cancelling downloads. Consumer disconnection does not revoke ownership
of admitted work. The primary drains local effects and downloads during composed
shutdown. See the [core contract](../rust/crates/pumas-core/README.md#local-intent-interface)
for availability evidence, restart, retry, deletion and downgrade limits.

Existing operational consumers remain supported. The desktop bridge and UniFFI
do not expose the intent methods. Nodes, fleet management, remote discovery and
additional discovery networks are later scope.

## Storage

Important launcher-root paths are:

| Path | Purpose |
| --- | --- |
| `shared-resources/models/` | Canonical model packages |
| `shared-resources/models/models.db` | SQLite model index and durable update feed |
| `shared-resources/cache/search.sqlite` | Hugging Face search cache |
| `launcher-data/cache/` | Download and runtime cache state |
| `launcher-data/metadata/` | Launcher metadata |
| `launcher-data/plugins/` | Source/runtime plugin descriptors |
| `<app>-versions/` | Managed external runtime installations |

Import, download, reconciliation, migration, and deletion must keep filesystem,
metadata, index, and event history consistent. Recovery outcomes must remain
distinguishable from normal success.

A model's category (what it is used for, such as `llm`, `decision`, `video` or
`embedding`) is metadata. It groups models for display and never decides where
a model is stored or what its ID is; only a family change moves a model. Core
owns detection and the person's explicit choice (`set_model_category`), which
later re-detection keeps. See the
[core guide](../rust/crates/pumas-core/README.md#model-categories).

The download store is schema 7. Older supported stores need the explicit,
one-way offline upgrade in [Opening an older library](library-upgrade.md);
ordinary startup never converts them.

## Desktop Boundary

The renderer accesses only `window.electronAPI`; it has no direct Node access.
Electron uses `contextIsolation`, renderer sandboxing, and an RPC method
allowlist. The executable method/request registry is
`electron/src/rpc-method-registry.ts`; `electron/src/ipc-validation.ts` applies
it before main-process dispatch.

Rust method behavior is owned by `rust/crates/pumas-rpc/src/handlers/` and core
services. TypeScript interfaces are consumer projections, not runtime proof.
New or changed operations must update the receiver decoder, producer, consumer,
and negative contract evidence together.

The RPC server accepts loopback binding only. There is no supported
`--allow-lan` mode; standalone deployment does not broaden exposure.

## Updates and Cached State

The backend exposes server-sent update streams for model-library, download,
runtime-profile, serving, and status-telemetry changes. Events are invalidation
or snapshot coordination signals; consumers must recover from missed events
using the corresponding snapshot/feed contract.

The frontend may display a decoded startup snapshot while the backend loads,
but freshness, provenance, and degraded refresh outcomes must be explicit.

## Inference Provider Model

Optional inference support distinguishes:

- app/plugin identity;
- runtime provider behavior;
- persisted runtime profiles;
- provider-scoped model routes;
- serving adapters and backend-owned served instances; and
- gateway endpoint capabilities.

Ollama and llama.cpp use external processes, ONNX Runtime is hosted in the Rust
process, and Torch uses the Python sidecar. The sidecar code ships inside
Pumas and runs against the interpreter and packages of an installed Torch
runtime, so every runtime runs the sidecar of the running Pumas
([layout](../torch-server/README.md#sidecar-and-runtime-layout)).

A managed runtime profile may pin an installed runtime version
(`RuntimeProfileConfig.runtime_version`); without one it follows the runtime's
active version at each launch. Each profile instance is its own process, so
profiles pinned to different versions of the same runtime run at once. A
running instance keeps the version it started from: it is reported in
`RuntimeProfileStatus.runtime_version` and recorded in a
`<pid file>.version` sidecar. Selecting a different active or default
version therefore affects new launches only. The version manager refuses to
remove a version while an instance that recorded it (or recorded no version)
is alive, and the refusal names those profiles. The `/v1` gateway is the public
facade: external clients target it, never provider-internal endpoints.
Provider processes expose private protocols (versioned handshakes,
capability advertisements, and internal request shapes) that the gateway
adapts into the public contracts; compatibility between the two sides is
established inside Pumas, not by clients. Every admitted generation uses the
one shared generation lifetime: connection-bounded but duration-unbounded
transport with no total, read, idle, or elapsed deadline, as defined in
[generation lifetime](contracts/generation-lifetime.md).

The legacy `ProcessManager` (`stop_ollama`/`stop_torch`) stops only processes
whose custody it can prove; it never searches the process table by command
line, so a user's own `ollama serve` or another project's `serve.py` is never
signalled. Custody is either the actual `Child` this manager still holds or a
PID record whose identity matches the live process. Each launch writes its PID
file plus a `<pid file>.identity` sidecar with the boot id, start time and
executable (Linux reads `/proc`; macOS and Windows use seconds-resolution start
time and executable path, a documented weaker check). Every signal is preceded
by an identity check, and a process group is signalled only while the verified
process still leads it. A PID file without an identity (for example one written
by an older Pumas), a reused PID, or a record from before the current boot is
never signalled: the stop reports it as skipped with a reason
(`stop_ollama_report`/`stop_torch_report`) and discards the stale file. The
`stop_ollama`/`stop_torch` RPC outcomes carry the same skips as an optional
`skipped: [{pid?, reason}]` list (the PID file path stays server-side), and the
desktop shows the reasons when a stop fails. The app-manager
`VersionLauncher::stop_version`/`is_version_running` and the Torch removal guard
read the same identity records.
Runtime-profile processes belong to the runtime-profile owner and are never
touched by the legacy stop.

Both the legacy launcher and the runtime-profile owner start their children
from an empty environment plus an allowlist (`platform::child_environment`):
process basics, locale and temporary directories, GPU selection and driver
discovery, proxy/TLS trust, and `OLLAMA_*` settings. Loader and interpreter
injection (`LD_PRELOAD`, `DYLD_*`, the inherited `LD_LIBRARY_PATH`,
`PYTHON*`) and credentials such as `HF_TOKEN` are not inherited; a launch that
needs one of them sets it explicitly in its launch configuration.

On Linux the manager retains each launched runtime's `Child` in one shared
observation/stop slot. Its observer uses non-reaping exit checks, and both the
observer and the stop lock the same slot. An owned group leader is reaped only
after a complete observation confirms no live group members remain. The
observer backs off on repeated observation failures and gives up after a
bounded number, leaving the child in custody for an explicit stop.

The durable provider decision is recorded in
[ADR 0001](adr/0001-onnx-runtime-provider-model.md); the Torch image
boundary split is recorded in
[ADR 0002](adr/0002-torch-image-provider-protocol.md).

## Build Variants

The default `pumas-rpc` build enables inference plugins. The library-only
desktop variant combines `pumas-rpc --no-default-features` with the frontend
`library-only` Vite mode. `PUMAS_INFERENCE_PLUGINS=false` selects that pair in
the shared launcher. Releases ship both variants (see [Releasing](../RELEASING.md)).

The [Rust guide](../rust/README.md#features-and-variants) owns the Cargo feature
facts, including why `pumas-library --no-default-features` is not a minimal
build.

## Known Boundary Work

The [current-standards remediation issues](plans/current-standards-remediation-2026-09-03/issues.md)
track gaps in RPC authentication/decoding, persistence atomicity and migration,
async shutdown, cached-state provenance, release evidence, accessibility, and
binding support. Those findings supersede optimistic claims in removed plans or
older documentation.
