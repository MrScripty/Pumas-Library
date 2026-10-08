# Bounded owned audio lifecycle

The selected-byte and PCM contracts use private ownership primitives for
native audio. The generic operation endpoint exists, while production audio
admission remains unavailable. Controlled workers establish the ownership state
machine; they do not establish that an installed model consumed selected bytes.

## Required ownership path

1. Qualify the installed interpreter, immutable runtime/loader code and complete
   processor/model read set. Acquire the selected `PreparedArtifactUse` owner
   and validate its held read source before provider effects.
2. Reserve serving/load admission and retain that owner under the exact managed
   child generation. Attach one composite cleanup guard immediately after spawn,
   before PID/readiness publication. Bind the runtime instance through a private
   inherited owning channel, never public request JSON.
3. Transfer a one-shot, nonserializable load plan before starting native work.
   The native actor retains device ownership and load custody independently of
   the caller. A successful native load binds the actual loaded object, runtime
   instance, slot and load generation. Lost/cancelled acknowledgement cannot
   create a READY replacement or release in-flight custody.
4. Project the existing generic typed operation contract into one exact slot.
   Normalize the actual declared PCM; admit once through the existing speech
   operation owner. Cancellation requests cooperative stopping and retains the
   worker, PCM, device lease and artifact borrow until observed settlement.
5. Close slot admission and require zero borrows before native unload. Release
   the slot owner only after observed worker exit, device synchronization and
   object disposal, or complete exact-child process-tree drain. Partial/unknown
   exchanges and failed cleanup retain custody without replay or retargeting.

## Implemented private boundaries

`runtime_profiles::audio_custody` binds retained records to the exact native
child. The shipping runtime qualification factory refuses
`UnqualifiedRuntime`; only unit tests can qualify a fixed controlled-process
code snapshot and complete fixture read set. Prepared-byte admission requires
that opaque runtime owner; decoded JSON cannot create one. Loads
are distinguished before and after their first wire effect. Opaque slot handles
allow exclusive in-memory operation borrows and exact unload settlement.
Each borrow binds the original native operation UUID and requires that same UUID
at completion; an earlier same-slot receipt cannot settle a successor borrow.
A confirmed-clean original failed-load receipt can release its admission and
reuse the child. An RPC error alone does not establish that cleanup.
Dropping an admitted handle preserves uncertainty. The child cleanup guard
survives failed process-tree drainage with `ManagedChild`'s parked custody.

`audio_runtime::installed::InstalledAudioRuntimeCandidate` now provides
non-admitting ownership preparation. It requires actual retained interpreter,
dependency and sidecar role capabilities, retains their cooperative mutation
leases, and owns the exact `PreparedArtifactUse` allocation and its selected
member set. It refuses a missing retained interpreter member, missing or changed
required worker code, import hooks and bytecode, and revalidates actual runtime
and copied model bytes before any later provider effect. Worker code identity is
checked against bytes embedded in this source build; equality does not establish
a complete executable read closure. Equal model manifests cannot retarget the
candidate to another prepared allocation. Dropping the candidate releases its
last custody references only when no other owner remains.

This candidate has no qualification flag or deserialization. The constructor
`AudioRuntimeOwner::for_installed_runtime(candidate, &selected)` implements
conditional ownership transfer from held capabilities, but its shipping policy
resolver still refuses. A fixed positive policy exists only in unit tests;
neither shipping nor `test-support` builds contain that policy. Remaining
requirements are a trusted interpreter/dependency recipe, complete native loader
read closure,
complete model read containment, and pinned native execution/lifecycle
acceptance. Captured selected trees and sidecar equality discharge none of these
requirements. Controlled candidate tests use dummy interpreter/dependency bytes
and synthetic unparsed weights; they establish custody and refusal behavior,
not an installed Cohere runtime or real transcription.

The conditional Python gate uses the same private provider protocol and staged
native acquisition. It retains each returned object and refuses cleanup when a
constructor leaves unknown native allocations. Its shipping catalog is empty;
the default factory and hello expectation remain unavailable. See
[conditional construction](installed-audio-constructor.md) for the source scope,
fixed fixture evidence and remaining production proof requirements.

`owned_audio.OwnedAudioActor` owns native load and unload independently of caller
tasks. Its unavailable default gate performs no model load. Controlled gates
test pre-start custody, exact loaded-slot authority, cancellation, device
cleanup and quarantine. Legacy arbitrary-path slots cannot acquire this actor's
authority, bypass its unload guard, or escape as raw inference models.

`owned_model_operations.OwnedModelOperations` privately projects the generic
contract to the existing speech operation owner. Transcription requires text;
classification requires labels and remains unsupported by the transcription
adapter. The current native primitive has a fixed 512-token bound, so other
requested bounds are refused rather than ignored. Status and cancellation use
local opaque handles; they cannot install a load plan or artifact authority.
Caller wait cancellation requests native cancellation without releasing custody.

The public Rust typed endpoint validates the same transcription/classification
output distinction and still reports audio unavailable before provider I/O.
The existing generic modality route now resolves a privately owned slot. Shipping
audio remains unavailable; no public status/cancel route, qualified recipe, model
download or production capability is added. See the transport contract for the
later controlled integration boundary.

## Concrete qualification blockers

The native dependency cohort and speech model remain unqualified. The bundled
image recipe pins Transformers 4.57.6, while the native ASR loader requires the
supported Cohere ASR implementation from Transformers 5.4.0. The
installer now materializes the speech operation owner, native audio loader,
PCM helpers and actor/bridge, and installed-byte custody retains their actual
bytes. A separately qualified compatible dependency recipe and native execution
closure remain required; the existing Torch version check is not a lasting
loader-code custody lease. The pinned model and access prerequisites are in
[Cohere qualification](cohere-native-qualification.md).

The prepared package's closed copied-member set does not yet prove the complete
installed Transformers read set, including path-valued configuration. The
private inherited-stdio transport binds the parent prepared owner to the
controlled actor plan and matches original load/use/unload replies.
Controlled real-process fixtures qualify that boundary; they do not qualify the
installed interpreter, third-party dependencies or real model loader read set.
A JSON manifest, path, slot receipt or `qualified=true` cannot substitute for
that missing production qualification. See [owned audio transport](owned-audio-transport.md).

The detailed native adapter now derives `stop` or `length` from observed
generated token IDs and the fixed generation bound. Ambiguous or missing
terminal evidence refuses typed success; legacy text callers keep their prior
behavior. Controlled fixtures exercise both finish reasons and refusal. Real
Cohere model execution and its terminal behavior remain unvalidated, so this
does not enable the public typed adapter.

## Drain and disposal limits

There is no invented generation/drain timeout. A hung native load/use/cleanup
can retain device and artifact custody indefinitely; the exact process owner
must stop and observe complete child-tree drain to recover. Cleanup failures
quarantine the slot and prevent successor work. Cancellation, HTTP closure,
caller loss, an empty cache or dropped registry rows do not prove cessation.

The ordinary last-owner path asks the existing process owner to stop; failed
drain parks the actual child and composite lease. Unresolved bookkeeping is
retained even on poison or unexpected last-owner loss. Poisoned bookkeeping
deliberately leaks the root grant and scratch even after confirmed child drain;
that exceptional state requires owning Pumas-process disposal/restart. Ordinary
unknown native/transport cleanup remains recoverable by exact child-tree drain.
Catastrophic host exit,
malicious same-user mutation and native Windows/macOS execution are outside the
Linux controlled-fixture evidence. No source-level fixture establishes real
ASR quality, GPU behavior or production readiness.
