# Exact direct-root checker successor — 2026-10-06

The successor closes the reviewed static checker gap: a selected distribution
must match every active direct root's **exact approved artifact**, not only its
name and version. This is bounded experimental validation evidence. The genuine
public uv run still selects the approved wheel; no uv misselection was observed.
Production P1 and all AQ gates remain open.

## Frozen base and exact tested source

Separate branch `experiment/torch-direct-root-9eda2944` starts at frozen
`9eda2944eb9b8ccbaef89d3bcd125742e6086be6`, tree
`b837a34884f7b9fa7cdc64fd2610b5afe71eadc7`. Tested successor source:
`3190bbfc9c88776b714c945b933c58846c11e066`, tree
`306ec391a504e0905e3c0fb39f713378e9853b07`.

Only the experimental Python checker changes, with a new focused control,
admission/status and separate report/evidence. The original experiment branch,
report, raw evidence, indices and Rust harness remain frozen. This report
supersedes its independent direct-root validation claim; the other bounded
evidence retains its original scope. No production/source, public product API,
contract, dependency/lock, recipe, native cleanup, selection/fallback policy,
PR/main merge or external reviewer contact changes. No applicable filesystem
AGENTS/skills were found in this workspace.

The unchanged shared-acquisition harness, uv0.12.23 and packaging26.3 hashes were
rechecked against the [frozen provenance](torch-offline-catalog-2026-10-06/provenance.json).
No Rust rebuild, dependency acquisition, real credential/provider, package hook
or runtime installation is needed. Existing main5e114f6d, compositiona1da98,
finite6308 and the frozen9eda experiment remain unchanged.

## Binding and independent check

Before invoking uv, the existing exact original-URL/SHA-256 admission identifies
each direct root's actual inspected candidate. Capture an immutable tuple of
candidate ID, canonical name, version, filename, **original URL**, SHA-256, size
and exact local projection. Keep its original requirement string for marker
evaluation. The version-normalized closure input remains useful for dependency
checking, but it is no longer the only independent root constraint.

After public lock entries map to the inspected catalog, compare the selected
candidate against each applicable direct-root tuple. Run this before dependency
traversal can coalesce repeated requirements for one name. Missing or different
artifacts refuse the packet even if their names, versions and their own catalog
digests are valid. Conflicting direct roots cannot disappear during coalescing.
Inactive root markers retain the existing behavior. Existing multiple-compatible-
file ambiguity and invalid-hash checks still refuse without choosing a file.

The experiment helper `selected_packet` now requires the separate `direct_roots`
argument; `inspect` always supplies every admitted direct-root binding. New
`artifact_identity` returns the immutable scalar tuple. These are experimental
Python interfaces, not product APIs. Original owner declarations, projection
URLs/hashes and the genuine uv lock are never rewritten. No new serialized
authority or installation receipt is introduced.

## Actual acquired-byte control and regression evidence

The focused fixture acquires two eligible metadata-only wheels through the
unchanged public shared consumer: `root-1.0-py3-none-any.whl` and
`root-1.0-cp312-cp312-manylinux_2_17_x86_64.whl`. They have the same project/version
and different actual hashes. The first is the approved direct root. Genuine
offline public uv selects it; the shared Using lease, exact receipt and Adopted
settlement are observed. Original source identity/hash and public output stay
byte-identical throughout the subsequent independent component controls.

Substitution inputs are clearly labeled **copies of lock data**. They neither
replace the genuine `pylock.toml` nor trigger resolution/source access/install.
The frozen checker is loaded from exact9eda source to demonstrate the gap,
rather than hypothesizing its earlier behavior.

| Independent control | Frozen checker | Successor |
| --- | --- | --- |
| Genuine uv lock selects the exact approved root | Accepts | Accepts exact candidate/hash |
| Copy substitutes the other admitted same-name/version wheel and its valid hash | Accepts: reproduces gap | Refuses exact root identity/hash mismatch |
| Copy uses the other wheel's path with the original hash | Refuses | Refuses original-source mapping |
| Copy lists both compatible files | Refuses | Refuses ambiguity |
| Two declared conflicting direct roots with the same name/version | Coalesces and accepts one | Refuses the unmatched direct root |
| Inactive Windows root marker in the Linux context | Accepts empty selection | Preserves that behavior |

**Six focused controls / 50 assertions and 24 existing regressions / 220
assertions pass: 30 controls, 270 assertions, zero failures.** The authoritative
focused/regression runs perform twelve traced public uv compiles including two
cache primes, with no observed IP socket/operation calls, package-tool/build
hook or installer invocation. The focus sees exactly two approved anonymous
acquisition requests, with no new request from substituted-lock checking. All
old URL/VCS, redirect, substitution, metadata/tag, completeness, config/cache,
target, error and receipt controls still pass. Scoped Ruff, Python syntax and
diff checks pass. The initial focused test oracle incorrectly indexed an empty
inactive-root selection; its corrected final run is acceptance evidence, and
the initial failure remains visible.

The [focused controls](torch-direct-root-binding-2026-10-06/check_direct_roots.py),
[focused evidence](torch-direct-root-binding-2026-10-06/binding-evidence.json),
[regression evidence](torch-direct-root-binding-2026-10-06/regression-evidence.json),
[provenance](torch-direct-root-binding-2026-10-06/provenance.json) and
[raw archive](torch-direct-root-binding-2026-10-06/raw-evidence.tar.gz) record exact
inputs, both admitted wheel identities/URLs/hashes/bytes, genuine/copy lock data,
baseline source, observations, state/receipts and logs. Archive: 489 members,
159812 bytes, SHA-256
`fc78c0d7e0ff7aada940239525bd3f4440a17cde9a1a3dc868c5c5827cb49996`.

Reproduce from this checkout using the previously hash-verified shared harness
and uv binary:

```sh
PYTHONPATH=./torch-server/tooling/packaging.zip python3 -S \
  docs/plans/artifact-acquisition/reports/torch-direct-root-binding-2026-10-06/check_direct_roots.py \
  --harness /path/to/verified/pumas-offline-catalog-experiment \
  --uv /path/to/verified/uv --output /tmp/direct-root-controls
```

The existing evaluator command in the frozen report reruns the24 regressions
using the successor checker. Rebuilding a harness would first require current
dependency feature checks as before; no build was performed for this Python-only
successor.

## Remaining authority and target requirements

The accepted scope is still an owner-declared **finite** allowed-object universe,
not global index completeness. Production needs approved catalog construction
and completeness/source/redirect authority, bounded enumeration and candidate-
byte cost, and handling of unavailable or omitted candidates without fallback.
Full target policy remains unresolved: the entire marker environment, Python
patch/implementation, ABI, libc and platform semantics must agree with uv and
the consumer before claiming compatibility. The synthetic Linux/Windows tag and
marker fixtures do not establish real Windows/provider execution or general
target equivalence. Exact-file ambiguity stays fail-closed.

Any production migration also needs an admitted owner packet/consumer and
persistence/recovery/cancellation/installation qualification. This narrow checker
repair does not make the exposed automatic/preview P1 safe, authorize a uv
dependency or replace the existing finite recipe. Parent owns independent review,
catalog/full-target decisions and any later production implementation admission.
