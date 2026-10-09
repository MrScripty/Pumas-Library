# Owned Linux native cohort

The optional audio preparation path selects an additional `NativeLibraries`
read-source role. This is a concrete byte owner, not production admission.
`VersionManager::prepare_torch_audio_runtime_bytes` first retains the installed
Torch selection, downloads the exact source-owned native recipe, and captures
all four role roots while retaining the original mutation leases.

## Provenance and scope

The initial cohort is Debian trixie amd64, pinned in
`pumas-app-manager/src/version_manager/audio_native_cohort/recipe.json`.
Each package has an exact official HTTPS URL, version, byte size and SHA-256.
Request data cannot choose these fields. Downloads refuse redirects, excess
bytes and mismatched hashes. The package index used for selection is identified
in that source file; it is not consulted dynamically during installation.

The 14 archives supply glibc and its matching ELF loader, GCC runtime libraries,
zlib, libcrypt, TBB, hwloc, libudev, libcap, and copyright/common-license texts.
Only the fixed native-library and notice namespaces are extracted. Package
maintainer scripts, package-manager metadata/configuration, global loader
configuration and unrelated executables are never installed or executed.
Sibling library aliases are copied as regular files from their pinned targets;
archive links are never materialized into the filesystem.

The selected files and provenance JSON occupy a fresh private staging root.
The retained-source owner holds that root's lifetime. Every selected file is
hashed again by the ordinary runtime read-source capture, and the root survives
until its final custody owner is released. Failed staging/capture does not
publish a partially admitted runtime. No ambient `/lib` or `/usr/lib` contents
become selected through this procedure.

The loader pair is intended for direct owned-loader invocation with its cache
inhibited and a selected library search directory. Merely constructing this
cohort does not launch that command or prove the complete dynamic read closure.

## Inert interpreter/package members

The owned worker runs with `-I -S -B`. Fixed import-hook, bytecode and
`__pycache__` namespaces are identity-tracked exclusions, not content read
capabilities. Selected `.py` modules use a source-only loader, bypassing valid
but unselected bytecode caches. Hook/bytecode files remain forbidden in the
selected model namespace. Native confinement must grant only selected files;
no recursive directory content grant may re-admit excluded bytes.

## License evidence

Package-specific copyright notices and Debian common-license texts are retained
beside the library bytes. GCC's notice includes its Runtime Library Exception;
glibc and other packages include per-file licensing details rather than a
single blanket license. These files preserve upstream evidence. They do not
constitute a legal review or approval to redistribute the resulting bundle.
Any release that redistributes binaries must satisfy the applicable notice and
source-availability requirements for the exact shipped files.

## Outstanding qualification

The native cohort does not turn the shipping policy catalog on. Qualification
still requires the exact interpreter and Python wheel cohort, the complete
confined child/session owner, a supported Linux Landlock ABI 6+ host, the
selected real Cohere model bytes, and real load/transcribe/cancel/caller-loss/
unload/retry lifecycle tests. Archive extraction, ELF inspection and fixture
custody tests must not be reported as real transcription or positive kernel
enforcement.

## Fixed candidate identity and owned command

Native preparation is restricted to the source-pinned candidate in
`pumas-core/src/runtime_read_source/audio_candidate_recipe.json`: the managed CPython identity, all 68
exact wheel artifacts, and framed hashes of the 3,487 interpreter and 23,630
selected dependency members. Comparison follows actual byte capture and is
followed by retained-source revalidation. Metadata alone cannot satisfy the
selected-byte hashes. This fixes candidate identity; it does not register a
production policy or prove native execution/lifecycle behavior.

The private installed spawn primitive requires an already-qualified runtime
owner bound to the exact prepared model allocation. It executes the held native
loader and interpreter through inherited descriptors, inhibits the loader
cache, streams only selected file/directory rules, and revalidates source bytes
before spawn. The exact managed child immediately retains both runtime and
model lifetime owners before diagnostic-pipe extraction can fail. Uncertain
child drainage therefore retains those owners in the original custody slot.
The control-channel/session producer must still drain diagnostics and bind its
owning native channel before publishing an endpoint. The shipping catalog
remains empty, so this primitive cannot admit the installed candidate today.


## Explicit confined dependency qualification

The `test-support`-only `qualify_audio_dependencies` example installs the existing
managed CPU recipe into an explicit empty isolated root. A read-only kernel
preflight refuses unsupported hosts before acquisition. It then retains the
fixed CPython, dependency and native selections, rehashes each against the
source pin, and runs only a constant probe through the owned loader and actual
native read boundary. No caller program or model path is accepted.

The probe requires denial of an unselected canary read/write and ambient socket,
checks read confinement in a spawned thread, imports Torch, Transformers and the
Cohere adapter classes plus native audio dependencies, and performs a CPU tensor
operation. Output is bounded to 32 KiB per stream; the read deadline is 120 s,
followed by bounded exit observation and child-tree drain. Success requires both
probe success and confirmed cleanup. Cancellation retains custody for cleanup.

Run on a supported Linux x86_64 host (external workflow timeout should bound the
public dependency acquisition too):

```
cargo run --locked --manifest-path rust/Cargo.toml -p pumas-app-manager \
  --features test-support --example qualify_audio_dependencies -- /absolute/empty/root
```

The JSON report explicitly has `production_available: false`. This gate proves
only confined dependency startup and CPU operation, never model execution, ASR,
full session cancellation or production catalog admission.

### Installation-root reproducibility

Two independent official managed installations produced the same 68 artifact
identities. Their dependency inventories differed only in 25 generated console
entrypoints and the 17 corresponding RECORD files. The recipe names those 42
exact relative paths as inert dependency inputs: full installer validation and
retained namespace identity still apply, but content reads are not granted.
All other dependency metadata remains selected. Both installations yield the
same 23,588-member dependency selection after this treatment.

The only differing selected interpreter member was the UV-relocated
`_sysconfigdata__linux_x86_64-linux-gnu.py`. It remains selected with its actual
bytes and actual hash. Recipe comparison replaces exactly 27 occurrences of the
JSON-escaped prefix derived from the held interpreter directory capability, then
requires a fixed normalized size/hash. Prefixes are restricted to ASCII letters,
digits, slash, dot, underscore and hyphen; quotes, backslashes, controls and
other path characters refuse. This narrows qualification scope and rejects otherwise
legitimate roots with spaces/non-ASCII; it is not evidence of an exploit. The
actual two-root test parses every affected value as a double-quoted JSON string
and accounts for all 27 substitutions without executing the source. Every byte outside those precise prefix
positions is fixed; arbitrary code normalization, receipt-supplied prefixes and
omitting executable source are prohibited. Both independently installed
3,487-member interpreter cohorts satisfy this comparison. This proves portable
recipe comparison, not successful confined execution.
