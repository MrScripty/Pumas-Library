# ADR 0005: Modalities are composable primitives

Status: Accepted (2026-11-06). Implementation is tracked in the
[modality primitives plan](../plans/modality-primitives/plan.md).

Affected boundaries: the typed model operations in
`rust/crates/pumas-rpc/src/handlers/model_operations*` (request grammar,
capability descriptors, provider projection and result checks), the
[model operations contract](../contracts/model-operations-v1.md) and the
consumer descriptor schema in `scripts/consumers/contracts/`.

## Context

Each new kind of input was added as its own capability path:
`image_to_text`, `multimodal_embedding`, decision `images`, and next
decision audio, chat audio and embedding audio. Every path carried its own
readiness check, input formats, provider encoding and tests, so supporting a
new model family meant new code even when its runtime already handled it.
Liquid AI's LFM3 and d1 models (text, image, audio and video in; text, audio
or decisions out) would have needed several more such paths.

llama.cpp already reports what a loaded model accepts and produces:
`/props` reports `modalities` (`vision`, `audio`) and `/v1/models` reports
`architecture.input_modalities` and `output_modalities`.

## Decision

- **A modality primitive is one definition of a kind of data**: its shape in
  the Pumas API, its validation and bounds, and its encoding to and from each
  runtime protocol. A primitive is the same whether it is an input or an
  output.
- **Content primitives** are `text`, `image`, `audio` and `video`. **Result
  primitives** are output-only: `json` (valid JSON that must match a schema
  supplied by the capability or the caller) and `vector` (embeddings: fixed
  dimension, finite values, comparable only within one model's space).
  Decision answers and relevance scores are `json` with their capability's
  schema.
- **A served model declares an input set and an output set.** They come from
  the runtime when it reports them (llama.cpp) and from the provider's static
  declaration otherwise (ONNX Runtime: text to vector; Torch image provider:
  text to image). Pumas does not configure modalities per model.
- **Protocols compose primitives.** Each runtime protocol (llama.cpp chat,
  embeddings, `/v1/systemone`, `/v1/rerank`) defines once how each primitive is
  packed into its envelope. A capability is a task over a protocol, plus the
  schema of its result. It accepts any input primitive the served model
  declares.
- **A part in an undeclared modality is refused before admission** with
  `unsupported_modality` naming the model's declared set. A runtime that
  misreports a modality yields the runtime's own error, with its reason.
- **Contract v1 stays compatible.** Existing capability, input-kind and output
  names keep their meaning. Discovery gains `modalities` per served model and
  `input_modalities`/`output_modalities` per capability; part lists gain new
  part kinds. All of this is additive under the v1 compatibility rules.

## Alternatives considered

- **Keep one capability per model and modality pair.** Rejected: the paths
  multiply with every model family, and each one needs Pumas to know the
  model's exact task before its runtime can be used.
- **Configure modalities per model in the library.** Rejected: it adds setup
  for users and can disagree with what the loaded runtime actually supports.
- **Treat structured results as `text`.** Rejected: a decision or ranking can
  be checked mechanically, and the check is what turns an incoherent provider
  answer into `invalid_provider_result` instead of a silent wrong value.

## Consequences

- A model whose runtime declares a supported combination works without a
  Pumas change. Pumas guarantees well-formed requests and primitive-level
  results; task-level correctness comes from the capability's schema where
  one exists.
- Image validation moves into the shared primitive layer unchanged. Audio
  input reuses the existing PCM part shape and bounds; Pumas adds the WAV
  header itself and never parses an audio container.
- Speech output is a defined primitive with no 0.8.0 transport: llama.cpp
  offers it only through its realtime WebSocket API and `llama-tts`, so it is
  reported unavailable with a reason.
