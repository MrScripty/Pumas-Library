# Local ASR operation contract proposal

Status: proposed producer contract, not implemented or advertised. This follows the Cohere loader milestone in `local-cohere-transcription.md`. No consumer should call these proposed methods until the producer implementation and generated contracts exist.

## Existing boundaries to reuse

- `PumasModelRef` and owner-fresh selected-artifact resolution identify installed library assets. A native `cohere_asr` configuration is necessary but is not model identity attestation.
- `ServingLoadOperation` and `RuntimeProfileProcessOwner` retain managed profile/load generation ownership. Caller-provided paths, ports and unowned sidecars are not accepted.
- `TorchClient` protocol 3 remains compatible for existing image/text consumers. Speech is an additive, explicitly versioned capability, not a global runtime upgrade or a claim that every Torch model generates images.
- `ModelManager.speech_lease` and the synchronous loader adapter retain the loaded model/device through native work. An executor future being cancelled is not a worker-exit receipt.
- The public RPC shutdown supervisor and existing managed process owner remain the owners of listener and process cessation. Do not add a detached shutdown task or a second competing process supervisor.

## Proposed public methods, contract version 1

All requests reject unknown fields. Existing local Host/Origin admission and request-body limits apply; speech has an additional explicit 1.5 MiB request envelope bound before JSON/base64 allocation.

1. `get_transcription_capability`: accepts a `model_ref` and `runtime_profile_id`. Returns `contract_version`, the canonical `model_ref`, a model-content binding, runtime recipe identity, an opaque `runtime_instance_id`, state/diagnostics, and audio/language/output limits. Availability requires an installed validated model, a separately qualified native ASR runtime, an exact owned live profile/listener, the speech protocol handshake, and one unambiguous READY speech slot. Discovery performs no acquisition or implicit model load. Unsupported/missing/busy are distinct states; process/provider type alone is insufficient evidence.
2. `start_transcription`: accepts the discovered model/runtime binding, a caller-generated `request_id` UUID, explicit language, and audio `{encoding: "pcm_s16le", sample_rate_hz: 16000, channels: 1, sample_count, data_base64}`. Sample count is 1 through 480,000; decoded bytes must be exactly twice that count, at most 960,000 bytes. Admission returns an `operation_ref` bound to profile/runtime instance/operation identity and a status snapshot. It acknowledges admission, not completion.
3. `get_transcription_operation`: accepts that exact `operation_ref`. Returns the canonical model/runtime identities, status, cancellation acknowledgement, cleanup status, optional bounded text, or a typed diagnostic. An unknown/expired operation or changed runtime instance never means success or authorizes replay.
4. `cancel_transcription`: accepts the exact `operation_ref` and requests cooperative cancellation. Repeated calls are idempotent for that operation. A response distinguishes cancellation requested from worker cessation; it cannot release model/device custody merely because the request was delivered.

States are `running`, `cancellation_requested`, `completed`, `cancelled`, `failed`, and `cleanup_unconfirmed`. Only confirmed worker completion/cessation allows a terminal reusable slot. `cleanup_unconfirmed` is observable but retains active custody and refuses new work on that device until the existing managed process owner proves the affected runtime has stopped. Preserve both operation and cleanup diagnostics; never mask the former with the latter.

Successful text is at most 16,000 UTF-8 bytes, accompanied by model and operation identity. Oversized/invalid output is rejected rather than truncated. Empty text is valid. The game must present text for user review and explicit submission; model output is never an instruction to execute a command.

## Private provider extension

Proposed loopback private routes under `/api/speech`: `capability`, `operations` (admit), `operations/{id}` (status), and `operations/{id}/cancel`. Each response includes `speech_protocol: 1` and the provider's startup-random runtime instance ID. The capability is named `local_transcription_v1`; the handshake may advertise protocol support without claiming an installed/loaded model is available. Older Torch protocol-3 images continue to serve their existing methods and fail speech capability checks explicitly.

The Python application owns a bounded operation registry, cancellation event and independently retained executor worker. No HTTP handler owns the only reference to admitted work. One active speech operation per runtime is the initial admission policy, with no hidden queue; the shared device lease still excludes image/load/unload conflicts. A disconnected polling or start client does not release or replay native work.

Admission binds a request UUID to a digest of all request fields and decoded audio. While its receipt is retained, repeating the same identity returns the same operation; changed payload under that identity refuses. Retention is an explicit memory policy: at most 64 settled receipts, up to 10 minutes, exposed in capability/status. Active or cleanup-unconfirmed owners are never evicted. Expired/unknown receipts are reported as unknown; there is no exactly-once or replay guarantee across expiry/runtime restart. Consumers must not automatically retry an ambiguous admission as new work.

Audio buffers are released only after confirmed synchronous work and required device synchronization. Do not persist audio/transcripts or log their contents by default. Buffers retained by `SpeechCleanupUnconfirmed` remain fenced inside the lease until process cessation; no universal secure-memory-erasure claim is made.

## Shutdown and resource policy

Closing admission requests cancellation of every admitted speech operation and retains every worker/lease. The Rust owner must coordinate this private drain with its existing exact profile generation before managed process teardown. Transport timeout/disconnect is unknown provider outcome, not permission to forget it. If a separately explicit shutdown policy escalates to process termination, the operation reports forced/incomplete cleanup and the existing process owner must still prove descendant cessation. No normal transcription duration timeout is introduced by this design; 30 seconds bounds input audio, not compute time.

The implementation review must settle how the existing process-owner shutdown phases compose with this cancellation drain before public routes are enabled. In particular, Python lifespan cleanup cannot release an unconfirmed CUDA lease, and a Rust waiter cannot deadlock teardown by awaiting a process that it alone must stop.

## Model loading and runtime prerequisites

An existing READY slot alone is not enough unless Pumas bound its selected artifact/content and qualified recipe at managed load admission. Reuse the canonical serving owner; extend Torch serving deliberately for Cohere rather than bypassing image-only allowlists with an arbitrary model path. Keep source/model protection through loading and validate binding freshness before publication. If the current load-target API does not supply the needed custody, add a narrow owned lease rather than treating its display path as authority.

The current image/text recipe pins Transformers 4.57.6. Native Cohere requires its own qualified exact lock and recipe for Transformers >=5.4, with CPU/CUDA-only initial support. Recipe/probe/handshake coverage must establish native class availability, local-only loading, safetensors-only model weights and remote-code refusal. No capability may become available from changing a version string or mocked handshake.

## Reviewable milestones

A. Implement the private Python operation owner and synthetic lifecycle fixtures on the loader branch, without public capability advertisement or runtime lock changes.

B. Add typed Rust client/DTO/RPC ownership and capability projection, including shutdown composition, model binding and generated consumer-contract tests. Keep unsupported states truthful until a qualified runtime and model exist.

C. Qualify a separate managed ASR runtime, then acquire the gated model only after approval. Exercise approved local synthetic speech through the actual Pumas producer and the game adapter. Native packaged execution, microphone consent/capture and speech quality require explicit evidence.

Required regressions include strict base64/sample/language bounds, busy admission, repeated/conflicting request IDs, lost acknowledgement without replay, status expiry/runtime replacement, cancellation before/after generation, repeated cancellation, late output discard, device synchronization failure with retained lease, shutdown during preprocessing/generation, and forced/unconfirmed child cleanup. A fixture or unavailable UI does not complete the requested voice feature.
