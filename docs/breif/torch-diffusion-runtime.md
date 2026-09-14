# Torch Diffusion Serving Brief

**Status:** Implementation active. Real Nunchaku and FLUX.2/FP8 generation, Tuldok display/save, and general Pumas FP8/NVFP4 Safetensors conversion have passed within their recorded verification scope. NVFP4 conversion is available; the current FLUX encoder runtime continues using FP8. Shared public runtime distribution acceptance remains pending.

**Implementation plan:** [Torch diffusion serving](../plans/torch-diffusion-serving/plan.md)

## Intended outcome

A user in Tuldok configures the Pumas endpoint, selects a served image-generation
model, enters a prompt, and receives an image that Tuldok displays and can save
for synthetic-dataset work. Installation and serving are managed through Pumas.
The existing VLM corner-detection workflow remains available as a separate task.

The first supported path is Nunchaku Z-Image-Turbo, followed by the downloaded
FLUX.2 Klein model. Success requires the actual Tuldok workflow against a release
build, not only a direct request to the Torch sidecar.

## Client contract

Use the Pumas gateway URL, not the llama.cpp router URL. Extend the existing
model-discovery response with image-generation capabilities and serve requests
at `POST /v1/images/generations`. The initial request supplies a model ID, prompt,
size, and optionally a seed; the response contains one base64-encoded PNG.
Tuldok decodes and displays it, with actionable loading, generation, and failure
states. Document the precise supported request and response fields during
implementation; do not imply full compatibility with every image API option.

Initially the user loads the model in Pumas before selecting it in Tuldok.
Preserve the gateway's loaded-model discovery policy. Broad on-demand loading
and catalog redesign remain outside this effort, as described in the separate
[unified gateway brief](unified-inference-gateway.md).

## Shared runtime management

Torch must use the existing runtime version-management system used by llama.cpp.
Release discovery, download progress and cancellation, installed-version state,
active-version selection, removal, and update UI belong to that shared system.
Do not introduce a second Torch updater or separate installation-state store.

Backend-specific installation strategies remain appropriate: llama.cpp installs
native artifacts, while Torch installs a managed Python environment and compatible
wheels. Extend the shared manager where necessary rather than duplicating its
lifecycle orchestration.

The existing Torch entry points already use `VersionManager`, but their
`pytorch/pytorch` source-release installation recipe does not supply a complete
Pumas serving runtime. Replace that recipe with a versioned compatibility manifest
covering the Pumas sidecar, Python, PyTorch/CUDA, Diffusers, Nunchaku, and their
dependencies. Resolve platform and GPU compatibility before installation. Keep
the environment isolated from system Python.

Install into staging, validate the environment and sidecar startup, then publish
the installed version through the shared manager. A failed or cancelled update
must preserve the previously usable version. Activation must respect running
inference rather than replacing its environment underneath it.

Model weights, text encoders, VAEs, and tokenizers remain library-managed model
assets. The runtime manifest specifies supported formats and compatibility;
runtime updates must not become a second model-download system.

## Optional inference build

Use the existing `pumas-rpc` Cargo feature `inference-plugins`, the same gate as
llama.cpp. Do not add an independently enabled Torch feature that bypasses it.
Torch management, process startup, RPC and image-serving routes must be absent
when this gate is disabled. Corresponding frontend controls must follow the
existing launcher build setting `PUMAS_INFERENCE_PLUGINS=false`.

PyTorch executes in the managed external runtime; its wheels and model weights
are not embedded into the Rust executable. A library-only build must not install
or start that runtime as a side effect. This requirement concerns optional
llama.cpp/Torch serving integration; it does not assert that all existing core
inference dependencies have already been removed.

## Serving scope

Prioritize Nunchaku Z-Image-Turbo, then the local FLUX.2 Klein checkpoint.
LLaDA-Image-Turbo-FP8 is deferred. Each adapter must validate its exact checkpoint
format and required pipeline assets before offering a model for serving.

Integrate Torch with Pumas model discovery and serving lifecycle and provide an
image-generation endpoint. Serialize GPU work initially, support cancellation,
protect active jobs from unloading, and expose any CPU-offload policy explicitly.
Use Pumas hardware telemetry for compatibility and memory decisions. The current
machine has an RTX 5090 Laptop GPU with approximately 24 GB VRAM; successful
installation alone does not establish that an entire pipeline fits in VRAM.

## Acceptance evidence needed

- Shared UI discovers, installs, activates, updates, and removes a Torch runtime.
- Failed and cancelled installations leave the previous runtime usable.
- Compatibility checks reject unsupported wheel/platform combinations before
  publishing an installation.
- Tuldok sends a prompt to a complete local Nunchaku model through Pumas and
  displays and saves the returned image,
  with measured latency, peak memory, and cancellation behavior.
- FLUX.2 loads through an adapter verified against its actual checkpoint format.
- Enabled and disabled inference builds pass their relevant checks; the disabled
  build exposes neither Torch serving routes nor installation/startup actions.

Existing baseline verification: `cargo check --manifest-path rust/Cargo.toml
-p pumas-rpc --no-default-features --offline` passed on 2026-09-13. This verifies
the current disabled-feature build, not the proposed diffusion implementation.
