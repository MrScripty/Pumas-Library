# Pumas Library: development takeover audit

Date: 2026-10-03 UTC. Status: source audit complete; baseline is not ready for S3 or remote exposure.

## Recommendation

Keep the existing architecture and the substantial acquisition work in PR #7. Repair its observed regressions and the existing RPC trust/lifecycle defects first. Implement S3 as a source adapter over the same acquisition, verification, and publication owners. Then add a separate, restricted node interface. Do not turn the existing administrative RPC server into a LAN server.

The latest requested priority supersedes the older default sequence of HTTP → package installer → S3 → generalized runtime management. Package and adapter generalization are not prerequisites for S3 or a restricted networked node. Stop at locally demonstrated networked Pumas; do not claim real-cluster operation, scale, failover, or production deployment qualification.

## Evidence baseline and coverage

- Main and `work/artifact-acquisition-runtime-plan`: `e37bbf4b964a0e2aadf25f80ab71edd8fa6b3eb3`.
- Existing draft [PR #7](https://github.com/MrScripty/Pumas-Library/pull/7): `work/acquisition-q1-http` at `3b2a279b4d35bd19e5cc1727cc905c7ee7477fa6`, 97 commits, 73 changed files, 39,282 insertions and 6,756 deletions. Its history and prior fixtures were preserved.
- Current [Coding-Standards](https://github.com/MrScripty/Coding-Standards/tree/dcc56f26e884ade260770beceba2501d3746200d): Core and Router, with implementation, verification, planning/proportionality, commit, library, Rust, and affected security/lifecycle guidance. This is an evidence-based audit, not a blanket compliance certificate.
- Read-only source tracing covered RPC admission/dispatch, desktop bridge shutdown, local IPC/intent ownership, acquisition/HTTP/store/workspace and importer boundaries, runtime installation and Torch loading, renderer download updates, CI and release variants. Plans and briefs were reconciled after the source investigation.
- Local probes used an isolated, disposable launcher root with the official v0.7.0 Linux release binary. No existing library, model, token, public listener, bucket, or runtime installation was changed. Those probes establish release behavior; current-source traces establish that the same admission/signal paths remain present. They are not a current-source build.

## Findings in priority order

### A9. High: native acquisition cleanup can delete a replacement workspace

PR7 addition. `acquisition/workspace.rs:148–171,203–208` checks the held descriptors against their own captured inode identities, but not the reserved root/child pathname binding. The native installer supplies a no-op root validator (`installer.rs:612–619`). Its `NativeInstallWorkspace` subsequently calls pathname-based `remove_dir_all` (`installer.rs:476–481`). If the reserved directory is renamed and another directory is created at its old path, acquisition still addresses the held original while cleanup deletes the replacement. This is a concrete mismatch between descriptor custody and pathname reclamation, not a claim that rename alone invalidates every capability.

Bind and revalidate the reserved parent/child identity, and reclaim only the proven owned directory. Add child- and root-replacement regressions with sentinel contents that must survive refusal. Keep HF's stronger destination-chain validator intact. This generic seam must be repaired before S3 uses it.

### A10. High: importer normalization can silently lose a selected input

Pre-existing on main. `model_library/importer.rs:1394–1406` normalizes each relative source filename and then copies with overwrite semantics. Distinct inputs such as `Model A.gguf` and `Model_A.gguf` can resolve to one destination, lose one file and retain duplicate metadata entries. Plan the complete source-to-destination mapping before effects and reject ambiguous collisions (including the supported filesystem's identity rules), rather than silently selecting a winner. Walking errors are also currently discarded by `filter_map(|e| e.ok())`; incomplete source enumeration must not authorize a successful import.

The same importer validates only the first sharded set (`importer.rs:1787–1820`, explicit `break`) and counts names instead of proving unique ordinal coverage. One complete set can conceal another incomplete set. Regressions must cover two sets, missing/duplicate ordinals and different source bytes that normalize to one name. Both defects should be repaired before claiming S3 multi-file import is reliable.

### A1. High: acquisition admission now performs upstream I/O before local refusal

PR7 regression. In `rust/crates/pumas-core/src/model_library/hf/download.rs:3740–3792`, `start_download_admitted` resolves the pinned repository file selection before `start_download_admitted_with_selection` checks mutation authority, destination-root configuration, persistence and durable intent-deletion claims. The earlier local refusal contract is therefore violated: an unconfigured or deletion-claimed request can contact Hugging Face and produce an unrelated network error.

Exact-head CI independently demonstrates this. Both default and no-default core suites fail `durable_intent_claim_refuses_admission_without_poisoning_shutdown` and `unconfigured_download_refuses_before_remote_or_destination_effects`; the former receives HTTP 401 instead of the expected local refusal. Preserve explicit-file completeness checks, but restore preflight before source resolution and recheck authority at publication. Do not weaken the existing regressions or make refused operations depend on a public network fixture.

### A2. High: local HTTP RPC accepts cross-origin simple requests

Pre-existing on main and v0.7.0. `server.rs` configures response CORS but no request-origin/Host admission. `handlers/mod.rs:427` accepts raw bytes without requiring a JSON media type. A synthetic `text/plain` POST with an untrusted Origin and Host reached the health handler and returned HTTP 200. The missing CORS response header prevents reading from some browser contexts; it does not prevent the operation from being dispatched. The same admission path reaches privileged local administrative methods.

Repair receiving-boundary checks before dispatch, with rejection-before-effects tests for hostile origins, rebinding-style Host values, malformed/duplicate headers and simple-request content types. Preserve existing non-browser local clients. Any remote interface needs a separate authentication, authorization and encrypted-transport contract; broadening the local listener is not a fix.

### A3. High: normal desktop termination bypasses Rust owner drainage

Pre-existing on main and v0.7.0. Electron `python-bridge.ts:570–624` sends a `shutdown` RPC and then SIGTERM. The RPC only stops managed runtime profiles (`handlers/mod.rs:1014–1040`), while Unix `main.rs` awaits only `ctrl_c()` before `server.shutdown()`. In the release probe, SIGTERM returned -15 without the graceful-shutdown message; SIGINT returned 0 through the cleanup path.

The fix must connect desktop shutdown and supported OS termination to the one composed owner. Cover active downloads/consumer effects, repeated requests, interrupted waiters and bounded desktop observation. Also review HTTP connection lifetime: the supervisor currently drops the `axum::serve` future rather than proving all accepted connections have drained. Do not equate listener closure with request completion.

### A4. High: model loading silently grants executable-code trust

Pre-existing on main. All three Torch text loaders (`safetensors_loader.py`, `sherry_loader.py`, `dllm_loader.py`) pass `trust_remote_code=True` to both tokenizer and model loading. The load request has no approved-code identity or trust decision. A model directory containing Transformers custom-code configuration can therefore select Python implementation code as part of ordinary loading. This is especially important before adding more sources and remote consumers.

Use a fail-closed trust contract. Ordinary tensor/model loading must not grant code execution. If custom-code support is retained, approval must identify its pinned code and source and be carried through the actual load boundary. Do not add an undocumented global environment toggle. Generic adapter infrastructure can remain deferred; unsupported custom-code models should receive a truthful outcome meanwhile.

### A5. Medium: resume failure overwrites a newer authoritative download snapshot

Pre-existing on main and retained in PR7. `frontend/src/hooks/useModelDownloads.ts:259–300` captures one download ID, but its rejection callback updates whichever object currently occupies the old map key. A pushed `downloading`/`completed` state, or another download reusing the key, can be replaced with `error` when the earlier resume call settles late. Pause and cancel already use invocation/object ownership; resume does not.

Apply the same current-invocation rule and associate errors by exact download identity. Add delayed-failure tests with a newer snapshot and replacement ID. This is a display-state defect, not evidence of underlying artifact corruption.

### A6. High integration blocker: current CI is red and stacked PRs are not automatically qualified

[Exact-head Build 37055028990](https://github.com/MrScripty/Pumas-Library/actions/runs/37055028990) was manually dispatched on PR7's current head. Its actual results are:

- Default core: 1,582 passed, 3 failed, 6 ignored.
- No-default core: 1,557 passed, 4 failed, 6 ignored.
- Workflow/release, frontend/desktop, and native Torch unit/fixture QA passed.
- All three native Torch RPC E2E jobs failed. Later release/package jobs were skipped.

Besides A1, ticket recovery fails at shutdown with `DownloadShutdownFailed { failures: 1 }`; its cause must be repaired and requalified. No-default additionally observes `Incomplete conversion task-state observation` in a router-child cleanup test; this is not yet established as the same regression and needs a focused process-lifecycle diagnosis.

The Build workflow automatically handles pushes only to main/tags and PRs only to main plus two old Torch stack bases. PR7's integration base is excluded. A PR-only Actions query misses the manual run, so inspect all event types for exact-head qualification. Add the actual stack bases, or an equivalent consistently enforced trigger, with a regression on the intended trigger contract.

### A7. Release blocker: selected Python distribution has missing declared license members

The native Torch E2E collector validates `PYTHON.json` against the selected full archive. Exact-head Linux logs show missing `LICENSE.zlib-ng.txt` and `LICENSE.zstd.txt`; the predecessor's Windows run lacks zstd, and macOS lacks both. This failure is outside the PR7 acquisition source diff and is not a Rust compilation failure.

Keep the attribution check strict. Resolve the pinned upstream archive's attribution through authoritative upstream evidence or a corrected, verified provider pin. Do not skip the members, fabricate notice contents, suppress the gate, or repeatedly rerun an unchanged input expecting success. CPU/GPU/model-serving acceptance remains separate from license collection.

### A8. Compatibility and qualification limits must remain explicit

PR7 advances the canonical download store to schema 7 and intentionally refuses old schema 4/5/6 writers until explicit offline conversion. The Rust migration API and fixture coverage exist, but a practical operator/CLI migration path, deployed retained-state population, old-writer exclusion and rollback qualification are not established. No live root should be silently migrated. Main and PR7 remain version 0.7.0 despite changed public constructors and persisted format; the eventual release needs an explicit compatible/breaking cutover decision.

Cold restart and ordinary HF pause/resume restart incomplete files at byte zero. Warm continuation is guarded by source/checkpoint evidence. This is a deliberate safety limitation, not full durable range-resume support. Keep it visible to users of large model transfers.

The current acquisition interface is a sound shared custody/verification owner but still HTTP-shaped (`AcquisitionHttpRequest`, `HttpAttemptHost`, `acquire_http`). S3 must adapt at the protocol/source boundary and reuse those lifecycle guarantees rather than introduce another downloader/store. Source selection must pin each object version or trusted content digest; multipart ETags are not cryptographic hashes and a prefix listing is not a coherent package snapshot.

## Additional verification and product gaps

- Local Node launcher/release tests: 63/63 passed on Node 24.19.0 (repo pin 24.15.0). This is focused supporting evidence, not the complete desktop suite.
- Local Torch unit discovery: 118 tests ran, 7 errors on Python 3.12.14, FastAPI 0.141.1/Starlette 1.6.0. Tests introspect `app.routes` as flat entries, whereas the installed supported-by-unbounded-requirements FastAPI exposes included routers. CI's optional FastAPI stubs do not establish real-ASGI compatibility. Treat this as a test/dependency reproducibility gap; it does not alone prove runtime routes are broken. Torch/model execution was not run.
- Full Rust builds were deliberately not attempted in the constrained shared environment; prior full-core builds exhausted memory and free disk at the local audit checkpoint was approximately 2.7 GB. Prepare focused regressions and use exact-head hosted qualification rather than claiming an unrun local pass.
- Published headless archives are intentionally inference-disabled. `RELEASING.md`, `artifact-plan.json`, the assembler and CI all agree. Source-built headless+plugins is supported, but there is no corresponding convenient prebuilt archive. Add a distinct inference-enabled headless cohort for consumers such as Lanternwake; preserve existing no-inference promises.
- Documentation drift is real: `SECURITY.md` still mentions removed `--allow-lan`, acquisition plan/issue/gate records cite several obsolete heads as current, and completed plans remain listed alongside multiple competing next slices. Consolidate current status and keep chronology in the ledger/history. Do not spend further implementation cycles re-certifying unchanged documentation fingerprints.

## Development order and acceptance boundaries

1. **Repair the baseline.** Main-based PR for HTTP admission and shutdown/trust fixes, with focused system/contract tests. Separate PR7-based repair PR for admission, ticket-recovery shutdown, and workspace reclamation. Repair import collisions/completeness before new multi-file source support. Preserve PR7 history. Resolve deterministic CI/release blockers and rerun exact-head gates before calling the baseline ready.
2. **Implement S3 next.** Continue the existing acquisition plan with S3 ahead of optional package generalization. Use a maintained S3 implementation, explicit endpoint/credential configuration, per-object immutable identity, scoped secret-safe access, the existing verified-file handoff, and a real model-import consumer. Deterministic fixtures and local S3-compatible integration can prove their named scope. AWS and non-AWS hosted claims remain blocked until authorized test resources exist.
3. **Make Pumas networked.** Introduce a version-negotiated, authenticated/encrypted, restricted node surface over the existing intent domain, with persistent identity and explicit peer configuration. Remote clients receive model/artifact references rather than ambient local paths or process-control authority. Reuse acquisition for any peer bytes. Prove two independent local processes, wrong credentials/protocols, interrupted transfers, restart and shutdown. Stop here; no fleet scheduler, gossip, cluster-scale qualification, or real-cluster reliability claim.
4. **Useful delivery follow-through.** Add the missing headless+inference packaging cohort and documented source/migration workflows as needed to consume the preceding milestones. Generic runtime installation IDs, arbitrary adapter registries, MCP, Xet/chunk CAS, coalescing and broad fleet management stay deferred unless a specific milestone proves them necessary.

Each branch remains draft until its required tests and applicable review gates pass. Integration is owned centrally; this audit does not authorize release publication, live migrations, credentials, bucket changes or real-cluster deployment.
