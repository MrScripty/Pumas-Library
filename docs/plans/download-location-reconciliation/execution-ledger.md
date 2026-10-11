# Execution ledger

Canonical authority: [plan](plan.md).

| Date | Event | Evidence / effect |
| --- | --- | --- |
| 2026-10-10 | User requested a standards-compliant remediation plan. | Planning only; no new implementation admission or live repair. |
| 2026-10-10 | Prior investigation incorporated. | [Investigation](../../investigations/download-path-discrepancy-2026-10-10.md): three historical relocations, current file stats and existing guard tests. Evidence limits preserved. |
| 2026-10-10 | Coding standards MCP routed the proposed scope and acceptance tooling. | [Standards](reports/standards.md): final snapshot, 42 selected policies, no unresolved routing categories. |
| 2026-10-10 | Plan, owner design, write sets, acceptance procedures and issue dispositions created. | Lifecycle Planned; C1–C10 pending; exactly one next slice M1. No source edits or tests claimed. |
| 2026-10-10 | Planning artifact verification passed. | Links in all eight documents resolve, required C1–C10 inventory/statuses checked; original tracked diff remains byte-identical to the preserved baseline, and the latest-main worktree remains clean. This verifies planning artifacts and preservation, not production behavior. |
| 2026-10-10 | Independent review amended required outcomes before implementation. | C5/C7 require explicit older-direct-client warning and separate exact pinned direct/RPC consumer tests before UI expansion. C2/C4/C6 require exact live dormant claim settlement after durable release, immediate same-destination transfer, stale-state fencing and unchanged paused claims. M1 remains Planned; C1–C10 pending. |
| 2026-10-10 | Reviewed documents selectively reconstructed on the current PR branch. | Sources: investigation `1568e90c` and plan `0b97dcb2` from `work/acquisition-q1-http`; accepted replacement `a36b91e8236a65b05bc4f6e3e2fc9fe7e79d511b`. Only those nine documents were transferred and the plan amended; no branch merge or acquisition-source transfer. [Review follow-up](../../investigations/pr69-review-followup-2026-10-10.md) records source-to-replacement lineage and preservation evidence. |
| 2026-10-10 | PR #69 regressions repaired separately from reconciliation. | `f6fb33fc` makes the test readiness constant portable; `58fa0382` aligns legacy-schema preflight with runtime HF initialization. Windows cross-target test compilation, Linux cancellation and focused headless startup/HF-disabled checks passed; full HF-enabled core suite: 2,070 passed, 13 existing ignored. These do not satisfy reconciliation claims. |
| 2026-10-10 | Review follow-up verification and audio diagnosis completed within stated limits. | Full headless core suite also passed: 2,070 passed, 13 existing ignored. Source-constraint resolution differs from the audio recipe at NumPy/narwhals; exact failed CI selection is unavailable and local installer replay stopped at archive transport. CI now retains installed selection on failure; audio qualification and native-platform reruns remain required. [Evidence](../../investigations/pr69-review-followup-2026-10-10.md). |
| 2026-10-11 | Live-download recovery warning corrected separately from reconciliation. | User confirmed progress was still increasing. The real activity-to-catalog merge and row-render regression reproduced the exact warning in all six tracked states. The row was projecting a missing inactive recovery ticket even when it already had live download ownership. Warning projection now follows the existing `isDownloading` ownership state; pause/resume retains exact download identity, and inactive partial models still require a current recovery ticket. Full frontend suite: 929 passed, one existing skip; typecheck and lint passed. Coding standards MCP snapshot `snapshot:v1:b24ca5dd-da8e-4746-9bf9-c47a3fbe4d6e`: 24 selected policies, no unresolved fact categories. No backend, library data, RPC or reconciliation-plan behavior changed; C1–C10 remain pending. |
| 2026-10-11 | Retained PR checkout restored inside the original repository after its temporary directory was removed. | Reconstructed only the clean task-owned index at protected head `3c43c7c8faa608202dddbd178657f635fe40ae10`, verified equality with `fix/main-test-validation`, and repaired its existing registration to the repository-local path below. Branch history and the original tracked dirty diff remain unchanged. Old logs and private recovery backups were preserved in the original repository's ignored `tmp/pumas-tmp-cleanup-20261011-bg7qr7w3/`; no model files were copied. |
| 2026-10-11 | Linux candidate rebuilt for the live-download warning fix. | Candidate source `f92d35f0b76bc8c5bf481e62501d03156eb227a7`; frontend and Electron production builds passed. Reused the validated release RPC from `ada3258e44edf9eb3108638917f28291414328ee`: the intervening diff contains only frontend and documentation changes, with no Rust/native producer input changes. Debian and AppImage bundle/resource checks, extracted RPC health/shutdown, and isolated Debian install/alternatives/Electron identity/removal passed. Outputs are in the original repository's ignored `electron/release/download-resume-20261011/`, with `local-build-record.json` and acquisition/manual-verification `SHA256SUMS`. Debian `pumas-library-electron_0.8.0-rc.1_amd64.deb`: SHA-256 `417fa9b23b567f625a11dd3f4fa6268af69e738982ce3cb3e6861f032aa94b39`; AppImage `Pumas.Library-0.8.0-rc.1.AppImage`: SHA-256 `a6cf7cbff647d24fb4fe806bda7c77573142db07ee7a79f32520c2321e87e3b1`. Build/test logs remain in the original repository's ignored `tmp/pr69-download-controls-*.log`. The running user application/download was untouched; live transfer operations, inference and foreign native platforms were not tested. No signing, publication or push; C1–C10 remain pending. |

The maintainer retains `fix/main-test-validation` and its locked worktree
`/media/jeremy/OrangeCream/Linux Software/repos/owned/ai-systems/Pumas-Library/tmp/pr69-download-controls-20261011`
for PR #69 review and user-managed push.
Target: `main`, admitted base `ad31e391`; published reviewed head `1a78ea8e`.
Owner: Jeremy, with the executing agent preparing local review amendments.
Synchronization is explicit fresh-base review; no automatic rebase or force push.
The worktree remains `retained-protected`, with commits reachable from that branch,
until the maintainer integrates or closes the proposal and its resource disposition
is verified. The original dirty checkout/branch remains user-owned and retained;
this selective document transfer grants no authority to retire it.

At implementation admission record selected plan/operation, fresh source/base and
Git state, standards snapshot, eligible consumer inventory and relevant drift.
At each semantic milestone record actual content revision, affected claims,
precise test execution/counts, required environments, standards findings and
dispositions. Record deviations before expanding implementation scope.

Terminal evidence must identify candidate package path/hash/source, PR/base/range,
all required claim links and governed resource disposition. This ledger tracks
material execution changes; it is not a command transcript or commit counter.
