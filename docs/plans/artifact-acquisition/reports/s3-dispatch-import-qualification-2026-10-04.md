# S3 dispatch and single-GGUF import milestone

Branch: `feat/acquisition-s3-import-2c7d6014`. Reader ancestor:
`2c7d6014658ef44575f3724f0ff6e7395f1fd7d8`. Accepted main ancestor:
`96c2dca97fad673a2735f9f778013067569fe692`, tree
`a0d7cae9bae5398064220115b6cec04588d020e2`. Their local composition is
`74ea1823aaf19448b0808c6b3433384418aceb6a`, tree
`a6a16b7afc13fc1753fbf5a636885f8067a57685`. Both source histories remain intact.
The accepted HF and speech-test files match main exactly. Reader/PR37 branches
remain unchanged. The coordinator owns PR creation, review, hosted CI and merge.

Status: locally qualified; independent review and exact-head hosted acceptance
pending. All 28 focused tests, strict all-target Clippy, headless compile,
feature/attribution/ownership checks and 20 release/workflow contract tests
passed on the final source. AQ-S3 remains not ready.

## Outcome and composition

The optional `AcquisitionS3Request` binds one exact reader selection to a consumer
demand, held workspace and positive finite retry budgets. A closed internal
HTTP/S3 source dispatch feeds the existing service's transfer loop. Both sources
share checkpoint custody, registered descriptor writes, retry scheduling,
SHA-256 verification, file promotion, FilesReady/Using transitions, receipt
issuance, consumer publication and adoption. No second transfer store, task
owner, schema, retry loop, signer, URL parser or import-publication protocol was
introduced. The S3 library retains ownership of protocol parsing.

The S3 adapter validates exact response version, ETag, total and range before
exposing a body to shared streaming. Continuation additionally requires the
existing owner's checked live prefix and the selected S3 version/validator.
SDK and HTTP automatic retries remain disabled. Source operation deadlines
include body polling, destination writes and progress; elapsed transfer limits
cap requests/backoff. Admission, durable observations, registered effect drain,
verification and consumer publication are not a hard wall-clock bound. A timed
out write waiter cannot authorize truncating its descriptor: registered effects
are joined before retry. Arbitrary external blocking work is not force-aborted.

`ModelImporter::import_acquired_gguf` is a source-neutral, single-file consumer
bridge. Its spec path must match the selected logical path, and its payload must
match the currently issued receipt's serialized spec. A matching forged JSON
receipt is insufficient: the held use checks the actual canonical store row and
issued receipt before model effects. Callers keep the use in the prepare result
and invoke the importer from the publication callback after receipt issuance.

The importer reads its verified descriptor without reopening an input pathname.
Its copied-input plan verifies the copied size/hash against the acquisition
receipt before invoking the existing model publication/confirmation path. A
refused copied import becomes an error, so a model collision cannot accidentally
adopt an unsuccessful request. The shared use and registered effect retain input
custody through the blocking model producer. Model readiness and indexing stay
with the existing model publisher. Failed publication retains Using and the
issued intent receipt; automatic reimport, deletion or output reconciliation is
not added. A receipt without a proven model output does not establish success.

The source adapter owns protocol changes; the shared owner owns retry/checkpoint
changes; the importer owns model-policy/publication changes. Callers need the
existing workspace/demand/retry/prepare/publish contract and model spec, not SDK,
credential, range or filesystem-recovery internals. Removing S3 dispatch would
remove the new capability; removing the descriptor bridge would force unsafe
path recovery or duplicate copying/publication across callers. The real HTTP
and S3 implementations justify this bounded internal seam, not an open adapter
registry. Other formats and multi-file model packages remain explicitly absent.

## Local evidence

Linux x86_64, Rust/Cargo 1.92, existing shared target, offline locked dependency
resolution. Command-local root debug/incremental/strip overrides and one build
job were used to fit this environment; no toolchain, permissions, credentials or
network settings changed. Runtime tests used a task-owned XDG config directory.

- `cargo test --locked --offline --manifest-path rust/Cargo.toml -p pumas-library
  --no-default-features --features s3 --test s3_acquisition -- --test-threads=1`:
  12 deterministic local integration tests. They observe a synthetic GGUF through
  the real indexed model importer and independent read-only selector as Ready,
  exact bytes, verified receipt/adoption, and durable acquisition reopening.
  Negative cases cover changed version, wrong digest, invalid logical input,
  unsupported content, model collision, unissued receipt, bounded shared retry
  counts, invalid/unrepresentable budgets, live checked-prefix pause/resume,
  cancellation, elapsed unfinished-body expiry, and dropped-waiter drain.
  A normal copied import also exercises the preserved directory-input path.
- All 11 reader protocol fixtures passed as reader regressions.
- All five public HTTP acquisition integration fixtures passed, observing existing shared
  consumer behavior after dispatch/streaming changes.
- Strict all-target core Clippy passed with `--no-default-features --features
  s3,test-support -- -D warnings`; the actual headless compile check passed with
  no default features.
- Repository feature graphs cover 12 configurations across Linux/macOS ARM64/
  Windows x86_64. Six separate default/headless core graphs omit the SDK. These
  are graph checks, not native builds.
- The unchanged canonical notice generator/checker and refreshed optional
  inventory retain the exact license texts. `bytes` is now a direct core
  dependency for its streamed-body boundary; its locked version and all existing
  package identities are unchanged. No third-party package was added here.
- All 20 release/workflow contract tests and the dependency-ownership checker passed, covering
  the enabled-S3 workflow projection and manifest ownership.

Exact commands, output logs, cache-retirement hashes and source hashes accompany
the review handoff. The first parallel focused build failed to compile two
fixture API calls and exhausted disk while linking HTTP tests; it produced no
suite result. The corrected serial fixtures passed. Clippy initially rejected a
nonminimal boolean; the predicate was rewritten without changing its meaning.
All-target compilation also exposed unit fixtures that still constructed the
old directory-only copy source or consumed a concrete reqwest response. Their
constructors/body collectors were adapted while retaining byte and publication
oracles; the literal chunked-response fixture remains the no-Content-Length
input. Unit-suite execution is still not claimed by the compile check.
Only identified outputs from these completed/superseded tasks were retired;
unrelated caches and worktrees were preserved. No full-suite pass is claimed.

## Hosted and unsupported scopes

`.github/workflows/build.yml` now explicitly enables S3 for reader/dispatch
integration tests and strict all-target Clippy in the headless job after locked
fetch. Ordinary default/headless CI omits S3 and must not be called S3
qualification. The added commands have local evidence; no hosted exact-head run
is claimed by this worker. Workflow source validation is not a hosted run.

These fixtures use anonymous HTTP/1.1 loopback endpoints and a minimal synthetic
GGUF header, not real model weights or inference. No live AWS, non-AWS provider,
MinIO, authentication/refresh, TLS/public DNS/HTTP2-NACK fixture, desktop/RPC source
workflow, prefix listing, multi-file model import, other acquired formats,
directory/access-point bucket, resource benchmark, native cross-platform build,
default ONNX build or complete unit suite ran. Consumer publication interrupted
across process restart still requires owned output reconciliation; this slice
does not claim automatic S3 model recovery.

The next slice is an explicit source-facing composition and exact model-output
reconciliation policy, followed by the separately authorized credential and
provider matrix. A reader/service-only milestone cannot close AQ-S3.
