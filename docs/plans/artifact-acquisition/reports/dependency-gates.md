# Acquisition prerequisites and runtime handoffs

**Q2 current-main composition (2026-10-06):** A separate patch candidate on
current main5e114f6d carries reviewed finite6308 and its shared Q2 handoff history,
with original evidence intact. Main and composed feature graphs pass33 each
before builds; current Node/HTTP/Cargo/native-owner source stays unchanged.
[Composition plan](torch-current-main-composition-2026-10-06.md) inventories
all source/API/conflict surfaces and remaining gates. Dynamic automatic/preview
resolution remains exposed and unqualified for P1. Disabling it would change
promised upstream/core behavior; parent disposition is pending, and no disabling
change or mandatory preset policy is implemented. Direct users need the shared
Torch capability. Native3a601625 remains unpublished and is not integrated.
No PR/main merge or package/runtime/source-safety acceptance is claimed.

**Q2 finite qualified recipe candidate (2026-10-06):** Tested source
`498b79611ded25151e28531b8f961dac1462061c`, tree
`97dbe68916150da6a1375abd026066293553f9a2`, on separate
`feat/torch-qualified-wheel-catalog-d87f560f` implements the optional existing
finite bundled recipe through public standalone packaging metadata APIs and
shared payload acquisition/local installation. Original lock/direct roots remain
exact; actual acquired identity/tags/Python/hash and complete marker/extras closure
validate before local --no-deps. All dependency URLs, including inactive branches,
refuse explicitly without fallback. Final229 Python/306 app-manager and strict
default workspace all-targets Clippy, Ruff/fmt pass. See
[bounded qualification](torch-qualified-wheel-catalog-2026-10-06.md).
Automatic and retained-preview resolution remain unqualified for source
preparation; private hooks remain unactivated. Actual Torch/GPU/provider,
enforced network denial, hosted/platform acceptance and independent review remain
open. AQ-HTTP and AQ-PACKAGES are not advanced; no shared/native cleanup changes.
Next existing Q2 work is review/composition and exact network-denied/provider
acceptance, with generic public resolver admission a separate decision.

**2026-10-06 automatic Q2 source milestone:** Source5946c473 uses accepted
resolver-only evidence and the common acquired-input installer/proof/publisher.
[Qualification](torch-automatic-wheel-handoff-qualification-2026-10-06.md) records
passed default/focused headless checks, unchanged11 generated headless errors,
original admissioncancel failures and correction. AQ-PACKAGES remains not ready:
fixtures are not actual automatic-provider/Torch or enforced-denial/platform/
hosted acceptance; qualified bundled recipe remains scoped. Parent owns review/
integration, and the separate native cancellation worker has no interface overlap.

**2026-10-06 Q2 repair:** Independent review found ambient requirements
injection in frozen `7a3264ac`. Child-only pip config isolation and hostile
controls are recorded in [qualification](torch-wheel-pip-config-repair-2026-10-06.md).
Custody/proof semantics remain unchanged. AQ-PACKAGES remains not ready; real
network-denied/provider/platform/hosted evidence remains separate. Automatic
resolution work is paused separately pending this repair's review.

**Owner:** acquisition integration, with one serial cross-plan integrator. This is the single status record for acquisition-provided gates. The runtime plan references these rows and does not independently declare them ready.

**Local S3 follow-up:** The staging-event and version-qualified identity fixes at verified remote `a606c80b0be41580c7af0583703d9f9af5f2cb44` have focused regression evidence. A test-only successor covers a valid large binding/bundle under the normal live watcher and public GetModel/cold output proof; [qualification](s3-staging-discovery-qualification.md) records 57 focused integration tests, strict Clippy, the failed notifier-replacement experiment and remaining limits. This supports parts of AC13/AC14, without closing AQ-S3 or authorizing live-provider credentials/configuration.
**Historical gate evidence before integration:** All gates were not ready. AC03 has real HF and llama.cpp consumer evidence for one Linux x86_64 source-built RPC scope. AC15 is accepted only for its recorded earlier candidate/scope. At Q1 handoff head `b0fef68bb4fdc5b5adc5f1e1a1bdeb4aea467dde`, the app-manager library suite passed 268 tests with 1 ignored in both default and no-default-features configurations; strict all-target Clippy passed in both configurations and scoped rustfmt passed. This is local evidence for the changed app-manager slice only, not branch-wide AC15 qualification. The latest PR-head combined commit status reported CodeRabbit success only; manual Build #36934392505 is recorded separately below. CI on prior remote Q1 head `96a1cbe8999576ca8dc70143201e6e98b0c6a37d` failed the orphan-partial recovery fixture in both default and no-default configurations. The corrected working-tree fixture passed its focused test in both configurations; full exact-head CI remains pending. Native AC05 evidence includes same-process receiptless and returned-error recovery fixtures plus a Linux x86_64 subprocess fixture killed by SIGKILL after durable destination rename and parent-directory syncs but before installed metadata publication. The subprocess test verifies cold-owner settlement and no request to its controlled loopback endpoint through both recoveries and a bounded drain. This establishes one process-loss boundary only, not power-loss or full AC05 evidence; the receiptless fixture retains only a partial orphan output. The real HF near-settlement `get_model_download_status` error (`-32603`), mutable `main` reference, schema-7 deployment population, old-writer exclusion, rollback and root disposition remain open. Independent AC18 source review found no substantiated ownership defect in migrated HF and llama.cpp paths; public-caller dispositions, future wheel/S3 composition, and complete architecture acceptance remain pending. Build #358 passed on the earlier documentation-only PR head, including Windows native QA. The other required Q1 claims and broader desktop, deployment, resource, public-client, shutdown and platform scopes remain open.

**Current integration qualification:** [Build 37141872406](https://github.com/MrScripty/Pumas-Library/actions/runs/37141872406) passed all seven ordinary jobs on `0dd38c707125facfdaae29c82704213e85ceb155` (tree `7611df7a568d56807589bb2dca2bfbb03e484dd0`): workflow/release contracts, frontend/desktop, strict Rust quality, headless without inference, and native Linux/macOS/Windows. This is exact-head supporting AC15 evidence for the checks that ran. Release archives and native model E2E remain unqualified; AQ-HTTP, external review disposition and maintainer integration remain pending. Review repairs after this head require fresh exact-head qualification. Outstanding AQ-HTTP evidence includes the broader acceptance matrix, retained-deployment migration/old-writer safety, representative resource bounds and public-client compatibility.

**Earlier exact-head Build:** Build #36953686002 ran on Q1 code head `9827759ef2b0eb0dc39369f00d65f66a6036d934`. Workflow/release contracts, Rust quality, headless-without-inference, frontend/desktop contracts and Torch native QA on Linux/macOS/Windows passed. All three Torch native RPC E2E jobs failed during CPython 3.14.7 license collection because declared zstd was absent from every archive and declared zlib-ng was absent on Linux/macOS.

**Historical completed hosted run:** Build [#36958240065](https://github.com/MrScripty/Pumas-Library/actions/runs/36958240065) ran on pre-slice code head `f0311afdce69dbae5ca0cc53d562e8113f237732`. Workflow/release checks, Rust quality, headless-without-inference, frontend/desktop contracts and Torch native QA on Linux/macOS/Windows passed. Its three Torch native RPC E2E jobs failed during CPython 3.14.7 license collection: Windows lacked the declared zstd member; Linux/macOS lacked declared zstd and zlib-ng members. This does not qualify the new Q1 test head or make AQ-HTTP ready; exact-head hosted validation is required after pushing it.

**Previous local AC15 candidate (2026-10-02):** Source candidate `89b890e780b74e1d6674465fc9671baaaecba1ac9a10ceb50d395b5b57f422e3` over HEAD `42a198bfc73e9f32e5107bc57fe5c25ed7e3c2c9` plus seven Rust/frontend paths passed the complete default/no-default Rust package suites, formatting, workspace all-target/all-feature check, strict Clippy, and frontend/Electron/launcher/desktop-contract checks. This evidence predates the current AC09 source addition.

**Historical AC09 source candidate (2026-10-02):** HEAD `23dd044744f086bc295c767f6b335f0c3c29cb69` plus Rust/frontend source diff fingerprint `9da31bf9e7d397621ba70320b81060b4890a0be78995e5cbf5592cce729494e7`. The focused AC09 test passed in default and no-default configurations, and the complete local AC15 Rust package, feature, formatting, check, lint and frontend/desktop-contract matrix passed on this corrected candidate without retries. Exact counts, ignored tests, warnings, toolchain version and scoped limits are in the execution ledger. Immediately before publication, PR #7 was OPEN/draft from `work/acquisition-q1-http` at `23dd044744f086bc295c767f6b335f0c3c29cb69` to `work/artifact-acquisition-runtime-plan`; CodeRabbit was the only reported current status context. The remote integration branch was `e37bbf4b964a0e2aadf25f80ab71edd8fa6b3eb3`. Build [#36995870643](https://github.com/MrScripty/Pumas-Library/actions/runs/36995870643) completed on that predecessor code head before the AC09 test was added: workflow/release, frontend/desktop, headless without inference, Rust quality, and Torch native QA on Linux/macOS/Windows passed. Torch native RPC E2E failed on all three OSes at CPython 3.14.7 full-archive license collection because declared license members were absent: Linux/macOS `LICENSE.zlib-ng.txt` and `LICENSE.zstd.txt`, Windows `LICENSE.zstd.txt`. Release/package assembly jobs were skipped. This failure is in unchanged Torch acceptance tooling and does not close hosted AC15; exact-head hosted validation is still required after committing the current candidate. AC09, AC15 current-head and AQ-HTTP remain pending; all gates remain not ready.

| Gate | Provider milestone / claims | Required consumer observation | Unblocks | Current status |
| --- | --- | --- | --- | --- |
| AQ-HTTP | Q1 / AC01–AC10, AC15, AC16, AC18 | Existing HF model acquisition reaches awaited model import; existing llama.cpp installer consumes the same neutral verified-file handoff and reaches its own validated extraction/publication. Cancellation/restart/UI evidence included. | Runtime R1 on the qualified targets. | Q1 in progress; not ready; AC03 accepted for recorded scope; AC15 integration-head ordinary checks passed in Build 37141872406; review-repair head requalification pending; AC07 has one bounded Linux local controlled-403/explicit-refresh consumer regression; AC01/AC08/AC16 have one bounded Linux public-core mixed-size status-poll/import-settlement fixture, one header percentage projection regression (`0.42` → `42%`, 28 component tests), one retained paused-status `/rpc` regression (1/1, stable identity/bytes/fraction, bounded shutdown), one retained paused cancellation/isolation/reopen `/rpc` regression (1/1, target-only cleanup, keeper preservation, durable inventory and same-process reconstruction), one retained weak-selection resume-refusal `/rpc` regression (1/1, exact public error and unchanged paused projection, full inventory and files across refusal/reopen), and one digest-backed retained resume/import-settlement `/rpc` regression (1/1, complete 24-byte payload stays `Downloading` at the held real importer write, then settles to receipt-backed `Completed`). AC10 has one bounded local two-worker HF saturation regression (1/1, typed admission refusal preserves durable/source state, retry reaches import after worker drainage), which does not measure representative resource envelopes; bounded AC18 source review found no defect in migrated HF/native paths but full scope remains open. The new RPC fixture uses complete retained bytes and does not exercise live HTTP continuation or diagnose the historical `-32603` |
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

**Additional AC10 measurement (2026-10-06):** Exact source `8a147160c2b3743c549e00a89b1e48f54d7a2390` (tree
`9544e16dde7802baa2033bf7ceea7d44c2ba3767`) passes one Linux synthetic 512 MiB public HF transfer through the
real importer/receipt settlement in separate default/no-default processes.
Inode continuity, retained-file inventory and raw memory/I/O are bound in the
[report](ac10-public-hf-measurement-2026-10-06.md); broader resource/platform/
provider claims remain unqualified. AC10 and AQ-HTTP stay pending. Q2 remains
gated and has no new admitted source work: approved exact wheel closure,
shared verified local wheel handoff, denied-network local-only child consumption,
lease-through-exit/cleanup and installed-output proof are still required, with
resolver/bootstrap traffic separately accounted. AQ-PACKAGES is not ready.

## AQ-HTTP positive elapsed-budget prerequisite — local feature complete

Source `0950c4d5839608c8dd5f52afcb3fd2424a9a1656` (tree `55c7dea28bae397ee9d70995897c5ff0c8e2a672`) enforces the existing
positive per-file HTTP budget across headers/body/retries/backoff, preserving
zero opt-out, attempt limits, exact identity/verification and receipt policy.
Ten owned loopback controls plus 153 acquisition-filter and 192 HF download
cases in each default/no-default mode, one public HF status/import control in
each, and strict Clippy in three feature modes pass. This supports bounded
AC02/AC06/AC10/AC15 behavior only. Already registered writes drain before return;
no hard filesystem/verification/import/child-cleanup return deadline is claimed.
See [http-elapsed-budget-qualification-2026-10-06](http-elapsed-budget-qualification-2026-10-06.md) for exact logs and independent source reviews.
No public shape, S3/native frozen source, generator or Q2 writes. AQ-HTTP remains
not ready; broader acceptance, current-head hosted checks, resource/platform/UI,
public-client and deployment dispositions remain pending. Q2 exact local package
handoff/denied-network installation remains the next gated implementation feature.

## Q2 retained-preview source milestone — gate remains not ready

The coordinator separately admitted source implementation on frozen HTTP8756,
with AQ-HTTP governing release acceptance. Tested source
`e246645bcf7d2f3bae2f68a0ba29215fafe335e9` (tree
`24eceee6dc62b0c368f666c43989c34ba3b2a3bb`) supplies exact shared wheel acquisition,
local-only consumption, post-probe installed-member/provenance proof and retained
input/child/receipt custody for unqualified retained previews. The 13 shared
controls, 301 app-manager tests and 210 pinned-tool Python tests pass. Strict
default checks and headless production binary pass; full headless test Clippy has
an unchanged helper failure. [Qualification](torch-verified-wheel-handoff-qualification-2026-10-06.md)
records exact evidence and failures.

AQ-HTTP and AQ-PACKAGES remain not ready. The denial fixture could not create its
UID map, so no enforced-denial claim exists. Real Torch, provider/platform,
installed/desktop and hosted evidence remain required. Automatic selection and
qualified recipes retain scoped existing paths; bootstrap traffic is separately
owned. Next Q2 source work separates automatic resolution from final payload
installation. No runtime gate, merge or acceptance is advanced.
