# Single AWS S3 SDK owner: isolated suitability spike

Date: 2026-10-05. Result: the candidate compiles and represents the required
read/control fields, but is **not suitable for production migration as tested**.
Two reproduced failures concern credential logging and malformed listing
completion. Passing reproduction probes does not make those failures acceptable.

## Scope and preserved state

The coordinator authorized an isolated single-SDK evaluation after the
[prefix decision](s3-prefix-discovery-decision-2026-10-05.md). Branch
`spike/s3-sdk-owner-62a32e12` starts at
`62a32e12affe45fcaebfe01d74278e3fdce23fc7`, tree
`52cfb3df2d44f5a9607542eceb40eee2bab29086`. That checkpoint and AC10
`d56b2b91b7d0d4cc38627e6eab541b937ef5f027` remain unchanged.

The standalone crate at `rust/spikes/s3-sdk-owner/` declares its own workspace,
manifest and lockfile and is not a production workspace member. Its suitability
adapter and network fixtures exist only under `cfg(test)`. No production manifest,
lockfile, public API, source reader, importer, watcher or repaired S3 manifest
changed. No second S3 protocol client is present in the spike graph; `object_store`,
`aws-config`, `ort`, `ort-sys` and the default AWS HTTPS client are absent.

All source calls use owned loopback fixtures and synthetic in-memory S3
credentials. TLS fixture certificates and signing keys are generated in memory.
No real provider/account, provisioning, credentials, paid service, ONNX download,
security bypass, upstream contact or hosted retry was used. Real-provider and
native-platform acceptance remain separate.

## Actual compilation, resolution and resource observations

The candidate is `aws-sdk-s3 =1.137.0`, default features disabled, with `rt-tokio`
and `http-1x`. Direct companion declarations are `aws-credential-types =1.2.14`,
`aws-smithy-runtime-api =1.12.3` and `aws-smithy-types =1.5.0`; Cargo.lock records
the complete resolved graph and registry checksums. SDK archive checksum and
upstream provenance are recorded in the prior decision report.

Both the minimal SDK build and the final fixture-backed build compiled on actual
Rust/Cargo 1.92.0. The isolated workspace uses resolver 3 and declares Rust 1.92;
the maximum declared Rust minimum in its inspected host graph is 1.92. This does
not prove resolution for independent library consumers using another resolver.
The newer S3 SDK release inspected in the prior decision requires Rust 1.94.1;
no toolchain upgrade was made.

The final isolated lock was seeded from the unchanged production lock before
resolving the spike. Under the spike's dev-feature unification, its SDK
normal/build closure contains 133 package versions, including the SDK itself;
42 package names and 51 package versions in that closure are absent from the
existing full production lock. The whole host graph with fixture dependencies
contains 207 package versions. These are gross comparison facts, not a resolved
production migration or net cost after removing the incumbent. Seeding retained
existing compatible versions rather than attributing an independent fresh
resolver's unrelated updates to the SDK.

All 133 runtime/build package metadata entries have license expressions. The
recorded expressions use MIT, Apache-2.0, Unicode-3.0, BSL-1.0, Unlicense and Zlib
alternatives/conjunctions. The AWS SDK/Smithy crates declare Apache-2.0. Fixture
TLS and transport dependencies are separately visible in the standalone manifest
and host inventory; this is license/provenance evidence, not a legal approval.

On this Linux 6.18.44 x86_64 container, with one Cargo build worker, dev/test
debug information and incremental compilation disabled:

| Invocation | Elapsed | Largest child maximum RSS |
| --- | --- | --- |
| Minimal SDK build, no-run test target | 86.93 s | 1,324,716 KiB |
| Production-lock-seeded fixture build and probes | 104.45 s | 1,330,984 KiB |
| Scoped all-target Clippy | 62.44 s | 1,071,808 KiB |

These single observations include normal Cargo dependency work and use different
cache/graph states. They are not a baseline performance comparison or acceptance
budget. RSS is `getrusage(RUSAGE_CHILDREN).ru_maxrss`, the largest child process,
not aggregate system memory or acquisition runtime memory. The shared spike
target occupied approximately 1.4 GiB after multiple graph/build/check variants;
that cumulative directory size is not one build's artifact size. Host facts were
Python 3.12.14, OpenSSL 3.5.7 and a 16-GiB cgroup memory limit. AC10's earlier
streaming measurements are not generalized by this experiment.

## What the adapter and wire probes establish

The SDK delegates HTTP to the existing maintained reqwest version, `=0.12.28`,
through Smithy's supported transport traits. The adapter supplies one pooled
client with explicit five-second request/connect bounds, proxy discovery off,
redirects/retries/compression off and an immutable receiving origin. SDK retry
configuration is explicitly disabled. Its behavior version is pinned to
`v2026_01_12`; its default connect-timeout setting is overridden to match the
explicit pool, and no independent idle-read timeout is introduced. The complete
selection/range prototype retains an outer five-second bound.

A fixed endpoint resolver preserves either the bucket path or the already
bucket-bearing virtual endpoint and refuses another bucket. The SDK owns key,
version and query encoding and SigV4 signing. An explicit, redacted in-memory
credential provider is supplied only for authenticated requests. Anonymous
construction omits that provider and enables the maintained no-auth scheme;
an always-present provider returning “not loaded” did not select anonymous auth
in the first experiment. No ambient configuration loader is used, S3 Express
session auth and multi-region access points are disabled, and the identity cache
is disabled. These choices do not weaken authenticated production HTTPS rules;
plaintext signed fixtures require explicit literal-loopback authority in test-only
code. Untrusted TLS certificates remain rejected.

The final probes observe:

- Anonymous, access-key/secret, and optional-token HEAD/range reads under both
  addressing styles, correct encoding of exact key and VersionId, conditional
  range headers and independent SigV4 verification anchored by RFC 4231.
- Equal version/size/opaque-validator selection evidence across authentication
  modes; changed versions, validators, totals, ranges, duplicate Content-Range
  and actual short/extra body counts are refused. Chunked ranges retain actual
  count verification without adding a Content-Length requirement.
- Correct virtual-host origin, explicit certificate trust, and refusal to use
  another bucket. A fresh child process proves supplied ambient AWS/proxy
  settings do not authorize or reroute the requests.
- No redirect, other-region following or retry for 301/302/307/308/403/500/503.
  The exposed adapter errors remain bounded static diagnostics even when the
  source error message echoes synthetic credentials.
- Streaming GetObject returns before the full body exists; dropping an incomplete
  body closes the owned connection and the fixture task is joined within its bound.
- ListObjectsV2 preserves the raw prefix without appending `/`, requested page
  size and encoded continuation token. The adapter can distinguish true/false/
  missing completion flags from present/absent/empty next tokens and reject
  contradictory completion evidence. Invalid scalar and oversized responses fail.

No complete prefix acquisition, discovery-to-HEAD race policy, source digest
authority, receipt construction, cache/store persistence or model/native consumer
handoff was implemented or accepted. Existing production receipt/schema code is
unchanged, but the spike is not receipt equivalence qualification for a future
migration. Windows/macOS compilation and provider acceptance were not performed.

## Reproduced migration blockers

**Credential trace exposure:** `probe_sdk_trace_exposes_access_key_id_before_transport_redaction`
captures TRACE events only in memory and reproduces the supplied access-key ID
in the SDK signing trace. The secret and session token are absent in that happy-path
probe. `aws-credential-types` includes the access-key ID in `Credentials::Debug`;
`aws-sigv4` traces signing parameters containing that identity before the
transport sees the request. Redacting the provider wrapper and request headers
does not close this earlier exposure. The capture is never written to disk or
printed; the report records the result, not credential-bearing trace contents.

**Malformed XML completion:** `probe_unclosed_and_duplicate_completion_xml_are_accepted_by_sdk`
reproduces successful decoding of these two bodies:

```xml
<ListBucketResult><IsTruncated>false</IsTruncated>
```

```xml
<ListBucketResult><IsTruncated>true</IsTruncated><IsTruncated>false</IsTruncated></ListBucketResult>
```

Both yield `is_truncated = Some(false)` and no next token. The typed output alone
cannot distinguish either invalid response from a complete empty page. A bounded
HTTP response stream limits bytes but does not provide that structural evidence.
This matters because [AWS explicitly requires handling invalid XML even on a
successful ListObjectsV2 response](https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListObjectsV2.html).
No local XML parser, fork, tracing filter or fallback SDK was introduced to hide
these counterexamples.

## Verification, failures and recommendation

Final locked tests: 13 passed, 0 failed, plus one ignored child fixture that the
parent probe explicitly executes and checks for one successful test. The probes
include expected incompatibility reproductions; a green test run is **not** a
green suitability decision. Scoped all-target Clippy with `-D warnings`, formatting,
host metadata/feature inventory and production-file isolation checks passed.

Commands from repository root:

```bash
cargo test --locked --manifest-path rust/spikes/s3-sdk-owner/Cargo.toml
cargo clippy --locked --manifest-path rust/spikes/s3-sdk-owner/Cargo.toml --all-targets -- -D warnings
cargo fmt --manifest-path rust/spikes/s3-sdk-owner/Cargo.toml -- --check
```

Initial failures exposed prototype API namespace/lifetime errors, the SDK's
default timeout settings, anonymous-provider selection, unsorted wire-query order
in the independent signature oracle, and the invalid-XML acceptance above. API,
configuration and oracle errors were corrected; the SDK failures were preserved
as explicitly named counterexamples. A host-filtered metadata query avoided
unnecessary uninstalled foreign-target artifacts. `/usr/bin/time` was unavailable;
the Python standard-library runner recorded elapsed time and child resource usage.
Logs, resolved metadata, feature/license inventories, lock comparisons and checks
remain under `/workspace/scratch/s3-sdk-spike/` with SHA-256 inventories.

Recommend **no production migration at this checkpoint**. One SDK remains a
coherent protocol-owner direction, but a maintained strict completion path and
an approved request-scoped logging/redaction mechanism must be demonstrated first.
Neither is represented by the inspected typed fields alone. Choosing an upstream
repair, a maintained validator at the source boundary or a telemetry adaptation
requires an explicit mechanism/maintenance decision; this spike selects none.
The existing bounded prefix-discovery feature remains pending. A second SDK,
toolchain upgrade, unapproved local protocol parser or relaxed credential policy
is not an automatic next step.

## PR41 Windows gate disposition

The unrelated Windows failure was investigated first. The managed-Python fixture
at PR41 `26a84e323cae566a46a8f76bef48fa1010aed48b` is byte-identical to approved
main `838eb2990905144a59830f1a16fe91b4e1105d4d`, blob
`ee8f3c21f5cbeeabf775a3d5bfa90612099f3d33`. Both descendant tests passed in an
earlier invocation of the same named test executable, then failed only at the
10-second startup-marker wait under the broader `native_` filter. Neither reached
the cancellation or timeout/drain assertions. This does not establish a PR41
production regression, and the hidden PowerShell/marker outcome prevents assigning
a precise host/fixture cause from that log alone.

The publisher owns the unchanged-head Windows retry, job `112003112044` in run
`37378173012`; no duplicate retry or fixture change was made. At the recorded
inspection, the managed-Python subset had passed and the previously failing broad
filter was pending. Any repeated failure needs startup/provider-task diagnostics
and owned failure-path drainage with the existing bounds/assertions preserved,
not a blind timeout increase. The focused Markdown diagnosis and decoded initial
job evidence remain under `/workspace/scratch/pr41-windows-fixture/`.
