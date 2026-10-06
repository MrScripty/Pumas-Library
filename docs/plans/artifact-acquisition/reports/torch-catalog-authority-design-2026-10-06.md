# Request-scoped wheel catalog authority — design checkpoint, 2026-10-06

Proposed implementation; production paths and acceptance are unchanged. Start
from frozen 2ce91b431afacdb4eb402af91f55b1258afe04fc, tree
e1dc46a0e45926bf1d1906813c456d2fed7e259c, on separate
design/torch-catalog-authority-2ce91b43. This checkpoint is saved before the
separately requested final executable-identity repair. No new package provider,
mirror, mandatory Torch version, source access or solver is activated here.

## Existing ownership and concrete gaps

The manager's TorchInstallSelection already retains tag/build/Python/adapter,
expiry and one-use selection identity. ManagedPythonIdentity supplies the exact
managed provider; the new selected-venv observer supplies complete target and
separate consumer executable evidence. CORE/IMAGE and their existing constraints
belong to the package profile, not acquisition. The selected Torch release/build
and exact discovered direct wheel/hash remain request data; the optional bundled
2.9.1 preset does not become a library-wide version requirement.

The current resolver requests the selected build's PyTorch index plus PyPI and
pip combines candidates across them. Existing validation permits PyPI wheel
payloads at files.pythonhosted.org/packages and PyTorch payloads at the existing
download.pytorch.org/download-r2.pytorch.org whl paths. Torch/torchvision have
stricter selected-build rules, with the existing macOS CPU exception. A new
mirror, repository tracking hint, dependency URL or resolver lock cannot enlarge
those grants. The generic acquisition service receives caller-provided
AcquisitionHttpSource and configured transport; it has no package repository
registry or package-specific source-selection API. NetworkConfig and
InstallationConfig contain operational settings, and AcquisitionCapacity supplies
shared worker/blocking/scope limits. RegistryConfig describes model storage;
it is not a package-index configuration mechanism.

AcquisitionConsumer::acquire_http, ArtifactManifest, expected SHA256/size,
ReservedDirectory and AcquiredArtifactUse already provide payload ownership.
Metadata discovery remains a source/provider protocol responsibility under the
existing managed child/stage custody; it must not become another wheel payload
fetcher. The shared HTTP stream checks its selected representation length before
writing each chunk, but an unknown-length body has no arbitrary caller byte cap.
Therefore unknown candidate sizes cannot silently satisfy a catalog byte budget.
The smallest first implementation requires size evidence from the authorized
index or a bounded provider metadata HEAD observation, sets expected_size on
ArtifactFile, and refuses missing/conflicting size as Incomplete. A later generic
bounded-source option can support unknown sizes; do not claim it already exists.

## One request and one retained result

Use a private app-manager request, not a new durable registry/framework:

| Request fact | Trusted producer and meaning |
| --- | --- |
| demand/selection | Existing operation, tag/build/Python/adapter, expiry and generation |
| repositories | Caller-owned immutable source profile: repository IDs, exact Simple base URLs, permitted project-path derivation, explicit metadata/payload/redirect origin and path rules, and transport/ephemeral access references |
| roots/constraints | Original public package requirements/extras/markers and exact direct roots, derived from this selection and the existing profile; explicit caller constraints are retained unchanged |
| target | Independently owned selected-interpreter observation and digest; no resolver/environment/cache inference |
| selection semantics | Existing union-of-approved-index candidates, existing prerelease/yank policy and native tag priority; no silent first-index or version-window change |
| budgets | Positive finite project/page/candidate/edge/metadata/ZIP/payload/elapsed limits and shared capacity configuration; saturation is Incomplete |

The initial normal caller uses only its current source profile. Explicit future
embedding configuration may select a subset or supply a separately approved
profile through the composition root; arbitrary request strings or metadata
cannot mint that configuration. A grant permitting normalized project pages can
allow newly discovered dependency names beneath its existing index bases without
authorizing a new repository. Credentials remain ephemeral, absent from request
fingerprints, catalog provenance, logs and receipts. HTTPS/trust/redirect/proxy
policy is supplied before any request; source links are checked before retrieval.
No ambient pip/uv indexes, config, credentials, cache or provider provisioning
may fill omissions.

The retained catalog records request/profile/target fingerprints, raw complete
project observations and their digests, normalized candidate identities,
original URL/hash/size, actual metadata/tag facts, dependency edges and explicit
completion obligations. A private Complete constructor is issued only after
checking all obligations; parsed JSON cannot mint it. Exact declared direct roots
retain artifact identity even when another wheel shares name/version. Multiple
repository files remain distinct candidates; namespace keys must include identity
so same-name/version/filename different bytes cannot collide in the local solver
projection.

## Completeness within the declared universe

Define U as the wheel candidates admitted by this request's repositories,
constraints, exact direct roots, target and explicit selection rules, from the
finite project-response observations obtained during this attempt. U is not all
PyPI, every upstream version, an atomic cross-repository snapshot, or future
availability. Record observation times/revisions; later additions do not rewrite
this result, and later changed payload bytes fail the original hashes.

1. Seed project names/requirements/extras from the original roots. For each
   reachable normalized name, obtain each applicable authorized repository's
   complete Simple project response under its byte/time/record limits. Retain
   the entire response; never treat its first N rows/versions as complete. A
   supported provider may supply a finite immutable catalog instead, with its
   declared coverage checked under the same request. Unsupported pagination,
   truncation, access failure, malformed/unknown protocol or unproved absence
   is Incomplete, including when another index has usable candidates.
2. Parse public wheel filenames/version and target facts. Sources/archives/VCS
   are outside the wheel-only universe and are never fetched/prepared. Check
   every admitted wheel URL against the preexisting receiving grant and require
   trusted expected SHA256 and bounded size. Index metadata filters cannot
   substitute for actual acquired wheel metadata; missing/contradictory facts
   remain incomplete/refused. Repo tracks/alternate-index hints are provenance,
   never grants.
3. Acquire candidate wheels through the existing generic service, under original
   source identities/digests and governed capacity. Inspect actual METADATA,
   WHEEL, Requires-Python, all Requires-Dist fields, safe ZIP namespace/expanded
   limits and direct-URL refusal, including inactive branches, before any solver.
   No source build or backend runs. Record exclusions explicitly; malformed
   approved candidates are refusal, not silent disappearance.
4. Traverse all eligible candidate versions conservatively, evaluate markers
   with the same full target, union possible dependency constraints and extras,
   and repeat until a fixed point. Do not intersect requirements from mutually
   exclusive candidate versions: A1→B<2 and A2→B>=2 require enumerating both
   branches. New extras revisit already observed candidates. Cycles terminate
   by deduplicated name/candidate/extra/edge facts under finite budgets.
5. Complete requires every reachable project/repository response complete, every
   admitted compatible candidate inspected, every newly applicable dependency
   covered, all direct-root bindings intact and the local projected namespace
   exactly equal to the admitted candidate set. An empty dependency candidate
   set can establish unsatisfiability only after complete observations and
   supported typed absence evidence; a solver's error text is not that proof.

Bounds are refusal limits, not version constraints. Existing 1 MiB finite index
and finite package/request/time settings are engineering starting points, not
qualification for a dynamic universe or real Torch payload volume. Supply exact
positive byte/count/time values in implementation admission, measure on inert
fixtures, and qualify provider volume separately. Count actual wire retries,
redirects and repeated selected payload transfers against the operation budget;
existing per-file retry limits alone are not an aggregate catalog deadline.
Do not make an over-large universe appear complete by pinning Torch globally,
keeping newest K dependencies, narrowing ranges or choosing another mirror.
A narrower explicit caller universe requires a new request and visible scope.

## Normal caller adoption and the smallest implementation sequence

1. **Authority and enumerator:** add one private catalog-owner module and focused
   Python public-packaging inspector. Reuse the existing source profile, selected
   interpreter owner, provider metadata child/stage custody and shared acquisition
   configuration. Implement the strict request/Complete boundary and monotone
   fixed-point algorithm with controlled provider observations. Acquire every
   wheel body through acquire_http. If catalog batches need a distinct consumer
   scope, its receipts describe acquired catalog snapshots, never installation;
   add its cold unresolved-use admission before any Torch startup cleanup.
   Do not assume adopted catalog files can be reopened with a stale Using lease.
2. **Offline solver and checked packet:** reuse the proven public offline CLI
   approach only after exact tool/version/licence/target qualification and
   explicit acceptance of its adapter. Original roots/provenance stay intact;
   project approved direct roots to exact acquired local files. Use only that
   owned wheel namespace, offline/no-index/no-build, fresh output, no ambient
   config/cache/credentials/provider downloads. Require a representable full
   target; current experimental projections still refuse unsupported libc,
   deployment and host-dependent marker contexts. Parse unchanged supported lock
   output and independently check closure/extras/markers, exact artifact IDs,
   local mapping, SHA256 and direct roots. Do not fabricate a pip report or
   implement a general custom resolver. Complete enumeration is necessary but
   does not resolve all current tool/target/selection qualification gaps.
3. **Coordinated callers:** both preview_torch_runtime_with_interpreter and
   resolve_selected_torch_attempt call that same owner. Preview remains a
   selection-only token initially; resolved plans retain the complete catalog,
   original target, provider/consumer identities and accepted selected closure
   under existing expiry/custody. Installation checks those retained facts and
   enters the existing exact selected-wheel acquisition/local consumer. Reuse
   candidate content only through a qualified generic complete-content grant;
   no such public reuse source exists today. Until implemented, any selected
   reacquisition is an explicit shared transfer counted in the total budget,
   never a hidden solver download. Preserve receipt scope and final target proof.

Every missing page, overflow, unknown representation, target projection refusal,
source/hash/metadata violation or unclassified solver failure ends this operation
as refusal/Incomplete. No online retry resolver, source build, new repository,
older interpreter/build or profile fallback follows incomplete enumeration.
Only an independently supported conclusive incompatibility for a complete
request can permit the existing candidate-order transition; acceptance remains
its existing point of no fallback. Do not map uv exit 1/2 to legacy pip exit 2/4.
Exact tie/prerelease/yank behavior still needs compatibility fixtures before
replacement; recording the existing policy does not prove tool equivalence.

## Decisive tests for implementation admission

| Control | Required observation |
| --- | --- |
| request A/B select different Torch versions/builds | Each honors its own original constraints/direct artifact; no global 2.9.1 pin |
| configured repo subset and injected repo/redirect/track hint | Only authorized project/payload origins are requested; rejected target receives zero requests |
| hostile ambient index/proxy/config/cache/provider settings | Child profile remains exact; zero undeclared source or provider/body retrieval |
| source/VCS/editable/archive dependency, including inactive marker | Refusal before source retrieval/build/solver; no backend call |
| two approved same-name/version wheels/direct-root substitution | Separate identities/namespaces; independent checker rejects substituted root |
| two repos, second page fails while first has a solution | Incomplete; zero solver/install/fallback calls |
| complete multiple-version branches A1→B<2, A2→B>=2 | Both B domains enumerated; no accidental intersection/newest-only filter |
| late extra activation, markers, cycles and incompatible target tags | Fixed-point coverage grows correctly and terminates; same full target everywhere |
| page/record/edge/size/ZIP/aggregate wire/deadline limit | Typed Incomplete/refusal; no Complete packet; children/effects drained under owned cleanup |
| missing size, incorrect HEAD size or changed bytes | Missing evidence refuses; body length checked before excess writes; original SHA remains authoritative |
| omitted candidate/unprocessed edge/changed target/manifest namespace | Complete cannot be issued or accepted; no consumption |
| completed scope followed by newer upstream index contents | Prior scope is unchanged; no atomic/global/future completeness claim |
| actual provider→catalog→offline CLI→local consumer success | Exact source/target/artifact and installed proof/receipt match; final install network-denied separately |
| cancellation/abandonment/cold Using/probe executable mutation | Retained custody and refused replay/publication; no premature cleanup or renewed approval |

These are test specifications, not newly executed acceptance evidence. Existing
frozen experiments supply limited supporting controls; they do not qualify the
proposed producer or real upstream completeness. Design verification checks
source inventory/hash/links and proves no production changes; no build or runtime
was needed for this documentation-only pass.

## Determined choices and genuinely new preferences

The existing repositories, selected release/build, managed interpreter ordering,
package profiles, full target, exact direct roots, shared acquisition custody,
HTTPS/scoped access, public tooling and no incomplete-enumeration fallback are
already determined by source/contracts. No user answer is needed to implement
this conservative existing-source request design. Parser/module layout, JSON vs
supported HTML adapter and numerical refusal budgets are engineering decisions
requiring written implementation admission and evidence, not personal preference.

A user decision would be needed only if the product wants an additional/private
repository, different index priority/prerelease/yank policy, a narrowed dependency
version universe or replacement of an unrepresentable supported-target policy.
Do not infer these preferences to make enumeration or solver projection succeed.
The current independent final executable P2 is repaired separately; this design
neither amends frozen producer evidence nor certifies P1/AQ gates.

[Source inventory and exact hashes](torch-catalog-authority-design-2026-10-06/source-inventory.json) accompany this checkpoint. Primary standards
checked 2026-10-06: [Simple repository API](https://packaging.python.org/en/latest/specifications/simple-repository-api/)
(project responses, file provenance and optional metadata are protocol data, not
source authorization) and [public compile CLI](https://docs.astral.sh/uv/reference/cli/#uv-pip-compile)
(offline/no-index/build/config controls require separate owned input admission).
