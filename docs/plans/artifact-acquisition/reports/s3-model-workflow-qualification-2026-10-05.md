# Native explicit S3 model workflow qualification — 2026-10-05

## Candidate and authority

The coordinator froze authenticated reader
`ffecce07220f6504fb1db2e348759473bd284de6`, tree
`0acb23784970e86859851c9063e76d04cd740b8b`, reported independent static acceptance,
and explicitly delegated the smallest useful source-facing native application
successor. Separate branch `feat/s3-model-workflow-ffecce07` commits and normally
pushes tested code `2b8e0a4401224212d84f4c02abcbd175369728ab`, tree
`d78dd9ed0113eaee6d728056ae22dfde14115d80`.

PR40 ancestor remains `63123fd8f9f866064a8315096ab3fb1fc81e8f0f`, tree
`163ef2442695b44555ac0e42ff79a6a88de9e817`. Accepted main remains
`05717338c2aea737483fb4b28c3ed4053a65de96`, tree
`c4dc1a7a0c28099a3ec66f925bb67db73a9177a5`. This is a descendant of the actual
frozen reader, not the old environment seed. Parent owns PRs, reviews, hosted
qualification and merges. No reviewer was contacted.

Repository AGENTS/local skills were absent in the earlier verified inspection.
The CONTRIBUTING-directed MrScripty/Coding-Standards checkout at
`188beda1fa477d21d576c233dd8c7f4c4c267d23` supplies the already read Core,
Router and applicable Rust/library/API/async/security/verification/documentation
standards. The existing route record is copied to the raw evidence directory.
Q3 and the credential contract select this scope; the coordinator excludes
frozen native watcher/importer/reconciliation and reader/manifest production paths.

## Public surface and ownership

With optional `s3`, the additive root exports are `S3ModelImportRequest`,
`S3ModelImportControl`, `S3ModelImportPhase`, `S3ModelImportProgress` and
`S3ModelImportError`; `PumasApi::import_s3_model` returns the existing
`ModelImportResult` after settlement. Existing public constructors/configuration
and anonymous behavior remain unchanged.

The owned request carries explicit endpoint/region/bucket/addressing/timeout,
pinned key/VersionId/logical path/SHA-256 facts, the selected primary GGUF import
spec, caller-reserved workspace, finite retry budgets, retained operation UUID
and optional explicit credentials. The request has no Debug or serde
implementation. Credentials only construct the ephemeral reader. The importer
payload is the existing model spec; credentials never enter that payload,
manifest, receipt, model metadata or progress.

Source selection uses a crate-private invocation on the existing bounded
acquisition consumer scope. It validates finite transfer budgets before I/O and
admits no durable record until selection completes. Transfer/verification and
receipts remain with `AcquisitionConsumer::acquire_s3_manifest`; the existing
importer owns single GGUF or GGUF-plus-explicit-data/text publication. There is
no new transfer, storage or publication owner and no persisted source/account
configuration. Production authentication still requires HTTPS, with existing
endpoint/proxy/redirect and SDK retry policies unchanged.

One request control exposes coalesced safe phase/current-file byte observation
and an atomic cancellation/finalization gate. Cancellation can win before
finalization, including during verification; the prepare callback closes its
admission before receipt issuance. Late stage/cancellation notifications cannot
replace a newer terminal observation. Progress bytes can reset and do not prove
verification. Receiver disconnection does not cancel work. Cancellation
acknowledgement is not a stopped-effects result; the caller keeps awaiting
owned drainage. Dropping the result waiter yields Interrupted, and shared
shutdown drains registered effects. Publication can be uncertain on interruption;
the facade does not promise successful settlement after a dropped waiter.

Typed failures preserve selection errors, operation errors and scope-drain
failure. Drain failure retains an already published result or the original
operation error. Errors/cancellation can retain staged/Using custody. The stable
demand owner is `model.s3.workflow`; callers keep the same operation UUID and
use the existing exact consumer/model-output reconciliation path rather than
replay uncertain publication with a fresh identity. No new automatic recovery,
blanket cleanup or format eligibility policy is added.

## Local verification

All commands ran from the repository root after sourcing
`/workspace/.pumas-tools/env.sh` (Rust 1.92.0, four Cargo jobs), locked/offline,
with disposable `XDG_CONFIG_HOME=/tmp/pumas-s3-auth-config`.
Test profile debug symbols and incremental compilation were disabled; check/
Clippy additionally used `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_DEV_INCREMENTAL=false`. No HOME, permissions, credentials,
network security or production TLS policy was changed.

```bash
cargo test --locked --offline --manifest-path rust/Cargo.toml -p pumas-library --no-default-features --features s3 --test s3_model_workflow --test s3_acquisition --test s3_reader
cargo test --locked --offline --manifest-path rust/Cargo.toml -p pumas-library --no-default-features --features s3 --lib acquisition::s3::auth_tests
cargo test --locked --offline --manifest-path rust/Cargo.toml -p pumas-library --no-default-features --features s3 --lib api::s3_models::tests
cargo clippy --locked --offline --manifest-path rust/Cargo.toml -p pumas-library --no-default-features --features s3,test-support --all-targets -- -D warnings
cargo check --locked --offline --manifest-path rust/Cargo.toml -p pumas-library --no-default-features
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
git diff --check
```

Seventy selected tests passed: six workflow, 41 existing S3 acquisition/import,
13 reader, nine authentication and one control test. Two isolated child runs
also passed. This is not the full Rust/native suite. Strict enabled-S3 all-target
Clippy, headless check, workspace formatting and diff checks passed.
Dependency/lockfile inputs are unchanged; the preceding reader's 12 graph
qualification is inherited, not claimed as rerun here.

The workflow fixtures prove public single/bundled import, Ready/GetModel,
exact bytes, same operation demand, adopted exact completion binding, clean
shutdown and cold reopened output without replay. Refusals cover primary
selection, unbounded retry configuration, missing VersionId, production plaintext
credentials, mismatched digest and pre-cancelled requests. Stalled HEAD and GET
exercise selection/transfer cancellation, one-byte progress observation, no model
publication and source socket closure before drainage returns. A dropped HEAD
waiter exercises Interrupted observation and shared shutdown drainage. The
co-located control test covers both admission outcomes and stale observations.

The Linux-only HTTPS child trusts the repository's existing synthetic localhost
certificate through an isolated OpenSSL certificate-file setting. Peer/hostname
verification remains enabled. It uses the production authenticated constructor,
captures explicit access-key/session-token headers on HEAD/GET, imports and
settles, then scans owned output, staging and cache files for all synthetic
credential values. No production root/transport API was changed. The successor
authentication oracle explicitly requires `x-amz-security-token` in SignedHeaders
and rejects token tampering for both addressing styles and methods; the frozen
reader branch itself is untouched.

Initial integration compilation caught a missing await in a test assertion
(E0599); the original failed log is retained and the assertion was corrected.
The later five-test exploratory run and final six-test plus regression run
passed. The final code differs from that final runtime tree only in documentation
comments; no behavior changed after verification.

## Evidence and remaining scope

Raw logs and command/status/hash inventory are under
`/workspace/scratch/s3-workflow/`: `integration-tests.log`, `auth-unit.log`,
`control-unit.log`, `clippy.log`, `headless.log`, `fmt.log`, `diff.log`,
`verification.json`, `feature-boundaries.log`, `standards-route.json`,
`code-commit.log`, `code-push.log` and `log-inventory.json`. Initial compilation/
exploratory logs are retained as `s3-workflow-tests.log` and
`s3-workflow-tests-2.log`. Final commit/push/ref evidence is recorded alongside
those logs after the documentation milestone. Frozen-file equality checks cover
reader, manifest, staging/VersionId fixtures, reconciliation and dependency
inputs; no importer/watcher file is changed.

This implements the selected native source-facing workflow. Static review,
exact-head hosted qualification and integration of this successor remain
parent-owned. Linux controlled TLS is not live AWS/non-AWS/MinIO acceptance,
packaged consumer qualification or a cross-platform trust test. Refresh,
account provisioning, desktop/RPC source entry, other/sharded weight formats,
large-payload concurrent discovery and frozen native repairs remain separate.
No real account credentials, paid provider, security bypass or external reviewer
contact occurred. AC13/AC14 and AQ-S3 remain pending.

The next existing-plan implementation feature is Q3 desktop/RPC explicit source
selection/configuration and progress/cancellation/result through this native
entry; real-provider acceptance requires its separate authorized environment.
