# Public S3 file/package import

The public S3 flow exposes the [shared acquisition/import bridge](acquisition-model-bridge-2026-10-08.md)
through the existing desktop request, generated contract and native importer
interfaces. The original S3 brief and acquisition contract govern the boundary
between source selection, transferred bytes and qualified model publication.

## Public behavior

S3 stores arbitrary bytes. Desktop request admission validates source pins,
bounded paths, the exact selected namespace and importer-reserved destinations;
it grants neither format qualification nor backend readiness. The RPC filename
extension guard and inert-only secondary-file guard are removed. The existing
single/authenticated-single and bundle/authenticated-bundle wire methods and
credential DTOs remain intact. Existing GGUF callers retain their semantics.

An exact selected primary may now be a safetensors shard or component path such
as `unet/model.safetensors`. Primary paths retain the desktop ASCII component
alphabet (alphanumeric initial character, then alphanumeric, dot, underscore or
hyphen), with safe slash-separated components and a 1024-byte overall bound.
Shared `ArtifactFile` admission checks portable paths. Existing shared manifest
and native manifest preflight check the complete selected set, per-file version
and SHA-256 pins, aliases and collisions. Shared importer reserved-path preflight
checks both normalized single-file destinations and primary namespace roots
before any RPC workspace allocation. Bundle destinations retain their existing
whole-set reserved-root check.

Desktop bundles remain bounded to 2–32 selected files and prefix discovery to
32 objects; the wider shared importer bound is not silently exposed. All model
content checks stay in the shared importer. No S3 format parser, second publisher,
new receipt owner, inference adapter or unsafe-code authorization is introduced.
The generated desktop contract is regenerated from the Rust exporter using the
existing generator and locked official dependencies; generated files are not
hand-edited.

The dialog now labels a primary weight logical path and additional package files.
It explains selecting indexes, every shard, configs, tokenizer assets, required
processors and standard SD/SDXL components while preserving package layout.
Help text distinguishes verified bytes, qualified model registration and backend
compatibility/inference readiness. Unsupported packages and custom code are
refused after acquisition rather than made runnable by an extension change.
The default GGUF filename and public methods remain compatible with old callers.

## Actual public-flow qualification

The new `frontend/conformance/s3-public-model-import.test.tsx` requires a real
candidate RPC binary with S3. It never silently skips or substitutes a fake RPC
producer. Run after building the generated contract and Electron preload:

```sh
ORT_SKIP_DOWNLOAD=1 CARGO_INCREMENTAL=0 cargo build --locked \
  --manifest-path rust/Cargo.toml -p pumas-rpc --no-default-features \
  --features s3,export-contract
rust/target/debug/pumas-rpc --export-desktop-contract > /tmp/s3-contract.json
node electron/scripts/generate-desktop-contract.mjs --schema /tmp/s3-contract.json
pnpm --dir electron build
PUMAS_S3_PUBLIC_RPC_BIN="$PWD/rust/target/debug/pumas-rpc" \
  pnpm --dir frontend exec vitest run --config vitest.conformance.config.ts \
  conformance/s3-public-model-import.test.tsx
```

Use the actual configured Cargo target directory if it differs from `rust/target`.
The test mounts the real dialog and hooks, runs the built preload in a Node VM,
uses the built privileged IPC request validators and response decoder, and calls
the actual local RPC HTTP server and its owned S3 worker. Only Electron's IPC
event/context-bridge plumbing is supplied by the fixture. This proves those
code paths in a DOM/VM harness, not a launched Electron application or an installed
desktop package. The local HTTPS S3 fixture uses the existing owned localhost
certificate; it preserves actual source versions, sizes, byte ranges and digests.
It supplies correctly bounded, lexicographically ordered prefix listings.

Actual tiny format fixtures prove these registration paths:

- Legacy single GGUF and GGUF with an inert `model_index.json` auxiliary.
- A genuine self-contained F32 safetensors file.
- A complete two-shard Transformers selection, through actual prefix discovery,
  per-file explicit selection and authenticated bundle import.
- Standard SD and SDXL directories, through actual prefix discovery and nested
  primary component selection, including the second SDXL encoder/tokenizer.
- A processor-bearing Whisper package with selected processor configuration.

Success proves exact copied output bytes/layout, canonical import `Ready`, one
start command, selected immutable versions and a persisted Adopted acquisition
with the exact confirmed model binding. It does not prove model inference.

Refusal controls select an invalid ONNX external-initializer package, a missing shard,
missing processor, malformed safetensors, missing Diffusers component, unsupported
pipeline class and custom-code config. They prove every selected object was
transferred, an issued receipt remains in Using custody, no model binding or
library model appears, and no import-completed callback fires. These are explicit
model-package refusals after successful byte acquisition. The ONNX fixture is
assembled from the official [ModelProto/TensorProto wire schema](https://github.com/onnx/onnx/blob/main/onnx/onnx.proto3)
with an external F32 initializer but no graph inputs/nodes. It remains outside
the later [bounded ONNX structural class](acquired-onnx-package-2026-10-08.md).
This fixture does not run ORT.

A stalled safetensors GET is cancelled through the real UI/public API. The source
connection drains, no consumer receipt or model appears, and the existing owned
cancel/finalization path settles. A reserved single-file destination is refused
before source I/O and leaves zero persisted acquisitions. Fixture HTTP/readiness,
cancel observations and graceful child shutdown have explicit deadlines; only
the fixture-owned child may be force-killed after a shutdown timeout, and that
timeout fails the test while remaining cleanup still runs.

## Verification, scope and composition

Recorded public-flow qualification passed all 16 real RPC-backed flow tests,
desktop S3 unit regressions, Electron S3 contract/transport checks, RPC S3
contract tests, generated-contract consistency, type checking, scoped lint and
Rust formatting/Clippy. Cargo used `ORT_SKIP_DOWNLOAD=1` without dependency or
lockfile changes. The no-inference RPC configuration had existing dead-code
warnings; Clippy denied `clippy::all` while retaining those baseline Rust
warnings. These checks establish the described DOM/VM/RPC paths, not native
Electron acceptance.

The [ONNX structural extension](acquired-onnx-package-2026-10-08.md) separately
admits bounded static FLOAT graphs and selected external tensors. Other ONNX
representations, pickle/PT/GGML, adapters, binary SentencePiece/tiktoken
tokenizers, additional Diffusers classes and unknown formats do not acquire
model-publication authority. Generic acquisition still stores their bytes.
Structural qualification does not establish architecture correctness, runtime
deserialization, backend compatibility, meaningful inference, live-provider
behavior, process-loss recovery or cross-platform durability. The desktop
32-file and primary ASCII limits can exclude larger packages accepted by other
import paths.

Changes to `contract/s3.rs` and `contract/export.rs` must be reflected by
regenerating desktop contract artifacts from the composed Rust producer. Rerun
the public-flow target and S3/RPC/desktop contract gates after such changes.
Source selection must continue delegating package qualification to the shared
importer rather than accepting bytes based only on a filename extension.
