# Torch provider protocol

Private protocol between the Pumas gateway and the Torch provider process.
Not a public API: external clients must use the public
[image contract](image-generation.md). This document is the only definition
of the provider side of the seam.

## Purpose

Carry one bounded image request from the gateway to a ready Torch slot and
return one PNG result, without duplicating the public contract inside the
provider process.

## Producer and consumer

- Producer: the Torch sidecar (`torch-server/`), a provider process managed
  by Pumas.
- Consumer: the Pumas `/v1` gateway (`pumas-rpc` open gateway handlers),
  which owns validation, adaptation, error mapping, timeout allowance, and
  cancellation propagation.

## Protocol version

Current protocol: `2`.

## Handshake

`GET /health` returns:

```json
{"status": "ok", "protocol": 2, "capabilities": ["image_generation"]}
```

- `status`: `"ok"` when the process is alive.
- `protocol`: exact-match integer. The gateway refuses a sidecar whose
  protocol differs from the qualified recipe.
- `capabilities`: additive list of provider abilities. Unknown entries are
  ignored by the gateway; absence of an entry only withholds that ability.

## Capability semantics

- `image_generation` advertises that the provider process can execute image
  work. The gateway publishes the public `image_generation` model capability
  only when a Torch slot for that model is ready; the provider advertisement
  alone never makes a model selectable.
- Capabilities are additive: adding a new capability must not change the
  meaning of existing fields, and clients must not treat an unrecognized
  capability as an error.

## Private generation endpoint

`POST /api/images/generate` accepts:

```json
{
  "model_id": "<provider slot identity>",
  "prompt": "A watercolor kingfisher beside a quiet stream",
  "width": 1024,
  "height": 1024,
  "seed": 12345
}
```

`model_id`, a nonblank `prompt`, `width`, and `height` are required;
`width` and `height` are positive integers with no defaults. `seed` is an
optional integer from 0 through 4,294,967,295; when omitted, the provider
selects a random 32-bit seed and reports it. Unknown fields are rejected.
The provider performs no public-shape adaptation: no `n`,
`response_format`, or `size` handling exists here.

A successful result is:

```json
{
  "png_base64": "<base64-encoded PNG>",
  "seed": 12345,
  "steps": 8,
  "guidance": 0.0,
  "memory_policy": "sequential_cpu_offload",
  "duration_seconds": 12.3
}
```

The PNG must match the requested dimensions and is limited to 8 MiB.
`steps`, `guidance`, and `memory_policy` are adapter-reported values the
gateway relays into public metadata unchanged.

## Provider error outcomes

The provider reports failures with stable codes the gateway maps to public
errors; tracebacks and filesystem paths are never forwarded:

| Provider outcome | Meaning |
| --- | --- |
| `model_unavailable` | No ready slot for `model_id`; load it in Pumas first |
| `unsupported_model` | Slot exists but does not support image generation |
| `runtime_busy` | One image operation per device; excess work rejected |
| `out_of_memory` | Insufficient GPU memory |
| `deadline_exceeded` | Provider deadline reached; work stopped |
| `cancelled` | Gateway disconnect; work stopped at a checkpoint |
| `backend_failure` | Adapter or pipeline failure |
| `invalid_backend_response` | Adapter returned an unusable image |

## Timeout and cancellation ownership

- The provider owns the 600-second generation deadline and disconnect
  cancellation: it stops work at a supported denoising checkpoint and
  retains the device lease until its worker and CUDA work have actually
  stopped.
- The gateway owns the 615-second transport allowance and maps provider
  outcomes to public errors. Gateway disconnect propagates as provider
  cancellation.
- Clients must not automatically retry after an uncertain result.

## Compatibility policy

- `protocol` requires an exact match between sidecar and qualified recipe;
  mismatch fails validation before any model loads.
- Capability entries are additive and optional.
- Request and result shapes are closed: unknown fields are rejected, and any
  shape change requires a protocol bump, never silent extension.

## Runtime recipe relationship

The runtime recipe (`torch-server/runtime/runtime.json`, `recipe_id`
`torch-runtime-0.1.5`) pins both the validated dependency set (CPython 3.12,
Linux x86_64, sm_120 CUDA) and the wire shape (`"protocol": 2`). Both must
agree: `validate_runtime.py` fails qualification when the sidecar handshake
protocol differs from its recipe.

## Verification locations

- Sidecar request shape: `torch-server/tests/test_image_request_contract.py`
- Sidecar failure mapping: `torch-server/tests_native/test_image_failures.py`
- Gateway adaptation and error mapping:
  `rust/crates/pumas-rpc/src/handlers/openai_gateway.rs`,
  `openai_gateway_images.rs`, and `openai_gateway_tests.rs`
- Protocol/recipe agreement: `torch-server/validate_runtime.py`
- Real GPU and browser evidence: `docs/plans/torch-diffusion-serving/reports/`
  (`nunchaku.md`, `flux2.md`, `tuldok.md`, `release-acceptance.md`)
