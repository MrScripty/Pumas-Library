# Local component ingestion boundary — 2026-10-09

The first owner API increment accepts local file capabilities through the existing shared acquisition store, verified-file handoff and consumer receipt lifecycle. It does not install the Pantograph wheel or enable managed Python startup.

The reviewed consumer contract is pinned to Pantograph commit `ccbff825faac4bebd3107e56b933b3e0e9643176`: [handoff](https://github.com/MrScripty/Pantograph/blob/ccbff825faac4bebd3107e56b933b3e0e9643176/docs/pumas-managed-tokenizer-handoff.md), [machine-readable facts](https://github.com/MrScripty/Pantograph/blob/ccbff825faac4bebd3107e56b933b3e0e9643176/docs/pumas-managed-tokenizer-handoff.json). Downloaded contract SHA256 values are `c6130a0e2151c9831f6138efd83fb610c5a1a06b87274f13945bd6952be91ff9` and `e2b05a4414e2619dfad5678a282f635b1959996406b9e0b3d9c31f726c7847ca` respectively.

## Rust interface

`AcquisitionLocalSource::new(File, Arc<dyn Send + Sync>)` takes an actual held regular file and a cooperating input-owner keepalive. It accepts no pathname, URL, request JSON or caller-selected runtime destination. Linux is the current supported platform. The keepalive preserves the input owner's cooperative lifetime; it is not proof of immutability, a no-symlink pathname origin, execution qualification or native-image retirement.

`AcquisitionLocalRequest` supplies the existing `AcquisitionDemand`, `ArtifactManifest`, `AcquisitionWorkspace`, one held source per manifest member in order, and finite positive `AcquisitionRetryPolicy` limits. Every selected file requires exact size and SHA256 evidence. Manifest source/revision/digest authorities preserve the caller's reviewed provenance; submitting those facts does not authorize execution.

`AcquisitionConsumer::acquire_local(request, host, prepare, publish)` uses the existing consumer scope, store and registered task/effect owner. It verifies local size and SHA256 before durable store admission. Verification and copying use fixed-size positional reads retaining both the actual file and keepalive in each registered effect. Existing workspace verification checks copied bytes again before consumer use. The existing `prepare(AcquiredArtifactUse)` and `publish(staged, AcquisitionConsumerReceipt)` callbacks retain consumer-owned extraction/publication authority; successful output settles the existing exact-demand completion receipt.

One checked local deadline starts at request admission and covers preverification, subsequent copying and retries. Expiry or host pause/cancellation stops new read admission. An OS read already running cannot be hard-cancelled: its file and keepalive remain held until registered drainage actually completes. A refusal reply may precede read completion; retain the consumer/service and await their shutdown before owner teardown. Verification and copies restart from offset zero; no local HTTP ETag or suffix checkpoint is invented. This is not an absolute wall-clock bound on blocking I/O, final verification, publication or shutdown.

## Component acknowledgment and remaining owner facts

The input tuple is the wheel `tokenizers-0.21.4+pantograph.snapshot1-cp39-abi3-linux_x86_64.whl`, 24,366,281 bytes, SHA256 `678b155145bb06c271ad6d8eb2df95a8a8173155323fafd4b102e50349940b95`, upstream Tokenizers `e892882fd4608b468dcf9dc33ea95283882b8e6d` plus the nine prepared hashes in the pinned contract. Its native image and embedded association remain distinct verified facts. The qualified cohort stays conventional-GIL CPython 3.12.3 on Linux x86_64; the abi3 filename does not expand that scope.

The neutral Rust input API above is the implemented first seam for reuse by existing Torch staging. It creates no parallel package store, runtime registry or consumer lock. Distinct immutable runtime bundle publication, interpreter registration and imported-image process-lifetime custody remain required future integration, not inferred from an acquisition receipt or input keepalive.

Before the component-bearing runtime can be installed or enabled, the owner needs the exact selected registered base environment, immutable revision and published manifest digest; its approved offline dependency closure; and the actual managed interpreter build and native-library compatibility evidence for the qualified cohort. The current handoff selects no base runtime. Existing Torch release-tag registration alone does not supply the proposed immutable component-bearing revision identity. Existing retained-runtime read sources hold version/depot locks and actual selected roots, but are explicitly independent of execution qualification.

The intended imported-image design uses owner-managed per-revision retention coordinated with the existing publication/deletion authority. Holding the current global `TorchVersionsLock` read guard for a whole process would block unrelated revision installation, so that existing global byte-read guard is not the proposed execution-lease representation. No per-revision execution API is exported by this slice; the retained-revision/unrelated-install test remains a required gate before that workflow is usable.

Prior owner-controlled interpreter initialization and provider import registration must also be established for the actual Pantograph process. Current file hashes, Python module labels, paths and serializable identities cannot retroactively prove a loaded image. The consumer's closed managed-start route remains closed; this source slice adds no positive proof constructor. Imported-image custody must outlive model/worker stop and be retained until actual process retirement.

## Validation and delivery limits

The source increment is based on published Pumas commit `5d6121aceee3cecf3a746fa4862bc84e7b33a556`, retaining the combined candidate and its earlier consumer reports. Four acquisition Rust files and this report are the entire write set; RPC methods, schemas, runtime installers and version identifiers are unchanged. The earlier frozen headless consumer executable retains its original qualification identity; these checks do not qualify a newly linked RPC executable.

Checks ran with Rust 1.92.0, locked offline dependencies, one Cargo job and `ORT_SKIP_DOWNLOAD=1`. No Cargo ONNX download occurred. The final receipts are:

| Check | Observed result | Log SHA256 |
| --- | --- | --- |
| Local ingestion cohort | 7 passed, 0 failed; 2083 filtered out | `52d80badbc0882bb9ac3b1347fb2be35b066b22dd26eec606ee24a73c69a49ee` |
| Shared acquisition regression cohort | 209 passed, 0 failed, 2 ignored; includes the seven local cases | `101367068f52a217093e4d8b9f0c708b93bad95561f5ef43c2734d92c772182c` |
| RPC binary, all-feature production compile check | Passed, including core and app-manager | `072fa7e0f733188e9ec0f066ed3550605d59259c379269135c5d6e6e61b7f4c9` |
| Library and tests Clippy, explicit baseline allowances | Passed with the three allowances below | `48085bc716016c9ab68789c8d16fc15224062000f0e25437b90d1ec9d4c01f38` |
| Rust formatting | Passed | `e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855` |

Reproduce the runtime cohorts from `rust/` with `cargo test --locked --offline -p pumas-library --no-default-features --features s3,test-support --lib FILTER -- --test-threads=1`, using `acquisition::service::local_tests` and then `acquisition::` as the filters. The two ignored shared-suite entries are fresh-process tracing helper tests invoked by their parent fixtures. The retained core test executable is 279,414,736 bytes, SHA256 `d5615ab3abc1e52954de12fd27b665a6f35269b65009d35e9f7db150a6b1a0f8`. The production check is `cargo check --locked --offline -p pumas-rpc --all-features --bin pumas-rpc`.

Strict Clippy remains failed on four unchanged core warnings: `nonminimal_bool` in `model_library/library/projection.rs:164` and `type_complexity` in `api/reconciliation.rs:547`, `:556` and `:732`. A run allowing those two classes then exposed an unchanged S3 fixture `map_flatten` warning at `acquisition/s3/conditional_tests.rs:525`. All three affected files are byte-identical to the base. The passing command was `cargo clippy --locked --offline --no-deps -p pumas-library --no-default-features --features s3,test-support --lib --tests -- -D warnings -A clippy::nonminimal_bool -A clippy::type_complexity -A clippy::map_flatten`. These classes are disabled across that target, so this is an allowance-limited pass, not a strict or repository-wide Clippy clearance. No source suppressions or baseline fixes were added.

Earlier iterations remain failed or unqualified: an out-of-space compilation, an interrupted superseded compilation, and a five-pass/two-fail test run. The two failing fixture expectations were corrected to the existing copied-byte `HashMismatch` behavior and the distinction between early refusal reply and actual registered-read drainage; production logic did not change for those corrections. Only the final seven-case receipt above is successful qualification.

The passing local cases cover retained-descriptor/path replacement and independent cursor use, exact digest/size refusal before durable admission, owner/member/budget validation, empty input, copied-digest mutation, caller-loss retention and preverification deadline refusal with pending-read custody. Drainage fixtures gate registered effects before the OS call; they do not demonstrate cancellation of a genuinely blocked OS `pread`. The previous-active-file sentinel is not Torch rollback proof. These synthetic fixtures are not actual wheel extraction, RECORD validation, ELF load, interpreter initialization, runtime installation, inference or a positive managed-start witness.

Supported Library preparation succeeded for the final handoff, but the required transfer helper returned `library file transfer failed: download failed` with exit 1. Its output did not identify connection failure or access denial. The archive was not verified or unpacked; there was no unchanged retry or alternate route. Alternate public artifact delivery is separately awaiting explicit authorization. Published contract facts permit source/API work while actual-artifact qualification remains gated.

Existing runtime staging and publication retain their current behavior. This first increment does not claim full Torch component rollback/publication qualification, a release, a main/PR merge, global Python changes, credentials, a trust bypass or model inference.
