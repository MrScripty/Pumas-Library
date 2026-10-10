# Conditional installed-audio construction

The conditional installed-audio constructor supplies source plumbing for the
existing private provider load path. It does not qualify or activate a
production audio runtime. The shipping policy catalog has no qualified entry.
Neither a candidate, an installer report, a child handshake, a path nor a
caller-provided manifest creates one.

The input remains an `InstalledAudioRuntimeCandidate`: actual retained
interpreter, dependency and sidecar capabilities, their cooperative mutation
leases, and the exact prepared model allocation. A conditional constructor must
match its source-owned policy against actual held bytes, preserve that exact
allocation binding, and transfer runtime custody into the child's existing
composite owner. Runtime ownership must not keep a strong model reference past
confirmed unload. Equal names or content cannot replace another prepared owner.

The fixed positive Rust policy exists under `cfg(test)` only. It is absent from
shipping builds, including builds using `test-support`. It validates a closed
controlled interpreter/dependency/member set and embedded worker code; its
synthetic bytes are not a real interpreter, recipe or model. The shipping
constructor still refuses when no source-owned execution qualification exists.

The Python conditional gate reuses `OwnedLoadPlan`, `OwnedAudioActor` and the
existing inherited private channel. Its original model, source and runtime
identity come from a private retained source. Wire requests can compare these
values but cannot supply a policy, path, qualification flag or source owner.
The default factory remains unavailable, and the existing hello expectation
remains `production_available=false`. A positive child boolean alone cannot
authorize a parent owner or change its qualification scope.

Native acquisitions must enter load custody as each feature extractor,
tokenizer, processor and model is obtained. If a native constructor raises,
returned Python objects do not establish the disposition of unknown internal
native allocations. The gate retains the partial acquisitions and error and
refuses confirmed cleanup/release. The actor's existing quarantine and exact
child-tree drainage behavior remains the recovery boundary. Successful
controlled loads exercise normal actor cleanup; they do not establish the real
installed loader's native disposal behavior.

## Held selected model reads

The private installed gate now requests `model_source` from the original
source-owned policy, rather than a model directory. It requires a
`HeldCohereReadSource` retaining that same original source object, validates the
source before and after capture, and puts the reader in load custody before
native construction. The reader retains original read-only regular member
descriptors and observes their inode, size and streamed SHA-256. Each selected member is copied from its
held descriptor to a Linux memfd, checked against the original owner's captured
size and SHA-256 (never a rebased observation during capture), and sealed
against writes, growth, shrinkage and seal removal before a read-only descriptor
escapes. The original source is revalidated after capture. There is no mutable
file fallback if memfd creation or sealing is unavailable. Its hashes observe
bytes; they do not manufacture source ownership or qualification.

The loader parses only these sealed descriptor snapshots and maps sealed weights.
A later writer to the original file cannot change an existing model mapping.
The original descriptors remain retained and revalidated, so this immutable view
does not permit replacing the original allocation or ignoring source mutation.
Capture failure closes partial copies; successful copies remain under native
load custody until confirmed disposal. Kernel-backed regression tests exercise
actual read mappings, denied writes/resizes/shared writable mappings, source
mutation and partial-copy cleanup with synthetic bytes. A deterministic
mutate-before-copy/restore-after-copy regression verifies that the original
proof digest cannot be replaced during reader capture. They do not run ASR.

Sealed snapshots add up to one full selected-model copy in shmem in addition to
the existing prepared copies and native allocations. Capacity, memory pressure
and actual model startup must be qualified with the intended installed model;
allocation failure is a refusal, not permission to use mutable weights.

The owning loader parses bounded held descriptor bytes and constructs fixed
native config, feature extractor, tokenizer backend and processor classes.
Tokenizer primitive options and redirects are refused before native import;
supported token declarations retain their properties and must match IDs in the
selected tokenizer. Unsupported or conflicting optional descriptor semantics
are refused, rather than silently omitted. Generation settings come from the
selected `generation_config.json`, or the selected model config when that
generic optional member is absent. The official Cohere revision still requires
the eight selected members documented in the acquisition plan; the generic
fallback does not qualify a five-member package as that revision.

Safetensors' filename-based mmap API receives only the fixed Linux
`/proc/self/fd/<sealed weights descriptor>` capability link. The owning model call
receives `None`, explicit config, held state dictionary and explicit generation
config; no package-directory/cache locator is passed to it. CPU device placement,
eager attention, automatic selected-weight dtype, local-files-only loading and
disabled remote code are explicit. Returned weights and model/loading-report
tuple enter custody before compatibility checks or evaluation. Incomplete or
unrecognized loading reports refuse readiness. The original source and reader
remain retained on constructor or disposal uncertainty; the reader closes only
after the original policy's confirmed native disposal.

This removes directory/cache resolution requested by the private owning model
loader and prevents mutation of the selected sealed inodes. It does not remove
interpreter or native filesystem reads, isolate hostile code in the same process,
or enforce the complete execution read closure. The legacy `load_cohere_asr`
path remains a directory-based adapter.
Pinned upstream source supports the owning call shape; no installed native
Transformers, safetensors or Cohere execution was qualified by this change. See
[Transformers 5.4 model loading](https://github.com/huggingface/transformers/blob/v5.4.0/src/transformers/modeling_utils.py),
[explicit generation config handling](https://github.com/huggingface/transformers/blob/v5.4.0/src/transformers/generation/utils.py),
[in-memory tokenizer construction](https://github.com/huggingface/transformers/blob/v5.4.0/src/transformers/tokenization_utils_tokenizers.py),
and [safetensors' Torch API](https://huggingface.co/docs/safetensors/api/torch).

## Production requirements

Adding a real policy requires independently qualified evidence for the exact
interpreter and entire dependency recipe, enforced native loader and model read
containment, and pinned real native execution and lifecycle acceptance. A static
ELF dependency graph, import filter, import probe or self-consistent
`installed-files.json` does not discharge these requirements. A policy cannot
replace enforcement with an approval boolean.

The Cohere acquisition and runtime requirements are documented in
[Cohere qualification](cohere-native-qualification.md). The selected model,
complete native dependency cohort and enforced read boundary remain unqualified.
A platform ABI probe or successful download cannot establish that boundary.

Local positive/negative fixture tests qualify only the source plumbing,
ownership transfer, identity comparisons, partial-load retention and refusal
behavior. They perform no real Cohere inference and do not activate production
availability.

## Mandatory Linux native read boundary

The production constructor first queries native confinement availability. Only
Linux x86_64 with Landlock ABI 6 or newer can proceed to recipe qualification;
an unsupported target, unavailable syscall or older ABI returns
`ReadConfinementUnavailable`. This query changes no host policy and never grants
admission. A supported kernel still returns `UnqualifiedRuntime` while the
shipping recipe catalog is empty.

The private platform boundary prepares exact held-file rules in the parent and
applies them before exec in the child. Directory grants permit enumeration,
not recursive file-content reads. Inherited descriptors must belong to granted
inodes; other descriptors become close-on-exec. New control/diagnostic pipes
replace ambient standard streams. The child drops capabilities, sets
no-new-privileges, applies Landlock, then installs a seccomp filter denying
ambient socket creation/connections, System V and POSIX IPC, cross-process memory
and descriptor acquisition, namespace/mount manipulation and process-group
escape. A narrowly admitted AF_UNIX stream socketpair supplies asyncio's
private wakeup channel; bind/connect/listen/accept remain denied. Any failed
confinement operation aborts spawn; no weaker fallback is supported.

This boundary does not select or trust a runtime recipe, make granted bytes
immutable, or implement the complete installed child owner. It is not wired to
an admitting shipping policy. The unignored tests establish rule construction,
held-descriptor validation and truthful refusal. The ignored native execution
test must separately pass on a host exposing ABI >=6 before kernel enforcement
is claimed. Its mapped-library selection is only test setup, never production
recipe authority. Trusted interpreter/system loader/dependency custody and
actual pinned-model lifecycle evidence remain required.

The Linux sealed-model reader uses the stable Linux UAPI seal commands even
when managed CPython omits their optional `fcntl` constants. It still requires
`memfd_create`, successful addition and readback of all four kernel seals, and
unchanged selected bytes. Conflicting exposed ABI values and missing/denied
kernel support refuse; ordinary temporary files are never substituted.

Rule construction streams exact grants into Landlock and closes each input
handle after the kernel retains its inode rule. This supports a verified
installed read set larger than the process descriptor limit without weakening
selection bounds. Explicitly inherited descriptors remain owned through exec
and must match granted inode identities. Kernel rules do not replace the
original install/model ownership and mutation leases.

The installed-owner spawn primitive now builds its command from the retained
native loader, interpreter, dependency, copied-code and copied-model
capabilities. It streams exact grants, revalidates current bytes before spawn,
and attaches original runtime/model lifetime owners to the exact managed child
before any fallible diagnostic-pipe extraction. It does not accept caller paths
or permission receipts. Only an already-qualified `Installed` owner can reach
this operation; the shipping policy catalog still refuses that owner. This is
not yet a completed production control-channel/session producer or real ASR
qualification. See [owned native cohort](cohere-native-cohort.md) for the fixed
candidate identity and remaining gates.

### Installed bootstrap source correlation

The owned launcher supplies the original prepared allocation's model and artifact
labels alongside its inherited model directory. These labels grant no authority.
The Python source owner opens only the closed Cohere read set as read-only,
non-link regular files, retains directory/file identities, and rehashes actual
held bytes. Directory enumeration is bounded by the nine supported member names.

Its correlation identifier is `pumas-cohere-owned-v1:` followed by the existing
Rust prepared-manifest SHA-256: domain `pumas-selected-artifact-bytes-v1\0`, each
UTF-8 model/artifact label framed by its u64 big-endian byte length, then sorted
members framed by UTF-8 name length/name, u64 big-endian size and ASCII SHA-256.
The parent sends its independently owned identifier on load; the private gate
compares it with the original source proof. Neither this ID nor CLI labels can
satisfy qualification or substitute another prepared allocation.

The conditional bootstrap factory checks the source-owned policy catalog before
opening model members. The shipping catalog is still empty, preserving the
unavailable handshake. A registered source policy may retain exactly one proof,
obtain the source-bound sealed reader, and release descriptors only after its
native disposal contract is satisfied. Unexpected proof/owner loss leaves
potentially admitted inputs for exact child-tree teardown. Fixtures exercise
these ownership/correlation steps without qualifying real inference or disposal.
