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
claims the root and returns an error when another live process owns it.
`PumasLocalClient` explicitly connects to an existing ready owner.
`PumasReadOnlyLibrary` reads indexed state without taking lifecycle ownership.

An embedding application or standalone RPC process constructs the owner; in
desktop use, Electron supervises that RPC process. Renderer state is a projection of
backend responses and update events; local optimistic state must not redefine
whether a model, download, route, or runtime is authoritative.

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
process, and Torch uses the Python sidecar. External clients should target the
Pumas `/v1` gateway rather than provider-internal endpoints when the provider is
served through Pumas.

The durable provider decision is recorded in
[ADR 0001](adr/0001-onnx-runtime-provider-model.md).

## Build Variants

The default `pumas-rpc` build enables inference plugins. The library-only
desktop variant combines `pumas-rpc --no-default-features` with the frontend
`library-only` Vite mode. `PUMAS_INFERENCE_PLUGINS=false` selects that pair in
the shared launcher.

The `pumas-core` HF/process/GPU feature names do not currently remove their
underlying dependency or public module surfaces. Do not describe its
`--no-default-features` build as minimal until the feature contract is fixed and
verified.

## Known Boundary Work

The [current standards audit](audits/current-standards-2026-09-03/README.md)
tracks gaps in RPC authentication/decoding, persistence atomicity and migration,
async shutdown, cached-state provenance, release evidence, accessibility, and
binding support. Those findings supersede optimistic claims in removed plans or
older documentation.
