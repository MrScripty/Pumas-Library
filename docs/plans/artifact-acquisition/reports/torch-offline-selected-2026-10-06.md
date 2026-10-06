# Qualified request-scoped offline selection — 2026-10-06

The dormant private boundary consumes a live accepted CompleteCatalog, invokes
one explicitly qualified public offline solver over its acquired/preflighted
local wheels, then independently checks the selected packet. Automatic/preview
callers, source/version/fallback policy, installation, main and frozen repairs
remain unchanged. No real provider payloads, accounts, credentials, new tooling
downloads or network/configuration changes were used. The two approved Library
transfers remain paused at their recorded failures.

## Exact source and admission

- Branch: `feat/torch-offline-selected-3f573449`.
- Accepted base: `3f573449b2fe96c3114fc2f5b952ce66da78da68`;
  tree `68e746e52327c848804157c3ec8c259fd4512203`.
- Tested source: `ca49df3290d087a1773980ef12f829bb53602932`;
  tree `f033cbedb85f03b48d5b9bcfb39e1052b5c69559`.
- Repo-local author/committer: MrScripty <TheEnvironmentGuy@protonmail.com>.
- Main observed/preserved: `5e114f6d8e4559e0a4d67e56000b423120a0fde0`.
- Nine source/admission/contract paths are enumerated and hashed in
  [provenance](torch-offline-selected-2026-10-06/provenance.json).
  The report/plan/evidence successor has no executable source changes.
- [Frozen proof](torch-offline-selected-2026-10-06/frozen-proof.json) verifies ten
  retained owner/tooling files byte-for-byte against the accepted base, including
  catalog inspection/format repair, target owner, actual local consumer,
  old resolver/finite recipe, installer.rs, S3 reader and artifact manifest.
  No dependencies/locks, native cleanup, staged-discovery or VersionId paths change.

## Capability, public tooling and independently checked packet

`CompleteCatalog::select` accepts a private SelectionContext containing the
existing installer/scope, runtime, selected interpreter and explicit solver path.
There is no Deserialize/imported acceptance/cold selection API. SelectedWheelPacket
retains the full accepted catalog, snapshot receipt, request/target, granted files
and pending stage. SelectionRefusal retains the same catalog. ManagedChild owns
that whole lease until process-group cleanup; registered blocking effects retain
it until they join. The settled snapshot remains legitimate on refusal; selection
does not issue installation receipts or reconstruct a stale AcquiredArtifactUse.

The executable must match the already qualified official uv 0.12.23 Linux x86_64
bytes, size 47,993,144 and SHA256
`abdc39eab8b4ad341dca91f3823a23a343fae94bdb22ebdd9e91694415206f2f`. No PATH discovery, bootstrap or automatic download
occurs. Existing official distribution/license provenance is retained in
[earlier tooling qualification](torch-public-uv-evaluation-2026-10-06/tool-provenance.json).
Child environment clearing and fresh owned home/config/cache/temp paths accompany
public `--no-config --no-cache --offline --no-python-downloads --no-managed-python`,
`pip compile --no-index --no-build --no-sources --keyring-provider disabled`,
explicit selected Python/full patch/platform and public pylock.toml/hash output.
Only eligible acquired candidate directories become find-links; original direct
roots project to their exact approved local file/SHA256. Yanked policy refuses
because this local profile cannot preserve it. No global settings are modified.
See [public CLI](https://docs.astral.sh/uv/reference/cli/#uv-pip-compile) and
[public lock specification](https://packaging.python.org/en/latest/specifications/pylock-toml/).
The checker intentionally admits the measured uv wheel-only lock profile, not
all optional public lock fields or arbitrary producers.

Owned packaging APIs rederive the original catalog plan and actual metadata
before/after the solver and compare complete evidence. The checker maps every
lock row, even inactive source rows, to exact acquired local path/hash/name/version
and original source identity; remote/new/non-wheel/unknown sources refuse. It
checks exact direct-root IDs before requirement coalescing, original constraints,
full target markers, actual extras/dependency closure, late extras/cycles, missing
and unrelated members. Ordinary admitted same-version files refuse ambiguity even
if uv emits just one; the already approved exact direct-root rule is the exception.
Unprojectable marker variables refuse even on inactive metadata branches.
Every solver nonzero/operational result stops this attempt without legacy 2/4
fallback classification. There is a separate 120-second selection deadline; the
catalog acquisition deadline is unchanged. Catalog/input/lock/packet limits stay
bounded. New registered Rust reads/hashes cap actual bytes and reject growth,
links, oversized inputs and namespace changes.

After the final checker exits, Rust rechecks original producer/request/catalog
bytes, selected executable, qualified solver and all acquired file namespaces,
sizes/hashes, including unselected candidates. It checks selected identities
against the accepted catalog, retained projection and actual lock digests, then
adds the independently checked qualified-solver identity to the private packet.
The JSON helper output alone never authorizes consumption or adds sources.
There are no public Rust/IPC APIs or store schema changes. The embedded private
helper CLI has prepare/check phases and bounded generic exits20/21.

## Measured qualification and retained evidence

**110 Python tests**, **58 Rust tests** (19 catalog/selection controls, including
seven new shared selection controls, plus39 existing handoff/adjacent controls),
**33 pre-build feature graphs**, offline/locked strict Clippy, scoped Ruff,
Rustfmt, syntax and whitespace checks pass. Explicit tooling-only Rust tests ran
with `--include-ignored`; the optional actual public solver Python control was
also enabled. No configured-tool control was skipped.

The actual shared path acquires exactly five inert wheels once and settles its
snapshot receipt. Public uv selects Torch2.14.0+cpu, branch2 and dependency2;
[shared private packet](torch-offline-selected-2026-10-06/shared-selected-packet.json)
retains approved HTTPS identities, exact sizes/SHA256 and request/catalog/target/
projection/lock/solver bindings. No installed metadata appears. Unqualified solver,
bounded unsatisfiable catalog, final selected-executable replacement and final
unselected-wheel replacement refuse; original granted files/receipt remain held.
Abandonment after selected proof creation demonstrably pins grant and runtime
until managed cleanup drains.

The separate real public-tool control selects an exact direct Torch root plus
leaf1 under original leaf<2 constraint and its tools-extra child;
[public checker packet](torch-offline-selected-2026-10-06/public-selected-packet.json)
and [raw evidence](torch-offline-selected-2026-10-06/public-solver-raw-evidence.tar.gz)
retain the genuine public lock, complete original observations, target, projection,
requirements, invocation, stdout/stderr and strace. A planted local uv.toml points
to an inert loopback trap; configuration isolation succeeds, cache stays empty,
and tracing the actual solver/descendants observes **no IP operations**. This is
observed behavior, not enforcement of network denial. Subsequent incompatible
constraint and missing interpreter controls return nonzero, and changed owned
projection refuses. Copied checker inputs additionally refuse wrong hashes, remote
locators, missing closure, undeclared/late extras, wrong roots/constraints/Python,
inactive source escapes, duplicate/ambiguous files, unknown profiles and changed
catalog/target/bytes. Raw logs, initial fixture/visibility correction failures,
final results and evidence SHA256s are retained adjacent to this report.

Qualification commands (under existing admission flock, serialized offline/locked
Cargo with jobs1, debug0 and incremental0):

```text
python scripts/release/check-dependency-features.py --s3
PUMAS_QUALIFIED_UV=<already qualified absolute path> PUMAS_SELECTION_EVIDENCE_DIR=<owned evidence dir> PYTHONPATH=<existing tooling>:torch-server/tests python3 -m unittest -v test_offline_wheel_selection test_wheel_catalog_owner test_wheel_target test_target_observation test_install_verified_wheels test_qualified_wheel_catalog
cargo test --offline --locked --manifest-path rust/Cargo.toml -p pumas-app-manager --features test-support version_manager::installer::torch::torch_ -- --include-ignored --test-threads=1 --nocapture
cargo clippy --offline --locked --manifest-path rust/Cargo.toml -p pumas-app-manager --features test-support --tests -- -D warnings
```

## Remaining parity, tooling and consumer gates

Actual native observation in this executor is glibc2.41. The pinned public CLI
only admits explicit floors through2.40. The unmodified producer→CompleteCatalog
path therefore **refuses before the solver**, retaining exact2.41 approval; no
production downgrade/generic-Linux substitution exists. Positive shared/public
controls explicitly use a synthetic declared2.40 fixture target whose tags are a
subset of native tags, with original fixture producer bytes retained/rechecked.
They do not qualify actual2.41 selection. Exact support for that floor needs a
separately qualified public tool/projection decision. macOS/musl/unsupported
floors/native-Linux exclusion and unprojectable markers remain refusals.

Tool distribution/provisioning and other executable/platform artifacts,
prerelease/yank/source-priority/ambiguous-file policy parity, real-provider finite
observations/volume and exact target profiles remain gated. The existing catalog
64MiB payload/expanded bound is unchanged and is controlled-fixture qualification,
not a real Torch wheel volume claim. There is no pip report fabrication, new
custom solver, installation conversion, selected-payload consumption receipt,
automatic/preview activation or AQ-PACKAGES/P1 acceptance.

The next existing-plan slice is separately admitted selected-packet→exact local
consumer composition with installation proof/custody, followed by coordinated
production caller adoption under established selection/source/fallback policy.
Parent owns review/CI/PRs/merges and these disposition decisions. No external
reviewer was contacted.
