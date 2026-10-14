# v0.8 qualification continuation

**Status:** Complete. **Acceptance:** Q1–Q4 evidence satisfied within stated scope;
final source adoption and release approval remain parent-owned.
**Current phase:** terminal qualification handoff.
**Next slice:** parent selects the source pin and completes the remaining native
artifact, trained-model, metadata and version/promotion gates in the decision report.

## Objective and admission

The parent's 2026-10-14 continuation explicitly starts this new plan after the
accepted preparation plan. Deliver a concrete release-blocking list with actual
CI, security, notices and package evidence. Preserve the immutable candidate
`df53db08e2ba224edfaf713522ddd410f7d4ac44`, its earlier receipt, shared history,
and the user's inference work. Do not assume errored continuations executed.
No candidate/final tag, release publication or huge model download is authorized.
Pantograph PR78 head `34dfedd27dde9a30274226e1ea26ec1e1b2de8e3` supplies a real
cross-repository untrained-CPU fixture. Its owner qualifies the Pumas candidate
separately; coordinate findings through the parent, without changing its pin.

## Claims

| ID | Observable result | Kind / environment / mode | State |
| --- | --- | --- | --- |
| Q1 | Build 38522709859 reaches a recorded terminal status for `df53db08`; failed applicable jobs are dispositioned | integration / required-real GitHub CI / automated | satisfied: original failed; repaired aa40 Build 38526618039 succeeded |
| Q2 | Current Cargo/pnpm audits and default/S3 notice collection have concrete results and shipped-versus-build dispositions | contract / representative Linux plus current advisory databases / automated and manual | satisfied: raw audits, exact closure triage and identical regenerated notices |
| Q3 | Available exact-source package identity, inventory, runtime and startup/inference evidence are inspected without overstating synthetic/pretrained or platform coverage | release-artifact / actual available target / automated and manual | satisfied: exact aa40 Linux review CI; local bytes unavailable |
| Q4 | Independent review is inspected, or precisely recorded unavailable; only actual remaining release blockers are handed to parent | contract / real report and source evidence / manual | satisfied: published consumer review inspected; separate Pumas diff review unavailable; final gates named |

## Scope and decisions

Write set: this plan directory; current `docs/release-qa-0.8.0.md` and directly
affected release-information links. Retain `v080-readiness/reports/candidate.json`
unchanged as the immutable earlier receipt. No production change is admitted
until a reproduced CI/review/security finding identifies its exact owning paths
and the relevant regression/acceptance evidence is added to this plan.

**Admitted repair:** CI headless job 115716297437 failed only
`pause_is_rejected_after_final_file_commit_and_during_cleanup`: its real worker
did not reach the fixture's terminal-cleanup observation within one second.
The 2,069 other unit tests passed. Add only this test's bounded setup/settlement
budget and stage diagnostics in
`rust/crates/pumas-core/src/model_library/hf/download.rs`. This is a fixture
admission wait, not a product latency promise; preserve both cleanup cutpoints,
false pause admission, unchanged Downloading state until release, Completed
settlement and zero retained destination claims. Run the focused test and full
no-default core unit cohort, Rust format and affected lint when available.
Rust/Async and Verification Oracles guidance apply to the test's separate worker
runtime and blocking channel observations. No production download code, ownership
authority or schema changes are admitted.

The existing sixteen-file artifact plan, RELEASING.md and binding support matrix
govern package claims. Native Windows/macOS and final model acceptance cannot be
inferred from Linux source tests or untrained graph execution. Installed speech
remains unavailable and is not promoted into a release prerequisite merely
because its future scope exists. Unrelated consumers need not release together.

Reuse current release scripts and accepted dependency dispositions. Current pnpm
audit already distinguishes zero production advisories from the known moderate
build-only sprintf-js finding. Audit raw counts are not shipped exposure proof.
Use cargo-audit 0.22.2 (official crates.io package, Rust 1.88 minimum, compatible
with pinned 1.92) under task-local tool/cache roots. Frozen JS dependency setup
must not rewrite manifests or locks; skip dependency lifecycle scripts for notice
collection. No home config/credentials or user libraries are qualification inputs.

Canonical standards read in preparation still apply. Additional reads select
Dependencies, Licensing and Security for current dependency/provenance review,
and the existing Build/Release/Verification/Platforms rules for artifact claims.
The application standards router remains unavailable; no executable certificate
is claimed. **Composed-design review:** not-applicable, because this slice uses
existing authorities/tools and changes evidence, not runtime composition.

## Milestone and gates

**Milestone:** qualified candidate evidence. **State:** Complete within scope.
Gate Q1–Q4: retain exact SHA/run/job/artifact identities, passed/failed/skipped and
not-run results, authoritative advisories and actual closure; inspect staged and
cumulative diffs, commit/push the coherent handoff, update draft PR70. Do not push
while its original exact-head run is active, to avoid cancelling that run.
The candidate's runtime/source identity stays pinned even if a later evidence-only
commit records its result. A source repair creates a separately named candidate.

## Blockers, assumptions and stopping conditions

- Both source cohorts reached terminal status. Original df53 failed one fixture;
  repaired aa40 passed all nine applicable jobs. Tag-only jobs were skipped.
- GitHub has no submitted reviews or inline threads for PR70; CodeRabbit states
  drafts are not auto-reviewed. The published Pantograph consumer report was
  inspected; no separate PR70 diff review was supplied. Parent owns disposition.
- Package verification depends on the actual downloadable artifact and native
  host compatibility. Inspect the existing CI-produced candidate first; do not
  rebuild/rename a different source or treat a development binary as the package.
- Replan before a source, dependency-resolution, workflow, migration or artifact
  contract change. Stop expanding when current evidence identifies the minimal
  remaining pre-tag/publication gates under the accepted release policy.

[Ledger](execution-ledger.md) · [Issues](issues.md) ·
[Prior preparation](../v080-readiness/plan.md) ·
[Final receipt](reports/qualification.json) · [Release gates](reports/release-blockers.md)
