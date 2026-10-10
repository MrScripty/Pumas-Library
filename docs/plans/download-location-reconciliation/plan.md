# Download location and failed-attempt reconciliation

- Status: **Planned**; implementation has not started.
- Created: 2026-10-10.
- Canonical identity: `docs/plans/download-location-reconciliation/plan.md`.
- Admission operation for a subsequent implementation request: `start`.
- Current phase: planning complete; awaiting implementation instruction.
- Exactly one next slice: **M1 — usable recovery through the core and desktop**.
- Objective acceptance: **pending**. Existing guard tests support the design;
  they do not satisfy this plan's new acceptance claims.
- Responsible implementation and integration owner: the agent executing this
  selected plan, acting for the repository maintainer. Product owner: Jeremy.
- Planning authority: the user's request to create this remediation plan.
  This document neither starts implementation nor authorizes live-library repair.

## Objective

Prevent a downloader's destination from being moved by classification or other
library mutations while it has custody. Make retained legacy failures usable:
show files found at a changed location, offer an explicit action to retain them
and resolve the obsolete failed attempt, and converge immediately and after
restart without redownloading, moving, deleting, or backing up model files.

The resulting code must satisfy the applicable coding standards at its changed
owners and boundaries. A passing build alone cannot close this plan.

## Evidence and scope

The [investigation](../../investigations/download-path-discrepancy-2026-10-10.md)
establishes three historical moves during active downloads, followed by attempts
to open the next file at the now-absent destination. All 38 expected files now
exist at the three catalog locations. Their aggregate sizes agree with SQLite;
their upstream provenance and byte integrity have not been established.

Reviewed source is `1a78ea8ecbe3ddf589d84db6783577eaf4efb79d` on
`fix/main-test-validation`, based on main
`ad31e391dcd94c204b3fcd748cc98d27b3281e65`. The current custody guard already
addresses the historical competing-writer path. The remaining product gap is
explicit reconciliation of old failures, plus evidence for the composed flow.
Do not describe current main as still performing the historical unguarded move.

Included: the shared model-workspace mutation family; exact legacy Error
attempt resolution; current-format metadata evolution; native core, desktop
RPC/IPC and UI; focused, composed, crash, full-suite and Debian artifact evidence.

Excluded: automatic takeover of unknown failed process claims; a new generic
acquisition framework; inference qualification of these model weights; changing
family classification policy; silently rebasing paths; a new SQL download
authority; rewriting old successful completion receipts; bulk live-library repair.

## Constraints and binding design direction

The detailed proposed interface and ordering are in
[design](reports/design.md). On plan admission, these decisions bind the slices:

| Decision | Owner and consequence |
| --- | --- |
| D1. Classification is metadata; it cannot change an admitted worker's workspace. | Existing core download lifecycle and `LibraryMutationAuthority`; all shared-workspace mutators use that authority. |
| D2. Download attempt, current local artifact, and verified HF completion are distinct facts. | HF persistence owns attempts and resolution history; model/index owners own local observations; importer owns actual completion proof. |
| D3. Legacy recovery explicitly accepts existing local files. | Core resolution interface records that disposition while preserving the original Error. It creates no HF completion receipt and grants no inference readiness. |
| D4. Resolution history and exact queue release commit together in the existing canonical `downloads.json` transaction. | HF model partition owns their meaning; acquisition store owns shared publication and locking. SQLite receives no duplicate repair journal. |
| D5. Current schema-7 libraries resolve in the running application. | Use the admitted root and existing task lifecycle. No manual close/reopen or offline helper for this resolution. Preserve separately supported old-format upgrade behavior. |
| D6. Recovery reads bounded metadata and selected-file stats only. | No model copies, payload reads/hashes, network download, directory recreation, model move or deletion. Backups, if retained, contain metadata only. |
| D7. Ambiguity, changed preview, unknown custody, invalid metadata and uncertain publication fail explicitly. | Core returns typed outcomes; transport and UI preserve their meaning. Preserve unresolved state until proof is adequate. |
| D8. Permanent mechanisms require a current owner and deletion justification. | Reuse existing native grants, transactions, tasks, import proof and contract generator; no second service, global registry, path-alias table or recovery daemon. |

The historical upstream revision is absent. Exact historical HF completion is
therefore unavailable, not assumed. The chosen action promises preservation and
resolution of an obsolete attempt against an explicitly selected local artifact.
The UI must state this limitation before the action. If product requirements
instead demand historical upstream verification, reopen D3 and its acceptance
claims before implementing; current upstream `main` cannot supply that proof.

Composed-design admission is **applicable**. The complete eight-part artifact
probe and the bounded sibling/consumer population are recorded in
[design](reports/design.md). They must be reviewed against the actual implementation.

## Milestones

These are semantic acceptance boundaries, not prescribed commit counts or
branches. Each milestone's exact allowed files are in
[write sets](reports/write-sets.md); that report is part of its admission.

| Milestone | Coherent outcome | Required gate | State |
| --- | --- | --- | --- |
| M1 | A real desktop user can inspect a retained legacy Error, keep the existing local files, and resolve the obsolete attempt through one core owner and atomic persistence transaction. Includes the authoritative contract/ADR, scoped format upgrade, typed RPC/IPC, generated consumers and accessible UI. | C2/C3/C5/C7 plus local resolution portions of C4/C6: tiny authored fixtures through actual core/storage and desktop producer/consumer paths, immediate refresh, reopen, precise negative cases and publisher outcomes. Composed/process portions remain for M2. No production placeholder or UI-only success. | Planned |
| M2 | The shared-workspace invariant holds across downloader, watcher, classification and sibling mutations; interrupted resolution converges without losing custody or files. | C1, C4, C6, C8: controlled multi-file transfer, real reconciliation, sibling checks, process interruption and supported-platform evidence. Preserve previously passing guard tests. | Planned |
| M3 | Full applicable suites pass and a rebuilt Linux Debian package performs the complete recovery workflow. | C9–C10, all other claims satisfied, scoped standards review closed, release provenance and checksum recorded. | Planned |

M1 is deliberately vertical because the observed gap is unusable product recovery.
M2 adds independent concurrency/failure evidence after that path exists. M3 proves
the delivered artifact and repository-wide compatibility. Core and transport
work may proceed in dependency order inside M1, but the slice is not accepted
until its real user-facing path works.

## Acceptance and execution

The [acceptance matrix](reports/acceptance-matrix.md) defines C1–C10 with evidence
kind, environment, mode and status. [Verification](reports/verification.md)
defines the executable procedures and independent observations. Every claim is
required. Missing environments produce blocked evidence, not a waiver, ignored
test, startup-only substitute, or stronger completion claim.

The implementer must reroute the coding standards MCP at admission and when
material scope changes. The [standards report](reports/standards.md) records the
planning snapshot, selected policies and their concrete design implications.
Review changed code against all applicable MUST requirements; record exact
findings and disposition. Formatting/type checks do not certify architecture.

Use the existing latest-main validation worktree while it retains its declared
purpose and ownership. Target `main`; published branch history is immutable.
At start, inspect current GitHub main, PR [69](https://github.com/MrScripty/Pumas-Library/pull/69),
worktree status, retained-resource contract and source drift. Select integration
from the actual current state; preserve published history. Do not import the
original checkout's unrelated acquisition edits or reset/stash that checkout.
The plan's documentation must be transferred as declared content before code
work begins if execution uses the separate checkout.

Use pinned Rust/Node/pnpm/Python toolchains and frozen dependency inputs.
Declare missing test/build dependencies before installing or running suites.
Keep Python `.venv` in the repository and ignored; use the existing original
repository environment if executing from the validation worktree. Isolate test
libraries/configuration, not the user's Python environment in `/tmp`. New runtime
dependencies are not anticipated; adding one requires a stated need, ownership,
license/attribution and write-set review.

Before each commit: inspect status; stage only the admitted coherent write set;
review the staged diff and generated changes; pass focused/affected gates; use
the repository's conventional commit format and active hooks. Before PR update
or release, review the explicit branch range and cumulative diff. Do not tag,
publish, install into the user's running library or rewrite shared history as
an incidental verification action.

## Blockers and re-plan triggers

There is no planning blocker. Implementation evidence and real desktop/package
environments remain pending. Track missing capabilities in [issues](issues.md).

Stop the affected slice and re-plan if: the current base changes a material
owner or format; an eligible attempt actually has active or unknown effects;
resolution needs model/index writes or multiple durable publications; candidate
selection needs unsupported evidence; a shared-workspace consumer bypasses the
owner; a contract generator cannot represent a required outcome; current-format
evolution cannot fail closed for old readers; a required platform cannot uphold
native exclusion; or an unlisted production file/dependency is necessary.

Expand the bounded consumer population only for new shared authority or reachable
failure, not merely because another file exists. Keep one next slice current.
Record acceptance changes and material deviations in the
[execution ledger](execution-ledger.md), with detailed evidence in `reports/`.

## Completion

Transition to Implemented when scoped behavior and documentation are complete,
then Verifying for remaining objective evidence. Accept only when C1–C10 are
satisfied, required standards findings are closed, the new Debian artifact is
identified, and governed branch/worktree disposition is recorded. Existing
retained worktrees stay protected until their owner selects a safe disposition.
Consolidate lasting decisions into the ADR, contract and operating documentation;
retire terminal execution artifacts according to repository policy with Git
preserving their history.
