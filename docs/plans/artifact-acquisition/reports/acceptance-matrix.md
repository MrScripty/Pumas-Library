# Acquisition acceptance matrix

**Acceptance status:** AC03 is accepted for the recorded Linux x86_64 source-built RPC consumer scope; AC15 is accepted for candidate `7963211bc5989f6b08712792bf1542a2314cb7fb` by Build #357 after the targeted Windows retry. Build #358 passed on the earlier documentation-only PR head. These historical results do not establish AC15 for the current Q1 branch: prior remote head `96a1cbe8999576ca8dc70143201e6e98b0c6a37d` failed the orphan-partial recovery fixture in both default and no-default CI. Its corrected local fixture passes the focused test in both configurations (1/1 each), while full exact-head CI is pending. AC01–AC02, AC04–AC14, and AC16–AC18 remain pending. The real-source and current-head evidence is detailed below and in the [execution ledger](../execution-ledger.md). The HF manifest used the mutable `main` ref (weak revision evidence), while the selected model weights were verified against the publisher's LFS SHA-256. Desktop, retained-deployment, public-interface, resource-bound, and supported-platform evidence has not been collected. AC03 and AC15 do not make AQ-HTTP ready. A required unavailable environment changes its claim to blocked, not satisfied. All claim owners are roles for the assigned integrator to resolve.

| ID | Observable claim / deciding procedure | Evidence kind | Environment | Mode | Milestone / owner | Status |
| --- | --- | --- | --- | --- | --- | --- |
| AC01 | **Immutable manifest and file identity.** Reject malformed versions, duplicate/colliding logical paths, contradictory evidence and incomplete sets. Preserve source key bytes, same-revision selection, zero-byte files and unknown sizes without fake percentages. | focused + contract | not-applicable | automated | Q1 / Acquisition/core | pending |
| AC02 | **Conditional HTTP and byte verification.** Controlled server exercises correct 200/206, ignored Range, bad Content-Range, 304/416, changed validator, truncated/extra body and compressed representation. Assert exact final bytes or refusal, never mixed-source/revision publication. | contract + integration | simulated HTTP server; real streaming/files | automated | Q1 / Acquisition | pending |
| AC03 | **Two real existing consumers.** Use actual HF resolution/download/import and current native archive installer through shared acquisition. Assert importer settlement and native consumer publication separately; no model row for executable archives. | system + integration | required-real HF and approved llama.cpp archive; Linux x86_64 source-built Pumas RPC backend | either | Q1 / Acquisition + model/native consumer | accepted for recorded scope |
| AC04 | **Retained recovery and migration.** Exercise supported current/legacy store fixtures, hidden admissions, revocation, uncertain publication and Pending cleanup refusal. Interrupt and reopen through production readers; preserve authored/model state and require old-writer retirement/isolation before actual mutation. | contract + system | representative retained-state replicas and real filesystem; actual deployment facts for rollout | either | Q1 / Core/recovery | pending |
| AC05 | **Consumer commit and handoff crash windows.** Interrupt before/after FilesReady, domain commit and settlement. Reopen, reacquire authority, preserve exact generation and demand, avoid repeated side effects, retain uncertain inputs, and never remove adopted output. Inject importer and extraction failures. | integration + system | representative filesystem/process/store adapters | automated | Q1 / Acquisition + consumers | pending |
| AC06 | **Cancellation, pause, cleanup and use custody.** Cancel queued/active/verifying/consuming work; verify no writes after Paused, stale generation cannot act, blocking/child work retains files through cleanup, release is idempotent and eviction preserves all live/durable demands. Coalescing is not required. | integration + system | controlled real files/workers/children | automated | Q1 / Acquisition | pending |
| AC07 | **Authorization and provenance.** Test unauthorized sources/cache use, local-service access, redirect credential scope, expired locators, identity-preserving refresh, unsafe paths, wrong digest and logs containing seeded secrets. Explicit private endpoint works without authorizing arbitrary internal hosts. | contract + integration | controlled HTTP/credential/filesystem boundaries | automated | Q1 / Security/acquisition | pending |
| AC08 | **Progress and actual user control.** Existing desktop HF/native controls start, pause/resume/cancel as supported; progress separates bytes and finalization, recovers missed events and stale requests, and never shows installed/model-ready at byte completion. | user-workflow + contract | representative Electron/backend plus controlled delayed sources | either | Q1 / Desktop + consumers | pending |
| AC09 | **Bootstrap and owner independence.** Construct neutral acquisition without Python/Torch/adapter imports and without opening a model DB. Fetch native input, then execute owner shutdown twice; observe complete or truthful incomplete drainage, not detached work. | integration + system | representative no-inference/empty-root backend | automated | Q1 / Core/composition | pending |
| AC10 | **Resource and file-representation behavior.** Verify configured queue/stream/buffer/hash admission bounds and overload behavior under concurrent transfers; measure peak RAM, disk/writes and network bytes on representative large artifact. Ordinary-file same-filesystem path does not unconditionally retain a second full model copy; copying fallback is explicit. | integration + system | representative filesystem and workload; simulated saturation | automated | Q1 / Acquisition | pending |
| AC11 | **Exact local package installation.** Existing Torch integration acquires a retained exact wheel closure, installs with network denied, rejects alternate same-name/version bytes and absent dependencies, verifies installed distributions/interpreter/build, and preserves source provenance. | contract + system | required-real supported Python/tooling and approved Torch wheel closure | automated | Q2 / Package/runtime | pending |
| AC12 | **Package and bootstrap boundary accounting.** Record resolver metadata/candidate and managed-Python provider traffic separately. Prove final payload installation has no hidden remote URL or re-resolution; input leases last through package child cleanup. No fake claim that every resolver byte used shared transfer. | integration + system | representative real package subprocess with egress observation | automated | Q2 / Package/runtime | pending |
| AC13 | **S3 semantics through shared owner.** Test versioned object/range reads, multipart-style ETags, credential rotation, changed unversioned objects, prefix pagination interruption, logical-path mapping, addressing modes and optional SDK retry budgets through the same store/lifecycle. | contract + integration | controlled S3-compatible endpoint with fault injection | automated | Q3 / S3/acquisition | pending |
| AC14 | **Real object-store and user workflow.** Fetch a pinned multi-file manifest on AWS S3, one non-AWS S3 service and local MinIO; verify normal files and producer/consumer results. Import an S3-sourced model through the supported model workflow, then observe its published library record through GetModel or EnsureModel. Use native source configuration/inspection API and representative desktop flow without separate provider-specific downloader. | system + user-workflow | required-real authorized AWS, non-AWS endpoint, MinIO and representative desktop | either | Q3 / S3 + desktop | pending |
| AC15 | **Existing static and feature contracts.** Run affected core/app-manager/RPC tests, Rust fmt/check/clippy and no-default variants, Torch and TS gates when changed. Do not promote empty current core feature flags into a new minimal-build claim. | supporting static + integration | representative pinned toolchains | automated | Q1/owning slice / Integrator | accepted on historical candidate `7963211b`; current Q1 branch requalification pending |
| AC16 | **Producer/consumer and error contract.** Canonical DTOs regenerate from owner, RPC/IPC/preload/renderer reject wrong version/type/path/status, preserve errors and progress meaning, and support named current public clients or explicit versioned cutover. | contract + integration | representative Rust/Node and actual affected interfaces | automated | Q1/owning slice / Contract/desktop | pending |
| AC17 | **Installed/native/deployment qualification.** Outside checkout, use actual packaged backend/UI to acquire model/native and S3 inputs; execute affected filesystem/cancellation/reopen mechanisms on each declared supported OS. Close independent client and old-writer dispositions; retain per-target proof and known exclusions. | release-artifact + system + user-workflow | required-real packaged/native targets and deployment facts | either | Q4 / Distribution/integrator | pending |
| AC18 | **Composed simplicity and single ownership.** Trace new source, native build, adapter artifact and recovery repair against locality matrix. Verify one authoritative transfer writer per migrated attempt, distinct consumer commits, deletion of selected duplicate byte loops and no downstream-to-acquisition dependency. | architecture review (separate from test success) | not-applicable | manual | Q1/each material composition / Independent reviewer | pending |

### AC03 real-source consumer evidence — 2026-10-01

The exact code candidate `7963211bc5989f6b08712792bf1542a2314cb7fb` was built as the production `pumas-rpc` binary with `inference-plugins` and run on Linux Mint 22.2 (Ubuntu noble base), x86_64, against a fresh isolated root at `/tmp/pumas-q1-live.jeAfvb`. `XDG_CONFIG_HOME` and the Pumas registry DB were isolated under that root, `HF_TOKEN` was unset, and the user's saved Pumas HF token was not read. The real `search_hf_models` and `start_model_download_from_hf` RPC calls reached Hugging Face; `get_available_versions` and `install_version` resolved and installed an official GitHub release. Health, model-list and version-status calls observed the local consumer state. The direct RPC calls exercised the backend operations used by the current UI. The renderer and packaged backend were outside this run; those remain for AC08 and AC17.

The real HF search resolved `HuggingFaceTB/SmolLM2-135M-Instruct`; the current request selected `model.safetensors` (269,060,552 bytes) and the importer also fetched its seven required auxiliary files. Acquisition record `2ee620c1-d723-4415-855b-13e04d76efa9` and its schema-7 `hf.model` completion receipt bind the actual verified output. The manifest records repo revision `main` with weak revision strength; the selected weight file is bound to Hugging Face LFS SHA-256 `5af571cbf074e6d21a03528d2330792e532ca608f24ac70a143f6b369968ab8c`, and the downloaded file's SHA-256 matched. `list_model_downloads` reported `completed`; `get_models` exposed exactly one model-library row, with `artifact=complete`, `integrity=clean`, and size 272,437,573 bytes. No executable archive appeared as a model. After a clean RPC restart on the same isolated root, the model still appeared once with the same complete/clean projection. The retained acquisition and consumer receipt are in `/tmp/pumas-q1-live.jeAfvb/launcher-data/downloads.json`; the transient download list was empty after restart, so persistent UI task history remains outside this evidence.

The actual native install selected release `b11312+vulkan` from the official llama.cpp catalog and installed asset ID `602322542`, `llama-b11312-bin-ubuntu-vulkan-x64.tar.gz` (31,480,843 bytes). Pumas resolved publisher SHA-256 `81b207361c95483e861763944cd371f916964a9d5626d649919690e6860e744a`; both the acquired archive and its durable `runtime.llama.cpp` receipt match that digest. The installer extracted and published `llama-server`; the release metadata and completion receipt bind the installed output tree and launcher. `get_version_status` reported exactly one installed, active version before and after restart, and startup validated one version. The host exposed an NVIDIA driver marker but had no `/dev/nvidia*`, `/dev/kfd`, or DRM render device node, so this run establishes archive acquisition, extraction and publication only.

One `get_model_download_status` poll near HF settlement returned JSON-RPC `-32603` (internal error). A subsequent `list_model_downloads` returned the same download as `completed`, and the model projection and durable receipt were successful. Preserve this as an AC08 progress/error-contract issue to investigate; it does not invalidate the separately observed importer settlement in AC03. The HF manifest used weak revision `main`; this run does not qualify commit-pinned revision selection or every file at a repository commit. Desktop controls, cancellation, pause/resume, packaged backend, other OS, deployed-state migration, and cross-platform durability remain outside this evidence.

### AC06 partial service evidence — 2026-10-01

The `pumas-library` acquisition service tests now drive the public
`AcquisitionConsumer::acquire_http` path with a real temporary workspace and a
local HTTP source, hold a blocking effect in that worker generation, and call
`AcquiredArtifactUse::withdraw_after_cleanup`. They prove that the file and
exact `Using` record remain while the effect is held, successful cleanup then
withdraws the record and clears its verified-file set, and cleanup failure
retains both the `Using` record and file while scope/global shutdown report the
failed effect. Two more public-consumer tests gate a local HTTP body after its
first four bytes, then pause or cancel the active transfer before releasing the
remaining source bytes. Cancellation wakes a stalled HTTP body and is reported
as cancellation, not pause. Once the operation returns, the fixture opens the
server gate to attempt the remaining bytes; the exact partial file still
contains only the first four bytes, no final file exists, and the entire
acquisition record is unchanged. The fixture does not establish successful
delivery of the gated remainder or crash durability. Commands and results are
recorded in the execution ledger.

A further public-consumer fixture pauses after the first four bytes, then calls
`acquire_http` again with the same demand, manifest and workspace. It verifies
that the second request sends `Range: bytes=4-`, accepts a valid `206` for bytes
4–7, and produces exactly `DATATAIL`. The original acquisition ID is retained,
the record settles as Adopted, and the returned receipt matches the persisted
receipt. This proves same-owner resume over the local HTTP fixture; it does not
prove cold reopen, power-loss durability, malformed-range handling or real
consumer publication.

This is partial service-level evidence. AC06 remains pending for queued and
verifying cancellation, idempotent release, and preservation of all live and
durable demands. The sequential stale-generation fixture below proves rejection
at `files_ready` from a new worker generation in one consumer scope. The
concurrent fixture following it proves that stale readiness cannot replace a
successful concurrent handoff; neither fixture exercises same-task-ID
replacement. These local HTTP/workspace fixtures are not real HF/native source
or supported-platform acceptance.

The `stale_worker_generation_cannot_seal_or_handoff_acquisition` service test
starts an operation in one worker generation, then invokes `files_ready` from a
fresh Worker generation in the same consumer scope. It asserts the exact
acquisition-custody rejection, unchanged Transferring record with no verified
files or receipt, unchanged final and partial bytes, and an unsealed workspace
that remains append-openable. Consumer and service shutdown drain before the
assertions. This covers the sequential generation mismatch at the acquisition
boundary; it does not establish concurrent replacement, filesystem metadata
equality, cold recovery, or crash durability.

The `concurrent_stale_worker_handoff_cannot_mutate_durable_acquisition`
service test gates two worker invocations in the same consumer scope on the
same demand and workspace. It lets the successor seal and hand off the durable
record, snapshots that winning record and staged bytes, then releases the
earlier worker to attempt `files_ready`. The stale handoff returns the exact
`Acquisition readiness is stale` validation error; the winner's row remains
equal, no consumer receipt is issued, and the final file, partial file, and
workspace entry count remain unchanged. Consumer and service shutdown drain
before assertions. The focused test passed 1/1 on Linux x86_64. This does not
establish same-task-ID replacement, crash or power-loss durability, queued or
verifying cancellation, idempotent release, preservation of all live and
durable demands, real-source/consumer behavior, or other-platform acceptance.


## Real procedures and proof boundaries

**Q1 model/native path.** Use disposable model and installation roots. Pin a small actual HF artifact/commit and one native release archive through current supported selection. Start through the real public model/native operations, inspect intermediate progress, interrupt/resume one transfer, and wait through model importer and installer finalization. Assert exact expected input bytes, model record only for the model, correct extracted executable role, and preserved unrelated state. Native startup/stop can use the existing path where available, but new installation binding remains runtime R1's claim. Repeat the real UI operation under recorded Electron conditions. Test setup does not need the new adapter registry or runtime installation schema.

**Q2 package path.** Produce a version-checked accepted resolution with the supported managed interpreter/tooling. Let shared acquisition fetch the approved wheel set. Close/deny network access to the installation process using an actual enforced mechanism and retain its evidence. Install from generated exact local inputs, verify package and native-build identity, then run the scoped CPU import/tensor/startup checks owned by the current installer. A successful package import is not model inference. Repeat with one corrupted/substituted wheel, one missing closure member and cancellation during package child work. Resolver network access before that leg is separately recorded, not counted as an offline failure or hidden.

**Q3 S3 path.** Use test-owned credentials and a fixture object set with recorded versions/digests. Validate a real AWS endpoint, a named independent S3-compatible endpoint, and local MinIO. Record actual supported features; an emulator proves its own behavior, not AWS or the other service. Fetch the same artifact description without changing consumer code. Through the supported model-facing operation, acquire one S3-backed model, await importer completion, and confirm its exact published library record through GetModel or EnsureModel; a byte-ready transfer alone does not pass. Exercise credential refresh, range resume and object-change refusal. The source settings and progress workflow must be observable through the actual API/UI. Failed listing pages cannot publish a complete package.

**Migration/restart.** Create disposable replicas of supported existing metadata and partial files. Inject interruption at each admission, byte-ready, consumer-commit, ownership-transfer and settlement boundary. Reopen through the production reader and source/consumer composition. Check content, state and effect authority rather than only error text. Prove old-writer retirement/isolation for the actual deployment before any real retained root is migrated. Explicitly keep unaccepted Pending replay unavailable.

**Native and resource evidence.** State selected OS/filesystem, features, artifact size, parallelism and budgets. Tests cover actual mechanisms on Windows/macOS when those promises change. A Linux process or cross-compile cannot close the native execution claim. Runtime gates may open for the qualified target scope, not for every platform. Performance observations establish resource behavior only for that workload; no unsupported throughput improvement is required or asserted.

## Planned test placement and supporting commands

The core public-consumer integration target `rust/crates/pumas-core/tests/artifact_acquisition.rs` exists and has passed 2/2 on a prior code candidate. The AC06 service regressions are unit tests in `rust/crates/pumas-core/src/acquisition/service.rs`; the focused service command passed 8/8 on the current local candidate, and affected-package Clippy passed with warnings denied. Build #355 passed code commit `43f106c7`; Build #356 passed the documentation-only head `59732c1f`. The new service test does not yet have exact-head hosted CI evidence. The planned standalone target `rust/crates/pumas-app-manager/tests/artifact_acquisition_install.rs` is not present; native install/recovery cases currently run in app-manager unit/fixture targets. These local and hosted checks do not close the real-source, desktop workflow, or deployment/platform acceptance claims above. Record each new command/result and broaden to affected aggregate checks:

```sh
cargo test --manifest-path rust/Cargo.toml -p pumas-library --test artifact_acquisition
cargo test --manifest-path rust/Cargo.toml -p pumas-library --lib acquisition::service::tests:: -- --test-threads=1
./scripts/rust/check.sh
npm run -w frontend lint
npm run -w frontend check:types
npm run -w frontend test:run
npm run -w electron test
npm run -w electron validate
python3 -m ruff check torch-server
python3 -m unittest discover -s torch-server/tests
```

Use repository-defined current exporter/freshness and feature/native commands after inspecting their actual definitions. Do not invent a passed CI result or copy a fixture snapshot as independent consumer proof. Package-network denial and real service tests need their named actual environment, not a monkeypatched network function alone.

## Evidence record

For each claim record candidate/material identity, source/fixture and authoritative oracle, command or operator procedure, environment, outcome, scope limits and evidence link. Keep failed/skipped/unavailable results separate. Link acceptance from the gate record only after the claimed observable result exists. Avoid adding an omnibus production verifier; focused tests and existing tools are preferred when they prove the obligation.
