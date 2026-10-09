# Immutable Torch component byte assembly — 2026-10-09

This increment assembles an immutable component-bearing byte revision through the existing `VersionInstaller`. It publishes no runnable interpreter entry and grants no initialization, import or serving permission. The owner also issues an opaque selection associating the retained physical revision and managed depot with its immutable manifest. Existing activation, identity/probe, dependency and both core launch paths refuse these assemblies before Python or child effects.

The source base is published `650b6cb38eb432b2dd39a58cc6ea14855d2f62ae`, tree `8855c3941dfaec9101e5cf2a1bf9a22dc4ccd43f`. The combined candidate, consumer cohort, diagnostic and local-ingestion/custody increments are preserved. The complete write set is fourteen Rust files and this report. There is no dependency addition, lockfile change, version bump, new RPC contract, alternate store or physical-store recovery implementation.

## Exact owner/consumer contract

Exported from `pumas_app_manager::version_manager`:

```rust
pub struct TorchComponentFile { pub path: String, pub size: u64, pub sha256: String }
pub struct TorchComponentManifest {
    pub distribution: String, pub version: String, pub wheel_name: String,
    pub wheel_tag: String, pub python: String, pub platform: String,
    pub archive_files: Vec<TorchComponentFile>,
    pub replace_base_members: Vec<String>,
    pub final_dependency_files: Vec<TorchComponentFile>,
}
pub struct TorchComponentAssemblyRequest {
    pub base_tag: String,
    pub expected_base_manifest_sha256: String,
    pub component: TorchComponentManifest,
    pub input: AcquisitionLocalRequest,
}
pub struct TorchComponentAssembly {
    pub revision_tag: String,
    pub manifest_sha256: String,
    pub progress: mpsc::Receiver<ProgressUpdate>,
}
pub struct TorchComponentSelection { /* private owner-issued fields */ }
impl VersionManager {
    pub async fn assemble_torch_component_revision(
        &self, request: TorchComponentAssemblyRequest,
    ) -> Result<TorchComponentAssembly>;
    pub async fn select_torch_component_revision(
        &self, revision_tag: &str,
    ) -> Result<TorchComponentSelection>;
    pub async fn select_torch_component_revision_matching(
        &self, revision_tag: &str, expected_manifest_sha256: &str,
        expected_interpreter_depot_manifest_sha256: &str,
    ) -> Result<TorchComponentSelection>;
}
impl TorchComponentSelection {
    pub fn revision_tag(&self) -> &str;
    pub fn manifest_sha256(&self) -> &str;
    pub fn interpreter_depot_manifest_sha256(&self) -> &str;
    pub fn qualification(&self) -> &'static str; // always assembled_unqualified
    pub fn retained_source(&self) -> &Arc<RetainedRuntimeReadSource>;
    pub fn validate(&self) -> Result<()>; // blocking
}
```

`TorchComponentSelection` is cloneable but has no public constructor, public fields or serde reconstruction. Only the existing VersionManager authority issues it for a finalized registered assembly after capturing actual revision/depot roots under the published custody mechanism. Matching validates canonical lower-case SHA256 declarations, revision/manifest equality and the actual selected interpreter digest. These are frozen selected byte facts, never a later active-tag lookup or metadata pin. The immutable manifest fixes `assembled_unqualified` and the restrictions `no_runnable_interpreter_entry` and `no_initialized_import_provenance`.

The exact accepted contract deliberately uses component selection names. Pumas has no separate owner environment identity to issue. Pantograph can use the immutable revision as its declared selection key and must keep its own process/configuration identities distinct. The [Pantograph broker proposal at e7aff5dd](https://github.com/MrScripty/Pantograph/blob/e7aff5dd736ca2166ebee6f9d5f41cc18f3e301d/docs/pumas-python-startup-broker.md) can retain this actual source Arc through its process pin mechanism; managed entry must continue refusing the fixed unqualified disposition. A dependency/consumer adapter, complete first-entry coordination and genuine initialization/import provenance remain separate work.

Capture/matching and blocking `validate()` belong outside Pantograph's short broker mutex and under actual consumer job custody. Retain the selection or its exact source Arc through registered effects and process retirement, including partial native exposure. Cloning a File does not retain the revision/depot lease. Pre-exposure rollback and post-exposure process pins are consumer responsibilities; this API creates neither a process owner nor a readiness witness. Same-revision physical mutation is excluded while any actual source Arc remains; unrelated revision publication and metadata changes retain the preceding custody behavior.

## Immutable identity and assembly policy

`torch_runtime_byte_manifest_sha256(source)` hashes compact serde JSON with domain `pumas.torch.selected-byte-manifest.v1` and sorted `(role, path, size, sha256)` tuples across selected Interpreter, Dependencies and Sidecar files. Role strings are `interpreter`, `dependencies`, `sidecar`. `torch_interpreter_depot_manifest_sha256(source)` uses domain `pumas.torch.selected-interpreter-manifest.v1` and sorted `(path, size, sha256)` tuples in the actual Interpreter role; absent selection refuses. Neither digest describes omitted paths, system ELF libraries, loader search paths or a complete model read set.

The compact immutable `component-manifest.json` has domain `pumas.torch.component-assembly.v1`, policy version 1, selected registered base tag and actual full base digest, actual selected interpreter digest, fixed disposition/restrictions, canonical complete component facts and the input ArtifactManifest provenance. Exact encoded identity is bounded to the retained reader's 64 MiB limit before admission. SHA256 of those exact bytes derives `torch-component-<64 lowercase hex>`. Provenance declarations are retained input facts; hashing them does not independently verify a producer's claimed build-source history. Attempt workspace, retry demand and staging names do not enter the revision identity.

Inputs require a registered supported Torch release-tag base, its exact actual captured digest, Python 3.12/Linux x86_64, one held wheel with exact nonzero size/SHA256, an explicitly supported wheel tag, complete exact archive manifest, explicit replacement list naming selected old base members, and the entire final dependency file manifest. These are reviewed byte-closure declarations; there is no semantic `Requires-Dist` resolver. Actual CPython 3.12.3/build and GLIBC/GLIBCXX/CXXABI compatibility must still be established for the real producer/consumer cohort; a filename/tag does not establish ABI qualification.

This intentionally narrow standard-ZIP policy supports Stored/Deflated regular files and selected directories; ZIP64, split/ambiguous archives, embedded alternate EOCD/ZIP64 signatures, encryption, symlinks/special files, path aliases/traversal, import hooks/bytecode and wheel `.data` relocation refuse. An archive is at most 2 GiB, a selected file at most 2 GiB, and each declared archive/final dependency closure at most 4 GiB. The preconstructor footer/central-directory pass bounds count, field sizes and central metadata (64 MiB); a full streaming signature scan prevents zip 2.x fallback to an earlier unapproved footer. Legitimate archives containing those embedded signature bytes are also outside this supported subset. All wheel parsing uses a fresh exact-size/SHA256 private copy of the held input.

METADATA Name/Version and WHEEL version/exact tag are checked. A bounded, restricted unquoted three-cell RECORD must describe every selected archive file exactly: SHA256 URL-safe unpadded base64 and canonical size, with only RECORD's own hash/size empty. Quotes, additional exemption/signature records and other hash algorithms refuse. Every extracted and copied output is fresh, size/SHA256 verified; complete final dependency namespace validation refuses omitted or added files. Admission counts final files and their distinct original-case parent directories against the retained 200,000-member namespace limit. Actual staged sidecar namespace limits (including excluded venv) and the updated 1 MiB recipe limit are checked before publication. No hardlinks to mutable base output are created.

The public acquisition-aware factory now supports Torch with consumer `runtime.torch.components`. Assembly uses the existing local-input acquisition owner, installer task registry, install serialization, pending stage, target revision/depot leases, validated staged files, atomic rename, metadata finalization and rollback. Caller loss does not cancel registered work; await `shutdown_installations()` for drainage. The actual base Arc remains in stage custody through blocking work, publication/finalization or rollback. Known completed validation refusal explicitly withdraws unreceipted Using authority after registered stage cleanup; uncertain failed effects remain conservative and require existing owner recovery. This does not claim cross-process crash recovery qualification.

The acquisition receipt phase is `component_bytes_prepared`: it acknowledges verified input preparation only. Runtime publication/finalization occurs later in the existing installer. Owner-derived stage-specific demand keys prevent joining an ephemeral prepared path from another attempt. Finalization records dependencies_installed=false. Inherited probe results are removed; runtime options report assembled_unqualified. No venv/bin interpreter or pyvenv.cfg is created, and both legacy ProcessLauncher and owned profile process launch refuse even with a caller override. Selection rechecks exact assembly topology, manifest, dependency closure and actual managed depot association.

## Controlled qualification

The final Torch-focused app-manager cohort passed **174 tests, zero failures and zero ignored**, with 149 filtered out: `cargo test --locked --offline -p pumas-app-manager --features test-support --lib torch -- --test-threads=1`. Log SHA256 is `a1d4cb623fc8a40ad867b49cd6e216eacb8d01de55ca77d8a93544462ce5c904`. The retained test executable is 144,797,032 bytes, SHA256 `236777af76fd5207caa84e80d2917c5996eddb417badf5040b98fad2cb36299e`.

Five new tests run real owner paths with controlled inert archives: publication and immutable selection/mismatch, clone custody versus detached File, pre-store base/hook/mixed-case namespace refusal, ten archive/RECORD/digest/path/link/final-closure/footer/metadata-bound refusal variants with actual durable withdrawal and no receipt, caller-loss registered drainage retaining the real base/depot, and metadata failure after actual atomic rename with rollback. The rollback case observes an adopted prepared-input receipt alongside an absent runtime revision, explicitly distinguishing those events. The fixed RECORD oracle was independently checked using Python SHA256/base64/byte counts; no production hash encoder generated the expected fixture record.

The targeted owned core launch refusal passed one test (2,090 filtered), log SHA256 `ad3c3564a70513840b00e7e3c37e28df33bcaa1bdb1c8cd9439858abde01e02a`. The actual owner returns an unsuccessful response receipt, no observation and no PID/log/child effects. The ordinary owned-launch admission/observed-stop regression also passed one test using the same executable, log SHA256 `ff01fef5a805b7026cb46d31ed22bd7f34ee75f9cde4fc6d56ea71e4bb3bd405`. Commands use `cargo test --locked --offline -p pumas-library --no-default-features --features s3,test-support --lib <filter> -- --test-threads=1`. The executable is 279,425,792 bytes, SHA256 `c97deb0e2c297c6b782969721e528d5a66020d752123f5a372d649490eb5021f`. These two tests do not qualify the full core cohort or native physical-store/recovery work.

Strict target-only app-manager library/tests Clippy passed on final frozen source without allowances: `cargo clippy --locked --offline -p pumas-app-manager --features test-support --lib --tests --no-deps -- -D warnings`, log SHA256 `d85df8fb02526618096cfe9d7db5542d120f31f144b389e17148f71541c653e7`. This does not clear the dependency-inclusive core baseline warnings recorded in preceding reports. Initial fixture compilation/owned-source lint failures and subsequent bound review corrections are retained in local evidence; the final runtime/lint checks above execute the corrected source.

The production all-feature RPC compile passed: `cargo check --locked --offline -p pumas-rpc --all-features --bin pumas-rpc`, log SHA256 `ad6d391e5db2e5b25aa747b962c97945010e26839926aeefb2b79ae707cadad1`. This checks the combined source without linking/running a newly qualified consumer executable. The previous 203,722,168-byte headless consumer remains byte-identical at SHA256 `61c20bd24bd179fbc14538183c6898c14e677d84209413de69e1d797584a0a47`; it was rehashed, not replayed.

Independent source review closed the archive-parser, receipt-cleanup, private selection and metadata/namespace-bound findings. Independent execution review verified all fourteen frozen Rust hashes, actual runtime logs and both executable hashes without replay. Its evidence SHA256 is `c969d2997e1bac66c70176fadb2456b1903794eaa773a60c9161873d58b8c830`; the frozen Rust source receipt SHA256 is `852cff78c589934e332b0d444e2a411e3732dd397ad6484dc9185e62b0967fa1`. Final format/diff checks and the explicit staged write set are verified before commit. These are controlled byte/owner tests and compile checks, not actual package import or real model inference. All checks use Rust 1.92.0, locked offline cached dependencies, one heavy Rust job, and ORT_SKIP_DOWNLOAD=1. No Cargo ONNX download or actual provider load occurred.

## Remaining qualification gates

The actual Pantograph wheel transfer failed through the supported Library helper; actual artifact installation/import/inference, alternative publication and positive managed startup remain on HOLD. The selected live base is a runtime acceptance input, not a source implementation blocker. No actual wheel was obtained or loaded for this increment.

Still required: successful authorized artifact transfer; verified producer association and complete exact wheel/RECORD/native file facts; a user-selected live registered base and reviewed complete final dependency closure within supported bounds; actual interpreter/provider/shared-library compatibility; a runnable interpreter/startup profile assembled by its owner; complete consumer first-entry coordination and observed CPython initialization/provider-image provenance; then real import/model inference qualification. Controlled inert archives and arbitrary fake native bytes qualify only these byte assembly/custody/refusal paths. The earlier headless consumer qualification remains pinned to its retained executable; a compile check does not qualify a newly linked consumer binary. Native physical-store/recovery and the original v0.8 acceptance gaps remain separate and unqualified; see the pinned [combined acquisition acceptance report](combined-ec03-acquisition-acceptance-2026-10-09.md) and [Chrema consumer report](chrema-native-consumer-2026-10-09.md). No release, tag, main merge or parent-owned PR change is included.
