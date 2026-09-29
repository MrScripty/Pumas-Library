# Acquisition execution ledger

## 2026-09-29 — coordinated planning delivery

The user requested a separate acquisition prerequisite plan alongside the runtime/model-adapter plan. The attached r3 plan and integration review were read; current Pumas branch and standards refs were verified. Pumas remains at `04e7f156`; standards is now `39d55dc` (the inspected intervening commit changes test/evidence material, not the retained normative planning/architecture rules).

The source review expanded into actual model-bound destination/recovery types, durable snapshots and task ownership. The design now places neutral acquisition below both consumer domains, with one proposed contract, existing HF/native and package consumers to prove it, and explicit HTTP/package/S3 gates. This avoids a cycle with the downstream new installation and adapter implementations.

Created this acquisition plan, a proposed shared contract, gate record, source and composed-design reviews, acceptance/write-set/issue records, and an updated runtime plan at its existing intended path. The planning package uses two sibling execution plans, not a third master plan. No production source, repository state, live store, installed runtime, model or release was changed.

**Evidence:** source/docs inspection and mechanical delivery checks only. All AC production claims and AQ gates remain pending/not ready. No Rust/Python production suite, source-service integration, network-denied package installation, GUI, migration, GPU workload or native release was executed. No independent reviewer was run; independent review is a later explicit acceptance requirement.

**Next slice:** Q1 after current checkout/consumer/retained-state preparation and exact source-work admission. Runtime R1 remains gated by AQ-HTTP.

## 2026-09-29 — Q1 started from accepted current main

- Resolved `a8359512` to `a8359512a580aa25fb2f9c9e4cd7e0dd64fd970d`. It was not an ancestor of current accepted `main` `e37bbf4b964a0e2aadf25f80ab71edd8fa6b3eb3`; the common ancestor is `04e7f1568f00693c0ef26c77e0150e5e4dd112ea`. Preserved the original planning commit through the plan-only integration merge `f4dd7ff9` on `work/acquisition-q1-http`. No accepted source was reset or replaced.
- Current GitHub inventory: no open PRs; PRs #4, #5 and #6 are the latest merged runtime/Torch changes. On the exact base SHA, Build run `36624219733` and Scorecard run `36624219480` both completed successfully. These are base-only CI results. See the [starting-state report](reports/q1-starting-state.md) for links and source evidence.
- Source preparation traced the normal model workflow, durable download owner, native runtime installer, and existing generic download API. The acquisition module, actual shared transfer owner and both production cutovers do not exist yet. The source inventory and independent architecture review are preliminary only; no Q1 gate claim is satisfied.
- Existing local-TCP restart/import tests and historical native installation evidence retain their original scope. No real HF service, current llama.cpp archive, desktop workflow, live migration, or Q1 production test has been executed in this slice.
- The store reader currently supports schema 5 and explicit schema 4 upgrade; deployed record population and old-writer retirement are unknown. No live retained state was opened or modified. Pending/unresolved cleanup replay remains refused under its existing recovery owner.
- Before the first Q1 source edit, the staged write set is limited to the validated manifest value module: `rust/crates/pumas-core/src/acquisition/{mod.rs,manifest.rs}` and `rust/crates/pumas-core/src/lib.rs`, with co-located manifest tests. Planned evidence: `cargo test -p pumas-library acquisition::manifest` plus formatting and clippy for the touched Rust crate. This establishes validated identities only; it does not satisfy AQ-HTTP, transfer lifecycle, persistence, consumer integration, or gate acceptance. The complete Q1 write set remains the one in [write sets](reports/write-sets.md); the exact next vertical consumer cutover will be recorded before those edits.
- The sole implementation slice remains Q1. Q2, Q3 and runtime R1 remain dependency-gated; no acquisition gate is ready.
