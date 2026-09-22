# Pumas generation lifetime

This is the single Pumas-owned generation-transport policy (`TIPC-GEN-01`).
It covers every admitted generation: image, text, streaming, non-streaming,
and future generation operations. It is the normal generation contract, not
an image exception, and it takes precedence over the generic
remote-operation deadline guidance for generation operations.

## Contract

An admitted generation has no total, response-read, idle, or elapsed-duration
deadline. Elapsed time and response silence never make an admitted generation
fail. Only connection establishment is bounded (10 seconds on the current
gateway and Torch image transports).

Streaming bytes are progress information, not lease renewal. A connected
generation may legitimately remain silent until its terminal result.

## Scope

The duration-unbounded transport applies to all registered generation routes:

- `POST /v1/chat/completions` (Ollama and llama.cpp through the shared
  buffered gateway handler);
- `POST /v1/completions` (same shared handler, client seam, and response
  function as chat);
- `POST /v1/images/generations` (Torch-only adapter through `TorchClient`);
- future generation routes, which must be admitted through this same
  transport seam.

The following keep independently justified budgets and must not use the
generation transport: models listing, embeddings, health, startup, model
loading, installation, non-generation inference, cancellation cleanup, and
shutdown. Those budgets must never become a maximum generation duration.

## Cancellation, custody, and uncertainty

- Bounded admission, busy rejection, retained resource ownership, explicit
  cancellation, and operator stop prevent this contract from authorizing an
  unbounded queue or detached work.
- An owning request abort or connection loss requests cancellation through
  the real gateway/private route. Dropping a Rust future, Python task, or
  thread wrapper is not proof that provider work stopped.
- Each generation provider retains its admitted work and resources until its
  own completion or sufficient cleanup. Worker and device custody is retained
  until the underlying work has stopped enough for safe reuse.
- A connected stalled operation may retain its admitted resources until a
  terminal event or authorized stop.
- Transport loss before a terminal result is an unknown outcome: Pumas never
  claims compute stopped and never replays automatically. A
  connection-establishment failure is an unavailable/connect outcome, never
  proof that admitted generation was cancelled or completed.
- There is no public cancellation API and no automatic retry after an
  uncertain result.

## Implementation seams

- Text generation: `generation_http_client()` in
  `rust/crates/pumas-rpc/src/handlers/openai_gateway.rs` (connect-bounded,
  duration-unbounded); non-generation routes keep their bounded policy on the
  gateway client.
- Image generation: `TorchClient::image_client` in
  `rust/crates/pumas-app-manager/src/torch_client.rs` (connect-bounded,
  duration-unbounded, whole-payload bound enforced while streaming).
- Compatibility before admission stays separate from transport: exact Torch
  protocol agreement plus the image-generation capability, rechecked live
  before admission and bound to the current process/profile identity. See the
  private [Torch provider protocol](torch-provider-protocol.md) and
  [ADR 0002](../adr/0002-torch-image-provider-protocol.md).

## Evidence

- Controlled long/silent transport integrations prove silence beyond the
  former policy on chat and completions through the shared construction.
- Route and registry tests prove every registered route uses this transport.
- Disconnect tests prove the Pumas-boundary cancellation signal with exactly
  one provider admission; they claim nothing about external text-provider
  worker cleanup, which stays with each provider lifecycle owner.
- Lost-transport tests prove the uncertain outcome is reported once and never
  replayed.
