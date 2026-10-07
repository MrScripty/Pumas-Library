# Q2 private selected-runtime reconciliation — saved environment 2026- 10-07

This candidate adds retained, same-owner reconciliation to exact committed output,
safe owned rollback or unresolved state. Automatic/preview caller adoption stays
OFF. No public Rust API, catalog receipt change, cold authority reconstruction,
installation/probe replay, S3 work or Q3 work is introduced.

## Exact lineage

- Freshly fetched published base: `04fde63b151fa019b73fcf71aca636b46a35c548`,
  tree `910a9abef17f94af4a4f88ee14312ea0e84299dc`.
- Preserved prior tested lifecycle: `568a5f1fca76b2253088d97f71b1d950e5beacc9`,
  tree `5e5d26a3540d468a7a737ff5d54ccbebe0d78260`.
- Fresh branch: `feat/torch-selected-reconcile-saved-04fde63b`.
- Tested source: `26d2ee9f7dc78bce13723ef3cf4c9e712f24616e`,
  tree `e0da034768d80e8b63939f8a6229f9c11218ca66`.
- Main's last verified commit preserved: `5e114f6d8e4559e0a4d67e56000b423120a0fde0`.

The saved repository already contained scoped reconciliation commits `4afec387`
and `0f1a2a05`. These were inspected and cherry-picked onto this fresh exact-base
branch as `f1a8a818` and `e7c700b5`; admission documentation was then bound to this
branch. The three changed Rust files are byte-identical to the prior reconciliation
source `0f1a2a05b0e0b9c663d8da09bc0997832a8af91d`. Its historical gate logs are not
substituted for this candidate's new qualification. Prior branches, evidence,
worker outputs and build directories were not modified by this task. Repository/workspace instructions and skills
were searched; no accessible AGENTS.md or relevant .agents/skills existed.

## Authority and decisions

Private `RuntimeRefusal::reconcile` and `PublishedSelectedRuntime::reconcile`
consume retained live custody, require the original catalog consumer owner and
run all readback, fences and cleanup in one registered blocking job. The live
stage keeps the versions lock. A non-deserializable `PublicationIntent` retains
the original metadata manager, root, native directory identity, exact record and
marker bytes, publication-attempt state and positive cleanup progress. No on-disk
record can recreate a catalog grant, packet or lease.

Committed acknowledgment requires matching installed metadata, exact native
output identity and record bytes, all original runtime/input/member/provider
fences, and repeated metadata readback. Missing publication marker alone is
acceptable only with complete committed proof and matching live stage custody.
Changed markers or outputs remain unresolved. Repeated committed observation
preserves the existing output.

Rollback requires absent metadata with no metadata publication attempt, exact
marker/record/native ownership and one unambiguous owned location. A started
metadata write with absent readback remains unresolved; later authored state is
preserved. Only a matching owned destination can be deleted. Successful deletion
is remembered by the same live intent, permitting an interruption before marker
cleanup to finish without inferring ownership from absence. Foreign or missing
output, conflicting locations, changed/missing evidence and operational errors
retain typed uncertainty.

A distinct selected-stage custody marker precedes publication effects. Unresolved
last-owner drop disables TempDir cleanup; legacy cleanup preserves stages with
selected or publication markers, including uncertain lookup errors. That marker
only preserves evidence. Legacy cleanup never accepts a selected installation
record. Exact acknowledgment/rollback releases only owned markers and attempts
directory synchronization. Evidence refusals are successful custody-job results,
so they do not manufacture supervisor shutdown errors.

## Qualification

Current results are retained in the [gate commands and raw result manifests](torch-selected-reconciliation-saved-2026- 10-07/run-gates.py),
[Rust results](torch-selected-reconciliation-saved-2026- 10-07/rust-results.json) and
[Python results](torch-selected-reconciliation-saved-2026- 10-07/python-results.json).
All 11 Rust gates pass against the frozen source. The current result is:

- 10 lifecycle tests pass, with 46 actual full-flow controls and 18 reconciliation controls; no skips.
- 64 affected Rust regressions pass; no skips (74 affected Rust tests total).
- 126 affected Python controls pass; no skips.
- Full default Rust workspace passes: library 1793/app-manager 332/RPC 298 unit tests, plus integration/doc groups. Ignored native/model/qualification controls remain reported; relevant selected controls are explicitly included above.
- All-target/all-feature workspace check, strict Clippy (`-D warnings`), no-default, doc, rustfmt and 33 full feature graphs pass.
- Full Python discovery: 314 pass, 2 inherited errors reproduced on exact base; full Torch-server Ruff passes.
- New and prior publication verifiers pass: three wheels and 21 installed members each. Source code remains unchanged through both gate lanes.


The local fixture carries actual catalog acquisition, pinned public-uv selection,
genuine pip installation/report/RECORD verification and the real selected-profile
probe through publication before injecting interruptions. Ten lifecycle tests
cover46 actual controls, including18 reconciliation controls: lost acknowledgment,
stable repeated commit, missing marker with full committed proof, pre/post-rename
rollback, missing/changed marker/record/member/directory, foreign output/metadata,
removed post-publication metadata, pre-metadata evidence refusal, abandoned
reconciler custody and interruption after owned deletion. Unresolved cases repeat
without changing metadata/marker/catalog receipt and preserve foreign keepers and
pending stages after last-owner drop and legacy cleanup reopening.

A new disposable fixture publication retains all three real wheel ZIPs,21 installed
members, genuine report/manifest/requirements, original catalog receipt/target/
lock/packet and actual probe/profile/record. The existing read-only verifier checks
this output and the prior lifecycle evidence through the single RECORD owner.
All 62 prior source/evidence hashes are checked against the exact base. Retained
file identities document executed live checks; fixture directories/providers and
metadata stores are disposed and cannot be reconstructed from this evidence.

Rust uses a fresh task-owned target, offline/locked Cargo, jobs2, debug0,
incremental OFF and defensive `ORT_SKIP_DOWNLOAD=1`. All33 Linux/macOS/Windows
feature graphs passed before compilation; ORT retains dynamic loading and no
build downloads. Rustler runtime linking is excluded because it needs BEAM; its
full feature graph is included. Tests use a task-local XDG directory. Python
uses existing official dependencies without bytecode/cache writes to prior tools.
No dependencies, lockfiles, credentials or network settings changed.

## Remaining boundaries

Full Python discovery's two existing `test_embedded_runtime_imports` parser errors
reproduce on exact base 04fde63b in the [baseline log](torch-selected-reconciliation-saved-2026- 10-07/python-base-embedding.log).
The parser still expects the pre-extraction embedded-source table. It is outside
this reconciliation feature and remains visible; the full gate is not claimed
fully passing.

This qualifies live same-owner readback/reconciliation and conservative cold
preservation. Hard-kill recovery, cold acceptance, complete crash-window coverage,
deployed migration/old-writer retirement and uncooperative concurrent writers are
unqualified. The legacy metadata writer lacks parent-directory synchronization;
no installed-metadata power-loss durability is claimed.

Actual native glibc 2.41 remains unsupported by the pinned exact uv projection.
Positive fixtures explicitly declare the existing synthetic compatible 2.40 target;
production never downgrades it. Synthetic arithmetic/protocol and a copied local
fixture provider do not establish real Torch/provider/native compatibility,
managed-provider provisioning, GPU/adapter/model execution, sidecar startup or
full recipe closure. Enforced network denial remains unqualified. Q2/AQ-PACKAGES/
P1 status is unchanged. Parent owns independent review, hosted checks and any
later PR/integration. No main merge, CodeRabbit request, new PR, Library transfer,
model/provider download, credential change or old-worker file manipulation occurs.
