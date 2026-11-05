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

The preload exposes the whole desktop bridge to any document it runs in, so
`src/window-security.ts` keeps every renderer on the bundled application
(installed for each `web-contents-created`, not only the main window):

- child windows are always denied; an http/https URL is handed to the system
  browser after `validateExternalUrl`, anything else is refused with a logged
  reason;
- `will-navigate` and `will-redirect` are denied unless the target is the
  bundled entry document (or, unpackaged with `--dev`, the dev-server origin);
  webview attachment is denied;
- browser permissions and device access are denied, except
  `clipboard-sanitized-write` for the application document (copy buttons);
- every `ipcMain` handler checks that the sender is the main window's top frame
  showing an application document; a refused `api:call` never reaches the
  backend.

The renderer opens external URLs only through `ExternalLink` /
`openExternalUrl` (`frontend/src/components/ui/`), which call `api.open_url`.
Frontend lint forbids `target` on links and `window.open`.

`--dev` (dev server) and `--debug` (DevTools, debug sidecar logging) apply only
to unpackaged builds. Packaged builds disable DevTools and install a menu
without DevTools or reload accelerators.

TypeScript interfaces are compile-time projections, not validation of runtime
messages. A contract change must update the receiver decoder, producer,
consumer, and negative tests together. Treat sidecar logs as sensitive: they
can contain backend diagnostics and must not expose credentials. Electron logs
sidecar stdout/stderr at `debug`, below the persisted file level (`info`), and
an unparseable RPC response is reported by method, HTTP status and size, never
by its body.

### Backend setup observation

`start_backend_setup(backend, expectedPreviousOperationId?)` and
`get_backend_setup(backend)` expose the retained Rust setup owner for
`python_conversion`, `llama_cpp`, `nvfp4`, `fp8` and `sherry`, independently of inference
plugin support. Preload and main IPC validate requests with the Rust-generated
decoders; preload validates the existing redacted setup outcomes. Neither layer
installs, retries or falls back automatically. A missing/unsupported RPC method
remains an error, not a request to use blocking setup instead.

Keep the selected backend alongside its snapshot; JSON-RPC request correlation
associates the response with that selection. Null setup is an owner-local absence,
not a readiness result. Omitted/null retry tokens attach without retrying; only a
matching terminal operation ID can admit a successor. Reads never install and
remain available after setup shutdown. IDs do not survive owner/process restart.
Core excludes setup and managed conversions at the same stable root; contention
becomes a failed operation, without queuing or automatic retry. Direct backend
calls, independent readiness probes and external tools remain caller-coordinated
(see the [core contract](../rust/crates/pumas-core/README.md)). This bridge
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
deserialization alias; methods still dispatched through the `Legacy` command
remain outside the generated contract.

`pnpm --dir electron run generate:desktop-contract` rebuilds that exporter and
generates identical `src/generated/desktop-contract*` artifacts in Electron
and frontend. `check:desktop-contract` regenerates from current Rust source
and rejects any stale artifact; CI runs that command. Normal UI builds do not
invoke Cargo or fetch generator dependencies.

Payloads that Electron owns (the launcher-root startup state, selection and
library-upgrade payloads, the model-import selection and update-stream errors)
are declared in `electron/contracts/*.schema.json`. The generator
merges them with the Rust export, so main, preload and the frontend share one
generated type and validator; a name declared by both sources is an error.
These declarations ship with their consumers and stay closed.

The response and stream validators generated from the Rust export are closed
as well (`additionalProperties: false`): the desktop and its backend ship and
upgrade as one unit. A packaged desktop ignores `PUMAS_RPC_BINARY` and always
runs the `pumas-rpc` in its own resources (`src/backend-path.ts`), so a newer
backend is never paired with older validators; the override remains for
source checkouts (`./launcher.sh`). An additive backend field reaches the
desktop only after regeneration. The "additive changes are non-breaking" rule
applies to external RPC consumers, whose clients ignore unknown fields; it
does not apply to this bundled pairing.

Update streams (model library, downloads, runtime profiles, serving status and
status telemetry) are declared by the backend and decoded in preload. Each
subscription takes a required `onError`: a payload that fails its contract is
never dropped silently. Its reason, naming the failed rule and location, goes
to `onError`, and the renderer reloads from the RPC snapshot. Decoders never
put a field's value in a rejection reason. Transport loss reaches the same
`onError`: main forwards each stream's "ended"/"failed" outcome on
`<stream>:error` (an `UpdateStreamError`), and a refused subscription is
reported there too.

The backend's startup diagnostic is the exception to closed declarations. Rust
writes one `PUMAS_STARTUP_FAILURE=` line holding a `StartupFailureFrame`
(`pumas-rpc/src/startup_failure.rs`) for every fatal startup error, with a
bounded `detail`. An unpackaged Electron may run a backend from another build, so the frame's
schema is open: readers ignore unknown fields and treat an unknown `reason` as
`backend-unavailable`. The detail reaches the recovery screen as the startup
state's `detail`.

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
Schema semantics. Generated wrappers return copied, frozen values, or a
diagnostic naming the failed rule and its location. Preload is bundled with esbuild while retaining Electron's
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

A ready backend is health-checked every 30 seconds. Three consecutive failed
checks mark it hung: Electron sends SIGTERM (SIGKILL after the force-wait on
Unix; Windows terminates directly), and the exit takes the normal crash
restart path, bounded by `maxRestarts`. Making `pumas-rpc` exit when Electron
itself dies is the backend's job (stdin EOF / parent-death signal).

The RPC server binds loopback only and has no LAN mode; see
[Security](../docs/SECURITY.md) for its Host/Origin admission checks.

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
`electron/resources/bin/`. Packages carry the compiled JavaScript only:
`dist/**/*.map` and `dist/**/*.d.ts` are excluded. CI packages with
`CSC_IDENTITY_AUTO_DISCOVERY=false`, so installers are unsigned and the
config declares no signing identity. Do not treat a successful package assembly as proof
that the application runs on that platform. See [Releasing](../RELEASING.md)
and the [current standards audit](../docs/audits/current-standards-2026-09-03/README.md).
