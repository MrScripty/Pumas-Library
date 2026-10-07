# Q2 private selected-runtime reconciliation — fresh saved environment, 2026-10-07

This candidate reconciles retained live publication custody to exact committed
output, safe owned rollback or unresolved state. Automatic/preview adoption remains
OFF. Catalog receipts are unchanged; no public Rust API, cold authority
reconstruction, installation/probe replay, S3 or Q3 work is introduced.

## Exact lineage

- Published base fetched successfully through existing authorized Git transport:
  `04fde63b151fa019b73fcf71aca636b46a35c548`, tree
  `910a9abef17f94af4a4f88ee14312ea0e84299dc`.
- Preserved prior tested lifecycle: `568a5f1fca76b2253088d97f71b1d950e5beacc9`,
  tree `5e5d26a3540d468a7a737ff5d54ccbebe0d78260`.
- Fresh branch: `feat/torch-selected-reconcile-fresh-04fde63b`.
- Tested source: `e1ca6959ef37ce2e8b0c96b75fc9e2ce3c0bf402`, tree
  `3a06d10ad7b5350c73ca4cbfa9db1335ac960196`.
- Main's last verified commit remains preserved:
  `5e114f6d8e4559e0a4d67e56000b423120a0fde0`; this task did not refresh or merge main.

The saved environment contained scoped Q2 implementation commits `f1a8a818`
and `e7c700b5`. They were reviewed and cherry-picked onto this new exact-base branch
as `3aba280c` and `4a291bef`; admission documentation was rebound in `e1ca6959`.
The three changed Rust files are byte-identical to the saved scoped implementation.
Prior gate logs are not substituted for this candidate's fresh executions. Prior
worker checkouts, evidence and targets are preserved. Repository contribution, architecture and development
guidance was checked; no
AGENTS.md or local `.agents/skills` were available. The referenced external
Coding-Standards checkout and MCP were unavailable in this environment; no
standards-compliance certification is claimed. Parent owns independent review
and integration.

## Private decisions and custody

`RuntimeRefusal::reconcile` and `PublishedSelectedRuntime::reconcile` consume the
retained live owner and require the original catalog consumer owner. Readback,
fences, acknowledgment and cleanup execute in one registered blocking job under
the retained versions lock. A non-deserializable publication intent retains the
original metadata manager/root, native directory identity, exact record/marker
bytes, metadata-attempt state and successful deletion progress. No saved record
can create a catalog grant, selected packet or execution lease.

Committed acknowledgment requires exact installed metadata, native directory and
record bytes, every original runtime/input/member/provider fence, and repeated
metadata readback. A missing publication marker is accepted only alongside full
committed proof and matching retained selected-stage custody. Repeated committed
observation preserves output. Mutated markers or foreign output remain unresolved.

Rollback requires absent metadata without a metadata publication attempt, matching
marker/record/native ownership and one unambiguous owned location. A started write
with absent readback stays unresolved, preserving later authored state. Only the
matching owned destination can be deleted. Successful deletion remains in the
same live intent, so interruption before marker cleanup can finish without guessing
ownership from absence. Missing/changed evidence, foreign directories/metadata,
conflicting locations and operational errors retain uncertainty.

A selected-stage custody marker precedes publication effects. Unresolved last-owner
drop disables TempDir cleanup; legacy cleanup preserves selected/publication
markers and lookup uncertainty. These markers preserve evidence without authorizing
cold acceptance. Exact acknowledgment/rollback releases owned markers and attempts
directory synchronization. Evidence refusal is a successful custody-job result,
so it does not turn expected uncertainty into a supervisor shutdown error.

## Fresh qualification

All 11 Rust gates pass against the frozen tested source. Commands and raw results
are retained in [run-gates.py](torch-selected-reconciliation-fresh-2026-10-07/run-gates.py),
[Rust results](torch-selected-reconciliation-fresh-2026-10-07/rust-results.json) and
[Python results](torch-selected-reconciliation-fresh-2026-10-07/python-results.json).

- [10 lifecycle tests](torch-selected-reconciliation-fresh-2026-10-07/lifecycle.log): 46 actual full-flow controls,
  including 18 reconciliation controls; no failures or skips.
  [Structured controls](torch-selected-reconciliation-fresh-2026-10-07/lifecycle-controls.json).
- [64 affected Rust regressions](torch-selected-reconciliation-fresh-2026-10-07/rust-affected.log): pass, no skips;
  74 affected Rust tests total.
- [126 affected Python controls](torch-selected-reconciliation-fresh-2026-10-07/python-affected.log): pass, no skips.
- [Full default Rust workspace](torch-selected-reconciliation-fresh-2026-10-07/workspace-test.log): pass, including
  332 app-manager, 1793 library and 298 RPC unit tests plus integration/doc groups.
  Native/model exclusions remain visible; relevant ignored selected controls
  were explicitly included in the affected gates above.
- [All-target/all-feature check](torch-selected-reconciliation-fresh-2026-10-07/workspace-check.log),
  [strict Clippy](torch-selected-reconciliation-fresh-2026-10-07/workspace-clippy.log),
  [no-default](torch-selected-reconciliation-fresh-2026-10-07/no-default.log), [docs](torch-selected-reconciliation-fresh-2026-10-07/doc.log),
  [rustfmt](torch-selected-reconciliation-fresh-2026-10-07/rustfmt.log), [Ruff](torch-selected-reconciliation-fresh-2026-10-07/ruff.log) and
  [33 feature contracts](torch-selected-reconciliation-fresh-2026-10-07/graphs.log): pass.
  [Raw full-graph commands](torch-selected-reconciliation-fresh-2026-10-07/full-feature-graph-results.json) retain all
  normal/build/dev dependency features on Linux/macOS/Windows.
- [Full Python discovery](torch-selected-reconciliation-fresh-2026-10-07/python-full.log): 314 pass, 2 inherited errors,
  reproduced on [exact base](torch-selected-reconciliation-fresh-2026-10-07/python-base-embedding.log).
  The full Python gate is not fully passing.
- [New publication verifier](torch-selected-reconciliation-fresh-2026-10-07/evidence-verification.log) and
  [prior publication verifier](torch-selected-reconciliation-fresh-2026-10-07/prior-evidence.log): pass, three wheels
  and 21 installed members each.
- [Prior hashes](torch-selected-reconciliation-fresh-2026-10-07/prior-evidence-hashes.json),
  [source provenance](torch-selected-reconciliation-fresh-2026-10-07/source-provenance.json),
  [saved source byte comparison](torch-selected-reconciliation-fresh-2026-10-07/saved-source-byte-comparison.json),
  [preserved checkpoints](torch-selected-reconciliation-fresh-2026-10-07/preserved-checkpoints.json) and
  [implementer review](torch-selected-reconciliation-fresh-2026-10-07/source-review.json) retain the exact scope.
  Parent independent review remains pending. The
  [artifact hash manifest](torch-selected-reconciliation-fresh-2026-10-07/evidence-sha256.json) covers 78 files,
  including this report; the manifest itself is separately committed.
- [Authorized source publication](torch-selected-reconciliation-fresh-2026-10-07/source-publication.json) succeeded
  for the frozen source checkpoint; no PR or CodeRabbit request was created.

The fixture drives actual catalog acquisition, exact pinned public-uv selection,
genuine pip installation/report/RECORD verification, and a real selected-profile
probe before injecting interruptions. Controls cover lost acknowledgment, repeated
stable commit, missing marker with complete committed proof, pre/post-rename
rollback, missing/changed marker/record/member/directory, foreign output/metadata,
removed post-publication metadata, abandoned reconciliation custody and interrupted
cleanup after owned deletion. Unresolved cases repeat without changing metadata,
marker or catalog receipt; foreign keepers and stages survive last-owner drop and
legacy cleanup reopening. These are actual local effects with controlled returned
errors/waiter abandonment, not hard-kill or power-loss tests.

A new disposable publication retains three exact local wheel ZIPs, all 21 installed
members, genuine report/manifest/requirements, original catalog receipt/target/
lock/packet and actual probe/profile/record. The unchanged read-only verifier uses
the single RECORD owner to verify these bytes and prior lifecycle evidence.
All 62 prior recorded source/evidence hashes were verified against the exact base
(7 source hashes and 55 unchanged evidence hashes). Retained temporary identities
document executed live checks; fixture providers/runtime/metadata stores are
disposed and cannot be reconstructed from evidence.

Rust runs offline/locked, jobs 2, debug 0, incremental OFF, with a fresh task-owned
target and XDG directory. All 33 Linux/macOS/Windows feature graphs pass before
compilation, including full workspace/all-feature/dev graphs and Rustler bindings.
ORT uses dynamic loading and defensive `ORT_SKIP_DOWNLOAD=1`; no build/provider/
model download occurs. Rustler runtime linking is excluded because it needs BEAM.
Existing official Python/uv dependencies are read without bytecode/cache writes to
prior tool directories. No dependency/lockfile, credential or network setting changes.

## Remaining boundaries

Full Python discovery has two inherited `test_embedded_runtime_imports` errors:
the test parser expects the embedded-source table before the lifecycle extraction.
Both errors reproduce on exact base `04fde63b`; the full gate is not fully passing.
They are outside this reconciliation slice and remain recorded.

This qualifies live same-owner readback/reconciliation and conservative cold
preservation. Cold acceptance, hard-kill recovery, complete crash-window coverage,
deployed migration/old-writer retirement and uncooperative concurrent writers remain
unqualified. The legacy installed-metadata writer lacks parent-directory
synchronization; no power-loss durability is claimed.

Native glibc 2.41 remains unsupported by the pinned exact uv projection. Positive
fixtures retain the explicitly synthetic compatible 2.40 target; production never
downgrades it. Synthetic arithmetic/protocol and a copied existing local fixture
provider do not qualify real Torch/provider/native compatibility, managed-provider
provisioning, GPU/adapter/model execution, sidecar startup or full recipe closure.
Enforced network denial remains unqualified. Q2/AQ-PACKAGES/P1 status is unchanged.
No main merge, CodeRabbit request, new PR, Library transfer retry, credentials or
old-worker file manipulation occurs. Source/evidence publication is separately
recorded; parent owns independent review, hosted checks and integration.
