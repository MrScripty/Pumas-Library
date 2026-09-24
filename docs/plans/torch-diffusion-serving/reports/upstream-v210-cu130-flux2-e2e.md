# Real upstream Torch 2.10 CUDA FLUX.2 and Tuldok trial

Date: 2026-09-23 America/Vancouver (2026-09-24 UTC). Pumas source commit:
`28c4ae1fa2839ade4cecb9a0da34cedfa863e000` on
`work/torch-version-management`. Tuldok source commit:
`a61daeec83779868cf03b14fb5c811fcdc608fc2`.

This is a real GPU and Tuldok browser result using the current Pumas RPC binary.
Pumas install, selection, startup trial, model load, and cleanup were invoked by
RPC. The Pumas desktop UI was not exercised in this run. An isolated launcher
root under `launcher-data/cache/torch-qualification/v210-e2e-root/` kept the
main library's Torch state unchanged. Its model directory linked to the existing
local model assets; Pumas indexed 89 real models from that directory.

## Exact environment and installation

- Upstream stable `v2.10.0` was previewed without a Pumas recipe and installed
  from 66 resolved, hash-locked binary artifacts. The resolved Torch wheel was
  `torch-2.10.0+cu130` for CPython 3.12 Linux x86_64, SHA-256
  `858f0cbcc78d726fea9499eb3464faa98392fa093845a3262209bd226b7844d6`.
  Its URL and every other artifact URL/hash are retained in the isolated
  runtime's `resolution.json` and `requirements.txt`.
- The chosen optional adapter was FLUX.2: Diffusers 0.37.0, Transformers
  4.57.6, Accelerate 1.12.0, and PEFT 0.18.1. The host used Python 3.12.3,
  an NVIDIA GeForce RTX 5090 Laptop GPU with 24,463 MiB, and driver 595.84.
  No interpreter provisioning or source build occurred.
- The managed installer published exactly one `v2.10.0` runtime without
  selecting it or setting a default. Install probes passed exact Torch import
  (`2.10.0+cu130`), CPU and CUDA tensor operations, sidecar app health/protocol
  3, and NVIDIA sm_120 visibility. FLUX.2 imports passed; model execution was
  correctly reported as inconclusive at install time.
- Explicit selection succeeded. The managed profile `torch-image-acceptance`
  passed its separate owned-listener, `/health`, and protocol-3 startup trial,
  advertising `image_generation`. It was generation `1` in this process.

## Real model and Tuldok result

Pumas loaded library model
`diffusion/black-forest-labs/flux_2-klein-9b-kv-fp8` through the managed
Torch profile and advertised it at `http://127.0.0.1:18778/v1`. The checkpoint
`flux-2-klein-9b-kv-fp8.safetensors` had SHA-256
`33f7da5625a00798349a719742999d3c7dd20c1a7eda14663922c363640728f1`.
The serving RPC returned `loaded: true` and one loaded model.

The actual Tuldok headless browser discovered this model from Pumas, sent the
red-teapot/yellow-lemon/blue-table prompt at **1280×720**, seed 42, displayed
the generated PNG at 1280×720, saved its bytes, and automatically added the
image to its temporary collection. Browser elapsed time was 20.732 seconds;
generation metadata reported 20.212 seconds, four steps and guidance 1. The
saved PNG is decodable at 1280×720, visually matches the prompt, and has
SHA-256 `54d5cf36b9156b4bad71b4702a42cb61eea6beaff1200a227b855390deb00555`.
Evidence is in `v210-e2e-root/tuldok-flux2-real/{result.json,saved.png,display.png}`.

FLUX.2 then unloaded, leaving zero served models. The exact managed sidecar
generation stopped. After an isolated Pumas restart, `v2.10.0` remained
installed and selected, the default remained unset, and no model was served.
A fresh startup trial passed after restart and was stopped. Both test Pumas
servers were terminated. The main library still had its prior installed
`torch-runtime-0.1.1`–`0.1.4` set, selected `torch-runtime-0.1.4`, and unset
default.

This result demonstrates one concrete non-2.9.1 install, startup, real model
load, and Tuldok image inference path. It does not cover Pumas desktop UI
installation/selection, Nunchaku on Torch 2.10, other Python/build combinations,
failed trials, cancellation, or switching in the real desktop.
