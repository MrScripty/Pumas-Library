# Torch provider protocol

Private protocol between the Pumas gateway and the Torch provider process.
Not a public API: external clients must use the public
[image contract](image-generation.md). This document is the only definition
of the provider side of the seam.

## Purpose

Carry one bounded image request from the gateway to a ready Torch slot and
return one PNG result, without duplicating the public contract inside the
provider process.

## Producers and consumers

- Request producer: Rust `TorchClient` in `pumas-app-manager`; request consumer:
  the Torch sidecar (`torch-server/`).
- Handshake, private result, and provider-failure producer: the Torch sidecar;
  consumer: `TorchClient` and the Pumas `/v1` gateway.
- Public response/error producer: the Pumas gateway; consumer: the external
  application. The gateway owns public validation, adaptation, safe error
  projection, generation transport policy, and disconnect propagation.

## Protocol version

Current protocol: `3`.

## Handshake

`GET /health` returns:

```json
{"status": "ok", "protocol": 3, "capabilities": ["image_generation"]}
```

- `status`: `"ok"` when the process is alive.
- `protocol`: exact-match integer. The gateway refuses a sidecar whose
  protocol differs from the qualified recipe.
- `capabilities`: additive list of provider abilities. Unknown entries are
  ignored by the gateway; absence of an entry only withholds that ability.

## Authentication

Every route except `GET /health` requires the header
`x-pumas-torch-token: <token>` (a bearer `Authorization` header is also
accepted). The Pumas launcher generates the token once per backend process,
never persists it, and passes it to the sidecar only through
`PUMAS_TORCH_API_TOKEN` in the environment, never argv. Every sidecar launched
by that backend process shares it, so a sidecar left running by an earlier
backend refuses the new one. A missing or wrong token is refused with
`401` and `detail: {"code": "unauthorized", "message": ...}`. `TorchClient`
sends the token on every request, never follows redirects, and bypasses
proxies for loopback sidecars.

The launcher also sets `PUMAS_TORCH_MODEL_ROOTS` (the library root plus the
directory of each external-reference model known at launch; load paths
outside them are refused, and a model imported by external reference after
launch needs a Torch restart) and `PUMAS_TORCH_PARENT_PID` (the sidecar exits
once that process is gone).

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

Dimensions are admitted before any generation, under the device lease. A
request above the adapter's supported maximum is refused with
`dimensions_exceed_limit`, whose detail also carries `max_pixels`. A request
whose pixels exceed the budget derived from free GPU memory
(`torch.cuda.mem_get_info`) and the adapter's per-pixel memory estimate is
refused with `out_of_memory`.

The PNG must match the requested dimensions and is limited to 8 MiB.
`steps`, `guidance`, and `memory_policy` are adapter-reported values the
gateway relays into public metadata unchanged. These six canonical fields are
required. Unknown additive private result fields are accepted within the
12 MiB whole-response bound, but the gateway never projects them publicly.

## Provider error outcomes

The provider reports failures as `detail: {"code": ..., "message": ...}`,
with additive typed facts where noted, on the image route and on the `/api`
control and `/v1` text routes alike. The codes are stable and the gateway maps
them to public errors; tracebacks, library error text and filesystem paths are
never forwarded:

| Provider outcome | Meaning |
| --- | --- |
| `model_unavailable` | No ready slot for `model_id`; load it in Pumas first |
| `unsupported_model` | Slot exists but does not support image generation |
| `runtime_busy` | One image operation per device; excess work rejected |
| `dimensions_exceed_limit` | Above the adapter's maximum, refused at admission; `max_pixels` names the limit (public 400 with the same code) |
| `out_of_memory` | Insufficient GPU memory, at admission (budget from free memory) or during generation |
| `cancelled` | Gateway disconnect; work stopped at a checkpoint |
| `backend_failure` | Adapter or pipeline failure |
| `invalid_backend_response` | Adapter returned an unusable image |

## Lifetime and cancellation ownership

- Admitted generation has no total, response-read, idle, or elapsed-duration
  deadline. Connection establishment remains independently bounded. The shared
  rule is defined by [generation-lifetime.md](generation-lifetime.md).
- The provider owns execution and disconnect cancellation: it requests a stop
  at supported denoising checkpoints and retains worker/device custody through
  cleanup. Diagnostics distinguish cancellation requested, cleanup pending,
  cancellation completed, normal completion, and runtime failure.
- Dropping the gateway request or provider connection requests cancellation but
  does not prove compute stopped. A lost response is an unknown outcome and is
  never replayed automatically.

## Compatibility policy

- `protocol` requires an exact match between sidecar and qualified recipe;
  mismatch fails validation before any model loads.
- Capability entries are additive and optional.
- Protocol stays `3` across additive changes: the authentication header, the
  typed `{code, message}` detail and the `dimensions_exceed_limit` code. An
  older client decodes an unknown code as `backend_failure`.
- Requests are closed and reject unknown fields. Results require the canonical
  fields but permit bounded additive fields; changing required semantics still
  requires a protocol bump.

## Runtime recipe relationship

The sidecar's `/health` reports protocol `3` and the `image_generation`
capability. `TorchClient` in `pumas-app-manager` admits only that protocol and
requires the capability before it routes image requests. No runtime recipe
pins a dependency set: each install records its resolved packages, and
`torch-server/runtime/verified-runtimes.json` records verified combinations as
evidence only.

## Verification locations

- Sidecar request shape: `torch-server/tests/test_image_request_contract.py`
- Sidecar failure mapping: `torch-server/tests_native/test_image_failures.py`
- Gateway adaptation and error mapping:
  `rust/crates/pumas-rpc/src/handlers/openai_gateway.rs`,
  `openai_gateway_images.rs`, and `openai_gateway_tests.rs`
- Protocol agreement: `rust/crates/pumas-app-manager/src/torch_client.rs`
  handshake tests and `torch-server/tests/test_image_provider_boundary.py`
- Real GPU and browser evidence: the completed diffusion serving plan's
  reports (`nunchaku.md`, `flux2.md`, `tuldok.md`, `release-acceptance.md`),
  kept in Git history: `git show 78c52fcf:docs/plans/torch-diffusion-serving/reports/`
