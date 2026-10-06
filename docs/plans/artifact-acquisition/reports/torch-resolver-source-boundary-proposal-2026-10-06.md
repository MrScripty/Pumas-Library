# Q2 resolver source-boundary proposal — 2026-10-06

**Disposition: production repair blocked on an explicit private-tooling exception.**
Frozen automatic source `5946c4738870bdcfc613430bb98fd873d9e6d6f3` and final
`cf3afb3f45762b61e882cf0ad4e738a6f18086f1` remain unchanged. The earlier
qualification's binary-only/dry-run controls do not establish absence of source
preparation. Ordinary exact-wheel preview resolution has the same transitive
exposure. Qualified-recipe work is paused; no recipe implementation is present.

## Exact starting point and scope

Investigation branch `fix/torch-resolver-source-boundary-cf3afb3f` starts at
`cf3afb3f45762b61e882cf0ad4e738a6f18086f1`, tree
`6fbb0dfb95646e7009dc14f9610cdeab5c6c591a`. Repository-local author remains
MrScripty / TheEnvironmentGuy@protonmail.com. Only owning evidence/plan/write-set
files change. No production Rust/Python, runtime recipe/pin/dependency, native
cleanup, S3, manifest, importer or watcher code changes. Parent owns PRs,
external review, hosted qualification and integration.

## Reproduced boundary and proposed enforcement

Installed pip 26.2.1 accepts a direct-URL sdist requirement from a PyTorch-origin
wheel despite `--dry-run --only-binary=:all:`. Real Factory requirement traversal
reaches LinkCandidate construction; construction normally prepares distribution
metadata. Our sentinel raises at construction, before preparation or retrieval.
No synthetic URL was fetched, source prepared, backend installed or hook run.

The unactivated [proposal](torch-resolver-source-boundary-proposal-2026-10-06/admission_proposal.py)
places admission before `Factory._make_base_candidate_from_link` delegates to
LinkCandidate/EditableCandidate. It rejects non-wheels, VCS, local files,
editable candidates, untrusted wheels, filename/name/version mismatches and
unexpected fragments. It preserves a recognized SHA256 fragment for pip while
checking trust against the fragment-free URL. Transitive metadata direct URLs
also refuse, consistent with the final local installer's existing restriction.
An exception is fatal admission refusal, never a `None` candidate eligible for
fallback. The worker must preserve that disposition as exit 3 even if pip catches
its exception, and emit only a bounded diagnostic without rejected source text.

A request guard belongs at `PipSession.send`, before adapter/cache dispatch and
every redirected request. Guarding pip's HTTPAdapter alone misses the sibling
CacheControlAdapter. The inert prototype admits approved HTTPS metadata/index/
wheel origins and rejects plaintext, foreign, local, credential and query-bearing
redirects. This is a source-admission proposal, not OS-enforced network denial.
It does not qualify legitimate Nunchaku GitHub release CDN redirects: their exact
receiving-origin/locator policy needs an explicit decision and fixture before
activation; no broad CDN exception is proposed. Production trust must reuse the
existing wheel policy rather than the test's synthetic allowlist.

## Contract and compatibility blocker

[Contract §11](../../../contracts/artifact-acquisition.md#11-package-consumption)
says “Use only supported public tooling.” Factory and PipSession monkey-patching
are private pip integration, with no claimed upstream support. Public binary,
dry-run, hash and build-isolation flags do not offer the equivalent boundary;
`--no-deps` would discard required closure resolution. Parent must either admit
this narrow private admission adapter explicitly or choose a larger supported
metadata/approved-wheelhouse design. The question is pending; elapsed time does
not authorize the exception.

The prototype supports **only pip 26.2.1** and checks both seam signatures before
activation. Other versions or incompatible shapes refuse. This is not a runtime
pin or provisioning change. Managed-provider pip versions remain unqualified;
shipping an exact-version guard could refuse otherwise valid installations.
Before implementation, the exception must specify its compatibility disposition,
coverage of both automatic and retained-preview non-install modes, and bounded
worker failure propagation. Legacy helper install remains separately scoped.

## Inert evidence and limits

[Tests](torch-resolver-source-boundary-proposal-2026-10-06/test_admission_proposal.py):
9 passing tests on Python 3.12.14 / pip 26.2.1. They cover original pre-preparation
gap, inert METADATA direct sdist/VCS/foreign references through real Factory,
source/local/editable/identity/fragment negatives, approved wheel constructor
sentinels, allowed and rejected redirect hops through real Session logic,
actual cached-adapter refusal before dispatch, unsupported version/seam refusal,
and a constant safe diagnostic.

The first fixture run failed because the artificial Factory omitted caches and
TargetPython and the Response fixture lacked case-insensitive headers. Those
fixture errors and corrected final output are retained; no real preparation was
performed in either run. Positive controls stop at constructor/transport
sentinels; they **do not** qualify a complete all-wheel resolver closure,
original report generation, managed interpreter, worker exit propagation or
production configuration/custody integration. Existing production source was
not changed or retested by this evidence-only milestone.

[Evidence hashes](torch-resolver-source-boundary-proposal-2026-10-06/evidence.json)
bind all retained proposal/tests/logs. Real Torch/provider/platform acceptance,
final enforced-denial installation and hosted AQ-PACKAGES remain open. These
nine tests are placement evidence, not a completed production repair.

## Next existing-plan work

First resolve the Q2 pre-preparation boundary under an admitted design, implement
and qualify it without fallback/config/custody changes, then resume the paused
qualified-recipe handoff slice. Q3 actual-provider and Q4 installed/platform
acceptance remain separately gated. Do not advance AQ-PACKAGES or report either
exposed resolver mode as source-preparation-safe before that repair.
