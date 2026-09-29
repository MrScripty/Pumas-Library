# Q1 starting state and source inventory

**Captured:** 2026-09-29. **Evidence type:** read-only repository, GitHub PR/CI, and source review. **Acceptance effect:** none; all Q1 claims remain pending.

## Repository and history

- Repository: `/media/jeremy/OrangeCream/Linux Software/repos/owned/ai-systems/Pumas-Library`.
- Starting `HEAD`, `origin/main`, and local `main`: `e37bbf4b964a0e2aadf25f80ab71edd8fa6b3eb3`. `HEAD` was equal to `origin/main` before task work.
- Requested plan commit: `a8359512a580aa25fb2f9c9e4cd7e0dd64fd970d`. It is a child of `04e7f1568f00693c0ef26c77e0150e5e4dd112ea`, which is the merge base with current `main`; it is not an ancestor of current `main`. The commit contains the coordinated plans and shared contract. It is preserved as an ancestor of the Q1 task branch through merge commit `f4dd7ff9`.
- Initial local branches: `main`; the only advertised remote heads were `main` and `ci/windows-native`. Existing archive refs and unrelated detached worktrees were left untouched. The task created no extra worktree.
- Initial worktree state contained the unrelated untracked [`docs/breif/future.md`](../../../breif/future.md). It remains unmodified and unstaged. `pumas-coordinated-plans.zip` was not present in this checkout. No stash, reset, clean, or deletion was performed.
- The primary branch is `work/acquisition-q1-http`, created from the exact accepted base above. The first commit `f4dd7ff9` integrates only the original plan/contract documentation and the docs index; it preserves the original `a8359512` commit.

## Current PR and CI state

The read-only GitHub query found no open pull requests. The latest relevant merged PRs are:

- [PR #6 — Torch installation flow, package validation, and progress](https://github.com/MrScripty/Pumas-Library/pull/6), merged 2026-09-29.
- [PR #5 — Managed Torch runtime foundation and cross-platform installation](https://github.com/MrScripty/Pumas-Library/pull/5), merged 2026-09-29.
- [PR #4 — Torch image adapter integration and dimension fixes](https://github.com/MrScripty/Pumas-Library/pull/4), merged 2026-09-29.

On starting SHA `e37bbf4b964a0e2aadf25f80ab71edd8fa6b3eb3`, [Build run 36624219733](https://github.com/MrScripty/Pumas-Library/actions/runs/36624219733) and [Scorecard run 36624219480](https://github.com/MrScripty/Pumas-Library/actions/runs/36624219480) both completed successfully. They qualify only the starting base, not this Q1 candidate.

## Current acquisition owners and paths

| Concern | Current source owner | Q1 boundary |
| --- | --- | --- |
| HF repository/revision/file selection and model destination | `rust/crates/pumas-core/src/api/hf.rs`; `rust/crates/pumas-core/src/model_library/hf/{metadata,types,download}.rs` | Preserve model selection and destination authorization at the model consumer. |
| HF worker generation, transfer state and finalization | `rust/crates/pumas-core/src/model_library/hf/lifecycle.rs` and `hf/download.rs`; `import_completed_download` awaits the importer | Move shared transfer custody coherently; keep model import/publication distinct. |
| Durable current HF attempt state | `rust/crates/pumas-core/src/model_library/download_store.rs`; recovery/root grants in `model_library/download_recovery.rs` | This is HF-specific today; it cannot silently become the neutral runtime store. |
| Normal intent acquisition | `rust/crates/pumas-core/src/intent/acquisition.rs` through prepared/owned HF operations | Keep this normal model-facing path connected to the canonical acquisition owner. |
| llama.cpp release/build selection and installation | `rust/crates/pumas-app-manager/src/version_manager/installer.rs` | Keep selection, extraction, install verification and installed metadata with app-manager; replace its independent byte-transfer lifecycle. |
| Existing generic HTTP helper | `rust/crates/pumas-core/src/network/download.rs`, exported from `network` | Public API; no internal production caller was found. External compatibility is unknown, so its removal or delegation is unresolved. |

The HF transfer loop builds revision-scoped URLs, sends Range requests, writes guarded partial files, observes cancellation/pause, validates selected content, promotes bytes and then waits for importer settlement. A focused existing local TCP fixture, `pinned_aux_and_payload_retry_resume_after_reopen_and_import_exact_revision`, exercises truncated transfer, failed/successful range resume, reopen, byte identity and import. It is not a live Hugging Face service run.

The llama.cpp path calls `download_archive` from `installer.rs`, keeps an installer-owned retry/progress/cancel loop, and caches under `launcher-data/cache/downloads`. `is_cached_download_valid` currently accepts matching file size; `GitHubAsset` has name, size, URL and content type but no digest. `do_llama_cpp_install` removes the existing tag directory and extracts directly into the final directory before `finalize_llama_cpp_installation` writes metadata. Existing tests cover platform asset selection and extraction/wrapper behavior, not a full live archive acquisition/reopen/publication sequence. Historical real llama.cpp and model-serving observations remain scoped to their recorded older candidates.

## Retained-state and recovery facts

Current code reads download-store schema 5 and has a specific schema 4 to 5 migration. The durable document contains download snapshots, recovery revocations, lifecycle cleanup quarantines, admission attempts, queue admissions, and released queue-admission proofs. Snapshots include model repository, filenames, model destination, status, request, revision and optional Hugging Face evidence; destination authority is separately reconstructed from the model root.

The current standards-remediation plan accepts selected incremental recovery and local-intent operations within their recorded Linux evidence. It explicitly leaves broader C3 recovery and hard-process-crash claims open and refuses unresolved admission/quarantine cleanup replay. The active local-intent plan records M1–M4 complete within its own bounded scope; those results do not accept the proposed shared acquisition owner.

Source-supported formats are visible, but no actual deployed retained root or older running writer inventory was read. Therefore the deployed record population, old-reader/writer retirement, and safe rollout/rollback facts are **unavailable**. Do not modify live roots or claim migration safety until an owner records those facts. Disposable v4/v5 fixtures and current-reader reopen tests are independent and may proceed.

## Plan and authority dispositions

- Acquisition Q1 is the only active implementation slice. AQ-HTTP, AQ-PACKAGES, AQ-S3 and AQ-COMPLETE remain not ready.
- The Torch package resolver/install path remains with the current Torch plans until acquisition Q2; Q1 does not change package resolution or attempt a network-denied install.
- Runtime R1 remains unstarted until AQ-HTTP passes at its target scope and the prerequisite is merged into current `main`. Runtime R2 remains gated by R1 and AQ-PACKAGES.
- No task-created worktree exists. All pre-existing `.muse` and `passeur_cache` worktrees, archived branches, and user-owned files remain untouched.
- Q1 real HF service/import, current llama.cpp archive/install, desktop control, capacity, deployed migration, and old-writer evidence have not run. No production code or live data changed in the preparation recorded here.
