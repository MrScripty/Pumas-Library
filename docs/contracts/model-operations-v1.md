# Model operations, contract version 1

This is the additive v0.8 HTTP contract. Existing `/v1` OpenAI-compatible
routes and model identities remain supported. Capability availability describes
the selected served model and exact runtime, not the mere presence of a loader.

The routes are `GET /v1/served-models` (discovery), `GET /v1/capabilities`
(one selected serving identity) and `POST /v1/model-operations` (typed finite
or streamed operations). Request IDs correlate responses; they do not authorize
retries or imply idempotency. Errors use fixed public codes and distinguish
`not_admitted` from an admitted operation whose outcome is `unknown`.
Unsupported or unqualified capabilities fail before provider admission. Never
replay generation after caller loss, transport failure, redirects, or ambiguous
provider completion.

Existing chat/completion `stream: true` requests progressively forward provider
SSE bytes. They retain the upstream transport and an admission permit until
body completion, failure, or disposal. The existing 32 MiB cumulative response
bound remains. There is no generation total, read, idle, or elapsed deadline.
Disconnect or body disposal closes transport. Server shutdown and exact
managed-session stop terminate header waiting and finite buffering immediately,
and terminate a streaming body when it is next polled. An unpolled or downstream
blocked body retains its transport and permit until polling or disposal; HTTP
shutdown also closes connections under the existing shutdown grace policy.
Closure requests cancellation without establishing provider cessation. A fault
after headers ends the body with a transport error, rather than a successful
terminal result.

## Compatibility rules

These rules apply to every route in this contract.

- **Responses grow additively.** Pumas may add response fields, capability
  descriptors, enum values (capabilities, formats, option names, availability
  reasons, error codes) and list entries within `contract_version` 1.
  Consumers must ignore fields and values they do not use and must not reject
  a response because it contains them. A consumer that only uses
  `text_embedding` keeps working when Pumas adds a new capability.
- **Existing operations do not change.** Changing the meaning, required fields
  or result shape of an existing operation requires a new `contract_version`.
  Pumas then advertises both versions in `supported_contract_versions` for an
  announced overlap.
- **Requests stay closed.** Pumas rejects unknown request fields, so a typo is
  reported instead of silently ignored.

## Request grammar

`POST /v1/model-operations` has one request shape:

| Field | Required | Meaning |
| --- | --- | --- |
| `contract_version` | yes | `1` |
| `request_id` | yes | Caller correlation ID, 1–128 of `A-Za-z0-9-_.:`. Echoed in every response. |
| `model` | yes | Served model alias. |
| `profile` | no | Exact serving profile; needed only when the alias is served by more than one profile. |
| `input` | yes | Tagged by `kind`: `text`, `text_batch`, `messages`, `image_messages`, `image`, `content_batch`, `decision`, or the reserved `audio`. |
| `output` | yes | `text`, `embeddings_float32`, `png_base64`, `decisions`, or the reserved `labels`, `pcm_s16le`, `pcm_f32le`. |
| `capability` | no | Selects one capability explicitly. |
| `semantic_task` | no | Narrows resolution to one semantic task. |
| `options` | no | Tagged options; defaults apply per capability. `image_generation` requires width and height. |
| `stream` | no | `true` for streamed text generation. |

Pumas resolves the operation from the selected model's current capability
descriptors, the input kind and the output. `capability` and `semantic_task`
narrow that resolution and must agree with the input and output. When several
available capabilities accept the same request, Pumas returns
`ambiguous_operation` and the caller adds `capability`. Message content may be
a string or a list of `text`/`image` parts; `image_messages` is the same
messages-with-parts shape.

## Errors

Every error response has the same shape:

```json
{
  "contract_version": 1,
  "request_id": "example:1",
  "error": {
    "code": "invalid_request",
    "outcome": "not_admitted",
    "message": "request does not match the contract: unknown field `optoins` ..."
  }
}
```

- `code` is the stable value consumers branch on.
- `outcome` is `not_admitted` (no provider effect happened) or `unknown` (the
  provider received the operation and its result cannot be established).
- `message` is a human-readable reason for the caller or operator. It names
  the field, limit, model, unavailable capability reason, or the provider's own
  reported reason. Credential-shaped text is redacted. Its wording may change;
  do not parse it.
- `request_id` is the caller's ID whenever the body was JSON with a valid
  `request_id`, including schema rejections; otherwise it is `null`.

The HTTP status matches the code: `invalid_request`, `unsupported_contract` →
400; `model_not_found` → 404; `ambiguous_model`, `ambiguous_operation` → 409;
`request_limit` → 413; `unsupported_modality` → 422; `capability_unavailable`
→ 503; `provider_failure`, `invalid_provider_result`, `response_limit`,
`transport_lost` → 502.

## Endpoints

`GET /v1/served-models` is the discovery entry point. It returns
`supported_contract_versions`, the byte bounds, and `models`: one entry per
served (model, profile) pair with `model` (the alias to send), `profile`,
`model_id` (library identifier), `provider`, `load_state`, optional
`context_tokens`, `profile_required` (the alias is served by more than one
profile, so requests must name `profile`), and `capabilities` (the same
descriptors as below). Consumers select models from this list instead of
configuring aliases or profiles by hand. `/v1/models` remains the
OpenAI-compatible list: each ID appears once, with its `profiles`.

`GET /v1/capabilities?model=<served-alias>&profile=<optional-exact-profile>`
requires an explicitly selected serving identity. It returns
`supported_contract_versions`, `model`, `profile`, `max_request_bytes`,
`max_response_bytes`, `max_stream_event_bytes`, optional `context_tokens`, and
`capabilities`. Each capability
has `capability`, `semantic_task`, `input_formats`, `output_formats`, `streaming`,
`availability`, and `option_bounds`. Availability is tagged as
`{"state":"available"}` or `{"state":"unavailable","reason":"..."}`.
Explicit source-task evidence takes precedence over broad modality signatures.
Availability requires both compatible semantics and the selected runtime's
actual adapter/readiness checks. It is a current observation; admission checks
the same serving identity again before handing work to a provider.

The implemented capabilities are `chat_generation`, `text_generation`,
`text_embedding`, `image_generation`, `image_to_text`, `multimodal_embedding`
and `decision`. Vision-language models (`image-text-to-text`) offer chat,
text generation and image-to-text. `multimodal_embedding` and image input to
`decision` need a dedicated llama.cpp session with its projector loaded.
`decision` is offered only when llama.cpp reports the model as a native
decision model (`decisions` output modality); such models offer no text
generation. `audio_transcription` and `audio_classification` are explicitly
unavailable with `unqualified_audio_runtime` until the production custody
bridge and installed runtime are qualified. Build/protocol identity is reported by
`pumas-rpc --build-info`; these endpoints define no competing identity type.

For example, a finite text operation is:

```json
{
  "contract_version": 1,
  "request_id": "example:1",
  "model": "served-alias",
  "profile": "optional-exact-profile",
  "capability": "text_generation",
  "input": {"kind": "text", "text": "Hello"},
  "output": "text",
  "options": {"kind": "text_generation", "max_tokens": 32},
  "stream": false
}
```

`profile` may be omitted when selection is unambiguous. All request records and
tagged options reject unknown fields. `request_id` is 1–128 ASCII letters,
digits, or `-_.:`; `model` is a nonempty, trimmed serving alias of at most 256
bytes with no control characters. Request and response bounds are 32 MiB.

Capability inputs and outputs:

| Capability | Input `kind` | Output | Options `kind` |
| --- | --- | --- | --- |
| `chat_generation` | `messages` (or `text`, sent as one user message) | `text` | `text_generation` |
| `text_generation` | `text` | `text` | `text_generation` |
| `text_embedding` | `text` or `text_batch` | `embeddings_float32` | `embeddings` |
| `multimodal_embedding` | `image` or `content_batch` | `embeddings_float32` | `embeddings` |
| `image_generation` | `text` | `png_base64` | `image_generation` |
| `image_to_text` | `image`, `image_messages`, or `messages` with image parts | `text` | `text_generation` |
| `decision` | `decision` | `decisions` | `decision` (optional) |

Limits come from the serving runtime, not from Pumas. Each descriptor's
`option_bounds` reports them: `max_tokens` is bounded by the served context
window when the runtime reports one. Text options are optional positive
`max_tokens`, `temperature` in [0, 2], and `top_p` in [0, 1]. Embedding options
accept optional positive `dimensions`; when the provider ignores it (llama.cpp,
Ollama), Pumas keeps the leading values and renormalizes (Matryoshka
truncation), and a request for more dimensions than the model produces fails.
The in-process ONNX adapter has fixed batch, length and dimension limits,
which its descriptor reports, plus the loaded model's own per-input token
limit (`input_tokens`, including special tokens) when the model config declares
`max_position_embeddings` or `n_positions`; an input over it is refused with
the limit named. Proxied providers enforce their own context and
batch limits and their reported reason reaches the caller. Image inputs share
decode-safety bounds (8 MiB per image and 16 MiB per request, 4 images, 4096 px
per side, 4 megapixels per image; `image_messages` also 128 messages and 128
content parts). Image-bearing descriptors advertise them in `option_bounds`
(`image_bytes`, `total_image_bytes`, `image_count`, `image_pixels`, `width`,
`height`, `message_count`, `content_parts`), and a `request_limit` refusal
names the bound and the request's value. The Torch image adapter bounds prompts at 4,000 characters and
validates its supported dimensions and the returned PNG.

`content_batch` is `{"kind":"content_batch","items":[{"parts":[...]}]}`: each
item's `text` and `image` parts produce one vector, in the same space as the
model's text embeddings. Task prefixes recommended by a model (for example
EmbeddingGemma's `task: search result | query: `) are part of the caller's text.

`decision` input is `{"kind":"decision","state":...,"questions":{...},
"images":[...]}`. `state` is a string or any JSON value. Each question has
`type` (`choice`, `score` or `noul`), `instructions`, and `criteria`: an
object of option to description (or `null`) for `choice`, a list of level
descriptions (lowest first) for `score`, and optional `true`/`false`
descriptions for `noul`. Option and level counts are model limits reported by
the runtime. The result is `{"kind":"decisions","answers":{...},
"input_tokens":...}`; each answer has the question's `type`: `choice` with
`choice`, `probabilities` and `confidence`; `score` with `score` (a
probability-weighted level index), `legend`, `probabilities` and
`confidence`; `noul` with `noul`, the probability that the answer is true.
Probabilities are scaled by the model's stored temperatures and are not
calibrated for any particular data.

A finite success has `contract_version`, the original `request_id`, and a
tagged `result`: `text` with `text` and `finish_reason`; `embeddings` with finite
float32 `vectors`; `image` with `png_base64` and `seed`; or `decisions` as above. Only recognized text
finish reasons (`stop`, `length`, `content_filter`) are accepted. The projection
checks a single text choice, embedding count/index/dimension coherence, and
rejects incoherent provider results. Provider envelopes and internal errors are
not exposed.

Errors follow [Errors](#errors). A request ID is never forwarded as a provider
idempotency key, and generation is never replayed.

Only text generation accepts `stream: true`. SSE names are `started`, `delta`,
`completed`, and `failed`; JSON `data` records include the matching `kind` and
original `request_id`. `started` also includes contract version, capability,
model, and exact profile. `delta` contains text. `completed` contains the
recognized finish reason. `failed` contains the fixed error/outcome record.
Completion requires a coherent provider terminal choice, `[DONE]`, and clean
HTTP body EOF. Truncation, malformed events, transport failure, or a missing
terminal cannot become successful completion. Incremental UTF-8, CRLF, SSE
comments, and multiline data are handled within a 256 KiB event bound and the
32 MiB cumulative bound. Buffered projected events still observe cancellation
on each downstream body poll. A silent or hung provider has no invented idle or
drain deadline; disposing the body closes the retained transport.

## Reserved audio

Audio transcription and audio classification are different capabilities even
when both produce text. Both are reserved and unavailable in this release.

The audio declaration reserves `kind: "audio"`, `encoding` (`pcm_s16le` or
`pcm_f32le`), `sample_rate_hz`, `channels`, `sample_count` (frames per channel),
and `data_base64`. Supported conversion is 8–192 kHz, one or two channels, and
at most 30 seconds; float32 samples must be finite and in [-1, 1]. The adapter
converts supported input to mono 16 kHz PCM16LE; it must not relabel samples.
Audio options reserve the existing closed language vocabulary and optional
`max_output_tokens`. The qualified transcription adapter generates up to a
fixed native 512 tokens, so the descriptor advertises `max_output_tokens` 512–512
and any other value is refused with that reason. These declarations do not
admit audio.

Production audio requires trusted selected-byte custody before loading: a
separate SHA-256 manifest, an owner-controlled copied read source, verified
installed recipe/code identity, and an exact managed process and slot generation.
The package observation fingerprint is not byte attestation. Custody survives
caller loss and uncertain load/unload outcomes. Release requires confirmed
native/device cessation with no outstanding operation borrows, or complete
exact-generation process-tree drainage. Finite-audio status/cancel routes and
runtime/slot references are not present until that custody is qualified; see
[installed audio session](installed-audio-session.md).
