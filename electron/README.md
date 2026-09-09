# Electron Desktop Shell

Electron owns the privileged desktop boundary between the React renderer and
the local Rust sidecar.

## Responsibilities

- create and secure application windows;
- expose the narrow preload API available to the renderer;
- start, monitor, and stop `pumas-rpc`;
- validate renderer IPC requests before forwarding them;
- own native dialogs and other desktop-only operations; and
- package the renderer, RPC binary, and platform icons.

Electron does not own model-library rules or durable runtime truth.

```text
React renderer
  -> preload API
    -> validated Electron IPC
      -> loopback HTTP/JSON-RPC
        -> pumas-rpc
```

## Security Boundary

Renderer windows use context isolation and sandboxing. Do not enable direct
Node access or add a generic IPC/RPC escape hatch. The executable RPC method
and request registry is `src/rpc-method-registry.ts`; `src/ipc-validation.ts`
applies it before dispatch.

TypeScript interfaces are compile-time projections, not validation of runtime
messages. A contract change must update the receiver decoder, producer,
consumer, and negative tests together. Treat sidecar logs as sensitive: they
can contain backend diagnostics and must not expose credentials.

### Backend setup observation

`start_backend_setup(backend, expectedPreviousOperationId?)` and
`get_backend_setup(backend)` expose the retained Rust setup owner for
`python_conversion`, `llama_cpp`, `nvfp4` and `sherry`, independently of inference
plugin support. Preload and main IPC validate requests with the Rust-generated
decoders; preload validates the existing redacted setup outcomes. Neither layer
installs, retries or falls back automatically. A missing/unsupported RPC method
remains an error, not a request to use blocking setup instead.

Keep the selected backend alongside its snapshot; JSON-RPC request correlation
associates the response with that selection. Null setup is an owner-local absence,
not a readiness result. Omitted/null retry tokens attach without retrying; only a
matching terminal operation ID can admit a successor. Reads never install and
remain available after setup shutdown. IDs do not survive owner/process restart.
Callers must exclude conversions and external environment use during setup;
this execution exclusion is not enforced by the installer lock. This bridge
contract does not add quantization controls or establish real tool/GPU readiness.

### Generated catalog and download contract

The selected catalog, FTS, download, and recovery declarations remain in Rust
`pumas-rpc/src/contract.rs`. The optional `export-contract` feature derives
Draft 7 schemas with Schemars 1.2.0: serialization contracts for results and
deserialization contracts for requests. Its export module selects existing
wire invariants, including UTF-8 byte limits and recovery-result correlations.
It does not confer filesystem or recovery authority.

Request projections describe the canonical desktop outbound spelling accepted
by Electron's registry and Rust. They do not promise every standalone Rust
deserialization alias; unselected `Legacy` methods remain outside this slice.

`pnpm --dir electron run generate:desktop-contract` rebuilds that exporter and
generates identical `src/generated/desktop-contract*` artifacts in Electron
and frontend. `check:desktop-contract` regenerates from current Rust source
and rejects any stale artifact; CI runs that command. Normal UI builds do not
invoke Cargo or fetch generator dependencies.

Export/check uses `cargo --offline --locked` and cannot update dependency
resolution. Provision missing locked Rust dependencies explicitly first with
`cargo fetch --locked --manifest-path rust/Cargo.toml` (as CI does).

`test:desktop-contract-conformance` uses a private temporary producer fixture
through `scripts/with-desktop-contract-fixtures.mjs`. The frontend's dedicated
`test:desktop-contract` command crosses the real bundled preload and renderer.
Both gates run in CI; selecting a conformance test without its required fixture
fails. Fixtures retain real temporary filesystem identity and are retired after
the command, rather than checking in or normalizing recovery tickets.

AJV 8.20.0 owns Draft 7 validation and compiles standalone validators. The thin
type generator rejects unsupported reachable constructs and format vocabulary.
Named Pumas refinements preserve product-specific wire consistency, not JSON
Schema semantics. Generated wrappers return typed diagnostics or copied,
frozen values. Preload is bundled with esbuild while retaining Electron's
sandbox; validators require neither runtime code generation nor Node imports.

The older cached Schemars 0.8 candidate was rejected because its default
deserialization view loses emitted-null versus omitted-field distinctions.
AJV 6 would require the archived `ajv-pack` for standalone output; AJV 8's
[maintained standalone generator](https://ajv.js.org/standalone.html) avoids
that dependency and runtime evaluation. Both tools are generation-only owned
dependencies; checked outputs are coordinated replacements, not a promise of
support for older backend cohorts.

## Sidecar and Events

Electron owns process lifecycle and forwards backend event streams to the
renderer. It must make startup, ready, timeout, exit, and shutdown outcomes
distinct. Events coordinate or invalidate cached state; they do not transfer
durable ownership into Electron.

The RPC server is loopback-only by default. Pumas RPC LAN mode is currently
unauthenticated and must not be exposed to an untrusted network.

## Commands

From the repository root:

```bash
npm run -w electron lint
npm run -w electron validate
npm run -w electron test
npm run -w electron build
npm run -w electron dev
```

The direct development command expects a built renderer and an available
sidecar. For the supported end-to-end workflow, use:

```bash
./launcher.sh --build
./launcher.sh --run
```

Packaging requires `frontend/dist` plus a matching `pumas-rpc` staged under
`electron/resources/bin/`. Do not treat a successful package assembly as proof
that the application runs on that platform. See [Releasing](../RELEASING.md)
and the [current standards audit](../docs/audits/current-standards-2026-09-03/README.md).
