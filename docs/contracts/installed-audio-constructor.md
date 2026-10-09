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
held descriptor to a Linux memfd, checked against that observation, and sealed
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
mutation and partial-copy cleanup with synthetic bytes. They do not run ASR.

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
