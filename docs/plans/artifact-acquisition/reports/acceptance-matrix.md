# Acquisition acceptance matrix

**Acceptance status: AC15 is accepted for code candidate `eadfb6bb`; AC01–AC14 and AC16–AC18 remain pending.** Build #351 passed on the exact code commit (see the [execution ledger](../execution-ledger.md)). The local Q1 candidate also has focused Rust tests and controlled loopback/filesystem evidence, but required real-source, desktop, retained-deployment, public-interface, and supported-platform evidence has not been collected. AC15 does not make AQ-HTTP ready. A required unavailable environment changes its claim to blocked, not satisfied. All claim owners are roles for the assigned integrator to resolve.

| ID | Observable claim / deciding procedure | Evidence kind | Environment | Mode | Milestone / owner | Status |
| --- | --- | --- | --- | --- | --- | --- |
| AC01 | **Immutable manifest and file identity.** Reject malformed versions, duplicate/colliding logical paths, contradictory evidence and incomplete sets. Preserve source key bytes, same-revision selection, zero-byte files and unknown sizes without fake percentages. | focused + contract | not-applicable | automated | Q1 / Acquisition/core | pending |
| AC02 | **Conditional HTTP and byte verification.** Controlled server exercises correct 200/206, ignored Range, bad Content-Range, 304/416, changed validator, truncated/extra body and compressed representation. Assert exact final bytes or refusal, never mixed-source/revision publication. | contract + integration | simulated HTTP server; real streaming/files | automated | Q1 / Acquisition | pending |
| AC03 | **Two real existing consumers.** Use actual HF resolution/download/import and current native archive installer through shared acquisition. Assert importer settlement and native consumer publication separately; no model row for executable archives. | system + integration | required-real HF and approved llama.cpp archive; supported host | either | Q1 / Acquisition + model/native consumer | pending |
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
| AC15 | **Existing static and feature contracts.** Run affected core/app-manager/RPC tests, Rust fmt/check/clippy and no-default variants, Torch and TS gates when changed. Do not promote empty current core feature flags into a new minimal-build claim. | supporting static + integration | representative pinned toolchains | automated | Q1/owning slice / Integrator | accepted on `eadfb6bb` by Build #351 |
| AC16 | **Producer/consumer and error contract.** Canonical DTOs regenerate from owner, RPC/IPC/preload/renderer reject wrong version/type/path/status, preserve errors and progress meaning, and support named current public clients or explicit versioned cutover. | contract + integration | representative Rust/Node and actual affected interfaces | automated | Q1/owning slice / Contract/desktop | pending |
| AC17 | **Installed/native/deployment qualification.** Outside checkout, use actual packaged backend/UI to acquire model/native and S3 inputs; execute affected filesystem/cancellation/reopen mechanisms on each declared supported OS. Close independent client and old-writer dispositions; retain per-target proof and known exclusions. | release-artifact + system + user-workflow | required-real packaged/native targets and deployment facts | either | Q4 / Distribution/integrator | pending |
| AC18 | **Composed simplicity and single ownership.** Trace new source, native build, adapter artifact and recovery repair against locality matrix. Verify one authoritative transfer writer per migrated attempt, distinct consumer commits, deletion of selected duplicate byte loops and no downstream-to-acquisition dependency. | architecture review (separate from test success) | not-applicable | manual | Q1/each material composition / Independent reviewer | pending |

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

This is partial service-level evidence. AC06 remains pending
for queued/verifying cancellation, resume-after-pause, stale-generation
rejection at the acquisition boundary, idempotent release, and preservation of
all live/durable demands. These local HTTP/workspace fixtures are not real
HF/native source or supported-platform acceptance.


## Real procedures and proof boundaries

**Q1 model/native path.** Use disposable model and installation roots. Pin a small actual HF artifact/commit and one native release archive through current supported selection. Start through the real public model/native operations, inspect intermediate progress, interrupt/resume one transfer, and wait through model importer and installer finalization. Assert exact expected input bytes, model record only for the model, correct extracted executable role, and preserved unrelated state. Native startup/stop can use the existing path where available, but new installation binding remains runtime R1's claim. Repeat the real UI operation under recorded Electron conditions. Test setup does not need the new adapter registry or runtime installation schema.

**Q2 package path.** Produce a version-checked accepted resolution with the supported managed interpreter/tooling. Let shared acquisition fetch the approved wheel set. Close/deny network access to the installation process using an actual enforced mechanism and retain its evidence. Install from generated exact local inputs, verify package and native-build identity, then run the scoped CPU import/tensor/startup checks owned by the current installer. A successful package import is not model inference. Repeat with one corrupted/substituted wheel, one missing closure member and cancellation during package child work. Resolver network access before that leg is separately recorded, not counted as an offline failure or hidden.

**Q3 S3 path.** Use test-owned credentials and a fixture object set with recorded versions/digests. Validate a real AWS endpoint, a named independent S3-compatible endpoint, and local MinIO. Record actual supported features; an emulator proves its own behavior, not AWS or the other service. Fetch the same artifact description without changing consumer code. Through the supported model-facing operation, acquire one S3-backed model, await importer completion, and confirm its exact published library record through GetModel or EnsureModel; a byte-ready transfer alone does not pass. Exercise credential refresh, range resume and object-change refusal. The source settings and progress workflow must be observable through the actual API/UI. Failed listing pages cannot publish a complete package.

**Migration/restart.** Create disposable replicas of supported existing metadata and partial files. Inject interruption at each admission, byte-ready, consumer-commit, ownership-transfer and settlement boundary. Reopen through the production reader and source/consumer composition. Check content, state and effect authority rather than only error text. Prove old-writer retirement/isolation for the actual deployment before any real retained root is migrated. Explicitly keep unaccepted Pending replay unavailable.

**Native and resource evidence.** State selected OS/filesystem, features, artifact size, parallelism and budgets. Tests cover actual mechanisms on Windows/macOS when those promises change. A Linux process or cross-compile cannot close the native execution claim. Runtime gates may open for the qualified target scope, not for every platform. Performance observations establish resource behavior only for that workload; no unsupported throughput improvement is required or asserted.

## Planned test placement and supporting commands

The core public-consumer integration target `rust/crates/pumas-core/tests/artifact_acquisition.rs` exists and has passed 2/2 on the prior code candidate. The AC06 service regressions are unit tests in `rust/crates/pumas-core/src/acquisition/service.rs`; `cargo test --manifest-path rust/Cargo.toml --locked -p pumas-library --lib acquisition::service::tests:: -- --test-threads=1` passed 6/6 on the current local source diff. The planned standalone target `rust/crates/pumas-app-manager/tests/artifact_acquisition_install.rs` is not present; native install/recovery cases currently run in app-manager unit/fixture targets. These local checks do not close the real-source, desktop, or platform claims above. Record each new command/result and broaden to affected aggregate checks:

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
