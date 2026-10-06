# Acquired finite offline-catalog experiment — 2026-10-06

The measured public uv adapter can select dependencies from an explicitly owned,
finite wheel universe after shared acquisition and actual-byte inspection. The
24 controls pass 220 assertions, with six exact positive selections and terminal
refusals for incomplete catalogs, source references, substitution and ambiguous
output. This is evidence for a bounded adapter contract, not production migration
or automatic source-safety acceptance. Parent rejects the preceding live-index uv
experiment as P1 closure. Existing automatic/preview P1 remains undispositioned;
the finite qualified recipe needs no uv replacement.

## Exact admission and frozen inputs

Separate branch `experiment/torch-offline-catalog-eed27495` starts at
`eed27495137fbd9cf933fd8f3d827fd66ea591ca`, tree
`3242e15d63bbe7b131d6b08934b4e41be7ddecdb`. Tested experiment source milestone:
`8eb93f5a61d6047e5d4bdeeaad47f6714494b887`, tree
`97bf6d38146cf7c5d91f943be549985adb2adc67`. Its compiled Rust source exactly
matches the committed source hash; final Python control hashes are in
[provenance](torch-offline-catalog-2026-10-06/provenance.json).

Unchanged refs: origin/main `5e114f6d8e4559e0a4d67e56000b423120a0fde0`;
composition `a1da98aaca3a2a63beae6a99edeaeba6138c62ec`; reviewed finite candidate
`6308e361934fc15bd35773cfd7a3747063885fc5`; live-index evidence
`eed27495137fbd9cf933fd8f3d827fd66ea591ca`. All production/source, contracts,
workspace Cargo/Node manifests/locks, pins, recipe, native cleanup and earlier
evidence remain unchanged. Writes are the pre-recorded admission, this report,
adjacent experiment/evidence, and an owning-plan status link. No applicable
AGENTS.md or filesystem skills exist in the checked workspace. No PR/main merge,
reviewer contact, new production dependency or private-tooling hook activation.

The tool is the already installed official uv0.12.23 Linux binary, SHA-256
`abdc39eab8b4ad341dca91f3823a23a343fae94bdb22ebdd9e91694415206f2f`.
Its original wheel/RECORD/official-source and MIT OR Apache-2.0 licence evidence
remain in the [prior provenance](torch-public-uv-evaluation-2026-10-06/tool-provenance.json).
Inspection uses existing licensed standalone packaging26.3 public APIs from
`torch-server/tooling/packaging.zip`, SHA-256
`3453711fd71407b7d84253d5b498e7c0430fff187e3fb73dbd6f463b924014b2`, with
existing RECORD-derived [source evidence](../../../../torch-server/tooling/packaging-source.json).
Python3.12.14, rustc1.92.0, strace6.13 and exact effective external manifest/lock
are recorded. The external lock introduces only the experiment root package;
every dependency identity already exists in the frozen production lock. This
does not qualify release supply-chain acceptance or a new runtime dependency.

## Measured adapter contract

1. The owner declares a finite catalog: candidate IDs, original anonymous remote
   URLs, exact filenames/names/versions, expected bytes/SHA-256, root requirements,
   target and explicit completeness. Source grants enumerate exact initial and
   redirect URLs. Fixture authority admits only literal127.0.0.1 HTTP with no
   userinfo/query/fragment. It does not weaken production HTTPS/auth policy or
   permit metadata to grant sources. Redirects require exact membership and a
   three-hop bound; proxies are disabled. No credential or remote account exists.
2. The external Rust harness calls public `AcquisitionConsumer::acquire_http`,
   using `ArtifactManifest`, `ReservedDirectory`, verified SHA-256 requirements,
   one attempt and a ten-second per-file transfer budget. It opens every file
   through `AcquiredArtifactUse::open_file`. The shared `Using` lease and workspace
   remain held while registered `run_blocking` joins inspection and the CLI child.
   No alternate Python payload downloader or acquisition implementation exists.
3. Before any uv invocation, inspect **all** acquired candidates, including
   ineligible/unselected versions. Recheck actual size/hash, filename identity,
   unique METADATA/WHEEL and dist-info identity, validated public Metadata,
   WHEEL/filename tag equality, Requires-Python, every Requires-Dist and fixture
   RECORD coverage/hash. Reject every dependency URL/VCS, including inactive
   marker branches. Reject undeclared local entries, symlinks, unsafe ZIP names,
   duplicate entries and unsupported metadata. This bounded fixture admits at
   most64 wheels of at most1MiB each, at most128 members per wheel and at most1MiB
   per expanded member. These limits are experimental, not new production quotas.
4. Retain the original catalog/root declarations unchanged. Write
   `pumas.experiment.offline-input.v1` projection with original URL/hash/ID to
   local acquired file mapping, inspected tags/eligibility and target environment.
   For an approved direct wheel root only, replace the solver input's locator with
   its exact local file URI and the same digest. Preserve extras and markers;
   independently bind its selected version to the original root. Do not rewrite
   acquired METADATA or the public uv lock. Named dependencies use only the finite
   owned directory. This projection is explicit solver input, not remote provenance.
5. Invoke public `uv pip compile` with `--offline --no-index --find-links` on that
   directory, `--no-build`, `--no-config`, `--no-cache`, `--no-sources`, disabled
   keyring, disabled Python downloads/managed Python, explicit trusted interpreter,
   Python3.12 and explicit Linux-manylinux2.17 or Windows target. Use fresh owned
   requirements/output and isolated HOME/XDG/config/cache/credentials/temp paths;
   inherit no UV/PIP/proxy/PYTHONPATH settings. NETRC and pip configuration are
   disabled. uv rejects combining `--no-build` with `--only-binary`; the final
   invocation uses the supported no-build flag. Any nonzero/unclassified exit is
   terminal/inconclusive, with no candidate, source, online, tool or interpreter
   fallback. The measured timeout kills and joins the child process group.
6. Read unchanged public PEP751 `lock-version=1.0`, `created-by=uv` output.
   Every applicable selected entry must map by exact local path, name/version and
   SHA-256 to one inspected owner candidate. Reject source/VCS/directory entries,
   missing hashes, escaped locators and multiple compatible files for one package.
   Independently verify actual selected requirements/extras/markers and direct
   roots, reject unrelated packages, then reinspect all wheel bytes/namespace.
   The result is an experimental packet containing selected original identities,
   projection/lock digests and observations. It is **not a fabricated pip report**.
7. Publish an **experiment-decision** consumer receipt, then observe shared
   `Adopted` settlement and explicitly shut down the consumer/service. Receipt
   manifest, acquisition/use IDs, operation and all verified file digests equal
   the held-use record. A refused decision can complete this experiment consumer;
   it is not a runtime installation receipt or authorization. Acquisition refusals
   retain the shared state and produce no inspection/solver call. No wheel install,
   runtime, GPU/provider or application API changes occur.

## What complete means here

Complete means **complete for the declared finite allowed-object universe and
bounded request**, never complete for PyPI/an index, all upstream versions, or
arbitrary future dependencies. The owner must explicitly declare the universe
complete. Every declared object is acquired and inspected, the local namespace
equals the declaration, and every active root/dependency has an eligible declared
candidate under this target. Coverage conservatively traverses every eligible
version for reachable names and unions requested extras. This intentionally can
refuse a satisfiable request when an unselected version needs an absent candidate;
it does not silently narrow or supplement the universe. The solver then decides
version consistency within that universe, and independent selected-closure checks
must pass. A missing candidate/declaration is inconclusive/incomplete, not proof
that a release is globally incompatible and not permission to try another source.

The measured target model is CPython3.12 with synthetic Linux/Windows metadata
and explicit tags; sys_platform/extra markers, Requires-Python and ABI/platform
tag selection are exercised. It is not native Windows execution or complete
equivalence for every marker variable, target, libc/ABI or Python patch policy.
Real migration must define the entire target environment and reconcile it with
uv's public target semantics before claiming completeness.

## Results and limits

Authoritative run `qualified-final`: **24 cases, 220 passing assertions, zero
failed assertions**; nine main solver invocations plus two public cache-priming
compiles; 53 HTTP requests, all exact owner-approved acquisition routes, zero
credential headers. All eleven traced uv compiles make zero IP socket/operation
calls. Exec observations contain only uv and its trusted Python introspection;
no git, pip, backend/build hook, installer or sentinel invocation. Inspection
refusals have no uv invocation. No fixture contains executable package code.

| Control | Observed outcome |
| --- | --- |
| Multiple versions + propagated dependency extras | leaf2 selected from1/2/3 under >=1,<3; helper pulled by leaf[tools] |
| Linux/Windows markers and native tags | Corresponding Linux/Windows leaf selected; inactive platform leaf absent |
| Requires-Python and cp311/cp312 candidates | cp312/3.12-compatible choice1 wins; newer cp311 and >=3.13 candidates excluded |
| Approved redirect and original root | Acquisition follows only declared target; root original URL/hash retained separately from local solver input |
| Unapproved initial source / redirect | Source grant refuses before fetch, or redirect refuses before unapproved target request; no uv |
| Actual source substitution | Shared SHA-256 verification fails; no inspection/uv or install |
| SourceURL/VCS/inactive URL/unselected candidate URL | Actual METADATA preflight rejects every reference before uv; no trap request |
| Root VCS and requirements option injection | Root is not an exact declared wheel / text is invalid; no uv |
| Missing candidate / no completeness declaration / extra local file | Refused before uv; no online/source/cache supplementation |
| Actual METADATA name / WHEEL-tag mismatch | Inspected bytes refused before uv, despite owner digest matching those fixture bytes |
| Conflicting finite constraints | uv exit1; terminal bounded-unsatisfiable decision, no fallback |
| Missing owner interpreter operational error | uv exit2; terminal unclassified failure, no fallback or Python download |
| Two compatible files for one selected version | uv exit0 inventories both; adapter refuses exact-file ambiguity |
| Local uv.toml/pyproject/.python-version and hostile parent options | Explicit isolation retains admitted closure; no trap/tool request |
| Public uv cache primed with version9 outside admitted catalog | Actual11-file private cache exists; admitted version1 still selected; cached leaf cannot make missing catalog pass |
| Held-use and exact receipt | Exact consumer identity/digests preserved; successful decisions settle Adopted and scopes join |

strace is observation, not an OS egress sandbox. Public offline/no-index flags and
the explicit input/environment boundary are the experiment's policy; no firewall,
TLS/security bypass or stronger adversarial containment claim is introduced.
No installation is invoked; filesystem controls observe no site-packages/venv
output. This does not replace a production installer proof, cancellation/process-
loss qualification or real-provider/target acceptance. Source-independent shared
custody is reused; native3a601625 remains unintegrated and its write set is untouched.

Feature graphs passed33 before the first build. The external default-features=false
crate built offline/locked; strict external Clippy, scoped rustfmt, Ruff, Python
syntax and diff checks pass. Initial offline lock generation tried an uncached
new tokio-macros and was refused; retaining the reviewed lock seed fixed it without
network. An initial external UUID map serialization error was fixed by serializing
existing public records as a vector. Early probes also exposed public Metadata's
typed Version/Requirement properties, URL-marker whitespace, and the conflicting
uv no-build/binary flags. Failed probes and corrected runs are retained; only the
final committed-source run is qualification evidence.

## Reproduction and migration feasibility

Use the admitted tool hashes and cached production Cargo dependencies. Run from
the checkout, with the already provisioned Rust environment:

```sh
source /workspace/.pumas-tools/env.sh
python3 scripts/release/check-dependency-features.py --s3
CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR=/tmp/offline-repro/target \
  python3 docs/plans/artifact-acquisition/reports/torch-offline-catalog-2026-10-06/build_harness.py \
  --output /tmp/offline-repro/crate
PYTHONPATH=./torch-server/tooling/packaging.zip python3 -S \
  docs/plans/artifact-acquisition/reports/torch-offline-catalog-2026-10-06/evaluate_offline.py \
  --harness /tmp/offline-repro/target/debug/pumas-offline-catalog-experiment \
  --uv /path/to/verified/uv --output /tmp/offline-repro/evidence
```

The [controls](torch-offline-catalog-2026-10-06/evaluate_offline.py),
[public acquisition harness](torch-offline-catalog-2026-10-06/acquire.rs),
[evidence index](torch-offline-catalog-2026-10-06/evidence.json) and
[raw archive](torch-offline-catalog-2026-10-06/raw-evidence.tar.gz) retain commands,
explicit environments, acquired bytes/specs, projections, public lock outputs,
stdout/stderr/syscall/source observations, shared records/receipts and earlier
failures. Raw archive: 2063 members, 355370 bytes, SHA-256
`c59fdbe5fd4f378d0bed30b3a03ee00da74393cff2f54c69ea7ea0bbc8652018`.
Logs include feature graph, final build, strict Clippy/Ruff and final controls.

Feasibility is measured for a **previously admitted finite catalog**. A production
adapter would still need approved catalog construction/completeness/source and
redirect authority, bounded enumeration/candidate-byte cost, full target and
selection policy, exact artifact ambiguity disposition, new owner packet
consumers/persistence/recovery, error classification and actual runtime/native
qualification. PEP751 cannot be silently reshaped into the existing pip report.
Offline mode cannot repair a catalog builder that already executes source or
retrieves unapproved content. No general automatic-policy migration or production
uv dependency is approved here. Next existing-plan work is the Q2 exposed
automatic/preview source-boundary repair and exact wheel handoff qualification,
after parent reviews this bounded contract and admits its chosen implementation.
Existing finite recipe stays unchanged. AQ-PACKAGES/AQ-HTTP and real-provider
acceptance remain open.

Primary public interfaces: [uv CLI](https://docs.astral.sh/uv/reference/cli/#uv-pip-compile)
and [PEP751 pylock](https://packaging.python.org/en/latest/specifications/pylock-toml/).
