# Pumas image generation

This is the public, Pumas-owned image contract. Pumas owns
`POST /v1/images/generations` and all validation, adaptation, error mapping,
timeout, and cancellation behavior described here. Provider internals execute
behind the gateway and are not part of this contract; they are covered by the
private [Torch provider protocol](torch-provider-protocol.md).

External clients must target this gateway contract. Compatibility with any
provider-internal endpoint is established inside Pumas (gateway adaptation
plus provider handshake), never by clients addressing the provider directly.

This contract is implemented behind `inference-plugins`. Nunchaku and FLUX.2 Klein with a locally converted FP8 Qwen3-8B encoder have
passed the real GPU and Tuldok display/save workflow.
Only the concrete adapters below are supported.

Use the Pumas gateway address reported by the running RPC server. The llama.cpp
router and the internal Torch listener are different endpoints.

`GET /v1/models` lists served models. Image models add
`"capabilities": ["image_generation"]` only when their Torch slot is ready.
Existing text entries retain their response shape. Clients should offer only
models with that capability for image generation.

`POST /v1/images/generations` accepts a JSON object:

```json
{
  "model": "<ID returned by /v1/models>",
  "prompt": "A watercolor kingfisher beside a quiet stream",
  "n": 1,
  "width": 1024,
  "height": 1024,
  "response_format": "b64_json",
  "seed": 12345
}
```

`model`, a nonblank `prompt`, `width`, and `height` are required (`model` 256
and `prompt` 4,000 characters maximum; dimensions required positive integers
with no defaults). Other fields use the illustrated defaults, except omitted
`seed`, which is random. Seed accepts integers from 0 through 4,294,967,295.
Dimensions are passed to the backend unchanged; the backend rejects resolutions
it cannot produce with its normal backend error. There is no allowlist and no
universal resolution support. The former `size` string is rejected. Unknown
fields, batches, and URL responses are rejected before backend admission.
Request bodies are limited to 32 KiB.

The response contains one PNG:

```json
{
  "created": 1789340000,
  "data": [{"b64_json": "<base64-encoded PNG>"}],
  "metadata": {
    "seed": 12345,
    "steps": 8,
    "guidance": 0.0,
    "memory_policy": "sequential_cpu_offload",
    "duration_seconds": 12.3
  }
}
```

The example duration is illustrative, not a performance measurement. PNG output
is limited to 8 MiB and the gateway JSON response to 12 MiB. Nunchaku uses eight
steps and guidance zero. Klein 9B KV uses four steps and guidance one, reporting
`scaled_fp8_to_bf16_sequential_cpu_offload`: checkpoint scales are applied before
BF16 execution with CPU offload. Seed and settings aid reproducibility but do not promise
bitwise deterministic GPU output.

The runtime admits one image operation per device and rejects additional work as
busy. Unload is refused while that device is busy. A disconnected or expired
operation requests cancellation at a denoising checkpoint and retains the device
lease until its worker and CUDA work stop. The runtime deadline is 600 seconds;
the gateway transport allowance is 615 seconds. Clients must not automatically
retry after an uncertain result. A controlled TCP integration test verifies gateway disconnect propagation;
real GPU cancellation acceptance is still pending.

Image errors use `error.code` and a safe `error.message`; known codes include
`invalid_request`, `runtime_busy`, `model_unavailable`, `unsupported_model`,
`out_of_memory`, `deadline_exceeded`, `cancelled`, `backend_failure`, and
`invalid_backend_response`. Existing gateway model-lookup errors retain their
existing shape. Backend paths and tracebacks are not forwarded.

The initial adapter resolves the Nunchaku FP4 rank-128 checkpoint and exactly one
library-managed `Tongyi-MAI/Z-Image-Turbo` bundle. Missing or ambiguous components
prevent loading. Runtime loading runs offline; acquisition belongs to the Pumas
library API. The managed profile owns process readiness and the serving operation
owns publication of the loaded model.

The Klein adapter uses the local `flux-2-klein-9b-kv-fp8.safetensors` checkpoint,
one library-managed `Qwen/Qwen3-8B` package and the standalone
`split_files/vae/flux2-vae.safetensors` from `Comfy-Org/flux2-dev`. It requires
42 GiB available system RAM according to Pumas telemetry. Acquire these components
through Pumas; the adapter neither downloads them nor upgrades its dependencies.
