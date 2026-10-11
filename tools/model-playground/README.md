# Pumas model playground

A local developer workbench for observing a running Pumas gateway and trying one
finite model operation at a time. It lives outside the Rust, desktop and Python
runtime owners. It neither installs nor starts inference providers, acquires
models, serves/unserves models, nor addresses private plugin endpoints.

## Run

Use the repository's Node 24 toolchain (`.node-version`). No package installation,
frontend build, model weights or API subscription is needed for the playground.

```bash
node tools/model-playground/server.mjs --backend http://127.0.0.1:8080
# Open http://127.0.0.1:8787 in a browser.
```

Choose the **public gateway** address of an already running, authorized Pumas
instance. The default UI port can be changed with `--port 8788`. The backend must
be a numeric loopback HTTP base URL, including `http://[::1]:8080` where available.
There is no remote endpoint mode. Optional `PUMAS_PLAYGROUND_TOKEN` supplies a
Bearer header from the server environment; it is never sent to the browser or
included in receipts. Current local Pumas HTTP does not require this header.

1. Select **Discover runtime** to observe build identity, served aliases and
   enabled plugin manifests. An empty model list is distinct from failed discovery.
2. Select a model, optionally give its exact profile to disambiguate it, then
   **Inspect capabilities**. Inspection returns the exact profile used by requests.
3. Choose an available capability, edit its public JSON request and **Run request**.
   Every click gets a new correlation ID. IDs provide no replay/idempotency promise.
4. Inspect the response and redacted HTTP receipts. Download receipts explicitly
   if needed. Generated PNGs can be downloaded separately.

Unsupported discovery returns an error and keeps inference disabled. This tool
requires the public discovery/fencing and typed v1 operation surfaces in published
Pumas source; the v0.7.0 release alone is not sufficient. It does not guess support
from a model name, file format, plugin installation, or legacy `/v1/models`
capability hints.

## Capability and coverage meanings

Each card has two dimensions: **supported / unavailable / untested** describes the
latest runtime observation and reviewed adapter; **untested / succeeded / error /
unknown** describes requests in this browser session and exact runtime generation,
model and profile. A supported card is not a real-model qualification claim.
Inspection is refreshed before every run; Pumas still owns final live admission.

| Modality | Playground adapter | Runtime requirement |
| --- | --- | --- |
| Chat / text | Finite typed messages or text; text output | `chat_generation` / `text_generation` available |
| Embeddings | Text or text batch; finite float vectors | `text_embedding` available |
| Transcription | Raw PCM16LE or PCM32FLE attachment; text output | `audio_transcription` available for the exact qualified audio owner |
| Image generation | Prompt, explicit dimensions, optional seed; PNG preview/download | `image_generation` available |
| Image to text | One PNG/JPEG plus prompt; text output | `image_to_text` available |
| Audio generation, image editing, 3D | Visible unavailable cards; no invented endpoint | No reviewed public contract at the inspected revision |
| Audio classification / future capabilities | Display descriptor and unavailability; no dispatch | Requires an implemented and reviewed adapter |

For transcription, set encoding, channels and sample rate **before** attaching a
raw `.pcm`/`.raw` file. The tool computes frames from bytes and checks the declared
length and float range. It does not decode WAV/MP3 or silently resample/relabel
audio. Pumas owns its documented conversion. For vision, the browser attaches
PNG/JPEG bytes; Pumas owns full input decoding and runtime pixel limits. Browser
PNG decoding must succeed before an image result is presented as successful.

Streaming availability and option bounds are visible in discovery receipts, but
this first harness intentionally sends only `stream: false`. Audio runtime
qualification can remain unavailable even when its descriptor or loader exists.
No test-only availability override is exposed by the launcher.

## Boundary and lifecycle

`contract.mjs` owns the bounded consumer subset, templates and decoded result
checks. `client.mjs` owns public discovery and finite operation sequencing.
`transport.mjs` owns loopback HTTP, byte bounds and receipt redaction.
`server.mjs` owns the local listener and one active invocation. `public/` owns the
browser session and native form controls. Node's HTTP/URL/JSON implementations
handle transport and serialization; there is no custom HTTP or SSE parser and no
new package dependency. A framework/build pipeline adds no needed capability for
this bounded form and JSON inspector; it can be introduced independently later.

Public producer authority was inspected at
[`ad31e391`](https://github.com/MrScripty/Pumas-Library/tree/ad31e391dcd94c204b3fcd748cc98d27b3281e65):

- [Typed operation contract](../../docs/contracts/model-operations-v1.md),
  [DTOs](../../rust/crates/pumas-rpc/src/handlers/model_operations/types.rs) and
  [live descriptors/admission](../../rust/crates/pumas-rpc/src/handlers/model_operations.rs).
- [Public HTTP identity](../../rust/crates/pumas-core/src/discovery/http.rs),
  [generation fences](../../rust/crates/pumas-rpc/src/discovery.rs) and
  [generation lifetime](../../docs/contracts/generation-lifetime.md).
- [Source plugin manifests](../../launcher-data/plugins/README.md) are inventory,
  not semantic readiness. Published audio source has evolved beyond portions of
  the narrative contract; the tool uses live descriptors instead of assuming
  every audio loader is executable.

Discovery and response DTOs accept additive fields while validating the consumed
v1 fields; unknown capabilities cannot dispatch. Requests are a closed, deliberately
smaller subset of the canonical DTOs, reserialized after validation. Full provider
semantics, image decoding, native/device custody and final admission stay in Pumas.
Future fixes can be adopted by restarting the selected backend and explicitly
refreshing discovery. The tool pins both instance and service generation headers;
stale identities are errors, never a silent rebind or inference retry.

The UI listener binds only `127.0.0.1`, checks exact Host/Origin plus browser fetch
metadata, uses a per-launch session header, and serves a fixed asset allowlist with
a restrictive CSP. This does not authenticate other native processes running as
the same local user. No CORS proxy, arbitrary RPC method, endpoint, file path,
runtime lifecycle mutation or external asset loader is exposed.

Limits: 16 listener connections, one active API invocation, no work queue,
16 MiB request, 32 MiB response, 8 MiB input file, 512 generated text tokens,
1,048,576 generated image pixels, and 30 seconds input audio. Live descriptor
bounds may be stricter. Embeddings and discovery have a 30-second observation
budget; connecting has a 10-second budget. **Generation has no total, header,
read or idle timeout.** An explicit cancel, browser disconnect or server shutdown
closes owned upstream transport. Transport loss after dispatch is **unknown**,
including embeddings. This is not proof that provider threads or GPU work stopped.
The playground cannot impose a hard GPU/RAM/compute limit or release backend
custody. There are no retries, redirects, implicit alternative endpoints or paid
inference services.

Receipts include timestamps, correlation, route, request, HTTP status, response
bytes, elapsed time and outcome. They never collect response or authorization
headers. Redaction removes known configured secrets, credential-named fields,
Bearer/HF/OpenAI token patterns and URL credentials/query strings. Binary content
is replaced with encoded length; strings/arrays/depth are bounded in receipts.
This is a diagnostic projection, not a byte-exact recording or general secret
detector. Arbitrary private prose can remain: review before sharing. The browser
holds only the latest receipt and response, uses no localStorage, and exports only
on demand; the server writes no request history. Embedding output may be long;
the receipt projection is capped while the on-screen result retains its vectors.

## Verify

Deterministic tests use test-only loopback fixtures and no model packages:

```bash
node --test --test-timeout=15000 tools/model-playground/test/*.test.mjs
```

Representative browser verification uses an explicitly installed Playwright
1.57.0 and Chromium. It installs neither:

```bash
PLAYWRIGHT_MODULE=/absolute/path/to/playwright/index.mjs \
CHROMIUM_EXECUTABLE=/absolute/path/to/chromium \
node tools/model-playground/test/browser-smoke.mjs
```

The browser test produces labeled simulator screenshots in `evidence/`. There
is no simulator option in the ordinary launcher. To observe an existing authorized
Pumas binary with an isolated empty root and registry (no inference):

```bash
PUMAS_RPC_BINARY=/absolute/path/to/pumas-rpc \
node tools/model-playground/test/real-observation-smoke.mjs
```

Supplying the two browser environment variables also captures that real runtime's
UI observation. This check launches and stops only its own disposable instance.
See [verification evidence](evidence/verification.md) for actual measured coverage,
screenshots, compatibility gaps, and the unpublished-fix dependency.
