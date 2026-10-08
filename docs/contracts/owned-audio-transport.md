# Private owned audio transport

This candidate supplies actual private load/use/unload transport. Its usable
execution scope is a controlled subprocess, not production ASR. The shipping
runtime qualification constructor refuses admission, and the established public
modality endpoint continues to report `unqualified_audio_runtime`. No separate
application transcription API or availability override is introduced.

## Exchange ownership

An exact managed child transfers its inherited stdin/stdout pipes once. Frames
contain a four-byte big-endian length followed by JSON. One retained reader owns
all original response reads; a serialized writer assigns monotonically increasing
IDs at admission. A pending settlement wait can overlap a finite cancel control.
The queue holds one request and at most 64 exchanges can be admitted. Requests
are bounded to 32 MiB and replies to 64 KiB; overflow refuses or quarantines.

Queued caller loss is checked before preparation and again before the first
wire effect. An admitted exchange retains its concrete custody through full
write/flush and the original correlated response. An early reply cannot settle
a partial write. No request is replayed. Original ID, operation, runtime UUID,
slot/load generation and operation UUID are validated, with closed receipt
schemas and duplicate JSON keys rejected at every depth. Partial I/O, malformed
or incoherent receipts and unknown native effects quarantine the channel and
request stop of its exact child. Valid original `not_admitted` RPC errors keep
the connection reusable; load/use/unload each apply their own non-start receipt.

`OwnedAudioClient` retains loaded slots and operation borrows. Losing a load
caller sends an independently correlated cancellation for that exact exchange;
its acknowledgement never releases the original load. Operation wait/caller
loss requests cooperative cancellation while the independently retained status
observer drains the original native settlement. Unload preparation can be
cancelled without fencing a ready slot; admission distinguishes an uncertain
wire effect. Original confirmed native unload or complete exact-child-tree
drain releases selected model bytes. Runtime code stays retained until the
child-tree drain. Mere EOF, HTTP closure or cancellation is not cessation.

## Controlled qualification

The Linux Rust process tests construct a genuine indexed Pumas model package,
acquire `PreparedArtifactUse`, and launch `ManagedChild` from a copied read-only
13-file Python code snapshot. The child receives a held selected directory FD,
reads all five fixed members, and computes a result from their actual bytes.
The controlled qualification binds the exact prepared owner with a weak opaque
allocation identity, so another root with identical selectors, names or hashes
cannot replace that inherited source. The weak binding does not extend the root
grant past validated native unload.
Native load, use and unload run through the same Python actor, private generic
modality projection and speech operation owner used by the private channel.
The original library root remains excluded from cooperating Pumas mutation and
the independent copied selected source stays retained until original native
unload or exact-child drain. The tests prove retained scratch,
root exclusion, same-slot reuse after clean cancellation/errors, and separate
runtime-code lifetime.

The controlled backend returns a byte-derived synthetic transcript. It parses
no tensors and performs no model inference. It uses only normalized 16 kHz mono
PCM16 in these process tests. Separate PCM tests exercise float/stereo conversion.
The fixture supplies controlled Torch/device behavior and runs isolated Python
without installed third-party dependencies. Its proof excludes the host Python
interpreter/stdlib, Torch, Transformers, NumPy's native libraries, GPU behavior,
real ASR and native macOS/Windows execution.

The detailed Cohere adapter preserves legacy string callers and adds explicit
native `stop`/`length` evidence. Missing or ambiguous terminal evidence is a
confirmed typed failure, never a fabricated stop. Fixtures exercise EOS, the
512-token bound and missing evidence; a real model remains unvalidated.

## Upstream integration and remaining gates

Import the private candidate over PR55 head `9e9ba63c022bce52c00256cd42f4a33afb0e6731`
and its private lifecycle parent `4e40ea0c0493547104dfd2c9985edd9ca3613596`.
The public PR55 author/committer is connector identity Puma; the separate local
format candidate used MrScripty and has the same source tree. Do not rewrite
published author history. Coordinate public changes with the root/integration
worker; this candidate is not published or merged.

Before exposing audio through the existing generic modality endpoint, the
owning runtime must qualify and retain the installed interpreter and complete
immutable dependency/loader code, materialize all imported modules in the
runtime recipe, and prove the real loader's full selected-model read set.
The bundled Transformers 4.57.6 recipe is not sufficient for this loader. Bind
the client and stop callback to that exact process generation, register its
private slot owner through the existing model-operation boundary, and derive
availability from this qualification rather than a reply or flag. Validate a
real model acquired through Pumas under the user's download/terms authorization;
no model acquisition or inference occurred for this candidate.

Use the recorded offline focused, aggregate and strict lint commands in the
private handoff. Pantograph consumers should continue using the existing generic
typed modality contract, retain caller correlation IDs and accept audio refusal
until the owning runtime gate is satisfied. Do not substitute HTTP status/cancel,
public paths, manifests or automatic retries for the private custody channel.

## Hung-peer and disposal limits

No production exchange, native-drain or generation timeout is invented. A hung
peer or native worker may retain custody indefinitely. Lost external callers
request cancellation but do not destroy the retained observer or prove native
cessation. An explicit owning-process stop/disposal must drain the exact child
tree; failed drain parks the real child and its composite lease. The low-level
channel's last admission sender closes and asks the exact stop callback to run;
retained in-flight native observers can keep owning-client references alive.
These scopes must not be confused with last external caller loss.

Poisoned bookkeeping deliberately retains root grants and scratch even after
child drain and requires owning Pumas-process disposal/restart. Normal unknown
transport/native cleanup can recover after exact-child-tree drain. Read-only
copies and held identity/hash validation do not exclude hostile same-user races
or catastrophic host exit. Test-only hang guards and process cleanup budgets are
harness safeguards, not newly invented production timeout policy.
