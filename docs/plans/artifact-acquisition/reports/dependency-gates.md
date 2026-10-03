# Acquisition prerequisites and runtime handoffs

**Owner:** acquisition integration, with one serial integrator. This is the current gate-status and consumer-handoff record.
**All gates:** not ready. The [2026-10-03 audit](../../../audits/development-takeover-2026-10-03.md) binds the current source to PR7 head `3b2a279b4d35bd19e5cc1727cc905c7ee7477fa6` and failed [Build 37055028990](https://github.com/MrScripty/Pumas-Library/actions/runs/37055028990). Prior AC03/AC15 evidence remains valid only for its recorded candidate and scope; it does not qualify this head. The [plan](../plan.md) now prioritizes baseline repair, S3, then restricted networking. Q2 and runtime generalization are deferred for this assignment.

| Gate | Provider milestone / claims | Required consumer observation | Unblocks | Current status |
| --- | --- | --- | --- | --- |
| AQ-HTTP | Q1 / AC01–AC10, AC15, AC16, AC18 | Existing HF model acquisition reaches awaited model import; existing llama.cpp installer consumes the same neutral verified-file handoff and reaches its own validated extraction/publication. Cancellation/restart/UI evidence included. | Runtime R1 on the qualified targets. | Q1 in progress; not ready; AC03 accepted for recorded scope; AC15 historical candidate accepted, current-branch requalification pending; AC07 has one bounded Linux local controlled-403/explicit-refresh consumer regression; AC01/AC08/AC16 have one bounded Linux public-core mixed-size status-poll/import-settlement fixture, one header percentage projection regression (`0.42` → `42%`, 28 component tests), one retained paused-status `/rpc` regression (1/1, stable identity/bytes/fraction, bounded shutdown), one retained paused cancellation/isolation/reopen `/rpc` regression (1/1, target-only cleanup, keeper preservation, durable inventory and same-process reconstruction), one retained weak-selection resume-refusal `/rpc` regression (1/1, exact public error and unchanged paused projection, full inventory and files across refusal/reopen), and one digest-backed retained resume/import-settlement `/rpc` regression (1/1, complete 24-byte payload stays `Downloading` at the held real importer write, then settles to receipt-backed `Completed`). AC10 has one bounded local two-worker HF saturation regression (1/1, typed admission refusal preserves durable/source state, retry reaches import after worker drainage), which does not measure representative resource envelopes; bounded AC18 source review found no defect in migrated HF/native paths but full scope remains open. The new RPC fixture uses complete retained bytes and does not exercise live HTTP continuation or diagnose the historical `-32603` |
| AQ-PACKAGES | Q2 / AC11, AC12 plus affected AQ-HTTP regressions | Existing Torch package integration consumes an approved exact wheel set locally with network denied during installation; no new adapter registry needed. | Runtime R2 after R1. | Not ready |
| AQ-S3 | Q3 / AC13, AC14 plus source-independent regressions | The same manifest/transfer/handoff contract works with version/credential/range conditions on AWS S3, one non-AWS compatible service and local MinIO; an S3-sourced model completes import and is observed through GetModel or EnsureModel. | Runtime or model S3 acquisition on qualified endpoint/target combinations. | Not ready |
| AQ-COMPLETE | Q4 / AC01–AC18 and all milestones accepted | Actual installed/public/native consumers and source-migration/deletion dispositions satisfy the complete acquisition scope. | Acquisition plan acceptance; not a hidden prerequisite for HTTP-only runtime work. | Not ready |

**Additional AC01 local evidence (2026-10-02):** The final HF file-selection implementation/evidence commit is `d1e9106b6e3c6184e69899ba0a8afb393d342b52`, based on `445af474762bebac6cc3b690c29c010a187c72ef`, with Rust source/test diff fingerprint `852582f7250cec44b68e31afd020ae1c5584deefc97b999d6862a716a76f69e1`. Final-source default and no-default explicit-selection filters passed 22/22 and 21/21; the existing pinned-commit refusal control passed 1/1 in each configuration. Strict all-target `pumas-library` Clippy, serialized workspace formatting and `git diff --check` passed. The public negative fixture rejects an incomplete explicit list before relocation of a retained indexed partial and observes no payload or admission; its positive control admits both requested files. This one synthetic Linux x86_64 result does not close AC01. The full AQ-HTTP gate remains not ready; exact-head hosted validation and the other AC01/AC02–AC10, AC15, AC16 and AC18 criteria remain outstanding.

**Additional local AC05 evidence (2026-10-02):** The Linux x86_64 native installer test `native_verified_archive_without_server_retains_custody_and_cold_reopen_refuses` passed 1/1 with a publisher-digested README-only tarball. The real extractor refused publication, and both direct consumer recovery and public manager reconstruction refused the receiptless `Using` record with the exact validation field/message. Only initial release metadata and archive requests reached the controlled source; post-drain durable store and workspace state were unchanged. The initial owner's bounded shutdown reported the failed native preparation; the cold owner's shutdown succeeded after the validation refusals. Positive archive publication and the preserved receiptless-native regression passed 1/1 each. This is same-process disposable Linux x86_64 evidence only; it does not qualify process/power-loss, other extraction failures, deployed migration, packaged/native platforms, or full AC05. AQ-HTTP remains not ready.

**Intermediate AC15 requalification status (2026-10-02; superseded by the normalized-candidate result below):** First frozen fingerprint `6de2c5f76ac6d8f818605753dc47d0c40949e00e216e8bf3697a4b8a8a61e6d0` passed the local aggregate suites and checks, recorded in the ledger and matrix. Exact staged review then found two test-only RPC `spawn_blocking` JoinHandles could be detached on timeout. The paths now await completion and observe errors; independent GPT-6.1 Sol High follow-up review confirmed the lifecycle repair and unchanged AC08/AC16 test oracle. Corrected candidate HEAD `42a198bfc73e9f32e5107bc57fe5c25ed7e3c2c9` plus the same seven Rust/frontend paths has fingerprint `e0809ff74536e8033eef64b67e4264d377a0da34f899f2dbb193f567d99890e9`. Its focused no-default RPC regression passed 1/1 after a normal-sandbox fixture setup `EPERM` was rerun with local filesystem/loopback permission. The earlier aggregate does not qualify the corrected fingerprint: complete AC15 local requalification and exact-candidate hosted validation remain pending. Historical AC15 is accepted; current-head AC15 and AQ-HTTP remain pending.

## One-way sequencing

```text
Acquisition Q1 -- AQ-HTTP --------> Runtime R1
       |                              |
       +--> Q2 -- AQ-PACKAGES ----> Runtime R2 --> R3 --> R4 --> R5
       |
       +--> Q3 -- AQ-S3 ----------> S3-enabled consumer operation
                  |
        Q1 + Q2 + Q3 --> Q4 --> AQ-COMPLETE
```

Current serial priority is baseline/Q1 repair, Q3 S3, then the separately admitted restricted node milestone. Q2 and runtime generalization are deferred unless a demonstrated dependency changes that decision. Each active plan has one next slice; integration remains serial for shared contracts/stores.

Q1 uses the existing native installer. Q2 uses the existing package installer. Neither depends on the new RuntimeInstallationId or registered-adapter implementation. This removes the circular dependency that would result if prerequisite evidence required runtime R1/R2 first. A narrow native/package bridge belongs to acquisition's write set until its acceptance; later runtime refactoring changes its consumer while preserving the contract.

## Gate scope and evidence

Before marking a gate ready, record:

- reviewed material source/candidate identity and implemented contract revision;
- exact claims and evidence links, including actual producer and consumer;
- OS/architecture, source/endpoint and dependency context to which it applies;
- known unsupported/unavailable variants and preserved preexisting behavior;
- public/persisted consumer dispositions; and
- integrator/reviewer outcome.

A Linux result is not Windows/macOS evidence. Runtime gates are evaluated for the target being integrated. Final cross-platform/release promises remain with Q4 and runtime R5. A source-only build or mocked success cannot open a required-real gate.

A status update or documentation-only change does not invalidate reviewed code. A change to content semantics, authorization, custody, persistence, package handoff or wire compatibility triggers a targeted review and re-run of the affected claims. Gate records retain the last accepted scope without authorizing an incompatible new candidate.

## Single-writer ownership and cutover

Acquisition owns the canonical shared contract, transfer lifecycle and gate records. Runtime owns installation identities, model-adapter registration and bound execution. Shared file writes in `pumas-core`, app-manager installer, Torch package integration, RPC/export and renderer projections are reserved to one integrator while a predecessor slice modifies them.

A worker may propose a contract change but cannot independently change the shared contract, both plan states and generated outputs. The integrator collects findings, reviews the changed composition, amends both plans and the affected gate status, then assigns disjoint implementation work. This is ordinary serial integration; a separate stale-proposal protocol is needed only if outstanding conflicting plan proposals actually coexist.

## Replaced revision-three sequencing

| Previous item | Current owner / meaning |
| --- | --- |
| Runtime S1a, generic acquisition | Superseded by acquisition Q1. No acquisition milestone remains in runtime. |
| Runtime S1b, installation identity | Runtime R1, gated by AQ-HTTP. |
| Runtime S2, packages plus adapters | Exact package acquisition/handoff foundation is Q2; independent registration is runtime R2, gated by AQ-PACKAGES. |
| Runtime S3/S4/S5 | Runtime R3/R4/R5; outcome scope preserved. |
| Runtime A29/A31/A32/A33/A34 shared-layer claims | Acquisition AC claims own the shared-layer proof; runtime retains explicit consumer obligations and references, not copied gate authority. |
| Runtime A30 exact installed closure | Runtime keeps the registered-adapter consumption claim; Q2 proves the prerequisite with the current package integration. These are different consumer observations. |
