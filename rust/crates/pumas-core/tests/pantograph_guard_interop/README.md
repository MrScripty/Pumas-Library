# Pumas → Pantograph guard interoperability

This standalone harness calls the unchanged `SelectedAudioLoad::validate` from
Pantograph commit `038dacaaa98ebd007c32e13d4608726ca5ccf63a`. It compiles against
that checkout's actual inference types and the current local Pumas library. It
also compiles the pinned host's unchanged entry-path privacy-normalization functions.
The runner checks the commit, guard blob, and tracked source cleanliness.

The fixture registers a synthetic managed Cohere package, resolves package facts
and an owner-fresh target through production Pumas APIs, serializes their actual
wire output, and adapts it to Pantograph's contracts. The positive admission,
primary-weight path regression, absent/blank fingerprint, model/artifact/path/
revision/contract/storage mismatch, and changed-package observation tests all
call the real consumer guard. The synthetic weight bytes cannot load a model.
No inference backend, model download, model loading, or credentials are used.

With an existing clean public checkout at the pinned commit and Rust source
dependencies cached:

```sh
ORT_SKIP_DOWNLOAD=1 python3 rust/crates/pumas-core/tests/pantograph_guard_interop/run.py \
  --pantograph /path/to/Pantograph
```

The default execution is `cargo test --offline --locked --lib -- --show-output`,
which records the observed scope/mode matrix for passing tests. The separate
`--resolve-source-dependencies` option permits public Rust source dependency
resolution for initial setup; it keeps all inference backends and Pumas's ONNX
runtime disabled. A retained `--work-dir` exposes the generated manifest, lock,
and module-path adapter for review. This harness tests admission and package
observation, not the consumer's private resident cache or real Cohere inference.

The scope/mode matrix retains one unmodified, production-generated cache row at
entry and sends the serialized real resolver response to the unchanged guard.
Summary rows are obtained through the production summary API. For indexed tests,
the competing cache scope is absent and all cache-row fields are unchanged after
resolution. OwnerFresh legitimately regenerates/repairs Summary before the shared
summary-first resolver; a detail-only input therefore accepts Summary after repair.

| Cache evidence at entry | Resolution mode | Accepted cache scope | Cache behavior |
| --- | --- | --- | --- |
| Summary only | OwnerFresh | Summary | Production observation also generates Detail |
| Detail only | OwnerFresh | Summary | Production observation repairs Summary |
| Summary only | ReadOnlyIndexed | Summary | No row mutation; Detail remains absent |
| Detail only | ReadOnlyIndexed | Detail | No row mutation; Summary remains absent |

Only negative guard tests manually mutate producer targets. Positive and matrix
tests obtain their path, identity, revision, fingerprint, and descriptor from
actual Pumas resolver DTOs; the existing host privacy projection removes selected
path data from the scheduler identity without changing the executable target.

Separate root-alias tests open the actual `PumasReadOnlyLibrary` through a relative
root and, on Unix, a symlink root. Both accept unmodified production Summary and
Detail cache rows, serialize real read-only resolver responses into the actual
guard, compare the entire projected target against the owner-produced identity,
and verify that cache rows remain unchanged. The relative fixture is created
inside the existing working directory; neither test changes the process working
directory or requires network access or a Windows share.

For a regression comparison against another existing producer checkout,
`--pumas-core /path/to/Pumas-Library/rust/crates/pumas-core` selects that source
without editing it, and `--test-filter actual_read_only_` restricts execution to
the two root-alias cases. The runner prints the producer manifest path explicitly.
