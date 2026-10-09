# Torch revision byte custody — 2026-10-09

This increment releases the global Torch versions read lock after selecting a registered revision and obtaining its retained byte lease. Revision A can remain retained while the existing installer publishes revision B. Publication, removal and owner recovery of A require the same revision's exclusive lease. This is Linux byte custody, not interpreter initialization, import registration or serving qualification.

The source base is published commit `c9fb9ce7e502bb04682a1ca640f8d0a446a4ee50`, tree `c8490a53e35fa87adeb1796cb948da2fe748f079`. It preserves the combined candidate, sanitized diagnostics and local acquisition increment. Six app-manager Rust files and this report form the entire change; there is no version bump, new RPC schema or physical-store recovery implementation.

## Published interface and lifetime

The existing public `VersionManager::retain_torch_runtime_bytes(&self, tag: &str) -> Result<Arc<RetainedRuntimeReadSource>>` signature is unchanged. The owner takes a short global shared lock to refresh and select an installed Torch registration, acquires a shared per-revision lease under that lock, then releases the global lock before capturing actual dependency, sidecar and managed interpreter roots and file identities. The blocking capture closure retains its lease if its caller disappears. Existing managed-depot custody remains in the returned source.

Acquire this source before interpreter/import work and retain the actual Arc through process retirement. Cloning preserves the same custody until the final owner releases it. Dropping a caller, model or worker handle does not establish process retirement. The caller must attach custody to its actual process/native owner and await registered drainage. The source itself creates no interpreter, import witness or process owner.

Registration and selection metadata may change while bytes are held: the public metadata-only `VersionState::remove_installed_version` does not delete dependency, sidecar or depot trees. This is not a registry pin. Cooperating physical deletion/replacement and recovery remain excluded until retention ends. Direct mutation by a hostile same-user writer is not prevented by these cooperative locks; retained validation still checks selected bytes and identities.

Revision lock files live outside deletable revisions at `.torch-revision-<sha256(exact-tag-bytes)>.lock`. They are permanent and are not pruned or recreated during ordinary deletion. Admission validates bounded registration tags, canonical roots, regular lock files, absence of symlinks and, on Unix, single-link named/opened inode agreement. Read admission requires the same-root global token; mutation admission additionally requires its exclusive mode. Nonblocking contention returns `PumasError::Io` with `WouldBlock` for replacement/removal. Owner cleanup skips a busy revision so unrelated work progresses, then retries after retention ends.

The existing installer holds both its global mutation token and target revision token through orphan rename, pending staging, atomic publication, metadata finalization, rollback and registered cleanup. Removal retains both tokens inside its actual filesystem/metadata worker. Pending publication/stage recovery and orphan pruning use the same revision token, including explicit reuse of the installer's already-held exact target token. This increment does not redesign installer-wide global mutation serialization.

## Assembler and startup registration boundary

The current owning assembler is the existing `VersionInstaller` Torch staging/publication pipeline in `version_manager/installer/torch.rs`: `install_torch_runtime_inner`, `TorchPendingStage`, staged-file validation, atomic destination rename and existing `finalize_installation`. The local component input seam published in the [preceding report](pantograph-local-component-ingestion-2026-10-09.md) is `AcquisitionLocalSource` plus `AcquisitionConsumer::acquire_local`; its verified-file/receipt output does not itself publish a runtime. The current consumer byte seam is `VersionManager::retain_torch_runtime_bytes` above. `TorchRevisionLease` is private owner machinery, not a serialized capability or new consumer lock.

A component-bearing revision assembler/API is **still required**. It must consume reviewed verified inputs, establish the offline dependency closure and compatibility facts, validate the complete staged manifest, and publish/register one immutable component-bearing revision through the existing owner. Release-tag registration alone does not establish the proposed runtime environment/revision identity. No standalone payload store, speculative ready state or unused component-plan API was added here.

A separate owner startup/import broker must register the actual linked CPython build, startup configuration and imported provider image against the retained selected revision and managed depot **before CPython initialization/import**. Later retention cannot prove an earlier unregistered load. A preexisting interpreter with unknown initialization/import history must remain refused. Manifest hashes, paths, module labels and serializable facts are not positive execution witnesses. The future broker must transfer real revision/depot custody into the process owner until actual retirement, preserving unrelated revision publication. Pantograph owns its consumer manifest seam and closed managed-start gate; this increment does not duplicate that code or invent the pending executable API.

## Validation scope

Checks use Rust 1.92.0, locked offline dependencies, one Cargo job and `ORT_SKIP_DOWNLOAD=1`. No Cargo ONNX download occurred.

The Torch-focused app-manager cohort passed **169 tests, zero failures and zero ignored**, with 149 filtered out: `cargo test --locked --offline -p pumas-app-manager --features test-support --lib torch -- --test-threads=1`. Log SHA256 is `32f76ef658a9f3232e0e544c92a48ffd5360d96a54b5e5937a65ab8d4261f52d`. The retained test executable is 142,596,824 bytes, SHA256 `411522f5fc6bcbba26ec3160e6fe7d13074501c7ca77f177c95d4ec5892e659b`.

New tests exercise permanent revision lock identity and clones, root/mode ownership, invalid and aliased keys, linked/special lock refusal, pending recovery and orphan pruning under retained A, actual existing atomic publication and registration of unrelated B, typed same-revision refusal before staging, removal after final retention ends, and actual generic managed-child parked cleanup custody. Staging uses inert test-only bytes; normal atomic rename/finalization and removal machinery run. The generic shell fixture invokes process-tree drainage and observes the managed leader retired; it does not independently witness a spawned descendant or qualify CPython/import behavior. These are synthetic backend/owner tests, not real model inference.

Strict target-only app-manager library/tests Clippy passed without allowances: `cargo clippy --locked --offline --no-deps -p pumas-app-manager --features test-support --lib --tests -- -D warnings`, log SHA256 `4e762a6def8c2ed415569562467ad8bd767821d95421c6310ca56ebe6b6fbd8a`. This does not clear dependency-inclusive repository lint; the preceding report records unchanged core warnings. A subsequent documentation-only correction clarifies byte retention versus registry pin; production behavior and test fixtures remain identical to the executed cohort.

The production RPC all-feature compile check passed: `cargo check --locked --offline -p pumas-rpc --all-features --bin pumas-rpc`, log SHA256 `7415cb8e5cf4a21a517325cdbc6c3020d7141b2d9cb0eed6f1337df5afd99981`. This checks the combined core/app-manager/RPC source without linking or running a new consumer binary.

Final API documentation was rechecked with the same strict target-only Clippy command, exit zero, log SHA256 `92ee1b449f2f2e8ad067aa5bc505adf8ada3fe5a9d41eb546444c2eea9a89e13`. `cargo fmt --all -- --check` and staged `git diff --check` also passed. No behavior or fixture changed after the 169-test run.

## Remaining qualification gates

The reviewed Pantograph handoff remains pinned to `ccbff825faac4bebd3107e56b933b3e0e9643176` and its conventional-GIL CPython 3.12.3 Linux x86_64 cohort. The actual Library archive transfer failed through the supported helper. Alternate wheel publication remains on HOLD awaiting its separate authorization. No wheel was unpacked, installed, loaded or imported here.

The selected registered base environment, complete immutable component manifest/offline closure, wheel RECORD/native compatibility validation, component-bearing assembler and pre-initialization/import broker remain unqualified. No positive managed-start witness, global Python mutation, actual provider load, model inference, release, tag or main/parent-PR merge is claimed. The previously frozen headless consumer binary retains its earlier qualification; compile checks below do not qualify a newly linked consumer executable.
