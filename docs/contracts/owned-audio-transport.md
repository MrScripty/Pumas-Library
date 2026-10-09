# Private owned audio transport

The private channel supplies actual load/use/unload transport. Its usable
execution scope is a controlled subprocess, not production ASR. The shipping
runtime qualification constructor refuses admission, and the established public
modality endpoint continues to report `unqualified_audio_runtime` in shipping
use. It now resolves an opaque loaded slot through the existing core runtime
profile service; only controlled integration fixtures can currently register
one. No separate application transcription API or availability override is
introduced.

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

Channel closure settles queued, permit-waiting and prepared-but-unadmitted
requests as `not_admitted`, without custody admission or wire writes. The final
admission check shares the pending-map lock with quarantine's drain: either a
closed channel refuses the claim, or quarantine retains the inserted admitted
entry as an unknown effect. Python load admission is marked at the actor's
one-shot plan claim, before launching work or constructing its returned status.
Use admission is marked when retained artifact-borrow custody has transferred;
unload admission follows clean refusal checks and observer allocation, at the
exact-slot unload claim. Routine pre-claim refusals preserve the original slot
and reusable channel. Post-claim exceptions retain unknown-effect custody and
cannot authorize replay.

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

## Installed byte custody and worker bootstrap

`VersionManager::retain_torch_runtime_bytes` reuses the installed version state,
existing `installed-files.json` package manifest and the existing Torch versions
mutation lock. A matching shared managed-Python depot lease excludes cooperating
provisioning for the retained interpreter tree. The returned core owner captures
actual selected interpreter, dependency and sidecar member bytes, identities and
held roots. Missing manifests, unreported package bytes, linked selected members,
changed identities or hashes refuse capture. This is selected-byte custody;
external system libraries and a complete real loader read set remain unqualified.
The runtime owner can retain this byte owner through the exact child cleanup
lease without changing audio qualification.

Installer validation and all runtime probes suppress Python bytecode generation,
including validation's sidecar subprocess. Isolated managed-Python, resolver and
read-only probe commands also use explicit bytecode suppression because `-I`
ignores Python environment flags. A fresh venv's bootstrap package bytes and
directories are snapshotted before resolution. They are removed only after the
entire bootstrap namespace is verified unchanged, before separately verified
resolved packages are moved in. A resolved package may therefore reuse a bootstrap
name without being removed or exempted from its dependency manifest. Existing
runtimes with unreported bootstrap material continue to refuse capture.

The installer-only `validate_runtime.py` member may be absent after direct or
resolved installation; when a bundled runtime retains it, its bytes must still
match the embedded source. Other required code remains mandatory. Capture still
rejects bytecode and unreported members; successful import checks do not weaken
the retained manifest.

The existing Torch installer materializes the owned worker modules. The private
worker starts under `-I -B -S`, accepts only three distinct inherited code,
package and model directory descriptors, and refuses ambient import roots,
`.pth`, customization hooks, bytecode and linked/special selected members.
Stdout is reserved for frames before native imports; diagnostics use stderr.
There is no new installer or decoded qualification flag. Linux descriptor
bootstrap isolation does not establish interpreter or native-library closure.

The generic route binds model/profile/Pumas-instance identity from the original
owned load. An alias resolves to that canonical selection before private use.
Admission is observed at the private writer; caller loss requests cooperative
cancel while original settlement retains custody. Confirmed non-start request
errors remain HTTP 400 and clean reuse is tested. Transcription has text output,
native stop/length evidence, no streaming, and the existing fixed 512-token bound.
Classification remains unsupported. Capability availability comes from the
retained slot, which is stronger evidence than advisory task metadata; metadata
cannot mint an endpoint. Unload, quarantine and child drainage close availability.
Controlled HTTP tests run through actual managed child, inherited descriptors,
bootstrap and original native status observation, without tensor inference.

The detailed Cohere adapter preserves legacy string callers and adds explicit
native `stop`/`length` evidence. Missing or ambiguous terminal evidence is a
confirmed typed failure, never a fabricated stop. Fixtures exercise EOS, the
512-token bound and missing evidence; a real model remains unvalidated.

## Production qualification and consumer contract

Before shipping audio is available through the generic endpoint, the runtime
must qualify and retain the installed interpreter and complete immutable
loader/dependency code, materialize every imported module in its runtime
recipe, and enforce the full selected-model read set. Bind the client and stop
callback to the exact process generation and register the private slot owner
through the existing model-operation boundary. Derive availability from this
qualification and retained slot, rather than a reply or caller-supplied flag.

Consumers use the generic typed modality contract, retain correlation IDs and
accept audio refusal until runtime qualification is satisfied. Obtain the
pinned model through Pumas's authorized acquisition path, verify every selected
member, and qualify real native load, finite inference and disposal. The
proposed model revision, member verification and compatible runtime requirements
are documented in [Cohere qualification](cohere-native-qualification.md).
The bundled image recipe uses Transformers 4.57.6; the native ASR profile pins
Transformers 5.4.0. Controlled transport tests do not establish real model
compatibility, transcription quality or complete native read containment.

HTTP status/cancel requests, public paths, manifests and automatic retries
cannot replace the private custody channel.

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
