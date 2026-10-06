# S3 release profile attribution and qualification

Successor branch `feat/s3-prefix-main95` starts exactly from integrated checkpoint
`7cf17383001d582056ac4299803c83d0f9ae68a7`, tree
`046771b37c9a9f563936376539301b3225b48656`. Approved main95 and the reviewed
SDK9077, AC0810f and HF4eb remain ancestors; frozen evidence is preserved.

The existing generator now accepts an explicit `--features s3` profile, selecting
`pumas-rpc` defaults plus S3 across the three desktop targets. Both metadata and
normal/build graph collection use that feature selection. The generator refuses
missing selected Rust notices. Inventories record the package, defaults, features,
targets and exact union of Rust versions; the checker rejects profile mismatch,
duplicate/missing closure entries or S3 inventories without the SDK owner.
Default legacy inventories remain readable; the regenerated default inventory
also carries the explicit profile. No new Rust API or dependency is introduced
by this first milestone.

`docs/release-attribution/0.7.0-s3` contains 414 entries and 387 Rust versions,
covering all 365 default notice entries plus 49 additional versions. These are
normal/build and runtime-notice supersets, not claims that every component is
linked or bundled. SIMD license text comes from previously qualified immutable
upstream evidence; its hash matches the frozen SDK collection. It is retained
under `scripts/release/licenses` with the exact declared crate revision. No
runtime, credentials or provider account was obtained or provisioned.

Both generators/checkers, eight attribution tests, eleven release contract
tests, dependency ownership and Python syntax checks pass. The expanded feature
checker passes 33 graph contracts: the existing 21 and twelve S3 default/headless
core/RPC combinations. The graphs retain dynamic ONNX loading/disable-linking
and reject download, copy and ONNX TLS features, ambient credential loaders,
hidden object-store owners and default AWS HTTP transports. Graph validation
does not establish cross-platform builds. Production default-plus-S3 RPC compiles
with locked offline Cargo and no test-support; all 302 default-plus-S3 RPC tests
pass (the Cargo test graph includes its existing dev fixtures). No ONNX runtime download or inference
acceptance is implied.

CI now validates both attribution profiles and S3 feature graphs, and explicitly
runs S3 signing/discovery units in its existing enabled-S3 job. No hosted run was
started or claimed. S3 distributions must use this profile's notices; unchanged
default packaging alone is not an S3 packaging certification.

Two intermediate failures are retained: the new identity projection initially
encountered an empty Cargo-tree row (`ValueError`, fixed by ignoring empty rows);
the first S3 selection still used default-only metadata and emitted 365 entries.
The new checker actually rejected that missing closure; the corrected generator
emits 414 and the regression refuses a selected SDK absent from metadata.

Logs are under `/workspace/scratch/s3-prefix-main95/`, including
`attribution-empty-tree-row-intermediate.log`, `attribution-s3-metadata-omission-red.log`,
the generator/check/test logs, `feature-contracts.log`, `release-contracts.log`,
`dependency-ownership.log`, `rpc-release-profile-check.log` and
`rpc-release-profile-tests.log`. Tight disk space required retiring named,
superseded debug test executables and recomputable incremental caches; retirement
paths and executable hashes were saved first. Source, frozen logs and packaged
qualification evidence remain intact.

The next admitted write set is bounded prefix discovery: core S3 reader/SDK and
new prefix/XML-guard/test children, optional roxmltree dependency and lock,
acquisition exports, focused S3 model-workflow integration tests, README/contract,
current acquisition plan/ledger and successor qualification/review reports.
`s3/manifest.rs`, watcher, importer/recovery, stores, retry/verification/receipt
formats and ONNX source remain outside that write set. Every page and listed
object stays under one SDK and caller-provided bounds. Listings receive exact
XML/SDK agreement validation; only complete enumeration followed by checked
immutable HEAD observations returns discovery results. Caller-provided digests
and explicit manifest/import owners remain required. No PR mutations are made;
parent coordinates publication and independent review. Provider/platform gates
remain pending.
