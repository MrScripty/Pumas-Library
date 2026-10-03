# Local ASR operation contract proposal

Status: private Python operation ownership is implemented and synthetically tested; the public producer contract below remains proposed and is not advertised. This follows the Cohere loader milestone in `local-cohere-transcription.md`. No consumer should call these proposed methods until the producer implementation and generated contracts exist.

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


### Private ownership checkpoint, 2026-10-03

Milestone A adds only `torch-server/speech_operations.py` and its synthetic tests.
The owner is bound to one event loop and one startup-random instance identity;
its synchronous admission turn validates, deduplicates and reserves without an
await point. Foreign-loop calls are rejected. The private raw-body boundary
rejects envelopes over 1.5 MiB before JSON/base64 decoding, then validates exact
fields, canonical request UUIDs/base64, explicit supported language and mono
PCM16LE sample metadata. A future transport must also enforce the cap while
reading its stream, before collecting the body.

Each admitted operation independently retains its native thread and
`ModelManager.speech_lease`. Worker completion is checked against actual thread
exit; neither caller cancellation nor a cancelled asyncio waiter is a cessation
receipt. Cooperative/repeated cancellation discards late text. Only confirmed
cleanup releases the slot and PCM reference. Settled receipts contain bounded
value diagnostics rather than exception objects/tracebacks, and are expired by
a timer as well as lookups; the default bounds remain 64 receipts/600 seconds.
Expiry/restart has no exactly-once or safe automatic replay guarantee.

Unconfirmed device cleanup preserves both diagnostics, the exception-owned
conversion buffer, original PCM and the live device lease in a retained,
cancellation-resistant quarantine task. It is never included in settled-cache
retention or evicted. There is no process-cessation acknowledgement/release API.
`close_admission` and bounded `drain` report incomplete custody without waiting
indefinitely on quarantine. A synthetic child-process regression checks that
ordinary `asyncio.run` shutdown cancellation does not reach async-generator
finalization and release the real lease; its test-owned external supervisor
terminates the fixture and observes child exit.

This deliberately does not implement the production process boundary. Milestone
B must coordinate bounded drain with the existing managed process owner before
listener/lifespan teardown. Forced loop closure or explicit async-generator
finalization while custody is incomplete is prohibited. An unconfirmed runtime
requires external process termination; awaiting its orderly asyncio shutdown
would hang. No runtime/process cessation claim follows from the private drain.
Model/content/recipe binding, protocol handshake, routes, Rust DTO/RPC ownership
and capability projection remain milestone B gates; the internal model-name
selector alone is not identity attestation. Qualified runtime/model execution
and speech quality remain milestone C gates.

Verification in the existing environment:

- 31 operation tests passed, including default-adapter shutdown during synthetic
  preprocessing/generation, concurrent duplicate/conflicting IDs, strict decode
  limits, cancellation/lost acknowledgements, late-output discard, quarantine,
  audio/traceback release, cache bounds/expiry, stale identities and bounded drain.
- Existing Cohere adapter/lease tests: 15 passed. Model manager: 2 passed.
  Model load lifecycle: 7 passed.
- Whole Torch Ruff 0.15.2 checks and formatting passed (35 Python files).
- The broad Torch suite ran 166 tests with 7 errors, all in existing FastAPI
  route enumeration (`_IncludedRouter.path`), matching the inherited 7-error
  environment mismatch recorded in the loader checkpoint. This is not a
  full-suite pass; no existing route test was changed.
- No route mounting, capability advertisement, Rust changes, dependency/lock
  changes, model acquisition, real model inference or speech-quality claim.


#### Exceptional startup and data-free diagnostics review

An exception from `Thread.start` is now an unconfirmed native-startup outcome,
not evidence that no worker exists. Such operations retain their thread, audio,
lease and a cancellation-resistant quarantine owner. Even an apparently
non-started exceptional startup requires the external managed runtime to be
stopped/recreated: an ordinary status/identity probe cannot prove non-start.
A late successful worker result does not convert startup quarantine to success
or release custody. Later inference/device-cleanup diagnostics and retained
conversion buffers remain separate from the original startup diagnostic.

Diagnostic receipts now use only stable codes, allowlisted exception categories
and fixed code-owned messages. They never call backend exception `str`/`repr`,
copy exception arguments, or expose custom exception class names that could
contain audio/transcript data. Original operation, startup and cleanup failures
remain distinguishable without copying backend content.

Regressions were observed failing before the correction for exceptional startup
and an exception containing submitted PCM. Added fixtures cover native start
followed by an acknowledgement error, unacknowledged startup, late unconfirmed
device cleanup, and an outcome notification emitted before the actual thread
exits. Every synthetic subprocess remains owned by its test supervisor through
termination and observed exit.

Post-review verification: 34 operation tests passed, along with the existing 15
loader, 2 manager and 7 load-lifecycle tests. Whole Torch Ruff checks/formatting
still pass. The broad suite ran 169 tests with only the same 7 inherited
`_IncludedRouter.path` errors; this is not a full-suite pass. No production
scope beyond this private owner was changed.


### Private exact-slot checkpoint, 2026-10-03

This continuation of milestone A replaces the private model-name selector with
exact runtime/slot/load identity. It does not complete milestone B or enable a
speech producer. `ModelManager` creates a startup-random full runtime UUID and a
canonical full UUID encoding of a checked monotonic 128-bit load counter.
Runtime UUID plus load generation is the complete generation identity. The
counter consumes failed/unloaded reservations, uses bounded memory, and refuses
on exhaustion rather than wrapping. Existing short public slot IDs and serialized
slot fields are preserved; a live slot-ID collision cannot overwrite another slot.

There is exactly one `SpeechOperationOwner` per manager runtime, across all
models and devices. Closing it does not permit constructing a replacement owner
inside the same runtime. Private admission now requires `runtime_instance_id`
and `slot: {slot_id, load_generation}`. Its request digest includes this exact
identity along with every existing audio/language/request field. Operation refs
and status retain the exact slot reference. Repeating an old retained request
returns its original receipt without reacquiring a model; changing a slot or
load generation under that request ID conflicts. Status/cancel reject a forged
slot binding even when the runtime and operation IDs match.

Admission captures the exact slot and loaded object and borrows artifact-use
custody before scheduling the native runner. Immediately before native work,
`ModelManager.speech_lease` takes the existing shared device lock and checks the
runtime, slot ID, load generation, READY state, model type, exact slot object,
loaded object, device and unreleased borrow again. It validates the authority
receipt under that lock. Authority refusal codes are restricted to a stable
allowlist at admission and native acquisition; unknown or data-bearing provider
codes become a fixed unavailable refusal. An unload/reload or replacement in
the admission-to-worker gap therefore fails the original operation without acquiring a successor,
even if the successor has the same name or slot ID. The runtime-wide single-
active policy, independent worker retention, cancellation handling and startup/
device-cleanup quarantine continue to apply.

#### Artifact authority remains deliberately unavailable

`speech_binding.py` defines only the narrow `ArtifactUseAuthority.acquire(ref)`
and `ArtifactUseLease.validate(ref)/release()` dependency seam. The production
default always refuses with `artifact_authority_unavailable`; no route,
capability, environment variable, readiness flag or production fake can make it
available. Existing app composition does not supply an authority. Returning a
private `SpeechSlotRef` supplies identity only, not speech readiness or custody.
Only test fixtures supply a synthetic authority and explicitly assume synthetic
evidence for selected references. Those fixtures do not attest actual files or
qualify a native model/runtime.

A future real authority must retain canonical model and selected-artifact byte-
content manifests, runtime recipe/code identity, and exact owned profile/process
generation from load admission through slot unload or confirmed process
cessation. The operation lease borrows that already-attested slot-lifetime
custody; a new claim at transcription admission cannot retrospectively attest
bytes loaded earlier. The synchronous methods operate nonblockingly on retained
evidence, with no filesystem/DB work or async admission gap. Identity values,
paths, configuration, READY state and caller assertions are not evidence. The
interface intentionally does not implement storage, deletion protection,
manifest attestation, runtime qualification or process-cessation authority.

The admitted operation retains its borrow across scheduling and throughout
native work. Confirmed worker/device completion allows borrow release while the
device is still fenced, as part of successful lease exit; cancellation alone
does not. Proven non-start may also release a borrow. Exceptional context exit
or generator finalization cannot release the device. A borrow-release exception
quarantines the operation and retains the device lease. Exceptional native
startup and unconfirmed native cleanup retain the borrow with the original PCM/conversion buffers and device
lease, even through cancellation, expiry, drain and orderly loop shutdown.
Releasing an operation borrow is not slot-lifetime custody release, artifact
deletion authorization or a secure-erasure claim. Existing external process-
termination requirements for unresolved quarantine remain unchanged.

Verification of this checkpoint:

- 36 private operation tests passed, including synthetic artifact-release and
  lease-exit failure subprocesses and retained-borrow assertions in startup/device
  quarantine.
- 15 focused binding tests passed: unavailable production default before native
  work; strict slot envelopes; generation-bound duplicates/conflicts; stale,
  released and foreign references; real manager unload/reload to a READY
  successor in the start-to-worker gap; same-ID/object replacement; revalidation
  during lock acquisition; authority revalidation under the shared lock;
  cancellation custody; runtime-wide ownership; generation nonreuse/exhaustion,
  failed-load reservations and slot-ID collisions; bounded provider diagnostic
  codes; and unchanged name-based text lookup/public slot serialization.
- Existing Cohere adapter/lease tests: 15 passed. Model manager: 2 passed.
  Model load lifecycle: 7 passed.
- Whole Torch Ruff 0.15.2 checks and formatting passed (38 Python files).
- The broad suite ran 186 tests with the same 7 inherited
  `_IncludedRouter.path` errors in FastAPI route enumeration. No unrelated route
  tests were changed or hidden; this is not a full-suite pass.
- No routes, capability advertisement, Rust DTO/RPC/schema changes, dependencies,
  runtime recipes, live stores, model downloads or real native ASR qualification.
