# Composed S3 qualification — 2026-10-05

Branch: `qualification/s3-composed-e6acb0ba`. This is a review candidate;
the coordinator owns PRs, review, hosted checks and integration. No release tag,
provider account, real credential, external review request or Library write was made.

## Exact composition and ownership

Normal merge `cd9d81917c99f1a2e8b0afcb628ee6d93dfa8577`, tree
`f3b51564cf7765df4f4ad673832423f7e1a63deb`, has these **ordered parents**:

1. Empty-member sibling `e6acb0ba5ec4b57dcf25e56a57489fd948f1fee3`, tree
   `8aa631e21d54aa04acd8c62d574c5afe61fdcfbf`.
2. Reserved-destination sibling `7dc6aceff52fd0d8b563ecedd3c5acbaf7ee4dc8`, tree
   `249f7308659c69748ef05d71df26cb8a65e8956a`.

Both frozen refs remain unchanged. Their frozen common base is
`e8082649a6d4aaebf996c30870d996d43bbee1d3`, tree
`050733c55a3011f23c8832e210d52d0d4d60d695`. Accepted PR40 composition
`d9d907cfe0e57b1e2bc7e296eff1327de835bc5e` remains an ancestor, with original
ordered parents `339032ff4acb53e43aa36c6edf96739101121620` and accepted main
`838eb2990905144a59830f1a16fe91b4e1105d4d` (tree
`f6d6c1d3c5ba5be0fb998a6fd157fcb31c63dc72`). Original PR40 `63123fd8` and
former main `05717338` remain ancestors. No refs were rewritten or force pushed.

The normal merge combined production and test changes without code conflicts.
Five documentation conflicts retain both histories and combine the namespace
preflight rule with known-zero support. Qualification successor
`7358bd3dd6825d17e329cc157f079002632a23c1`, tree
`1626cd89e0c05e17c74fa7c7fb547442e82a8754`, adds only the independent installed
client; Rust, Cargo, Electron and renderer inputs are identical to the merge.
The final documentation head/tree and remote verification are recorded in
`/workspace/scratch/s3-composed-qualification/final-state.json`.

PR40 watcher/importer/manifest and publication-protocol implementation stays
byte-identical to the frozen common base. Its S3 integration entry preserves
every original test body and adds only the empty-member module declaration.
`repair-preservation.json` records the checked paths. The sibling staging helper
change is separate from the unchanged publication protocol tests.

Public API changes are exactly the sibling changes: additive pure
`ModelImporter::validate_acquired_payload_paths(&[&str]) -> Result<()>`, reused
before native/RPC source resolution; known-zero selection support within
`S3ObjectSelection::open_acquisition`, with no new DTO, schema or reader API.
Anonymous constructors and distinct explicit authenticated constructors remain
compatible. No credential, endpoint, proxy, redirect, identity, retry, deadline,
verification, receipt, persistence or production HTTP rule changes are introduced
by composition or qualification. See the two sibling reports and shared contract.

## Provider and platform evidence matrix

| Scope | Observed result | What it proves and what remains open |
| --- | --- | --- |
| Controlled Rust HTTP/TLS protocol fixtures | Pass: 64 S3 integration tests and 220 RPC tests; RPC includes 14 HTTPS scenarios | Combined empty-member and reserved-path behavior through acquisition/import/registration; fixed pins, ranges, cancellation, receipts and secret boundaries. Fixtures are not actual S3 services. Twelve existing live/intent RPC cases remain ignored. |
| Installed Linux x86_64 production S3 backend | Pass: 10 independent Python/TLS scenarios, plus standard release health smoke | Normal release build, no test-support, copied into a qualification archive, extracted and executed outside checkout. Anonymous single/bundle; authenticated bundle with and without token; wrong empty digest, missing object, unknown length, truncated nonempty primary, cancellation and empty primary. No inference or shipping-installer qualification claim. |
| Native Linux filesystem/process owners | Pass: 45 native tests (one child marker ignored in the parent inventory) and eight direct llama.cpp installer tests | Synthetic executable installation, exact receipts, cancellation, repeated shutdown, cold reconciliation and real SIGKILL interruption without source replay. Test-support build and synthetic launcher; not an official llama.cpp runtime or an inference run. |
| Renderer DOM and Electron Node tests | Pass: 14 focused renderer tests and 242 Node tests; one existing native sandbox smoke skip | Form/transport/secret-clearing and lifecycle logic. DOM/Node evidence does not qualify an actual browser or packaged desktop session. |
| Supported browser/desktop session | Blocked before page load | Fresh sandbox-enabled Chromium launch aborts: `/usr/lib/chromium/chrome-sandbox` is owned by `nobody:nogroup`, mode 4755, but Chromium requires root ownership. No sandbox-disable flag or permission change attempted. |
| Actual local MinIO service | Blocked during ordinary official source installation; no server executed | No preinstalled MinIO binary or Docker image. Official Go 1.27.1 installation and archive SHA verification succeeded. Official MinIO module ZIP download returned HTTP 403. Installation stopped there; no alternate registry, direct-download fallback or proxy bypass. |
| AWS S3 | Not run | Requires separate authorized provider fixture/environment; controlled protocol evidence is not AWS acceptance. |
| Independent non-AWS S3 provider | Not run | Requires a named authorized implementation/version and fixture/environment. |
| Windows x86_64 / macOS arm64 and shipping desktop installers | Not run | Require supported hosts, exact candidate packages and supported sandbox/UI environment. No cross-platform conclusion from Linux. |
| Default/full inference and external client/deployment disposition | Not qualified | This candidate explicitly enables S3 and disables default inference. Existing release no-inference build commands omit S3; this archive is an unreleased qualification artifact, not the release candidate cohort. External old-writer/client facts and full inference gates remain open. |

## Installed production proof

Build command:

```text
cargo build --locked --offline --manifest-path rust/Cargo.toml -p pumas-rpc --release --no-default-features --features s3
python scripts/release/qualify-s3-installed.py --binary rust/target/release/pumas-rpc --output /workspace/scratch/s3-composed-qualification/installed-qualified
python scripts/release/smoke-rpc.py /workspace/scratch/s3-composed-qualification/installed-smoke/pumas-rpc --inference-disabled
```

The repository release profile is unchanged: LTO, one codegen unit, stripped
debug information. Cargo's actual release fingerprints show RPC `["s3"]` and
core `["hf-client", "s3"]`, no test-support and no Rust flag overrides.
Rust/Cargo 1.92.0 and dynamic linkage are recorded in `release-toolchain.log`;
`production-build.json` binds source head/tree, command, fingerprints and bytes.
The initial optimized build took 13m49s; the final exact-head cache check passed.

Binary SHA-256:
`f23bbf56e22e7e15debfd176549827e72613cf7cf8693017837c26e5d024bb2a`.
Qualification archive SHA-256:
`cbb2caeaa8b7af27454fbb4417371fa3445899296d52ff75c86196d8d9fd6447`.
The archive contains the backend, project license, release/S3 notices and safe
qualification metadata. It contains no credentials, model fixture, runtime state
or user data. The executable SHA is checked after extraction.

The Python client uses only the standard library and repo synthetic TLS fixture.
It trusts that fixture CA in the child environment, with normal HTTPS validation;
there is no system trust modification or TLS validation bypass. Synthetic
access/secret/token are memory-only. All three ambient AWS variables are replaced
with unselected synthetic values; anonymous requests still have no authorization
or session-token header. Authenticated HEAD/GET signatures are independently
recomputed with standard-library HMAC-SHA256, and the wrong-secret negative
oracle must fail. A supplied token must be among the signed headers.

Every bundle case first rejects reserved roots, aliases and descendants before
source I/O or workspace creation, then admits a corrected same-owner UUID.
Both versions use the same key to exercise identity separation. Empty auxiliary
files issue HEAD but no GET, carry verified zero-byte/empty-SHA receipts and
contribute one acquired file and zero acquired bytes. Truncated primary reads
resume at the existing bounded offsets before failure. Cancellation closes the
source body before drained exit. Successful imports preserve exact publication
receipts and registration through cold restart without source replay. Invalid
sources retain cleanup custody and reject implicit replay. Empty primary fails
model validation and returns the existing truthful drained shutdown failure.

The 24-byte GGUF is a minimal synthetic parser fixture with no tensors; it is
not usable model weights. This proves model import/index/proof, not inference.
Backend debug output is bounded and scanned in memory, never retained raw;
safe responses and all owned runtime files are scanned for both explicit and
unselected synthetic credentials before cleanup. Retained results contain safe
method/version/count facts, never authorization headers or credential values.

## Checks, logs and missing acceptance inputs

`/workspace/scratch/s3-composed-qualification/verification.json` records log
hashes, result inventory and qualified heads. Strict core Clippy passes; scoped
RPC Clippy passes with the previously recorded inherited `dead_code` exception.
Formatting and Python compilation pass. The raw evidence directory also retains
the production build/feature-tree logs, installed result/archive, native/renderer/
Node results and sanitized browser/MinIO failure logs.

Official [MinIO source installation instructions](https://github.com/minio/minio#source-only-distribution)
resolve to module `v0.0.0-20260212201848-7aac2a2c5b7c`. With only the official
Go proxy configured (no direct fallback), the module ZIP at the official GCS
proxy returned Forbidden. Its public signed download query is redacted in the
retained log. The verified official Go archive SHA-256 was
`63d339f0da5ab53635a56f2490a7984dfe12dfcff22ad749f63edaf590168445`.

The exact external inputs for the still-open work are:

- A supported sandbox-enabled browser/Electron environment with correctly
  installed sandbox helper or supported user namespaces, plus the candidate UI
  and backend; no sandbox or OS-security bypass.
- For local MinIO, approved access to its official source ZIP/dependencies or
  a preinstalled, versioned official service with TLS and an owned synthetic
  fixture. No real account credentials are needed for that fixture.
- For AWS and the named independent provider, a separately authorized test
  environment identifying endpoint, region, addressing mode, implementation,
  version-enabled bucket, exact VersionIds/digests and representative licensed
  model set. Its owner must separately supply scoped read-only authorization
  and optional valid temporary-token/expiry facts; no credentials are requested,
  obtained or used by this task, and no grants/buckets are provisioned here.
- Supported Windows/macOS hosts and exact release packages; release feature
  selection, actual inference runtime/model and independent client/old-writer
  deployment facts must be dispositioned separately.

AC13/AC14 and AQ-S3 remain pending: refresh/rotation, prefix discovery, actual
provider behavior and required real workflow are not certified by these fixtures.
AC17/Q4 remains pending for the full installed/native/UI/platform scope. The next
existing-plan work is Q3 actual-provider acceptance when those inputs exist,
followed by Q4 shipping-artifact and supported-platform qualification. This task
does not invent a new feature or advance those gates automatically.
