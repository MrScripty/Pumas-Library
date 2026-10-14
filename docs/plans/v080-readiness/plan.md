# Pumas v0.8.0 checkpoint preparation

**Plan status:** Accepted. **Acceptance status:** satisfied for source checkpoint
preparation only; release publication remains unaccepted.
**Current phase:** parent review and promotion handoff.
**Next slice:** none.

## Objective and scope

Provide the parent integration owner with one immutable, tested source candidate
for Pantograph, Chrema and Tuldok demonstrations, current migration/release notes,
and a bounded publication decision. This is checkpoint preparation, not release
publication or qualification of every future modality.

The user explicitly authorizes this plan's `start` operation. Branch
`readiness/v080-20261014` belongs to this task, targets `main`, and starts at
`ef9eead9fe6595f5b3152ef8df19c5dace30190b`. Preserve the original `work` and
`integrate/http-acquisition-baseline` refs at `0dd38c70`; no history rewrite,
user library migration, model download, final tag or release publication.
Retain the task branch for draft-PR review; parent owns integration and retirement.

## Acceptance claims

| ID | Observable result | Kind / environment / mode | State |
| --- | --- | --- | --- |
| A1 | Current no-inference core/RPC suites qualify the source foundation; catalogue custody failures are reproduced or explicitly dispositioned | system / representative Linux / automated | satisfied: full cohorts passed; catalogue failure did not reproduce |
| A2 | Existing release contracts, version alignment and current default/S3 notices pass | contract / representative Linux / automated | satisfied: release QA and receipt |
| A3 | Release notes and changelog describe current contracts, migration, actual consumer pins and unqualified boundaries | contract / source inspection / manual | satisfied: release notes, QA, receipt and link review |
| A4 | Qualified source commit is pushed with a draft PR and exact evidence; final tag action remains parent-controlled | integration / required-real GitHub / automated | satisfied: `5da74a2c9bd1fe3c0b1fa0184eef3667f31eed88` pushed, draft PR #70 open |

## Decisions and standards

Current main already contains PRs #64–#69 and versions `0.8.0-rc.1`.
Do not reapply historical fixes or change versions merely to create a new name.
Re-run the existing headless gate before deciding whether the earlier catalogue
timing failure still blocks. Preserve production audio's unavailable outcome and
all model/runtime qualification gates. The current artifact plan requires sixteen
files: ten desktop installers, three no-inference headless archives and three
inference headless archives. Native archive/model acceptance remains separate
from the ordinary PR checks.

No repository AGENTS.md or local .agents/skills exists in this saved checkout;
the workspace .agents directory is empty. Read CONTRIBUTING, Architecture,
Development, Releasing and current integration/consumer/distribution plans.
Coding-Standards was fetched separately at
`188beda1fa477d21d576c233dd8c7f4c4c267d23`. Core/Router select Planning,
Implementation, Verification, Development Proportionality, Commit,
Documentation, Release, Library, Contracts and Contract Evolution for this
release-information slice. No runtime composition or language mechanism changes
are planned. Re-route if a reproduced defect requires source changes.
The application-purpose executable route is unavailable: its reference CLI
returns `Application invocation is unavailable; use the published interface
contract.` No connected standards tool is exposed. Direct canonical reads are
guidance, not an executable routing certificate.

**Composed-design review:** not-applicable. The initial write set changes release
information and evidence only, adding no state, process or API composition.
A repair that changes composition triggers a design review before editing.

## Milestone: reviewed checkpoint

**Status:** Accepted for checkpoint preparation.
**Write set:** this plan directory, CHANGELOG.md, docs/release-notes-0.8.0.md,
docs/release-qa-0.8.0.md; directly affected current release guidance links only.
Amendment before fixture edits: `scripts/launcher/commands.test.mjs` owns two
reproduced false failures. Immediate Node startup exceeded the 100 ms negative
fixture window; the Linux descendant was actually terminated (`/proc` state Z)
but container PID 1 did not reap it. Give the early-exit fixture startup margin
and observe Linux execution state as well as PID existence. Keep actual process
group termination, force-path and child-close assertions. Add Launcher,
Resilience and Verification Oracles/Platforms guidance for these fixture facts.
No production lifecycle or runtime composition changes are admitted.
Consumer repositories are read-only inventory inputs, owned by their respective
integration agents; this task does not change their pins.

**Gate:** A1–A4. Run existing checks, inspect the cumulative and staged diff,
commit conventionally, push without force, open a draft PR. Do not add a new
validation harness for prose. Linux source/fixture tests cannot qualify native
Windows/macOS packages, pretrained inference, GPU behavior or external consumers.

## Blockers and re-plan triggers

Final release remains blocked on the required native artifact/model evidence
selected by RELEASING.md. Parent qualifies only the actual consumer dependency
edges used by each demo; unrelated repositories need not release together. A missing
optional production capability is later scope when it is advertised unavailable;
a false availability claim, broken supported migration or reproduced ownership
failure is a release blocker. A new owner/contract, migration, dependency change
or unsafe operation requires an explicit scoped amendment before implementation.
Stop investigation once current tests and source inventory support a bounded
handoff; do not turn checkpoint preparation into open-ended feature expansion.

[Execution ledger](execution-ledger.md) · [Issues](issues.md) ·
[Release QA](../../release-qa-0.8.0.md) ·
[Immutable receipt](reports/candidate.json) ·
[Draft PR #70](https://github.com/MrScripty/Pumas-Library/pull/70)
