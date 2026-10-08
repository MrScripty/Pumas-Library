# Model operations, contract version 1

This is the additive v0.8 HTTP contract. Existing `/v1` OpenAI-compatible
routes and model identities remain supported. Capability availability describes
the selected served model and exact runtime, not the mere presence of a loader.

`GET /v1/capabilities` returns supported contract versions and selected-model
capabilities: semantic task, typed input and output formats, streaming support,
closed option bounds, and availability. Audio transcription and audio
classification are different capabilities even when both produce text.
Protocol/build identity uses the single shared type owned by the release
integration lane; discovery and operation APIs must not define competing types.

`POST /v1/model-operations` accepts `contract_version`, `request_id`, a nonempty
served model alias (and an exact profile where selection is ambiguous), a
capability identifier, tagged input/output, and closed typed options. A finite
response contains the correlated typed result. A text stream contains typed
`started`, `delta`, `completed`, or `failed` SSE events. Request IDs correlate
responses; they do not authorize retries or imply idempotency.

Errors use fixed public codes and distinguish `not_admitted` from an admitted
operation whose outcome is `unknown`. Unsupported or unqualified capabilities
fail before provider admission. Never replay generation after caller loss,
transport failure, redirects, or ambiguous provider completion.

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

Audio transcription accepts declared encoding, channel count, and sample rate.
The adapter validates these and actually converts supported input to mono
16 kHz PCM16LE; it must not relabel samples. Finite operations expose their exact
runtime/slot/operation reference for status and cancellation. Language and input,
output, and generation bounds are closed and validated before admission.

Production audio requires trusted selected-byte custody before loading: a
separate SHA-256 manifest, an owner-controlled copied read source, verified
installed recipe/code identity, and an exact managed process and slot generation.
The package observation fingerprint is not byte attestation. Custody survives
caller loss and uncertain load/unload outcomes. Release requires confirmed
native/device cessation with no outstanding operation borrows, or complete
exact-generation process-tree drainage. Legacy arbitrary-path slots remain
unattested. Packaging supplies runtime qualification; discovery supplies the
compatible existing owner and its public HTTP endpoint.

Implementation is qualified and integrated in small slices: progressive legacy
streaming first, generic typed capabilities/operations second, and attested
production audio third. This document freezes the contract direction; endpoint
availability must reflect the slice actually installed. No scheduler, model
downloads, acquisition policy, or consumer UI is introduced here.

## Typed HTTP slice

`GET /v1/capabilities?model=<served-alias>&profile=<optional-exact-profile>`
requires an explicitly selected serving identity. It returns
`supported_contract_versions`, `model`, `profile`, `max_request_bytes`,
`max_response_bytes`, `max_stream_event_bytes`, and `capabilities`. Each capability
has `capability`, `semantic_task`, `input_formats`, `output_formats`, `streaming`,
`availability`, and `option_bounds`. Availability is tagged as
`{"state":"available"}` or `{"state":"unavailable","reason":"..."}`.
Explicit source-task evidence takes precedence over broad modality signatures.
Availability requires both compatible semantics and the selected runtime's
actual adapter/readiness checks. It is a current observation; admission checks
the same serving identity again before handing work to a provider.

The implemented capabilities are `chat_generation`, `text_generation`,
`text_embedding`, and `image_generation` through the existing qualified adapters.
`audio_transcription` and `audio_classification` are explicitly unavailable with
`unqualified_audio_runtime` until the production custody bridge and installed
runtime are qualified. The shared build/protocol identity is a separate
integration dependency; this endpoint does not invent an identity DTO.

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

The capability combinations are closed:

| Capability | Input `kind` | Output | Options `kind` |
| --- | --- | --- | --- |
| `chat_generation` | `messages` with text `system`, `user`, or `assistant` messages | `text` | `text_generation` |
| `text_generation` | `text` | `text` | `text_generation` |
| `text_embedding` | `text` or `text_batch` | `embeddings_float32` | `embeddings` |
| `image_generation` | `text` | `png_base64` | `image_generation` |

Text options are optional positive `max_tokens`, `temperature` in [0, 2], and
`top_p` in [0, 1]. Embedding options accept optional `dimensions` in [1, 8192]
and reuse the native adapter's input bounds (at most 128 texts and 65,536
characters). Image options require positive `width` and `height` and optional
unsigned 32-bit `seed`; the existing image adapter also validates the selected
runtime's supported dimensions and the returned PNG. Image prompt length is
bounded by the existing 4,000-character validator.

A finite success has `contract_version`, the original `request_id`, and a
tagged `result`: `text` with `text` and `finish_reason`; `embeddings` with finite
float32 `vectors`; or `image` with `png_base64` and `seed`. Only recognized text
finish reasons (`stop`, `length`, `content_filter`) are accepted. The projection
checks a single text choice, embedding count/index/dimension coherence, and
rejects incoherent provider results. Provider envelopes and internal errors are
not exposed.

An error has `contract_version`, `request_id` (null if no valid request was
parsed), and `error` containing `code` and `outcome`. Codes are
`invalid_request`, `unsupported_contract`, `model_not_found`, `ambiguous_model`,
`capability_unavailable`, `provider_failure`, `invalid_provider_result`,
`request_limit`, `response_limit`, and `transport_lost`. `not_admitted` means no
provider effect occurred; `unknown` means the operation was handed off and its
outcome cannot be established. A request ID is never forwarded as a provider
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

The audio declaration reserves `kind: "audio"`, `encoding` (`pcm_s16le` or
`pcm_f32le`), `sample_rate_hz`, `channels`, `sample_count` (frames per channel),
and `data_base64`. Supported conversion is 8–192 kHz, one or two channels, and
at most 30 seconds; float32 samples must be finite and in [-1, 1]. Audio options
reserve the existing closed language vocabulary and optional positive
`max_output_tokens`. These declarations do not admit audio in this slice.
Production finite-audio status/cancel routes and runtime/slot references remain
part of the following custody slice, and are not present in this implementation.
