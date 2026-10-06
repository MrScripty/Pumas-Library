# HTTP elapsed-budget qualification — 2026-10-06

The selected Q1 feature enforces the existing positive HTTP per-file source-wait budget. It repairs a concrete AQ-HTTP prerequisite for Q2: a wheel acquisition must not wait indefinitely on response headers, a stalled successful body or retry backoff while ignoring the caller's elapsed budget. It does not implement the package installer or declare AQ-HTTP ready.

## Existing pieces and the missing behavior

The public shared HTTP consumer already admits an exact manifest/source set, loops over all selected files, seals the complete set, grants a retained verified `Using` lease, executes consumer prepare/publication, and settles a bound receipt. Exact multi-file selection, SHA-256 verification, workspace capabilities and consumer receipts were present before this slice. Package resolution and installation semantics stay with the package owner.

Before this repair, `AcquisitionSource::Http::transfer_deadline` returned None, HTTP open/header waiting had no owner deadline, and HTTP body streaming had no response deadline. `elapsed` was checked only after retryable errors, and backoff had no remaining HTTP deadline to clamp against. A server could hold headers or a successful partial response indefinitely under an explicitly positive budget. This is missing behavior, not a need for more S3 failure fixtures.

## Source and public compatibility

Branch `feat/acquisition-http-budget-0c02dcfb` retains base `0c02dcfb81a0f7ab2a3858c2b5cee329816b3ce5`, tree `78c663ed397264284cb95662d49a3354380755d4`. Tested source `0950c4d5839608c8dd5f52afcb3fd2424a9a1656`, tree `55c7dea28bae397ee9d70995897c5ff0c8e2a672`; `acquisition/service.rs` blob `83478facd105a08f8a52e36e6ce9780d09e76d2a`, SHA-256 `3e611252c15f6c87aa2f4f22519d1ed80b2dbe1f22855089b77cb8aeec839c4f`. Production code changes only that owner file, with co-located tests; core README, the owner contract and admission/evidence documentation describe the change.

There are no public struct/signature, persisted schema, source identity, retry-attempt, authorization, verification or receipt-shape changes. A positive `AcquisitionRetryPolicy.elapsed` now creates one absolute deadline per file before local preparation, reused across response headers, body streaming, attempts and capped backoff. A new selected member gets its own deadline; a retry does not. Unrepresentable positive clock budgets fail before public worker/store admission. Explicit HTTP elapsed zero remains the legacy opt-out. The HF consumer already supplies its configured positive elapsed field; its source waits now honor that field. The native archive consumer explicitly supplies zero, which remains unchanged.

HTTP waits use biased deadline selection and a small per-poll clock check. Locked Tokio 1.49.0 `Timeout::poll` polls its inner future before the timer; `Sleep` can also yield under exhausted cooperative budget. The clock guard checks expiry before polling Sleep and before the request/body branch can be polled on that decision. It creates no new task, retry owner or capacity policy. Existing outer pause/cancel priority and body control flags remain. This remains cooperative: it cannot preempt an already-running poll or filesystem syscall.

Already registered writes retain their descriptor/workspace execution capability and are joined through the existing drainage path before retry or return. Timeout retains Transferring custody and partial bytes, without consumer preparation/publication or a completion receipt. Local verification, effect drainage, import/install and child cleanup are not given this source-wait deadline; no hard caller-return latency is claimed. S3 deadline branches, diagnostics and all S3 reader/manifest/native production write sets are unchanged.

## Acceptance controls and validation

The ten public HTTP controls use owned synthetic loopback sources:

1. A header wait expires without consumer handoff.
2. A successful response stalls after two observed partial bytes; expiry retains exactly those bytes and Transferring/no-receipt state.
3. Ten-second backoff cannot exceed a five-second remaining budget or issue another request.
4. Four seconds before a first 503 plus two seconds at the next header wait exceed one five-second file budget; retry cannot reset it.
5. An admitted write remains gated across expiry; the operation and workspace capability remain held until the write drains, then retain its bytes without publication.
6. A partial-open effect is gated after headers/body bytes are buffered; after expiry/release no buffered-body write starts and the partial stays empty.
7. Explicit zero survives six seconds of simulated source hold and settles normally when released.
8. Three exact SHA-pinned members each consume three seconds under independent five-second budgets; the nine-second set completes with its original exact manifest and a validated bound receipt.
9. Cancellation paired with header expiry retains its cancellation outcome.
10. Overflow refuses before any accepted source connection, callback, durable row or file preparation.

Clocks advance only after actual source/effect observations; guards are separate from configured production budgets. Every started fixture releases gates and performs bounded cleanup before assertions. Unconfirmed owner drainage retains the disposable root. Source monitors stay owned through drainage; the overflow monitor also observes a bounded quiet window. Held capability probes do not claim OS-level exclusion. Cancellation under body/backoff and warm pause/resume are existing-suite coverage, not additional claims of the header-cancellation control.

Final-source acquisition-filter tests pass **153/153 in default and 153/153 in no-default** (including all ten new controls). HF download tests pass **192/192 in each configuration**; mixed-size public HF status/import/receipt controls pass **1/1 in each**. Strict all-target core Clippy with warnings denied passes in default, no-default and default-plus-S3. Scoped rustfmt and `git diff --check` pass. These are affected-slice checks, not full workspace or hosted qualification.

All Cargo runs use Rust/Cargo 1.92.0, one serialized job, incremental disabled, offline/locked dependency resolution and explicit dev/test debug=0. The unchanged workspace ort dependency disables defaults, enables `load-dynamic` and does not enable `download-binaries`; explicit runtime acquisition remains unchanged. No ONNX runtime/model was downloaded or provisioned. Repository-local MrScripty identity is verified before/after commits. Parent owns PRs, hosted qualification and integration; no external reviewer was contacted.

[Evidence JSON](http-elapsed-budget-2026-10-06/evidence.json) binds exact commands, source profiles, exits and SHA-256 hashes of a raw-log archive. Original scratch is `/workspace/scratch/http-elapsed-budget/`. Diagnostic bytes are preserved; collected environment values are limited to named build settings, not credential stores or full environments.

## Failures and review corrections

The initial test-only old-owner run failed six controls and passed three compatibility controls (exit 101). Its exact patch/blob/log remain retained. These are the pre-refinement fixtures; later review strengthened root retention, overflow source-I/O observation and cumulative multi-file timing. An intermediate compile failed because a conditional TempDir.keep moved the root before a later path assertion; saving an owned state path repaired it. The nine-control repair then passed. The timer-ready variant passed 153 acquisition-filter cases before final cooperative clock refinement. Those intermediate results are historical and do not qualify the final blob by themselves. A final rustfmt invocation without the repository toolchain environment failed before formatting (read-only default rustup home); the ordinary configured-environment invocation passes, and both logs are retained. Both independent specification and standards reviews accepted the exact final blob with no remaining concrete finding.

Authorized inactive reproducible test binaries were hashed before removal, and own temporary PR42 dependency copies were parked to preserve build headroom. Source, retained custody/evidence roots, release archives and frozen branches remain intact. All three temporary PR42 dependency directories were restored to their original paths after Cargo drainage, with before/after tree digests matching; the restoration manifest is archived.

## Remaining gate and next feature

This completes the bounded positive HTTP source-wait feature. AQ-HTTP remains pending on the broader AC01–AC10/AC15/AC16/AC18 acceptance, hosted/current-head qualification, public-client and deployment dispositions, representative resources, actual UI and supported-platform evidence. Linux loopback/core tests do not establish those claims.

The next existing-plan implementation feature remains Q2 accepted exact wheel closure → shared verified local handoff → local-only installation under denied network → installed-output proof, with input custody held through package child exit/cleanup and resolver/bootstrap traffic separately recorded. `resolve_runtime.py --install` currently resolves/installs before reporting its lock/result, and the Torch installer consumes that combined result; this missing package boundary is not implemented here. Q2 remains gated until the applicable AQ-HTTP scope is accepted. PR42 generator source/evidence and AC10 source/evidence remain separate and frozen.
