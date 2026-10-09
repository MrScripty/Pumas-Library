# Image to text adapter contract

This adapter adds finite caption and annotation requests to the existing selected
model operation protocol. It uses a dedicated llama.cpp profile's chat transport;
it does not add a model importer, inference framework, or acquisition endpoint.
Runtime and model qualification remain separate from a successfully decoded image
and from controlled backend tests.

## Consumer parser update

`contract_version` remains `1`. The closed `Capability` enum gains
`image_to_text`, making seven descriptor variants. Named operation input gains
`image` and `image_messages`; input formats gain `png_base64`, `jpeg_base64`, and
`messages_image`. Descriptor option names gain `image_bytes`, `image_pixels`, and
`image_count`. Consumers with closed enums must update their parser and pin the
adapter cohort before consuming these descriptors. Existing capability formats
remain present. Discovery's shared `PumasBuildInfo` advertises
`pumas.model-operations.image-to-text` schema version `1` when inference plugins
are compiled. This is a protocol advertisement, not model readiness.

The actual production Rust DTOs derive JSON Schema under `export-contract`.
Export the six request/response schemas with:

```sh
mkdir -p /tmp/pumas-image-contract
PUMAS_IMAGE_CONTRACT_EXPORT_DIR=/tmp/pumas-image-contract \
ORT_SKIP_DOWNLOAD=1 CARGO_BUILD_JOBS=1 \
cargo test --locked --offline --manifest-path rust/Cargo.toml \
  -p pumas-rpc --bin pumas-rpc --all-features \
  handlers::model_operations::types::image_to_text_schema_tests::image_to_text_actual_dto_schema_export
```

This emits `operation-request`, `modality-request`, `capabilities-response`,
`capability-descriptor`, `operation-response`, and `error-response` schema JSON
files. Schemas describe serialization. The bounds and readiness rules below also
apply at runtime; JSON Schema alone does not prove admission or execution.

## Tuldok adoption boundary

The Tuldok repository owner must update its closed parser before pinning this
Pumas cohort. No Tuldok source or application acceptance is supplied by this
Pumas change. In addition to the discriminants above:

- Preserve all seven capabilities: `chat_generation`, `text_generation`,
  `text_embedding`, `image_generation`, `image_to_text`, `audio_transcription`,
  and `audio_classification`. Preserve the existing unavailable reasons;
  `available` is a live observation, not a durable model qualification receipt.
- Require the `pumas.model-operations.image-to-text:1` build advertisement and
  select the explicit model/profile with an available Image→Text descriptor.
  Pass `semantic_task:"image_to_text"` in facade requests and `stream:false`.
- Preserve ordered role/parts, encode only PNG/JPEG bytes, and apply all bounds
  below, including aggregate compressed bytes and total message parts. Images
  belong only to user messages. Never convert paths or URLs into input authority.
- Decode the response as `{contract_version, request_id, result}` with
  `result.kind:"text"`, text and finish reason; require matching request IDs.
  Do not require a response `model` field or expose private provider envelopes.
- Preserve HTTP 400/413/422/503 pre-admission refusals and 502 provider failures,
  including `error.outcome`. Do not replay after `unknown`, disconnect, or an
  uncertain response. Cancellation of external HTTP does not attest native stop.
- Exercise actual application caption/annotation, cancellation and session
  cleanup against the final source/schema pin. Keep borrowed owners alive after
  consumer cleanup; for managed children, observe exact child/session drainage.

The production DTO export and controlled HTTP tests are the Pumas-side oracle.
They do not validate Tuldok's parser implementation or its process lifecycle.
The integration lead must regenerate any combined desktop contract after the
provider task enum and other branches have been composed.

## Requests and results

An image-only facade request is:

```json
{
  "contract_version": 1,
  "request_id": "caption-17",
  "model": "selected-model",
  "input": {"kind": "image", "encoding": "png", "data_base64": "<standard base64 PNG>"},
  "output": "text",
  "semantic_task": "image_to_text"
}
```

For annotation, retain role and part order:

```json
{
  "contract_version": 1,
  "request_id": "annotation-18",
  "model": "selected-model",
  "input": {"kind": "messages", "messages": [
    {"role": "user", "content": [
      {"kind": "text", "text": "Describe the objects and their relative positions."},
      {"kind": "image", "encoding": "jpeg", "data_base64": "<standard base64 JPEG>"}
    ]}
  ]},
  "output": "text",
  "semantic_task": "image_to_text",
  "options": {"kind": "text_generation", "max_tokens": 512},
  "stream": false
}
```

Named requests use `capability: "image_to_text"`, mandatory
`options: {"kind":"text_generation"}`, and `input.kind: "image_messages"`
instead of the facade's `messages` for image-bearing histories. Only user
messages may contain images. System and assistant messages may contain text.
At least one user image is required. Text parts must contain non-whitespace text.
The image-only form inserts `Describe this image.` before the image. Text and
image order is preserved; audio parts and mixed audio/image requests are refused.

The output is the existing typed text result with bounded text and finish reason.
The request carries the request ID and public model reference. The response
contains only `contract_version`, `request_id` and `result`; it has no `model`
field.
The provider request uses the selected canonical model ID and standard ordered
chat text/`image_url` parts containing the validated compressed bytes. The
provider result must report exactly that canonical model ID; missing or different
model identity, tools, and malformed results produce `invalid_provider_result`,
HTTP 502, `outcome: "unknown"`. A public alias in the provider's result does not
satisfy the canonical check. No backend envelope fields are copied into the
public typed result.

## Admission and supported runtime

The selected record must explicitly declare the image-to-text task using
`image-to-text`, `image_to_text`, or `image->text` in existing pipeline/primary task
metadata. Modality labels alone do not enable inference. Existing model/profile
selection and semantic ambiguity errors still apply; an options kind does not
choose a semantic task.

Only enabled `LlamaCppDedicated` profiles are supported initially. Router,
Ollama, Torch, and ONNX vision adapters are unavailable. Endpoint bases with
queries or fragments are refused. A read-only query to
`/props?model=<canonical-id>&autoload=false` must declare `modalities.vision:true`,
`is_sleeping:false`, and a nonempty build string of at most 256 bytes. The request
has a two-second connect timeout and five-second whole-body timeout, forbids
redirects/retries, and bounds the body to 64 KiB even when chunked. This follows
the [llama.cpp server properties protocol](https://github.com/ggml-org/llama.cpp/blob/master/tools/server/README.md).
Feature declaration alone does not attest model files or semantic quality.

The final readiness query runs again within the existing generation permit and
managed session stop watch. Profile eligibility and selected serving identity
are checked again after that query and before POST. Managed dedicated launch
with a matching sibling projector supplies `--alias <canonical-model-id>`.
Projector/model/runtime compatibility still requires actual qualification.
The existing [llama.cpp multimodal protocol](https://github.com/ggml-org/llama.cpp/blob/master/docs/multimodal.md)
defines the chat image part mapping.

External profiles have no Pumas-owned backend session or native stop watch.
Closing their HTTP request does not prove remote native computation stopped.
Serving row equality and `loaded_at` equality do not prove immutable artifact
identity or prevent external process/model replacement. Managed stop watches
fence the bound managed process; this adapter adds no recovery attestation.

## Decoding and resource bounds

Both facade and named requests validate options, strict canonical standard
base64, complete PNG/JPEG containers, and full raster decoding before any backend
GET or POST. The declared encoding must match actual bytes. Corruption,
truncation, trailing container data, APNG, and multi-picture JPEG are rejected.
Progressive still JPEG is supported. Inputs are bytes, not URLs, paths, or
data-URL envelopes. Codec validation preserves the compressed input bytes.

| Limit | Value |
| --- | --- |
| Entire request | 32 MiB |
| Compressed image bytes | 8 MiB each, 16 MiB aggregate |
| Images | 4 |
| Width and height | 4096 each |
| Pixels per image | 4,194,304 |
| Messages and total image-bearing message parts | 128 messages, 128 total parts |
| Generated tokens | default 512, allowed 1–2048 |
| Temperature and top-p | finite, 0–2 and 0–1 respectively |
| PNG codec allocation budget | best effort 64 MiB |
| JPEG decoded output budget | 64 MiB, plus codec-managed scratch |

PNG uses `image`'s PNG codec. JPEG uses the safe `turbojpeg` 1.5.1 wrapper
with `turbojpeg-sys` 1.2.0's checksum-pinned bundled libjpeg-turbo 3.1.0.
Header dimensions and pixel counts are checked before allocating the RGB output
raster. The full decode must return success: warnings, including premature
entropy exhaustion before all MCU data is available, are errors and refuse the
request. Marker framing alone does not establish entropy completeness. Baseline
and progressive still JPEG fixtures exercise the same production decode path.
The native codec requires CMake and a C compiler; the default build uses the
bundled static library without a system codec lookup. Platform build and shipped
archive acceptance remain separate gates. This software is based in part on the
work of the Independent JPEG Group.

Compressed and dimension/pixel bounds precede raster decoding. Two blocking
decode permits bound concurrent work; capacity refusal is HTTP 503 before
admission. A blocking codec operation already running cannot be preempted. It
retains its input and permit until bounded, effect-free validation ends after
caller loss. Allocation settings are not hard OS memory containment.

## Cancellation and qualification gates

Image to text is finite; `stream:true` is unavailable. Disconnect, shutdown, and
managed session stop cancel the final readiness GET including its body and the
provider request through the existing generation owner. The admission marker is
set immediately before a possible inference POST. Pre-admission errors report
`not_admitted`; after possible provider effects, loss/failure is `unknown` and
never replayed. Format limits use HTTP 413; malformed inputs use HTTP 400;
unsupported facade pairings use HTTP 422; unavailable capabilities use HTTP 503.

The cohort includes real PNG/JPEG decoding and controlled HTTP backend tests.
Those tests are not pretrained model inference. Tiny numerical vision forward
passes, real caption/annotation semantics, matching runtime/model/projector
fixtures, consumer parser integration, and packaged consumer acceptance remain
distinct gates. Do not mark these gates passed from property declarations or
synthetic text replies. Acquire and verify the exact runtime/model/projector
cohort through the existing acquisition and runtime controls before real-model
qualification.

The next native qualification candidate, explicit two-GGUF import proposal,
resource budget and current execution blocker are recorded in the
[native qualification contract](image-to-text-native-qualification.md).
