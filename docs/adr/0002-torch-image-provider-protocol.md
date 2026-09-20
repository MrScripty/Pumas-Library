# ADR 0002: Torch Image Provider Protocol

- Status: Accepted and implemented
- Date: 2026-09-18

## Context

The Torch sidecar duplicated the public image contract: it served
`POST /v1/images/generations` with the same `model`, `n`, `response_format`,
and `size` handling the Pumas gateway already owned. Every public-shape
change then required synchronized edits on both sides of the boundary with
no stated owner, and clients could address the provider directly, bypassing
gateway validation, error mapping, and timeout policy.

The gateway already adapts provider results into public responses
(`openai_gateway_images.rs`), and runtime qualification already pins a
sidecar/recipe pair (`validate_runtime.py` plus `runtime.json`). What was
missing was a named private protocol giving the provider side its own
versioned, capability-bearing contract.

## Decision

Split the image boundary into two contracts with separate owners:

- Public facade: `POST /v1/images/generations` as defined in
  [image-generation.md](../contracts/image-generation.md), owned by the
  Pumas gateway. The gateway owns validation, adaptation, error mapping,
  the 615-second transport allowance, and disconnect propagation.
- Private protocol: `POST /api/images/generate` as defined in
  [torch-provider-protocol.md](../contracts/torch-provider-protocol.md),
  owned by the Torch provider process. The provider owns execution, the
  600-second generation deadline, checkpoint cancellation, and the device
  lease.

The provider handshake is `{status, protocol, capabilities}` on
`GET /health`. `protocol` requires an exact match with the qualified
runtime recipe; `capabilities` are additive, with `image_generation`
advertising image ability. The gateway publishes the public model
capability only for ready Torch slots; the provider advertisement alone
never makes a model selectable.

Packaging is immutable: `torch-runtime-0.1.5` declares protocol 2, and a
sidecar whose handshake protocol differs from its recipe fails validation
before any model loads.

## Ownership

| Concern | Current owner |
| --- | --- |
| Public image contract | `docs/contracts/image-generation.md`, enforced in `rust/crates/pumas-rpc/src/handlers/openai_gateway*.rs` |
| Private provider protocol | `docs/contracts/torch-provider-protocol.md`, implemented in `torch-server/` |
| Provider handshake and capabilities | `torch-server/serve.py` (`GET /health`), consumed by `pumas-rpc` gateway handlers |
| Runtime recipe and protocol pin | `torch-server/runtime/runtime.json`, checked by `torch-server/validate_runtime.py` |
| Provider execution, deadline, lease | `torch-server/image_api.py` (`owned_generation`) and `model_manager.py` |
| Gateway transport allowance and mapping | `rust/crates/pumas-rpc/src/handlers/openai_gateway.rs` |

## Consequences

- Public-shape changes touch the gateway contract and its tests only;
  provider-shape changes touch the private protocol and its tests only.
- Adding a provider capability is additive and cannot change existing field
  meaning; removing or reshaping a field requires a protocol bump.
- The sidecar no longer serves a public-compatible `/v1` image route, so
  clients cannot bypass gateway policy by addressing the provider.
- Recipe qualification now covers wire shape as well as dependencies: a
  protocol mismatch is a validation failure, not a runtime surprise.

## Rejected Alternatives

- Keep the duplicated `/v1` route in the sidecar: this preserves the
  ownerless synchronized-edit problem and the direct-addressing bypass.
- Version only the recipe, not the wire shape: dependency identity does not
  prove request/response compatibility across sidecar changes.
- Merge both contracts into one document: a single contract cannot assign
  conflicting field ownership (`model` vs `model_id`, adapted vs raw
  results) to two sides.

## Revisit When

- a second image provider needs the same private shape;
- capability negotiation outgrows a static advertisement list;
- the gateway's authentication or external compatibility contract changes; or
- generation becomes asynchronous with durable job identity.

## Amendment 2026-09-20: shared generation lifetime and protocol 3 (TIPC-M1)

The decision above is preserved as history. Only its Torch-specific
lifetime, compatibility, and result-evolution clauses are replaced:

- **Lifetime.** The provider-owned 600-second generation deadline and the
  gateway 615-second transport allowance are removed. Admitted image
  generation follows the shared
  [generation lifetime](../contracts/generation-lifetime.md): one
  connection-bounded but duration-unbounded transport for
  `/v1/chat/completions`, `/v1/completions`, `/v1/images/generations`, and
  future generation routes, with no total, response-read, idle, or elapsed
  deadline. Disconnect requests cancellation without proving that sidecar
  work stopped; a lost transport is an unknown outcome and is never replayed.
  There is no generation-deadline outcome and no public cancellation API.
- **Compatibility.** The wire protocol is now exactly `3`: a protocol-2
  sidecar speaks the old lifetime and is rejected as incompatible, so mixed
  versions cannot be admitted. The handshake stays strict and closed on
  requests: malformed bodies fail before classification, and liveness,
  protocol mismatch, missing capability, unavailability, and process/profile
  replacement each keep their owning outcome at the serving, listing, and
  admission boundaries. Compatibility is rechecked live before model load and
  again before admission, bound to the current process/profile identity.
  Ready-slot state and supported-operation checks stay independent of the
  handshake. Packaging remains immutable; the installer qualifies artifact
  identity, checksum, recipe identity, recipe protocol `3`, and environment
  separately.
- **Results.** Valid results still require `png_base64`, `seed`, `steps`,
  `guidance`, `memory_policy`, and `duration_seconds`, with canonical PNG,
  dimension, base64, and size validation and the whole-payload bound enforced
  over the entire received payload. Unknown additive private result fields
  (for example `peak_vram_bytes`) are accepted within that bound and never
  projected publicly: the public shape owns exactly `data[].b64_json` plus
  `metadata` with the five canonical values, and no deadline outcome exists.

Ownership is otherwise unchanged: the gateway still owns validation,
adaptation, error mapping, transport policy, and disconnect propagation; the
Torch provider process still owns execution, checkpoint cancellation, and the
device lease. The broader Torch surface (text handlers, responsiveness,
shutdown) stays with its existing remediation owner.
