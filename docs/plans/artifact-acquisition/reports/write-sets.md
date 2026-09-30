# Acquisition implementation write sets and coordination

These are the admitted exact paths/closed path families. The actual source files selected for the current Q1 slice are listed in the execution ledger; remaining paths below are authorized Q1 boundaries, not claims that those implementations exist. Before a slice edits files, its integrator records the exact members and actual tests in its ledger. New authority/consumer boundaries trigger re-plan.

## Q1: one HTTP acquisition owner with real consumers

**Canonical module paths:** `rust/crates/pumas-core/src/acquisition/{mod.rs,manifest.rs,http.rs}` currently hold validated source-neutral selections and the HTTP representation/body-streaming protocol. The GitHub release adapter is `acquisition/github_release.rs`, with its fresh asset-metadata resolver in the existing `network/github.rs` owner; the public/cache release DTO remains unchanged. The adapter maps publisher asset identity/digest evidence into a verified manifest while keeping the retrieval URL ephemeral. The remaining Q1 design still needs one durable transfer owner and capability workspace; place those with the canonical module rather than creating another downloader. Source-reader adaptation may remain in that module or a focused `acquisition/sources/` child if the actual design supports it.

**Existing owners allowed to change:**
- `rust/crates/pumas-core/src/lib.rs`, `network/{mod.rs,download.rs,github.rs}`, `models/github.rs`, `model_library/hf/{mod.rs,download.rs,lifecycle.rs,types.rs,metadata.rs}`;
- `rust/crates/pumas-core/src/acquisition/{mod.rs,manifest.rs,http.rs}` and focused source adapters under that module when the selected source evidence requires them;
- `rust/crates/pumas-core/src/model_library/{download_store.rs,download_recovery.rs}` only for the extracted authority and explicit supported migration;
- `rust/crates/pumas-core/src/tests.rs` when a builder/reopen fixture depends on the selected completion-evidence invariant;
- directly affected core `api/hf.rs`, `api/state.rs` and current model-importer/intent completion call sites, selected from the actual producer/consumer trace before editing;
- `rust/crates/pumas-app-manager/src/version_manager/{installer.rs,ollama.rs,progress.rs,state.rs}` for the acquisition bridge and transfer progress, not installed-unit identity migration;
- current core atomic JSON/capability-filesystem modules only where the same selected invariant requires a targeted change, never as an unrelated filesystem rewrite.

The current root-owned extraction-preparation sub-slice is limited to `rust/crates/pumas-core/src/model_library/{download_recovery.rs,hf/lifecycle.rs}` and the co-located lifecycle regression. It adds a physical-root equality key and lets the existing supervised lifecycle owner hold distinct root grants concurrently while coalescing independently reopened handles for the same root. It does not move the lifecycle owner, add a transfer/persistence authority, or advance AQ-HTTP.

The isolated native-custody worker proposal is complete and integrated from `work/q1-native-custody` commit `560cec71` into the Q1 branch. Its exact five-file write set is `rust/crates/pumas-app-manager/src/version_manager/{installer.rs,mod.rs,ollama.rs,state.rs}` and `rust/crates/pumas-rpc/src/server.rs`. The `ollama.rs` extension closes the public wrapper's owned state-mutation drain. It repairs native install/removal custody, metadata coordination, pending file-I/O settlement, and fallible stage cleanup; it does not change the source/download selection algorithm or replace the independent Ollama downloader. Shared acquisition consumer cutover remains outstanding.

**Tests:** proposed `rust/crates/pumas-core/tests/artifact_acquisition.rs`, `rust/crates/pumas-app-manager/tests/artifact_acquisition_install.rs`, plus existing co-located GitHub metadata, HF lifecycle/recovery, and installer regression tests. Fixtures must reach the owner under test with independent expected byte/effect outcomes.

**Serial adjacent writes:** `rust/crates/pumas-rpc/src/contract.rs`, `contract/export.rs`, affected HF/version/status handlers, `electron/src/{preload.ts,rpc-method-registry.ts,ipc-validation.ts}`, actual corresponding frontend download/install/source views and generated DTOs. Enumerate outputs from the actual exporter rather than guess or edit generated files manually. Reuse the existing model/native UI; this is not a dashboard redesign.

**Docs:** the canonical shared contract, acquisition plan records, bounded handoffs in the existing HF/Rust remediation and upstream-runtime plans, and current architecture/development documentation where behavior has landed. Preserve the `docs/breif` path spelling; do not rename unrelated source-intent files.

**Forbidden:** runtime installation-ID/profile migration, new model-adapter registry, package dependency reinterpretation, arbitrary source/plugin execution, consumer data deletion, claims of completed Pending replay, and concurrent second writers to the same transfer state.

## Q2: exact package-file handoff

Allowed: Q1 acquisition types/service only for demonstrated missing file-set/lease semantics; `torch-server/resolve_runtime.py`, retained preview/lock consumers in `rust/crates/pumas-app-manager/src/version_manager/{torch_preview.rs,installer/torch.rs}`, corresponding existing package/integrity/progress tests, and `artifact_acquisition_install.rs`. Keep package resolution/install semantics with those consumers. The exact managed-Python/provider files are first traced for a migrate-versus-retain traffic disposition; no speculative private integration is authorized.

No arbitrary adapter registry or new host/task API is needed for this prerequisite. The existing wheel path proves local exact consumption. Runtime R2 subsequently owns the new adapter-package caller and its acceptance.

## Q3: S3-compatible source

Proposed `rust/crates/pumas-core/src/acquisition/s3.rs` (or the already selected source directory), source configuration/credential projection at the existing authority, manifest resolution and focused S3 cases in the acquisition test target. Dependency admission can change `rust/Cargo.toml`, `rust/Cargo.lock`, `rust/crates/pumas-core/Cargo.toml` and licensing/ownership inventory through one integrator. Source URI/configuration operations and the smallest useful frontend source workflow change their canonical RPC/generated/preload/renderer projections together.

No broad list/search capability, cloud-specific UI product, remote writes, custom signer, new model modality or provider-specific installer is included. AWS/non-AWS/MinIO environment setup is test-owned, not user account mutation without authorization.

## Q4: qualification and cutover

Affected installed/public/native consumers, source installation/build packaging and exact generated outputs. Change CI/release scripts only where necessary to observe a named acceptance claim. Final docs include the active source-of-truth guide and consumer/migration limits. Remove a public legacy helper only with its actual supported consumer/version disposition; otherwise retain a delegating facade without a second lifecycle owner. No release publication is authorized.

## Shared roles and integration

| Role | Primary write authority | Shared/forbidden | Required handoff |
| --- | --- | --- | --- |
| Acquisition worker | Admitted neutral service/HTTP internals and focused tests | Shared DTO/store migration requires integrator; no app-manager identity refactor | Invariants, exact files, cases/results, lifetime and remaining gaps |
| Consumer worker | Current HF/native/package bridge after contract fixed | No independent transfer state or source policy | Actual producer/consumer observation and completion semantics |
| S3 worker | S3 reader/config adapter after gate contract fixed | No global retries/store/manifest redesign or shared lockfile edits | Version/range/auth/endpoint evidence and dependency decision |
| Desktop worker | Admitted views/interaction tests against generated contract | No backend compatibility or completion policy; generated outputs integrator-owned | Real workflow, async freshness, keyboard/focus and error observations |
| Integrator | Both plan states, shared contract, persistence changes, lockfiles, exports, shared fixtures | Preserves unrelated work and history | Gate status, exact staged diff, integration evidence and commit disposition |
| Reviewer | Read-only candidate/context/evidence | Never edits | Architecture, migration, trust and lifecycle findings; narrow repair verification |

Default serial order prevents Q1/Q2 installer changes racing runtime R1/R2. Record disjoint primary/allowed-adjacent sets when delegating. Reuse existing branches/worktree conventions; the presence of two plans alone does not require new worktrees or a custom multiagent coordination framework.
