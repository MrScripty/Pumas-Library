# Acquisition prerequisites and runtime handoffs

**Owner:** acquisition integration, with one serial cross-plan integrator. This is the single status record for acquisition-provided gates. The runtime plan references these rows and does not independently declare them ready.
**All gates:** not ready. AC03 has real HF and llama.cpp consumer evidence for one Linux x86_64 source-built RPC scope. AC15 is accepted only for its recorded earlier candidate/scope; exact-head requalification for the current Q1 branch is pending. CI on prior remote Q1 head `96a1cbe8999576ca8dc70143201e6e98b0c6a37d` failed the orphan-partial recovery fixture in both default and no-default configurations. The corrected working-tree fixture passes its focused test in both configurations; full exact-head CI remains pending. The Linux x86_64 native receiptless-`Using` and receipt-bearing post-rename cold-owner fixtures pass locally and provide bounded AC05 evidence. Both are same-process; the former has a partial orphan output, and the latter does not directly observe zero source-request attempts. Neither establishes hard-process/power-loss or full AC05 evidence. Independent AC18 source review found no substantiated ownership defect in migrated HF and llama.cpp paths; public-caller dispositions, future wheel/S3 composition, and complete architecture acceptance remain pending. Build #358 passed on the earlier documentation-only PR head, including Windows native QA. The other required Q1 claims and broader desktop, deployment, resource, public-client, and platform scopes remain open.

| Gate | Provider milestone / claims | Required consumer observation | Unblocks | Current status |
| --- | --- | --- | --- | --- |
| AQ-HTTP | Q1 / AC01–AC10, AC15, AC16, AC18 | Existing HF model acquisition reaches awaited model import; existing llama.cpp installer consumes the same neutral verified-file handoff and reaches its own validated extraction/publication. Cancellation/restart/UI evidence included. | Runtime R1 on the qualified targets. | Q1 in progress; not ready; AC03 accepted for recorded scope; AC15 historical candidate accepted, current-branch requalification pending; bounded AC18 source review found no defect in migrated HF/native paths but full scope remains open |
| AQ-PACKAGES | Q2 / AC11, AC12 plus affected AQ-HTTP regressions | Existing Torch package integration consumes an approved exact wheel set locally with network denied during installation; no new adapter registry needed. | Runtime R2 after R1. | Not ready |
| AQ-S3 | Q3 / AC13, AC14 plus source-independent regressions | The same manifest/transfer/handoff contract works with version/credential/range conditions on AWS S3, one non-AWS compatible service and local MinIO; an S3-sourced model completes import and is observed through GetModel or EnsureModel. | Runtime or model S3 acquisition on qualified endpoint/target combinations. | Not ready |
| AQ-COMPLETE | Q4 / AC01–AC18 and all milestones accepted | Actual installed/public/native consumers and source-migration/deletion dispositions satisfy the complete acquisition scope. | Acquisition plan acceptance; not a hidden prerequisite for HTTP-only runtime work. | Not ready |

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

Default serial order is Q1, Q2, then the integrator selects the ready runtime R1 or acquisition Q3 work from product priorities and disjoint writes. Each plan still has exactly one next slice. Parallel development is allowed only under its declared ownership; integration remains serial for shared contracts/state/generator files.

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
