# Acquisition implementation write sets and coordination

These are the admitted exact paths/closed path families. The actual source files selected for the current Q1 slice are listed in the execution ledger; remaining paths below are authorized Q1 boundaries, not claims that those implementations exist. Before a slice edits files, its integrator records the exact members and actual tests in its ledger. New authority/consumer boundaries trigger re-plan.

## Q1: one HTTP acquisition owner with real consumers

**Canonical module paths:** `rust/crates/pumas-core/src/acquisition/{mod.rs,manifest.rs,http.rs,service.rs,store.rs,workspace.rs}` now hold validated source-neutral selections, the HTTP representation/body-streaming protocol, the durable lifecycle/store, and capability workspace. The GitHub release adapter is `acquisition/github_release.rs`, with its fresh asset-metadata resolver in the existing `network/github.rs` owner; the public/cache release DTO remains unchanged. The adapter maps publisher asset identity/digest evidence into a verified manifest while keeping the retrieval URL ephemeral. The local Q1 candidate has one durable transfer owner; consumer-specific receipt interpretation stays with its existing model or native owner. Source-reader adaptation may remain in this module or a focused `acquisition/sources/` child when an accepted source requires it.

**Existing owners allowed to change:**
- `rust/crates/pumas-core/src/lib.rs`, `network/{mod.rs,download.rs,github.rs}`, `models/github.rs`, `model_library/hf/{mod.rs,download.rs,lifecycle.rs,types.rs,metadata.rs}`;
- `rust/crates/pumas-core/src/acquisition/{mod.rs,manifest.rs,http.rs}` and focused source adapters under that module when the selected source evidence requires them;
- `rust/crates/pumas-core/src/model_library/{download_store.rs,download_recovery.rs}` only for the extracted authority and explicit supported migration;
- `rust/crates/pumas-core/src/tests.rs` when a builder/reopen fixture depends on the selected completion-evidence invariant;
- directly affected core `api/hf.rs`, `api/state.rs` and current model-importer/intent completion call sites, selected from the actual producer/consumer trace before editing;
- `rust/crates/pumas-app-manager/src/version_manager/{installer.rs,ollama.rs,progress.rs,state.rs}` for the acquisition bridge and transfer progress, not installed-unit identity migration;
- current core atomic JSON/capability-filesystem modules only where the same selected invariant requires a targeted change, never as an unrelated filesystem rewrite.

The completed root-owned extraction-preparation sub-slice changed `rust/crates/pumas-core/src/model_library/{download_recovery.rs,hf/lifecycle.rs}` and its co-located lifecycle regression. It adds a physical-root equality key and lets the existing supervised lifecycle owner hold distinct root grants concurrently while coalescing independently reopened handles for the same root. It did not move the lifecycle owner or add another transfer/persistence authority.

The native-custody prerequisite was integrated from `work/q1-native-custody` commit `560cec71`. Its exact five-file write set was `rust/crates/pumas-app-manager/src/version_manager/{installer.rs,mod.rs,ollama.rs,state.rs}` and `rust/crates/pumas-rpc/src/server.rs`. The `ollama.rs` extension closes the public wrapper's owned state-mutation drain. It repairs native install/removal custody, metadata coordination, pending file-I/O settlement, and fallible stage cleanup; it did not replace the independent Ollama downloader. The current candidate separately routes llama.cpp through the shared acquisition consumer; current-head review and acceptance remain pending.

**Tests:** proposed `rust/crates/pumas-core/tests/artifact_acquisition.rs`, `rust/crates/pumas-app-manager/tests/artifact_acquisition_install.rs`, plus existing co-located GitHub metadata, HF lifecycle/recovery, and installer regression tests. Fixtures must reach the owner under test with independent expected byte/effect outcomes.

**Serial adjacent writes:** `rust/crates/pumas-rpc/src/contract.rs`, `contract/export.rs`, affected HF/version/status handlers, `electron/src/{preload.ts,rpc-method-registry.ts,ipc-validation.ts}`, actual corresponding frontend download/install/source views and generated DTOs. Enumerate outputs from the actual exporter rather than guess or edit generated files manually. Reuse the existing model/native UI; this is not a dashboard redesign.

### Completed worker admission — durable acquisition owner and Hugging Face cutover

Parent milestone: `work/acquisition-q1-http`, draft PR #7, target `main`. Worker branch `agent/q1-hf-acquisition` started at `efc4d20bde6ae426c96f6a5fcb55ce0022788d09` and delivered `65274dffd90e9c5272a89ec5a3e9c1ae564bfa0c`. It was integrated by merge commit `bc9e9da4147a3e64f2a5e67093604eddeddc5391`; root then committed the cross-record custody repair `d0b71b51075815cf307cea95c9364be0ed25472d`. The primary integrator owns PR history, the shared semantic contract, gate state, later native composition, review and final candidate evidence.

Primary worker write set:

- `rust/crates/pumas-core/src/acquisition/{mod.rs,http.rs,task_custody.rs,service.rs,store.rs,workspace.rs}`; the last three are new canonical acquisition modules.
- `rust/crates/pumas-core/src/model_library/{download_store.rs,download_recovery.rs,mutation_authority.rs,library.rs,mod.rs}`.
- `rust/crates/pumas-core/src/model_library/hf/{mod.rs,download.rs,lifecycle.rs,acquisition_source.rs,types.rs}`.
- `rust/crates/pumas-core/src/{lib.rs,api/builder.rs,api/state.rs,api/hf.rs}`.
- Co-located regressions in those files and new `rust/crates/pumas-core/tests/artifact_acquisition.rs` when a public composition/reopen path is required.

The worker implements one durable neutral acquisition owner/store and makes the ordinary HF path consume its verified-file handoff through awaited import/finalization. Schema 6 extends the same canonical file and preserves all current model snapshots, hidden admission attempts, revocations, queues, release proofs and quarantines. Fresh roots use schema 6; ordinary open/admission does not migrate schema 4/5. A separate explicit offline migration operation is required, exposed independently from normal API construction; its caller must ensure all old readers and writers are stopped. Without migration, acquisition admission reports a typed migration-required result and performs no transfer effects. The model mutation authority must read the canonical store's custody view. One store file/transaction authority remains; no second transfer writer, model marker initializer in neutral workspace code, inferred v4/v5 compatibility, cross-store exactly-once claim, or new public wire representation is allowed. The worker does not modify `docs/contracts/artifact-acquisition.md`, plan/gate/ledger files, RPC/generated/frontend outputs, `pumas-app-manager`, Cargo manifests/lockfiles, or package/S3/runtime/adapter state. Report any discovered contract or scope change before editing outside this set.

Worker handoff evidence completed for schema-4/5 fixture inventory, read-only normal admission, explicit offline fixture migration/reopen, legacy custody/Pending preservation, controlled HTTP through ordinary HF acquisition/import, cancellation and lease drainage, focused store/recovery/HF/acquisition tests, Clippy, no-default-features and formatting. The exact consumer-commit/reopen proof remains open: `Using` without a durable importer receipt fails closed, but no receipt yet exists. Root added and tested the schema-6 duplicate-demand/active-workspace validator after independent review. This HF-only vertical cutover is not AQ-HTTP acceptance; importer receipt/reopen, orphan guard, native shared acquisition, and the plan matrix remain outstanding.

**Docs:** the canonical shared contract, acquisition plan records, bounded handoffs in the existing HF/Rust remediation and upstream-runtime plans, and current architecture/development documentation where behavior has landed. Preserve the `docs/breif` path spelling; do not rename unrelated source-intent files.

**Forbidden:** runtime installation-ID/profile migration, new model-adapter registry, package dependency reinterpretation, arbitrary source/plugin execution, consumer data deletion, claims of completed Pending replay, and concurrent second writers to the same transfer state.

### Completed primary slice — importer and orphan-adoption custody guard

Parent milestone: `work/acquisition-q1-http`, draft PR #7 to `main`. Worker commit `8b6c5f70c55a3d124189d0cd88dc85c780f47c84` was based exactly on primary candidate `d0b71b51075815cf307cea95c9364be0ed25472d`, reviewed read-only, and integrated by merge commit `9719ecb50d84df796db887832b87b42e8992988a`. The resulting tree `0c3e472c8b5f05073ede4ddbe2252a2a34e54baf` is identical to the worker tree. The worker branch/worktree remain task-owned; no source was copied or rewritten. Independent review found no substantiated P0–P3 issue and ran only `git diff --check`; all test results are worker-reported and recorded in the execution ledger. The root integrator owns PR/Git history, docs, composed verification and the next slice.

Primary write set:

- Source-neutral exact operation/use identity evidence and current-record validation: `rust/crates/pumas-core/src/acquisition/{service.rs,store.rs}`; permit the one crate-private validation visibility change in `acquisition/workspace.rs` so a proof can revalidate held workspace identity without model-specific paths. HF admission and stage capabilities remain owned by `model_library` and do not add model-specific policy to acquisition.
- One model-side snapshot of current legacy queue/hidden/quarantine and acquisition custody through the existing canonical publisher: `rust/crates/pumas-core/src/model_library/download_store.rs`.
- The shared guard and exact managed-HF exception using the existing root grant: `rust/crates/pumas-core/src/model_library/mutation_authority.rs`.
- Owned ordinary import and narrowly admitted managed-HF finalization before idempotency shortcuts or Diffusers delegation: `rust/crates/pumas-core/src/model_library/importer.rs`.
- Retain the existing root grant through model metadata/index/classification effects: `rust/crates/pumas-core/src/model_library/library.rs`.
- Pass exact current HF operation evidence and the already-held execution grant at the existing partial metadata/index stage and normal/restored finalization: `rust/crates/pumas-core/src/model_library/hf/download.rs`.
- Canonical guard behavior and these owning records: `docs/contracts/artifact-acquisition.md`, `docs/plans/artifact-acquisition/{plan.md,execution-ledger.md,issues.md,reports/write-sets.md}`. Standards usability remains in its separate report.
- Co-located ownership/lifecycle regressions; `rust/crates/pumas-core/tests/artifact_acquisition.rs` only if the builder-level production path needs a cross-module fixture.

The worker edited only the listed production paths and directly co-located unit tests. It did not edit gate/plan records, generated/RPC/frontend files, public API contracts, schemas/migrations, manifests/lockfiles, unrelated lifecycle owners, or shared files assigned to another writer. The handoff includes exact source commit/tree and diff, real temporary-filesystem/store/owner evidence distinguished from synthetic faults, cancellation/root-grant/failure observations, final standards route, review focus, and a clean task worktree. The branch was integrated through a history-preserving merge.

No startup/state/reconciliation caller change is admitted unless source evidence shows it can still bypass the common importer effect boundary. `importer/recovery.rs` may receive co-located scan/test changes, but the decisive recheck stays in the common in-place import path. Existing `RuntimeTasks` owns ordinary imports; the acquisition `TaskContext` owns managed HF finalization. Do not add a registry, runtime, sidecar store, public DTO, SQLite/metadata schema, lockfile, generated interface, app-manager, adapter, or frontend path. Direct importer calls built from `ModelLibrary::new` without installed trusted mutation authority fail closed with the typed authority-unavailable result; no unguarded fallback is permitted.

The guard must acquire/reuse the configured root grant, revalidate the exact model destination, and inspect durable custody immediately before any metadata-present shortcut, indexing, Diffusers delegation, or other importer effect. A public/orphan import has no custody exemption. Preserve the existing partial HF metadata/index stub: it may use only a private capability matching the current queue admission and `Transferring` generation, manifest/demand, workspace, destination, and held root grant, and that capability is limited to the stub upsert plus its index projection (including metadata projection performed by indexing). It cannot authorize full import, package-fact resolution, or another target. Full HF finalization separately requires its current verified-file use lease and matching admission. Neither HF capability may bypass unrelated hidden/queue custody or Pending/quarantined state. Stale stage, lease, wrong demand/workspace/manifest/destination, or unrelated custody refuses. Legitimate FIFO followers must not prevent their admitted predecessor from finalizing. The held owner context and root grant remain alive until all nested importer effects settle, including when the caller stops waiting or a worker panics.

Required evidence on this completed slice: actual stale orphan-scan/acquisition-admission race; matching and stale/wrong partial and final HF capabilities; matching/unrelated retained phases; hidden, Pending and quarantine refusal; metadata-present and Diffusers branches; FIFO follower preservation; cancellation/drop during held metadata/index effects; shutdown/panic drainage; root/destination replacement; unconfigured direct importer refusal; and existing HF regressions. Worker results and evidence-kind distinctions are recorded in `execution-ledger.md`. This guard closes the importer custody bypass only; it does not make reopened `Using` recoverable or open AQ-HTTP.

### Completed Q1 slice — exact HF finalization receipt and no-replay reopen

The custody guard is reviewed and integrated. The root integrator routed this exact replay/persistence slice through Coding-Standards before source edits and inspected the document/output lifecycle. All production `downloads.json` publication converges on `AcquisitionStore` transactions: neutral updates preserve model state; `DownloadPersistence` decodes the strict schema-5 projection and republishes it through `publish_model_partition`; `document_with_partition` replaces the entire flattened legacy map. Therefore the receipt must be a separate envelope partition, never a key in the legacy map or a `ModelMetadata` field. The source currently supports missing state, schema 6, and read-only schema 4/5; pending cleanup remains non-authorizing. Normal HF completion order is durable `FilesReady`, marker removal, guarded importer metadata/index work, optional pinned package-facts resolution, exact `Using` acknowledgment, then durable queue release. Marker removal or metadata/index presence alone cannot establish completion.

The schema-6 consumer path publishes importer metadata through the held capability-backed atomic target and requires `Durable`, but metadata equality can skip a new publication. Package-facts resolution writes summary/detail rows, while its source fingerprint is only cache freshness (metadata/descriptor/dependency JSON and file length/mtime; not payload bytes) and typed metadata contains a `HashMap`. Do not treat that fingerprint as exact output identity. Receipt version 1 binds the exact acquisition ID, persisted `Using` lease, HF demand/operation, current admitted queue identity, manifest and ordered verified-file receipts, resulting model ID and destination identity. Its output proof is versioned independently from the envelope and package-facts schemas and uses canonical JSON: recursively sort object keys; preserve array order, explicit nulls and every field in the declared metadata/index/package-facts projection; serialize integers and strings with the repository JSON serializer; reject non-finite numbers and unknown proof versions. The projection includes model identity and HF provenance, imported metadata fields, the model-index row needed to resolve that identity, and—when the pinned revision path requires package facts—the exact detail facts content and package-facts contract version. The projection contract names its included and excluded fields in source, and cold validation compares current outputs against the issuer-published digest rather than deriving a new expected digest from current outputs alone. Before issuing it, the importer must explicitly publish the observed final metadata durably, including when it equals the prior bytes, and confirm required output projections. Receipt issuance is private to complete managed-HF finalization after all importer and package-facts effects succeed; partial imports and ordinary import callers cannot issue one.

Receipt publication is conditional on the same canonical transaction observing the exact current `Using` record/lease, demand, manifest, verified files, non-revoked current queue admission, destination and workspace, with the held root grant still valid. It rejects an existing conflicting receipt and treats only an identical receipt as idempotent. A fresh root grant cannot renew or replace a persisted `Using` lease before cold validation. A private receipt-qualified settlement capability is minted only after read-only validation of the historical lease, exact queue identity, selected-file receipts, receipt and output projections. It performs the `Using` → `Adopted` acquisition transition and exact queue release in one `AcquisitionStore` document publication, retaining the immutable receipt alongside the adopted record as completion history. There is no separately durable receipt/ack/release intermediate state to reopen. Unknown publication outcome reports failure, preserves custody and is resolved by cold read-only observation; importer replay is forbidden.

The receipt requires a schema-7 acquisition document with a distinct, strictly validated model-consumer receipt partition. An explicit offline conversion supports schema 4/5 and receipt-free pre-receipt schema 6; it preserves all acquisitions, model custody, leases, queue/revocation history, hidden admissions, quarantines and cleanup dispositions and never fabricates a receipt. The operator must stop every old reader/writer, including processes with cached state, before conversion; holding the new file lock alone is not evidence of exclusion. All supported entry points reject unsupported schemas without rebuilding, defaulting, or publishing. Receipt/acquisition key mismatch, orphan or duplicate receipts, malformed/unknown receipt versions, or a receipt paired with an invalid acquisition phase fail closed. A receipt remains with its `Adopted` acquisition as immutable completion history; no implicit pruning is added. Any future terminal-record compaction must remove acquisition and receipt evidence in one exact publication after its owning retention policy accepts the disposition. A pre-receipt `Using` record remains recovery-required even when output files happen to match. Actual deployed schema-6 population, overlapping processes, rollback policy, separately retained model/index state and index-only writers remain unknown; no live retained root is authorized, so this branch's migration and reopen claims use disposable fixtures only.

Admitted source write set: `acquisition/{service.rs,store.rs}` for the schema-7 envelope, exact use/receipt transaction and receipt-gated cold lease; `model_library/{download_store.rs,importer.rs,library.rs,mutation_authority.rs,hf/download.rs}` for model-owned receipt proof, durable output checks, retained-store projection and normal/restored public-workflow reconciliation; directly necessary `download_recovery.rs` or index accessors only if held-destination or exact output observation cannot use current APIs; and their direct tests. Update only these owning contracts/plan records and the separate MCP usability report. No new task owner, sidecar, metadata field, generated interface, model/API branch, or live retained-state mutation. If the current source cannot expose one exact atomic acknowledgement-plus-queue-release publication, that is a design stop requiring a corrected owner contract before implementation; do not emulate it with sequential writes. Final source routing must be repeated against the actual implemented design before this slice is complete.

### Current Q1 candidate slice — native receipt settlement and cancellation withdrawal

Exact current source paths are `rust/crates/pumas-core/src/acquisition/{http.rs,mod.rs,service.rs,store.rs,task_custody.rs,workspace.rs}`, `rust/crates/pumas-core/src/model_library/{download_recovery.rs,download_store.rs,importer.rs,library.rs,mutation_authority.rs}`, `rust/crates/pumas-core/src/model_library/hf/{download.rs,types.rs}`, `rust/crates/pumas-core/src/network/github.rs`, `rust/crates/pumas-core/src/tests.rs`, `rust/crates/pumas-core/tests/artifact_acquisition.rs`, `rust/crates/pumas-app-manager/Cargo.toml`, `rust/crates/pumas-app-manager/src/version_manager/{installer.rs,mod.rs}`, `rust/crates/pumas-rpc/src/{main.rs,server.rs}`, and `rust/crates/pumas-uniffi/src/bindings.rs`. These cover the shared transfer/store/custody owner, exact receipt and output projections, HF reopen reconciliation, native shared-service consumer and attempt withdrawal, RPC lifetime ordering, the exhaustive UniFFI conversion for the new typed capacity error, and direct regressions. This slice gives llama.cpp the existing shared acquisition service, binds a durable receipt to exact extracted output and metadata, reconciles committed publication after reopen, and withdraws only an unchanged unreceipted native `Using` lease after its attempt is revoked and workspace cleanup succeeds. The `library.rs` descriptor-refresh future is boxed because the exact Torch serving RPC regression overflowed the default worker stack in the composed candidate; the existing integration test now passes with the normal stack size. Build #350 found that adding `PumasError::AcquisitionCapacityExhausted` left the UniFFI conversion match non-exhaustive. The admitted adjacent fix uses an existing FFI error shape and a co-located conversion regression; it adds no new public FFI enum variant. Temporary diagnostic logs/test edits were removed. This source and its local fixtures do not qualify live sources, desktop behavior, deployed schema-6 population/old-writer isolation, or cross-platform durability. Keep AQ-HTTP not ready until objective acceptance is complete.

The app-manager manifest is a hashed input to release attribution. The test-support dev-dependency required for the fresh-owner cancellation regression therefore also refreshes only `docs/release-attribution/0.7.0/inventory.json`; `THIRD-PARTY-NOTICES.txt` remains byte-for-byte unchanged because the dependency is the existing internal library and adds no third-party package.

### Admitted Q1 repair — llama.cpp progress and effect custody

Base: current `work/acquisition-q1-http` head `0ecccd9c0c223fac32e12ec571126cd7fa39c238`; parent PR #7 remains draft. A read-only GPT-6.1 Sol High composed-design review found three lifecycle gaps: a full progress channel can block cancellation/shutdown in the shared llama.cpp HTTP host and after publication; registered `AcquiredArtifactUse::run_blocking` work does not retain the held workspace execution lease if its waiter is cancelled; and `VersionManager::new_with_acquisition` reads retained acquisition records through a raw `spawn_blocking` outside consumer custody.

Exact write set, including co-located regressions:

- `rust/crates/pumas-app-manager/src/version_manager/installer.rs`: make the shared llama.cpp HTTP progress wait observe cancellation and shutdown, apply the same bounded behavior to post-publication setup progress, and add a regression that fills the live progress channel and proves both control signals release the composed operation.
- `rust/crates/pumas-core/src/acquisition/service.rs`: retain a clone of the held `AcquisitionWorkspace` through every registered `AcquiredArtifactUse::run_blocking` closure; add a cancellation regression proving the workspace execution lease remains held until the registered effect actually completes.
- `rust/crates/pumas-app-manager/src/version_manager/mod.rs`: perform retained-acquisition lookup through the existing consumer's registered `run_blocking` boundary; add focused coverage if a deterministic existing seam can observe the constructor lookup's cancellation/drain behavior without adding production-only state.
- `docs/plans/artifact-acquisition/reports/write-sets.md` and `docs/plans/artifact-acquisition/execution-ledger.md`: admission and resulting evidence only.

No new store, supervisor, progress owner, retry policy, dependency, generated contract, live retained-root access, unrelated Torch edit, acceptance-state change, merge, or published-history rewrite is admitted. Use GPT-6.1 Sol Medium for implementation because current Passeur status reports a missing `profile.open` file and agent discovery fails; do not start overlapping Rust builds. Planned evidence is the focused regressions, affected app-manager/core checks, formatting and staged-diff checks, with Cargo commands run one at a time on the shared target.

### Admitted Q1 milestone summary refresh

Base: `work/acquisition-q1-http` at `c5deb493172d3fbddab55141d129f2ac7346bb3b`, after the admitted shutdown/custody implementation. Exact write set:

- `docs/plans/artifact-acquisition/plan.md`: refresh only the current-phase summary to include the three composed-path shutdown/custody fixes and the boundaries of their local evidence. Preserve the Q1 gate, AC matrix, and downstream milestone states.
- `docs/plans/artifact-acquisition/reports/write-sets.md` and `docs/plans/artifact-acquisition/execution-ledger.md`: this admission and its resulting evidence only.

No source edits, acceptance-state changes, gate reopening, or changes to the Q2/R1 admission order are included. This records the current Q1 milestone before updating its existing draft PR.

### Admitted Q1 app-manager test lint correction

Base: `work/acquisition-q1-http` at `367373e77520dde108bb26b15a6ad1c875e94b5a`. Build #360's Rust-quality job failed on an unchanged `clippy::type_complexity` warning in the native receipt cold-reopen test helper in `version_manager/mod.rs:2042`. Exact write set:

- `rust/crates/pumas-app-manager/src/version_manager/mod.rs`: replace only that helper's nested tuple return type with test-local named aliases or an equivalent named type. Preserve fixture behavior and assertions; make no production behavior change.
- `docs/plans/artifact-acquisition/reports/write-sets.md` and `docs/plans/artifact-acquisition/execution-ledger.md`: admission and resulting verification evidence only.

The repair is limited to enabling the existing strict all-target Rust-quality command. It does not change product acceptance, the Q1 gate, the milestone order, or the other workstreams.

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

### Admitted Q1 milestone PR-order clarification

Base: `work/acquisition-q1-http` at `2e0bfc5b770a28176edd143697b0ef448db62900`. The user requested that milestone changes and their required plan/evidence corrections be completed on each milestone branch before its PR is advanced, with PR integration following the documented dependency order. Exact write set:

- `docs/plans/artifact-acquisition/reports/write-sets.md`: this admission only.
- `docs/plans/artifact-acquisition/execution-ledger.md`: record the clarified PR sequence and correct the latest Q1 status summary.
- Existing PR #7 body: replace the inaccurate linear Q1 → Q2 → Q3 → Q4 description with the order in `reports/dependency-gates.md`, after pushing the branch documentation update.

No source changes, acceptance changes, gate changes, PR merge, or retargeting are admitted.

### Admitted Q1 follow-up — AC02 validator-bound warm resume

Base: `work/acquisition-q1-http` at `556d958f3baf0952da7d932a5910c286bebe0f3f`. The read-only AC02 review confirmed that a `.part` length currently authorizes ranged append without binding the prefix to the response that produced it, and that a full-size digestless `.part` can bypass HTTP. Exact write set:

- `rust/crates/pumas-core/src/acquisition/http.rs`: retain strong ETag and effective response resource facts; issue conditional range requests for warm continuation; validate status, returned range/total, validator, identity encoding and actual byte count before append. `304`/`416` remain refusal outcomes. A full `200` response replaces from byte zero and is never appended to an old prefix.
- `rust/crates/pumas-core/src/acquisition/service.rs`: maintain a private in-memory per-file checkpoint across pause/retry under the same live service/workspace owner. Bind it to the exact acquisition, demand, manifest/file, workspace capability, effective response resource, strong ETag and verified prefix length/hash. Do not reconstruct it after owner loss. Invalidate it on publication, cancellation cleanup or a conflicting binding. Remove physical length as resume authority; a complete-size partial may bypass HTTP only when its selected expected SHA-256 is verified.
- `rust/crates/pumas-core/src/acquisition/workspace.rs`: verify the actual regular-file prefix under the held workspace capability and retain the same checked descriptor for append, avoiding a verify-then-reopen race.
- Co-located tests in those modules and `rust/crates/pumas-core/src/model_library/hf/download.rs` where its pause/resume fixture currently changes source origin. Assert exact final bytes or refusal for changed/missing validators, changed resource, mutated prefix, cold owner loss, digestless full-size partial, ignored Range/200, and existing 304/416 and body-bound cases.
- `docs/contracts/artifact-acquisition.md` §§5 and 9, this report, `docs/plans/artifact-acquisition/execution-ledger.md`, and `docs/plans/artifact-acquisition/reports/coding-standards-mcp-usability.md`: document warm-only checkpoint custody and the safe byte-zero restart after cold owner loss; record exact results and residual limits; preserve MCP usability findings separately from product acceptance. Do not change AC02's acceptance status in this slice.

No store/schema, manifest/receipt version, durable sidecar, source-adapter API, or unrelated consumer change is admitted. Strong ETags are the only partial-continuation validator in this slice; absent or weak validators require a fresh transfer. A whole-file digest remains a final publication check and does not authorize cross-resource or cross-origin splicing. Keep AQ-HTTP not ready until the full AC01–02 acceptance evidence is met. One Rust build at a time with `CARGO_BUILD_JOBS=1`; implementation and review worktrees must not start concurrent Cargo builds.

### AC02 candidate handoff within the admitted write set

The isolated implementation candidate is commit `0f287f81e2ec9ca8c01ee4399d493101f36b70c1` (tree `d378d12509255b92d30bc59a09aa4c03c31b4169`) at base `b0f78c1d1cdac8e1e6142ace330364611a97faac`. It changes only the four admitted Rust paths and contract §§5/9 plus the admission/evidence records. The source contains the in-memory checkpoint and checked-descriptor continuation, `Range`/`If-Match` admission, safe owner-loss restart and expected-digest-only complete-partial shortcut. HF co-located fixtures preserve their producer/consumer assertions while reflecting reconstructed workspace grants and same-origin restart. No source-adapter API, schema, receipt version or durable sidecar is introduced. Root's serial verification passed the red-to-green cold-reopen regression, all 102 acquisition tests, two focused HF retry/import fixtures, both public acquisition integration tests, the loopback llama.cpp install/reopen integration test, and strict all-target library Clippy; exact commands and evidence limits are in the execution ledger. The Q1 milestone branch was fast-forwarded to the candidate commit, preserving its original identity and review provenance. This remains a verified implementation candidate, not AC02 acceptance.

### Admitted Q1 follow-up — immutable revision at the existing HF download entrypoint

Base: `work/acquisition-q1-http` at `4756a40e`. PR #7 remains draft. Source tracing confirmed that the intent-acquisition path resolves a Hugging Face selector to a commit before selecting metadata and files, while the current desktop `start_hf_download` entrypoint still passes `DownloadRevision::legacy_main()` into that planning flow. Exact write set:

- `rust/crates/pumas-core/src/api/hf.rs`: on a fresh existing `start_hf_download` operation, resolve the default `main` selector to a validated immutable commit before model metadata, repository tree or file selection. Keep resolution inside the download invocation lifecycle so a closed API refuses before network access; pass the resolved revision into the existing revision-bound planning and transfer APIs.
- The co-located `api/hf.rs` tests: use the production API entrypoint with a controlled Hugging Face server. Assert that `/revision/main` establishes one commit and subsequent snapshot, tree and payload requests use that commit; malformed or missing resolution evidence must stop before destination mutation or download admission. Cancel the stalled payload in the test so no model import depends on fixture bytes.
- `docs/plans/artifact-acquisition/execution-ledger.md`, this report and `docs/plans/artifact-acquisition/reports/coding-standards-mcp-usability.md`: record this admission, exact source/test results, and separate MCP usability from product acceptance.

No `DownloadRequest`/RPC/generated/renderer contract change, arbitrary branch/tag selector, persisted schema, store, or migration is admitted. The existing UI request continues to mean default `main`, but each fresh operation becomes commit-pinned before selection and persists/uses that commit through existing APIs. AC01 and AQ-HTTP remain pending; this entrypoint repair alone does not close AC01's manifest completeness, collision or broader identity claims. Preserve the existing closed-lifecycle behavior and use the shared external Rust target with one Cargo build at a time.

### AC01 composed final/staging namespace collision repair — admitted 2026-10-01

Parent milestone: `work/acquisition-q1-http`, currently tracked by draft PR #7 to `main`. This is a bounded source-integrity correction on that milestone branch; it must not be treated as a separate accepted milestone or as AQ-HTTP completion.

**Exact write set:**

- `rust/crates/pumas-core/src/acquisition/manifest.rs` — validate the selected final paths and the `.part` staging sibling derived for each path as one namespace. Reject exact aliases and file/directory prefix conflicts under the existing portability comparison, for either manifest entry order. Keep the existing error family and preserve opaque source keys.
- `rust/crates/pumas-core/src/acquisition/workspace.rs` — provide/use one private staging-path mapping consistently for inspection, opening, verification, removal and publication so validation and mutation cannot derive different names.
- `rust/crates/pumas-core/tests/artifact_acquisition.rs` — public constructor, persisted schema-7 reader and valid-acquisition regressions; add no public API or persisted format.
- `docs/contracts/artifact-acquisition.md` §3 and `docs/plans/artifact-acquisition/{plan.md,execution-ledger.md,issues.md,reports/write-sets.md,reports/architecture-review.md}` — record the invariant, admission, bounded findings and actual evidence. Keep acceptance/gate state unchanged.

Required regressions: direct `weights`/`weights.part` collision; nested `weights`/`weights.part/config` conflict; case-varied and reverse-order forms; schema-7 persisted input refused through the production reader without rewriting stored bytes; a non-conflicting `.part` filename, opaque source key and ordinary multi-file receipt remain valid. Independent source review must confirm all workspace staging operations use the shared mapping. Existing target-platform normalization and Unicode collision qualification remain open; this slice does not close them.

No schema change, public DTO, new error authority, stage-layout change, unrelated HF progress repair, or acceptance-matrix/gate transition is admitted. Run Rust checks serially with `CARGO_BUILD_JOBS=1` and the shared target. Do not update or advance PR #7 until this slice and its milestone-branch verification are committed and the remaining required Q1 changes are accounted for.

### AC01/AC08 mixed-known-size HF progress denominator repair — admitted 2026-10-01

Parent milestone remains `work/acquisition-q1-http` / draft PR #7 to `main`. This is the next Q1 branch correction after commit `d5a98e8c` closed the output/staging path collision. It does not authorize starting Q2, runtime R1, or another milestone.

**Exact write set:**

- `rust/crates/pumas-core/src/model_library/hf/download.rs` — fresh and retained-recovery admission total calculation plus co-located regressions. Only expose an aggregate total when every selected file has a known size and a checked sum; keep zero total as unknown. Do not change the RPC progress validator or public request/persistence schemas.
- `docs/contracts/artifact-acquisition.md` §10 and `docs/plans/artifact-acquisition/{plan.md,execution-ledger.md,issues.md,reports/write-sets.md,reports/architecture-review.md}` — clarify the denominator invariant and distinguish the source-derived defect from the unproven incident cause.

Required regression evidence: exercise mixed known/unknown file sizes through fresh and recovery admission; report actual transferred bytes beyond the known-size subtotal; verify producer progress does not exceed the valid range and serializes; preserve all-known, all-zero and checked-overflow behavior. Do not weaken the receiving RPC range validation. A representative delayed public status poll remains external AC08 evidence; a local producer fix cannot prove that the prior `-32603` had this cause.

No RPC handler/validator change, UI change, request/schema change, acceptance-matrix update, or AC01/AC08 acceptance is admitted. Use one Cargo build at a time with `CARGO_BUILD_JOBS=1` and the existing shared target; code/review agents must not run Cargo or rustc.

### AC01 desktop revision candidate within the admitted write set

The existing desktop entrypoint now resolves default `main` to a validated commit before all revision-sensitive selection and passes it through the existing pinned flow. The local-server public-entrypoint tests prove that metadata, tree, and payload use that commit; missing commit evidence returns before download admission or destination mutation. The stalled payload is cancelled and no incomplete file is published. The focused API tests (2), existing closed-lifecycle regression (1), strict all-target pumas-library Clippy, rustfmt, and diff check pass with serialized Cargo and a shared target. This remains a narrow verified implementation candidate: broader AC01 manifest/source identity, incomplete-set, and collision acceptance evidence remains open, and no acceptance or dependency-gate state changes.

### Admitted Q1 follow-up — bound optional warm-checkpoint retention

Base: `work/acquisition-q1-http` at `5c046477a55ec8117e8ff4f381c0c1ca2b00f62e`. The AC10 source review found that the private warm-checkpoint `BTreeMap` has no entry or per-entry metadata limit and retains a full cloned `AcquisitionRecord` per paused file. Dead workspace owners are not pruned on ordinary lookup/insertion. Exact write set:

- `rust/crates/pumas-core/src/acquisition/service.rs`: use the shared supervisor's validated `AcquisitionCapacity.workers` value as the per-service entry limit across consumers and `with_store` views. The map remains one private shared map and reads the limit from that same supervisor, avoiding a duplicate capacity authority. Do not add a public capacity field or consume active worker permits for paused evidence. Set a private 64 KiB encoded-metadata ceiling per candidate. Measure the borrowed record and source/validator strings with a capped counting writer before cloning; never allocate an encoded copy for this check. Prune dead workspace owners on lookup and insertion. Count one entry per `(acquisition_id, file_index)`, allow same-key replacement at capacity, and retain distinct file indices independently. If the entry limit is full or candidate metadata exceeds the ceiling, skip retention without evicting live entries, deleting partials, changing demand, or changing the original pause/cancel/transfer result. With no retained checkpoint, a later incomplete attempt must follow the existing byte-zero restart path. Successful global shutdown continues to clear the map; failed shutdown keeps its truthful incomplete outcome.
- `rust/crates/pumas-core/src/acquisition/workspace.rs`: add a non-owning weak-owner liveness query so pruning never upgrades and drops the last workspace reference while the checkpoint mutex is held. Continuation still requires all existing exact-record, owner, source, validator, prefix, and checked-descriptor proofs.
- `rust/crates/pumas-core/src/acquisition/task_custody.rs`: document that `workers` also bounds optional per-service warm-checkpoint entries, while paused checkpoints do not hold worker permits.
- Co-located acquisition tests in `service.rs` and `workspace.rs`: cover entry saturation and byte-zero retry, dead-owner reclamation without file deletion, same-key replacement, per-file accounting, oversize refusal before clone, shared limits through service views, and concurrent insertion remaining at capacity. Retain current cancellation, retry-exhaustion, unknown-publication, shutdown, validator, and prefix-integrity outcomes.
- `docs/contracts/artifact-acquisition.md` §5: specify finite optional retention and byte-zero fallback under capacity pressure. `docs/plans/artifact-acquisition/issues.md`: expand AQ-I16 with the concrete uncapped-clone finding and this fix disposition. `docs/plans/artifact-acquisition/execution-ledger.md`, this report, and `docs/plans/artifact-acquisition/reports/coding-standards-mcp-usability.md`: record admission, exact results, limits, and MCP behavior.

No persisted format, receipt, schema, HTTP protocol, source adapter, public API, consumer, or acceptance-state change is admitted. This slice bounds checkpoint count and encoded metadata only; it does not measure or qualify peak RSS, queue/stream buffers, hash memory, file descriptors, disk writes, or network traffic. AC10 and AQ-HTTP remain pending.

### AC10 checkpoint-retention candidate within the admitted write set

Implementation is confined to the admitted service, workspace, and task-custody paths. The shared supervisor supplies the worker-derived entry limit, the candidate is size-counted before cloning through a bounded serializer, expired workspace owners are pruned without upgrading, and capacity/metadata pressure preserves the existing transfer outcome and durable custody. The focused checkpoint suite passed (21 tests), the complete acquisition unit-test group passed (110 tests), strict all-target `pumas-library` Clippy passed, direct rustfmt and `git diff --check` passed. These local results establish this bounded runtime component only; no representative peak-RAM, disk, stream, or network measurement was made, so AC10 remains pending.

### Q1 orphan-partial recovery fixture correction — admitted 2026-10-01

Parent milestone remains `work/acquisition-q1-http` / draft PR #7. Exact write set:

- `rust/crates/pumas-core/src/api/hf.rs`: change only `api::hf::tests::ticket_recovery_admits_exact_partial_and_public_cancel_preserves_other_artifacts`. The seven-byte `.part` written directly by `indexed_partial_ticket` has no live same-service/workspace checkpoint. Assert that the request has neither `Range` nor `If-Match`, and serve a stalled full `200 OK` response with `Content-Length: 12`. Preserve the assertions that public cancellation closes the response, removes the selected partial/final outputs, and preserves `unrelated.bin`.
- `docs/plans/artifact-acquisition/reports/write-sets.md` and `docs/plans/artifact-acquisition/execution-ledger.md`: record this exact admission, the prior default/no-default CI failures, the fixture diagnosis, and verification.
- Serial-integrator status synchronization: update `docs/plans/artifact-acquisition/plan.md`, `docs/plans/artifact-acquisition/reports/acceptance-matrix.md`, and `docs/plans/artifact-acquisition/reports/dependency-gates.md` to distinguish historical AC15 evidence from current-branch pending CI. This changes no acceptance or gate state.

The production checkpoint, prefix-proof, strong-ETag and exact-resource-binding rules remain authoritative. This fixture correction admits no production source change, acceptance or gate transition. The source writer must not run Cargo, rustc, builds or tests, or commit, push or merge; root's serial focused default/no-default results are recorded in the execution ledger. `rustfmt --edition 2021 --check rust/crates/pumas-core/src/api/hf.rs` and `git diff --check` pass. Passeur submission failed with `PATH_NOT_FOUND`, so the authorized Sol Medium fallback owns this bounded edit.

### AC01 canonical Unicode path-collision repair — admitted 2026-10-01

Base: `work/acquisition-q1-http` at `2b71e8745a158dac8c0b21bf9c7ccabd4e943255`. Parent draft PR #7 remains the Q1 milestone review path. Exact write set:

- `rust/crates/pumas-core/src/acquisition/manifest.rs`: compare each logical-path component after NFC normalization, lowercase mapping, and NFC normalization again. Apply the existing collision check to final paths, `.part` staging siblings and file/directory prefixes. Preserve original path and source-key values; do not reject unrelated non-ASCII paths.
- `rust/crates/pumas-core/Cargo.toml` and `rust/Cargo.lock`: add direct `unicode-normalization` dependency only for this comparison and record its resolved dependency graph.
- Co-located manifest regressions: reject NFC/NFD aliases across final, staging and prefix cases; accept distinct Unicode names; assert existing typed `CollidingLogicalPath` refusal.
- `docs/contracts/artifact-acquisition.md` §4: specify the portable NFC/lowercase/NFC collision key while preserving original names and keeping target-filesystem identity claims separate.
- `docs/plans/artifact-acquisition/issues.md` (AQ-I21), `plan.md`, `reports/acceptance-matrix.md`, this report, `execution-ledger.md`, and the Coding-Standards MCP usability report: identify the candidate, exact commits and focused result; leave AC01/AQ-HTTP pending.

The manifest module passed 10/10, including the exact NFC/NFD collision test and distinct-name acceptance: `cargo test --offline --jobs 1 --manifest-path rust/Cargo.toml -p pumas-library --lib acquisition::manifest::tests:: -- --test-threads=1`. The new dependency also passed `cargo check --offline --jobs 1 --manifest-path rust/Cargo.toml -p pumas-library --no-default-features`. The initial default-feature compile took 4m08s. The source candidate is `aeb1c304456e9c50d195538d2a45652108600ddb`, integrated as `83d6adc178ead52babfdf3283bf8009e5b2b6c79`. The local NFC/NFD fixture does not establish the mounted filesystem's identity rules on macOS or another normalizing target. AC01 and AQ-HTTP remain pending. Do not run concurrent Cargo/rustc builds or infer hosted CI acceptance.

### AC02 duplicate `Content-Encoding` refusal — admitted 2026-10-01

Base: `work/acquisition-q1-http` at `2b71e8745a158dac8c0b21bf9c7ccabd4e943255`. Parent draft PR #7 remains the Q1 milestone review path. Exact write set:

- `rust/crates/pumas-core/src/acquisition/http.rs`: inspect all `Content-Encoding` values and reject more than one with the existing typed `artifact.http.response` validation before returning an admitted body. Preserve no-header and single-identity acceptance, plus single compressed-encoding refusal.
- Co-located controlled-response regressions: cover identity/gzip and gzip/identity duplicates, repeated identity, and valid absent/single identity responses with body-byte assertions.
- `docs/contracts/artifact-acquisition.md` §5: require refusal of duplicate `Content-Encoding` fields before body admission and permit only absent/single `identity` values in this slice.
- `docs/plans/artifact-acquisition/issues.md` (AQ-I22), `plan.md`, `reports/acceptance-matrix.md`, this report, `execution-ledger.md`, and the Coding-Standards MCP usability report: identify the candidate, exact commits and focused result; leave AC02/AQ-HTTP pending.

The HTTP module passed 20/20, including duplicate-field refusal in both orders, repeated identity, absent/single-identity admission, existing compressed-encoding refusal and the surrounding range/representation tests: `cargo test --offline --jobs 1 --manifest-path rust/Cargo.toml -p pumas-library --lib acquisition::http::tests:: -- --test-threads=1`. The source candidate is `779a91b0a0e062e0bcc09bf764e6f5338ca4c96b`, integrated as `0f272db144914aa5c182a3334d8a7e57ab09c438`. This source fix does not close the full AC02 protocol matrix or exact-head hosted CI. Do not overlap Rust builds.

### AC02 duplicate `Content-Range` refusal — admitted 2026-10-01

Base: `work/acquisition-q1-http` at `412409866caca4aedfd219addd869b4c12af6fc3`. Parent draft PR #7 remains the Q1 milestone review path. Exact write set:

- `rust/crates/pumas-core/src/acquisition/http.rs`: require exactly one `Content-Range` header for a resumed `206` before returning a body-bearing response. Reject duplicate fields with the existing typed HTTP response-validation outcome.
- Co-located `http.rs` tests: use valid selected-file, strong-ETag, resource and offset evidence; serve conflicting `Content-Range` values in both orders; assert `PumasError::Validation` and await the fixture server. Preserve valid single-header range behavior and other existing refusal cases.
- `docs/contracts/artifact-acquisition.md` §5: specify one unambiguous `Content-Range` field for a resumed partial response.
- `docs/plans/artifact-acquisition/issues.md`: track AQ-I19 and its deciding regression.
- `docs/plans/artifact-acquisition/reports/write-sets.md` and `docs/plans/artifact-acquisition/execution-ledger.md`: record this exact scope, final route/review, and serial verification results.
- `docs/plans/artifact-acquisition/reports/coding-standards-mcp-usability.md`: record tool usability separately from product evidence.

No schema, manifest, public API, downstream consumer, or acceptance/gate change is admitted. This bounded parser/test does not close the full AC02 matrix or AQ-HTTP. Passeur contributor discovery and coordinated submission most recently failed with `PATH_NOT_FOUND`, so GPT-6.1 Sol Medium is authorized for implementation; the worker must not run Cargo, rustc, tests or builds. Root owns serial verification with `CARGO_BUILD_JOBS=1`; do not start a build until existing local Rust processes have stopped and do not overlap Rust compiles.

### AC04 strict duplicate-member reads for canonical downloads.json — admitted 2026-10-01

Parent milestone: `work/acquisition-q1-http`, carried by draft PR #7 into `work/artifact-acquisition-runtime-plan`. This is a partial AC04 source-integrity correction and does not satisfy retained schema-6 migration, old-writer exclusion, deployment, rollback, or full AC04 evidence.

**Exact write set:**

- `rust/crates/pumas-core/src/metadata/atomic.rs`: add a strict reader scoped to the canonical `downloads.json` target. Deserialize through Serde's JSON parser with a recursive visitor that compares decoded object member names before constructing each `Value`; preserve JSON syntax/trailing-data errors and map only the repeated-member marker to `downloads.duplicate_member`. Keep generic `atomic_read_json` behavior unchanged.
- `rust/crates/pumas-core/src/acquisition/store.rs`: route schema eligibility, migration, transaction, acquisition, model-partition, custody-partition and receipt reads through the strict canonical reader.
- `rust/crates/pumas-core/src/model_library/download_store.rs`: route model inventory/projection and HF receipt reads through the strict reader and add no-rewrite refusal coverage.
- Co-located tests: cover repeated top-level, receipt-key and nested receipt members; decoded escaped-key aliases; unrelated permissive JSON readers; syntax/trailing errors; and production model/receipt reads refusing duplicates without rewriting the stored bytes.
- `docs/contracts/artifact-acquisition.md` §9, `docs/plans/artifact-acquisition/issues.md` (AQ-I20), this report, the acceptance matrix, execution ledger, and Coding-Standards MCP usability report: record the exact semantic boundary and evidence while leaving acceptance/gates unchanged.

No persisted schema, public API, generic JSON policy, migration behavior, or acceptance/gate transition is admitted. The strict read closes one ambiguity before `Value` collapses duplicate members; AC04 and AQ-HTTP remain pending. Root owns serial Rust verification on the shared target. Do not overlap Cargo/rustc builds.
