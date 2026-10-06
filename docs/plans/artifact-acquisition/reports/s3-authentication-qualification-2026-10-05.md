# Explicit request-scoped S3 authentication

Status: implemented and locally qualified; independent review, exact-head hosted
qualification and integration are coordinator-owned. AC13/AC14 and AQ-S3 remain
pending. This does not establish real-provider acceptance.

## Exact source and authority

Branch: `feat/acquisition-s3-auth-63123fd8`.

- Base/PR40 head: `63123fd8f9f866064a8315096ab3fb1fc81e8f0f`.
- Base tree: `163ef2442695b44555ac0e42ff79a6a88de9e817`.
- Preserved main: `05717338c2aea737483fb4b28c3ed4053a65de96`.
- Tested code milestone: `d74114c63a9717692fa7051ef5146d7253a2c5c5`.
- Code tree: `bd21dd5e46970b74e18706076adc9defa7de9586`.

The selected environment initially contained older `work` source at `0dd38c70`.
Normal fetch obtained current PR40 and main; the expected PR40 head/tree were
checked before creating the distinct feature branch. Implementation uses that
verified base. Main is an ancestor. No history rewrite, old seed reuse, external
review contact, PR creation, merge, native repair, or provider provisioning ran.
The frozen watcher/importer/reconciliation and both S3 manifest implementations
are byte-identical to the base, as are Cargo manifests and lockfile.

No applicable AGENTS or `.agents/skills` files exist in this environment/checkout.
CONTRIBUTING, architecture/development docs, Q3 plan and credential contract were
read. The documented local Coding-Standards path is absent; the normal connected
GitHub reads and a temporary clone supplied current standards
`188beda1fa477d21d576c233dd8c7f4c4c267d23`. Its native executable Router returns
`complete` for the actual library/Rust/API/async/security/diagnostic/contract,
implementation/verification/documentation/commit and test-oracle facts, including
required closure. This records applicability, not a whole-repository compliance
certificate or independent review.

## Public behavior and implementation

The additive public exports are `S3Credentials`, its fallible
`new(String, String, Option<String>)`, and
`S3Reader::new_authenticated(S3ReaderConfig, S3Credentials)`.
`S3ReaderConfig` and anonymous `S3Reader::new` retain their shape and behavior.

Credential construction validates printable nonempty ASCII without whitespace;
access-key IDs also exclude `/`, `,`, `=`. Authenticated signing regions are
ASCII alphanumerics/hyphens. These admitted inputs prevent the pinned signer's
infallible header conversions from receiving malformed values. Validation errors
never echo inputs. All credential fields, including the access-key ID, are
redacted by `Debug`. Credentials have private fields and no serialization,
cloning, discovery, refresh or persistence API.

The authenticated constructor requires HTTPS even with `allow_http: true` and
retains normal peer verification. A private `cfg(test)` authentication variant
admits literal IPv4/IPv6 loopback HTTP only for unit fixtures. It is absent from
production and from the public `test-support` feature. Tests through the actual
public API refuse plaintext loopback with that feature enabled.

Both paths explicitly install a static credential provider; anonymous signing
remains disabled and uses empty credentials. The authenticated path supplies
only the owned explicit key/secret/token and enables the maintained
`object_store = 0.12.4` SigV4 implementation. Reader/selection memory owns that
capability; selections may outlive the reader, and callers scope them to their
bounded acquisition. No expiry/rotation mechanism or memory-erasure guarantee
is added.

A small existing-connector extension marks Authorization and security-token
headers sensitive before reqwest receives the signed request. Remote SDK
protocol errors are contained to a fixed safe message because upstream errors
can include echoed bodies/headers. Existing error variants remain available;
raw protocol-error text changes for anonymous callers too. No new logger or
restricted diagnostic store is introduced.

Endpoint/addressing authority, no proxy discovery, no redirects, zero nested
retries, operation/transfer budgets, exact key and VersionId, If-Match/range
checks, verifier, receipt schemas, publication and cold-recovery ownership stay
with their existing owners. Authentication does not change source identity or
receipt identity. No credential data enters the manifest, continuation resource
identity or durable acquisition state. No dependency or feature graph changes.

## Deciding evidence

Nine private authentication unit tests pass on Linux x86_64/Rust 1.92.0:

- Captured HEAD and conditional range GET signatures verify for both addressing
  styles with and without session tokens. The test-only RFC 2104 HMAC oracle
  uses existing SHA-256 and is independently anchored by RFC 4231 case 1. It
  checks AWS canonical-request/key-derivation semantics; wrong secrets and
  key/version/range/If-Match tampering invalidate captured signatures.
- Selections retain their signing capability after reader drop. Session-token
  headers appear only for explicitly supplied tokens; secrets are never sent as
  plaintext request fields.
- Invalid credentials and signing regions return exact configuration errors;
  credential Debug redacts every field. Private fixture transport rejects domain
  names, non-loopback IPs and HTTPS in that plaintext fixture path.
- Signed 302/307/400/403/503 fixtures return contained failures with one request;
  the destination listener observes no followed redirect. Echoed credentials
  and a malformed range-header token never escape Display/Debug or the shared
  error projection. Failed metadata validation writes no output.
- One isolated child test process supplies synthetic ambient AWS credentials,
  metadata/web-identity endpoints, region and proxy variables. Anonymous access
  still has no Authorization/token; signed requests use the explicit credential
  and region. Parent environment is never mutated.
- Anonymous, long-lived-key and session-token selections have equal manifests
  and acquisition resource identities. A signed request traverses the real
  shared acquisition owner, verifies SHA-256 bytes, issues/settles its existing
  receipt, closes both owners and cold-reopens the adopted record. Persisted
  `downloads.json` and receipt/record Debug contain none of the synthetic access
  material. Source sees exactly HEAD plus GET, with no cold source replay.

The two added public-constructor tests cover HTTPS admission and credential-free
origin validation; the unchanged eleven reader oracles remain. All 13 reader
tests and all 41 existing S3 acquisition/import regressions pass, including the
staging and distinct-VersionId successors. The existing loopback fixture setup
is extracted once for reuse; public production authentication is not weakened.

Primary protocol authorities:
[AWS SigV4](https://docs.aws.amazon.com/IAM/latest/UserGuide/reference_sigv-create-signed-request.html),
[RFC 4231](https://www.rfc-editor.org/rfc/rfc4231), and the exact locked
[object_store source](https://docs.rs/crate/object_store/0.12.4/source/src/aws/credential.rs).
These authority reads and local fixtures do not establish live-provider behavior.

## Commands and logs

Setup uses the environment's existing pinned toolchain:

```sh
source /workspace/.pumas-tools/env.sh
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_DEV_INCREMENTAL=false
export CARGO_PROFILE_TEST_DEBUG=0 CARGO_PROFILE_TEST_INCREMENTAL=false
cargo test --locked --offline --manifest-path rust/Cargo.toml -p pumas-library \
  --no-default-features --features s3 --lib acquisition::s3::auth_tests -- --test-threads=1
XDG_CONFIG_HOME=/tmp/pumas-s3-auth-config cargo test --locked --offline \
  --manifest-path rust/Cargo.toml -p pumas-library --no-default-features \
  --features s3,test-support --test s3_reader --test s3_acquisition -- --test-threads=1
cargo clippy --locked --offline --manifest-path rust/Cargo.toml -p pumas-library \
  --no-default-features --features s3,test-support --all-targets -- -D warnings
cargo check --locked --offline --manifest-path rust/Cargo.toml -p pumas-library --no-default-features
cargo fmt --manifest-path rust/Cargo.toml --all -- --check
python3 scripts/release/check-dependency-features.py
git diff --check
```

All listed final commands pass. Unit tests: 9, plus the isolated child test's own
1-test run. Integration: 41 acquisition and 13 reader tests. Clippy is strict;
headless compilation passes; feature checker passes 12 normal/build graphs over
Linux, macOS ARM64 and Windows. Graph evidence does not prove target compilation.

The first integration attempt had 11 passes and 30 setup failures because API
fixtures tried to create `/home/agent/.config/pumas` on a read-only filesystem.
Using a disposable `XDG_CONFIG_HOME` resolves that setup without changing home,
permissions, production source or the frozen tests. Earlier test-authoring
compile errors were corrected before final qualification.

Raw final logs, the initial setup-failure log, Router JSON and push log are
retained in `/workspace/scratch/s3-auth/`, with exact byte counts and SHA-256 in
`log-inventory.json`. Final unit log SHA-256:
`fb4427dfaacf27d03a76e99e0b2154e655c8a1a752549fb3ecd8119ee12309ee`.
Final integration log SHA-256:
`60aed5ca1f1894d5ee9ae0484e049f9e60917d0be19bf29da7b45028c7244851`.
No actual account credentials were obtained, sent or provisioned. Only synthetic
loopback fixture credentials are used. Code milestone push was normal/non-force.

## Remaining scope and next existing-plan feature

No concrete implementation blocker remains for this credential slice. Independent
review and exact-head hosted acceptance remain with the parent. Real AWS S3,
a non-AWS compatible service, MinIO, live credential expiry/refresh, authenticated
TLS-provider acceptance, desktop/RPC source configuration, complete default/full
native suites, and AQ-S3 qualification are not claimed.

Next existing-plan implementation feature: Q3's direct explicit source-facing
application workflow/source configuration, using the existing model-facing
operations and newly explicit access capability. Real-provider acceptance stays
separate and requires authorized resources; native repair remains frozen.
