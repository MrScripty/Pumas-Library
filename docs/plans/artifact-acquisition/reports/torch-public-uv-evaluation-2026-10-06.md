# Supported public uv resolver evaluation — 2026-10-06

Public uv pip compile is a viable candidate for a separately admitted resolver
adapter with a tested no-build boundary. It is not a drop-in pip report adapter,
not a before-retrieval gate, and not yet a fully qualified replacement for the
promised dynamic selection/fallback behavior. Parent chooses migration after
this evidence. Composition a1da98 remains frozen with its P1 undispositioned;
no merge-ready PR or production uv dependency is created.

## Frozen inputs and tool provenance

Evidence branch experiment/torch-public-uv-a1da98aa starts at
`a1da98aaca3a2a63beae6a99edeaeba6138c62ec`, tree
`0ccb3da586e01343e6acc13b93877526df56237f`. Current main remains
`5e114f6d8e4559e0a4d67e56000b423120a0fde0`; finite6308 and source498b7961
are unchanged. No Rust, Torch production/test, contract, Cargo/Node lock,
provider/runtime pin, ORT policy, native/shared cleanup or original evidence
file is edited. No applicable filesystem AGENTS/skills were found earlier in
this same workspace. Writes are this separate report/controls/provenance/evidence,
owning status and write-set admission only.

Official uv0.12.23 was downloaded from PyPI's version metadata and its HTTPS
files.pythonhosted.org wheel, then installed without dependencies in a separate
/tmp tool environment. That environment contains only pip and uv. Downloaded
wheel: uv-0.12.23-py3-none-manylinux_2_17_x86_64.manylinux2014_x86_64.whl,
20579564 bytes, SHA-256
`565c6e2874dbeae86c02f3dea97255e878fec672659a73d4930c6b93fcab2fff`.
Upload recorded by PyPI: 2026-10-03T17:31:39.705167Z.
Installed native binary equals the original wheel's scripts/uv member:
47993144 bytes, SHA-256
`abdc39eab8b4ad341dca91f3823a23a343fae94bdb22ebdd9e91694415206f2f`.
Every hashed wheel RECORD member was checked. Version output is
uv 0.12.23 (x86_64-unknown-linux-gnu). The package licence expression is
MIT OR Apache-2.0; exact upstream licence files and hashes are retained in
[tool provenance](torch-public-uv-evaluation-2026-10-06/tool-provenance.json).
This verifies official transport/metadata/RECORD byte identity, not a publisher
signature or complete release/transitive-dependency licence acceptance.

## Controls and measured outcomes

The [reproducible evaluator](torch-public-uv-evaluation-2026-10-06/evaluate_uv.py)
uses subprocess public CLI and stdlib HTTP/TOML/ZIP APIs. Fixture wheels have only
METADATA, WHEEL and RECORD; no importable code. Anonymous loopback HTTP serves
controlled indexes/wheels. Source traps initially refuse bodies; refined sdists
contain only PKG-INFO and declarative pyproject.toml with a missing backend and
unavailable build-trap dependency. Local VCS contains only that declarative
project; system/global Git configuration and hooks are disabled and only the
local file protocol is allowed. Trusted remote-VCS sentinels record an attempted
git command then exit before fetching. No source/backend/package code is served
or executed, no hostile external URL is fetched, and no build dependency or real
GPU/runtime is installed. Public local pip installs three metadata-only fixture
distributions into a disposable target to qualify the consumer proof.

Child configuration is explicit: --no-config, --no-cache, --only-binary :all:,
--no-sources, --keyring-provider disabled, --no-python-downloads,
--no-managed-python, --python selected executable, --python-version requested
minor and --python-platform requested target. NETRC is disabled, credentials/XDG
paths are private fixture paths, UV/PIP/proxy ambient options are not inherited,
and HTTP retry count is zero. No TLS/security bypass option is used. A conflicting
local uv.toml is ignored. Fixture request records assert no Authorization header.
The application must independently bound/drain this child; CLI options alone are
not an OS egress denial or lifecycle proof.

| Observation | Measured outcome / implication |
| --- | --- |
| Valid named closure and propagated extras | root → middle[fast] → leaf resolves exactly; inactive Python branch absent. Actual wheel preflight and local pip/RECORD proof pass |
| Source root and direct-wheel transitive sdist | Data-only dynamic archive may be fetched; then exit1 Building source distributions disabled, with zero build-trap requests and no lock |
| Source/VCS references from registry metadata | Undeclared URL dependency refuses before source access; this is uv's URL policy, not proof that all URL metadata is globally preflighted |
| VCS root and direct-wheel transitive VCS | Native git can be invoked/cloned first. Metadata-only local repo then refuses with build disabled before any build dependency request |
| Inactive URL marker | uv ignores it and can produce a lock; existing final preflight still refuses every dependency URL, including inactive branches |
| Python/OS markers and Requires-Python | 3.12/3.13 target changes closure; >=3.13 fails3.12 and succeeds3.13; Windows/Linux markers select corresponding leaves without managed-Python downloads |
| Target wheel tags | cp312/cp313/Linux/Windows versions select according to the requested target; explicit manylinux2.17 excludes2.28, explicit2.28 permits it |
| Generic Linux target | Current uv with the explicit local interpreter permits the2.28 fixture; initial assumption that generic Linux always meant2.17 was false |
| Original direct wheel root | Original URL and declared digest survive as a lock archive entry; no root version substitution |
| Incorrect declared direct SHA-256 | Compile still succeeds and retains the incorrect digest. Actual acquired-wheel preflight refuses it before pip; compile output is not verified bytes |
| Index priority | uv default/first-index chooses the higher-priority extra index even when its version is older. Current pip chooses the highest across both; explicit unsafe-best-match matches that highest-version fixture |
| Same-version different-index identity | uv unsafe-best-match and current pip both choose extra-index bytes/its dependency in this fixture; this is one tie control, not a general equivalence guarantee |
| Prereleases | uv default and pip26.2.1 both permit the only pre-release fixture; explicit uv disallow refuses. Earlier assumption based on older pip behavior was wrong |
| Lock/report shape | Public PEP751 lock-version1.0, created-by uv, requires-python, packages with wheels or direct archive, hashes and optional markers; two compatible artifacts can remain for one package. No pip selected-file report is emitted |
| Fresh output versus reuse | Fresh requirements text is deterministic. Existing output can silently pin an older version; --upgrade removes that preference. Production owner must require new owned output |
| Missing fixed closure / HTTP503 | No lock or root-version substitution; 503 produces exit2 even with a usable lower-priority index. Unsatisfiable, unsupported URL and source-build refusal share exit1 |

The source-root/VCS retrieval findings remain limitations; the revised controls
retain them rather than claiming early retrieval denial. No backend existed to
run; the explicit build-disabled refusal and zero build dependency requests
establish the bounded pre-build result. Actual malicious providers/backends,
cache reuse, credentials/redirect authority and supported platforms remain
separate qualification work.

Initial36 probes retained five failed assumptions: before-retrieval guarantees
for source/VCS roots, a fixed generic Linux2.17 baseline, compile-time direct-hash
verification and an old pip prerelease default. The raw initial script/results
are retained. Refinement demonstrates the actual required pre-build boundary and
keeps those counterexamples explicit. The hash negative still refuses at actual
acquired-input verification, never by discarding or replacing the original hash.

## Comparison with existing Pumas requirements

The existing resolver uses public pip with a Torch primary index and PyPI extra
index, explicit discovered direct Torch wheel/hash, CORE package names and adapter
requirements. Current managed-Python/build order tries newest compatible native
CPython and only moves to an older candidate on conclusive incompatibility;
accepted payload failures never re-enter selection. Final shared acquisition
requires exact selected source URLs/digests and public actual-wheel closure/proof
before local pip. Existing helper refuses all dependency URLs, even inactive ones.
These policies are unchanged by this experiment.

Default uv first-index would change that union-index behavior; do not adopt it
silently. unsafe-best-match is the supported CLI option nearest current pip's
union selection, and only approved immutable authorities can be admitted. Its
name is not a TLS bypass, but the selection/security tradeoff needs parent approval.
Equal-version/hash/tag/marker selection parity still needs broader qualification.
Current uv/pip agree on the measured prerelease-only control; neither old docs nor
that one control qualifies all prerelease/yank/conflict/backtracking behavior.

An explicit target lock is not an exact interpreter identity: requires-python
>=3.12 does not authorize reuse on any newer interpreter. Bind actual selected
provider/executable hash, native implementation/platform/machine/tags, full input
and root identities alongside unchanged lock bytes. Target platform strings must
match existing native tag policy; do not use a guessed glibc baseline.

The human error stream is not a public typed incompatibility schema. UV exit1
includes source/VCS/unsupported URL failures as well as dependency unsatisfiability;
exit2 includes operational failures. Mapping either directly to legacy retry2/4
would silently change fallback. Treat every unclassified UV failure as inconclusive
and fatal for automatic fallback. Existing separate official-wheel discovery can
still supply conclusive root incompatibility. Conclusive transitive dependency /
Python incompatibility needs an additional supported evidence design before full
automatic-fallback parity can be claimed. That is a migration gate, not a private
hook or stderr heuristic proposed here.

## Concrete supported integration proposal for parent decision

1. Admit a separately pinned/provenanced/licensed native uv tool per supported
   platform, before target package installation. Preserve no-ORT/bootstrap policy;
   no ambient tool download or silent version upgrade. Public subprocess CLI only.
2. Use the existing managed-provider/candidate owner. Supply its selected actual
   interpreter and explicit original named CORE/adapter requirements/direct wheel
   roots. Validate root source authority, wheel form and original hashes before
   invoking uv; reject editable/project/source/VCS root inputs. Disable ambient
   config/credentials/cache/Python provisioning and preserve candidate order.
3. Compile once to a fresh owned PEP751 lock, with binary-only, selected target,
   explicit index/prerelease policy and a retained original argv/tool/input record.
   Define a bounded source-metadata authority policy for transitive references:
   binary-only stops builds but does not itself stop their retrieval or Git use.
   Do not use a private hook, executable blocker hack or tool.uv.sources rewriting
   as production admission. If before-retrieval rejection is required, this public
   CLI evidence does not provide it; parent must choose supported external metadata
   preflight or a different admission contract/design.
4. Implement a strict public lock adapter: require version1.0, bound packages and
   bytes, evaluate target markers, canonicalize names/versions, validate approved
   wheel URLs/SHA-256/target tags and exact direct-root bindings. Refuse source/VCS/
   directory/editable candidates and unknown formats. Keep original lock intact;
   never fabricate a pip report or silently collapse multiple artifact choices.
   Bind one approved wheel per distribution with an explicit qualified ranking or
   ambiguity refusal, then emit a versioned Pumas accepted resolution packet.
5. Keep existing shared payload acquisition, exact actual-wheel hash/metadata/
   Python/tag/marker/extras preflight and no-deps local pip/RECORD/probe/receipt
   pipeline. Original digests remain expected authority; compile success does not
   make wheel bytes verified. Existing dependency-URL refusal stays explicit.
6. Translate failures conservatively without legacy exit-code reuse or candidate
   fallback after acceptance. Before replacing all promised dynamic behavior,
   qualify conclusive failure evidence, candidate/index/tie/prerelease policy,
   source/credential/network boundaries, child/use drainage and actual runtime
   targets. Parent approves compatibility/migration and an exact write set first.

This proposal requires no private pip API or custom general resolver. Bounded
pre-build evidence is promising, but it does not close the authority, error-schema,
selection parity or objective package acceptance gates. The known P1 is not
accepted as safe coexistence; frozen a1da98 has no merge-ready PR.

Official primary references, checked 2026-10-06:
[uv binary/index compatibility](https://docs.astral.sh/uv/pip/compatibility/),
[public compile CLI](https://docs.astral.sh/uv/reference/cli/#uv-pip-compile),
[HTTP credential isolation](https://docs.astral.sh/uv/concepts/authentication/http/),
[NETRC/environment](https://docs.astral.sh/uv/reference/environment/#netrc).
