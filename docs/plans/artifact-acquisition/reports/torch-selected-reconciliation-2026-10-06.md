# Q2 private selected-runtime reconciliation — 2026-10-06

This successor adds live retained-owner reconciliation of selected publication to
exact committed output, safe owned rollback, or unresolved state. It keeps the
original catalog receipt unchanged, exposes no public Rust API and leaves
new automatic/preview caller adoption OFF. Parent owns independent review,
hosted CI, PR creation and integration.

## Lineage and scope

- Published base: `04fde63b151fa019b73fcf71aca636b46a35c548`, tree
  `910a9abef17f94af4a4f88ee14312ea0e84299dc` (fetched and verified before edits).
- Prior tested code/evidence: `568a5f1fca76b2253088d97f71b1d950e5beacc9`, tree
  `5e5d26a3540d468a7a737ff5d54ccbebe0d78260`; preserved unchanged as an ancestor.
- Branch: `feat/torch-selected-reconcile-04fde63b`.
- Tested source: `0f1a2a05b0e0b9c663d8da09bc0997832a8af91d`, tree
  `02441072384c59a234cf0a1506df7e4357d8db02`.
- Main observed/preserved: `5e114f6d8e4559e0a4d67e56000b423120a0fde0`.

No accessible repository AGENTS.md or .agents/skills was present. Admission is
[the bounded write set](write-sets.md), following the user's explicit delegation,
existing plan AD8 and contract section11. Changed source is limited to the private
runtime owner, existing fixture tests and Torch stage/cleanup custody. No
acquisition/store/public API, resolver/recipe/dependency/lock, native installation,
importer/watcher, S3, caller routing or original evidence files changed.

## Decisions and custody

`RuntimeRefusal::reconcile` and `PublishedSelectedRuntime::reconcile` consume their
live private owner and require the original catalog consumer owner. A retained
non-deserializable `PublicationIntent` binds the original metadata manager,
versions root, native directory identity, exact generated installation record and
marker bytes. Serialized evidence cannot recreate a packet, catalog grant or
acquisition `Using` lease. Reconciliation never invokes installation, the solver,
probe, acquisition transfer or catalog settlement.

One registered blocking job retains the validated runtime and packet through all
readback, runtime/input/member/provider fences, decisions and cleanup. Its
original stage retains the versions lock. Expected evidence refusals are returned
as successful job output, so unresolved states do not create supervisor shutdown
failures. The retained exploratory red log shows the earlier error-propagation
mistake; it is not final-source qualification.

Committed acknowledgment requires exact installed metadata, expected native
output identity and exact record bytes, the full original live runtime fence,
and repeated metadata readback. Existing output remains unchanged. A missing
publication marker is acceptable only with that full committed proof and original
stage custody proof; a changed marker remains unresolved. Repeated committed
reconciliation is stable.

Rollback requires no metadata publication attempt, absent installed metadata,
exact marker/record and native ownership, and an unambiguous source or destination.
Only an exact owned destination can be removed. A metadata write that started but
has absent readback remains unresolved, protecting subsequent authored changes.
Both locations present, both absent without remembered live deletion, missing or
changed evidence, foreign identity and operational uncertainty cannot authorize
cleanup. Successful output deletion is remembered by this same live intent so
interruption before marker cleanup can finish without inferring ownership from
an absent directory.

A distinct selected-stage custody marker is created before publication effects.
The last live unresolved owner's drop disables TempDir cleanup; legacy cleanup
preserves the corresponding stage while that marker or a publication marker
remains, and treats lookup errors as uncertain. Legacy cleanup cannot interpret
or accept the selected publication record. On verified acknowledgment/rollback,
only exact owned markers are released and directory synchronization is attempted.
The companion marker is a preservation fence, never a cold authority importer.

## Qualification

- [10 runtime/reconciliation tests](torch-selected-reconciliation-2026-10-06/lifecycle.log): pass, no skips;46 actual full-flow controls, including18 new reconciliation controls.
- [64 affected Rust regressions](torch-selected-reconciliation-2026-10-06/rust.log): pass, no skips;74 affected Rust tests total.
- [126 affected Python tests](torch-selected-reconciliation-2026-10-06/python.log): pass, no skips.
- [Full Python discovery](torch-selected-reconciliation-2026-10-06/python-full.log):314 pass,2 inherited parser errors; [exact-base reproduction](torch-selected-reconciliation-2026-10-06/python-baseline-embedding.log).
- [Full default Rust workspace](torch-selected-reconciliation-2026-10-06/workspace-test-xdg.log): pass under task-local XDG config; library1793/app-manager332/RPC298 unit controls plus integration/doc groups. Ignored model/native/qualification controls remain reported in the log; selected ignored controls are explicitly qualified above.
- [33 complete feature graphs](torch-selected-reconciliation-2026-10-06/graphs.log), [all-target/all-feature workspace check](torch-selected-reconciliation-2026-10-06/workspace-check.log), [strict workspace Clippy](torch-selected-reconciliation-2026-10-06/workspace-clippy.log), no-default/doc/rustfmt and full Torch-server Ruff: pass.
- [New retained publication verification](torch-selected-reconciliation-2026-10-06/evidence-verification.log) and [prior evidence verification](torch-selected-reconciliation-2026-10-06/prior-evidence.log): pass; three wheels and21 installed members in each.

The ordinary workspace run initially failed because this sandbox forbids writes to
`/home/agent/.config/pumas`. The explicit XDG rerun on the same source passes; its
log/result supersedes that failure. XDG was set only for a task-local test process,
without HOME, account or network changes. The first long gate wrapper emitted one
argument fragment after its runner file was updated while active; no Rust test
command/result was lost. The retained reproduction script is the corrected,
complete script. Each Rust/Python test outcome is backed by its actual log; these
wrapper/setup errors are not source qualification failures.

Real local fixtures perform complete catalog acquisition, pinned public-uv local
selection, genuine pip installation/report/RECORD verification and the actual
selected-profile probe before interrupting publication. New controls cover
committed/lost acknowledgment, missing marker with exact committed proof,
pre/post-rename owned rollback, missing/changed marker/record/member/directory,
foreign output and metadata, removed post-publication metadata, and missing
pre-metadata evidence. They repeat stable/uncertain decisions, preserve foreign
keepers and pending source evidence after the last owner drops, and reopen the
existing legacy cleanup owner without accepting cold evidence. A paused registered
reconciler drains after its caller is abandoned. Another interruption occurs after
successful owned deletion but before marker cleanup, and finishes using retained
live progress. Original catalog bytes and transfer request counts stay unchanged.

The new retained publication fixture contains actual wheel ZIPs, installed members,
local pip report/manifest/requirements, original receipt/target/lock/packet and
probe/profile/record. The unchanged prior evidence verifier validates it using the
single existing RECORD owner. Fixture directories/provider/metadata stores are
then disposed; retained file IDs are evidence of executed live checks, not a
substitute for those directories or permission to reconstruct custody.

## Remaining boundaries

The full Python discovery gate has two inherited errors in
`test_embedded_runtime_imports`: its text parser expects the pre-extraction
`for (name, contents) in [` source table. Both reproduce on exact base04fde63b.
They remain visible; fixing that independent embedding-test parser is outside
this reconciliation slice. Ambient newer FastAPI initially added seven unrelated
route-shape errors; rerunning with the existing requirements-dev versions removes
those seven. No dependency or lock declaration was changed.

This qualifies same-process live readback/reconciliation and conservative cold
preservation only. It does not qualify selected hard-kill recovery, cold acceptance,
all crash windows, power-loss durability or deployed migration/old-writer retirement.
The legacy installed-metadata writer lacks parent-directory synchronization;
no installed-metadata power-loss durability is claimed. Arbitrary uncooperative
concurrent writers are outside the retained-lock/live-fence claim.

Native glibc2.41 selection remains unsupported by the pinned exact uv projection.
Positive fixtures explicitly use the existing synthetic compatible2.40 observation;
production does not downgrade it. Synthetic arithmetic/protocol and copied existing
local provider do not establish real Torch/provider/native compatibility, managed
provider provisioning, GPU/adapter/model execution, sidecar socket startup or full
recipe closure. No enforced network-denial policy was introduced or qualified.
AQ-PACKAGES/Q2/P1 classifications remain unchanged. No model/provider payload,
credentials/network-settings change, Library transfer, old-worker file manipulation,
S3 authentication/Q3 work, external review contact, PR or main integration occurred.

## Reproduction

See [commands](torch-selected-reconciliation-2026-10-06/run-final-rust-gates.sh),
[explicit XDG rerun](torch-selected-reconciliation-2026-10-06/run-workspace-test-xdg.sh),
[tested identity](torch-selected-reconciliation-2026-10-06/tested-source.txt),
[gate results](torch-selected-reconciliation-2026-10-06/gate-results.txt) and
[source/evidence hashes](torch-selected-reconciliation-2026-10-06/provenance.json). The fresh cache was populated only with locked official Cargo
crates and needed official Python tooling/test dependencies. The uv0.12.23 executable
hash matches `abdc39eab8b4ad341dca91f3823a23a343fae94bdb22ebdd9e91694415206f2f`.
All final Cargo commands use offline/locked operation, jobs2, debug0, incremental
OFF, a shared bounded task-local target and `ORT_SKIP_DOWNLOAD=1`. All33 feature
graphs are inspected before final build gates, including full workspace/all-feature
normal/build/dev graphs on Linux/macOS/Windows. ONNX retains load-dynamic/disable-linking
and has no build-download features. Rustler linking is excluded from workspace
runtime gates because it needs BEAM; its graph remains inspected. Hosted and native
provider acceptance remain parent-owned separate gates.
