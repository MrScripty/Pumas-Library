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
closure requests cancellation without establishing provider cessation. A fault
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
