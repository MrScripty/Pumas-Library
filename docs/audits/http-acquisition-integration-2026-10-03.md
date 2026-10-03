# HTTP acquisition integration checkpoint

## Scope and provenance

The [approved integration plan](../plans/http-acquisition-integration-baseline.md)
composes current main/B1 with PR7, PR10, PR15 and PR16. It preserves all four
feature histories and their milestone commits. The original PR7 branch is not a
replacement baseline and is not rebased away.

Pinned parents:

- Main/B1: `58b74e83fdf34131290933f576c7e338bde4a49d`, tree
  `57b285ad99994de9e2cc0b83cc62916a3c184ae6`.
- PR7: `3b2a279b4d35bd19e5cc1727cc905c7ee7477fa6`.
- PR10: `57ed33db8c2995e4633f559d1a8d31d591b0dcca`.
- PR15: `dd04fdcf48132ef7237e61feba5e5c1d3629d49a`.
- PR16: `f344e7fed8a8cbc6f4c4831ec7c78aa4c44c2d64`.

Main's independent Build 37127500248, CodeQL 37127499502 and Scorecard
37127500224 passed before implementation began. Those results do not qualify
this new composition.

## Semantic joins

1. Held download destinations keep B1's private staging, exclusive copy creation,
   physical descendant binding, publication uncertainty and directory-mode rules.
   Acquisition workspaces borrow the same verified authority.
2. Ordinary and raw metadata writes preserve copied-publication identity.
   Only the copied producer may establish its Pending identity. The canonical
   receipt parser remains shared by ambient-root and held-destination observers.
3. Importer keeps B1's runtime-owned copied producer and PR7's supervised HF
   completion. Guarded reads cannot bypass canonical copied readiness.
4. Library writes keep both acquisition mutation guards and B1's conditional
   index updates, stale-observation checks and retained Pending diagnostics.
5. Startup installs one shared acquisition owner before HF restore, retains the
   exact failed-start claim guard, and keeps shard discovery read-only. Guessed
   repository names never authorize a download.
6. RPC drains accepted HTTP responses concurrently with local domain owners and
   native installation consumers. Only after these consumers settle does it
   close the shared acquisition supervisor. Every observed failure participates
   in the repeated final receipt.
7. Runtime managers receive that same acquisition service. The current HTTP
   lifecycle policy and signal/acknowledgement contract remain in force.
8. The dependency lock keeps main's HTTP transport ownership plus PR7's pinned
   Unicode normalization dependency. Attribution retains every package/source
   object from both parents; only changed ownership fingerprints are refreshed.
9. The frontend preserves main's preview fixture reset/readiness corrections and
   PR15's command ownership behavior.
10. Workflow coverage is the union of current baseline/native gates, all-base PR
    triggers and native acquisition cleanup. Release and managed-runtime E2E
    jobs retain their existing tag/manual restrictions.

## Receipt-validation lifetime correction

Composition exposed a grant-lifetime gap: HF receipt validation moved its only
root grant into the initial blocking read, dropping it before output proof
validation and durable settlement. The integrated path retains the owning grant
through those later effects. A test-only pause after the held read itself owns
no grant; the regression checks that a successor remains excluded after caller
cancellation, through valid settlement or missing/wrong-lease receipt failures.
Failed outcomes preserve custody, bytes and repeatable failure evidence.

Acquisition receipts, copied-publication receipts and native cleanup receipts
remain separate contracts. None can substitute for another owner's proof.

## Qualification matrix and honest limits

Five cross-owner behavior groups cover:

- Real copied Ready output through guarded observation, plus damaged/oversized/
  replaced evidence and cache/load-target refusal.
- Copy/HF exclusion before effects, including hidden predecessor custody.
- Distinct receipts, warm/cold retention and the post-read grant lifetime gap.
- Legacy schema refusal and coexistence of retained HF custody, copied Pending
  evidence and incomplete local shards.
- Actual copied/HF owner drainage after caller loss, accepted HTTP shutdown/SSE
  completion, and a synthetic native partial-transfer consumer on its separate
  versions workspace.

The hosted runner lists the exact filter before running it and rejects empty,
ignored, incomplete or failed execution. Native Linux/macOS/Windows runs include
core defaults, core HF-only features, RPC fixture defaults and inference-disabled
RPC. Package-separated no-default full suites avoid workspace feature-unification
claims. Ordinary Rust, frontend, Python, attribution and release-gating checks
remain required.

The native source constructor is a non-default test-support seam with literal
loopback validation. Both metadata and archive requests reuse a no-proxy,
no-redirect transport, and archive URLs must keep the exact validated origin.
A synthetic explicitly configured proxy test checks routing without reading or
changing ambient credentials/environment; real fixture requests reject any
Authorization header. This guarantee belongs only to the new native metadata/
archive seam. Composed API startup still inherits ordinary HF token resolution
and connectivity probes; these tests do not claim globally offline startup.
Its RPC feature forwarding is weak: enabling test support
without inference cannot introduce app-manager. Product/release dependency and
workflow assertions keep fixtures out of release builds. Synthetic bytes are
never launched as a runtime or represented as model inference evidence.

At this source checkpoint, Rust formatting, 20 release/workflow tests, three
qualification-runner tests, 122 targeted download-command/preview frontend tests,
and Python lint/format checks have run. Rust compilation and all native behavior remain pending hosted
execution and independent source review. No successful filter/CI claim is made
from source inspection alone.

Schema-7 fixtures do not qualify upgrades of deployed schema-5 stores. Normal
schema-4/5/6 acquisition startup/mutation remains fail-closed, preserving the
original document. No live store, historical process namespace or credential was
reconciled or changed. S3, public speech, actual model inference and networked
node qualification remain separate milestones.
