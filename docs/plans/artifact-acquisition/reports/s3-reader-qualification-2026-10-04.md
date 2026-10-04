# Optional S3 reader milestone

Base: `c70a78f7232f46dfb3a6b73db8bcc75fa9293acf`, tree
`4a63335d0ff5f2399ebcff8128839b138cdd5337`.
Branch: `feat/acquisition-s3-reader-c70a78f7`. The coordinator owns PR creation,
independent review, integration, and final gate decisions.

This records the reader candidate `2c7d6014658ef44575f3724f0ff6e7395f1fd7d8`.
Its later [dispatch/import successor](s3-dispatch-import-qualification-2026-10-04.md)
implements shared service use and one-file GGUF import; those claims do not
retroactively extend this reader-only evidence. The optional license inventory
is refreshed for the successor's manifest/lock inputs, retaining exact texts.

**Status: implemented reader milestone; independent review and hosted
qualification pending.** The coordinator resumed this slice after the separately
committed PR37 repair. Addressing style now participates in stable source identity;
the same-endpoint/key fixture observes distinct physical request paths and
identities, and stable identity across repeated equivalent reader construction.
All 11 local protocol tests and strict headless all-target Clippy passed on this
source. The complete S3 acquisition/import workflow and AQ-S3 remain open.

## Implemented boundary

The explicitly enabled `s3` feature exposes an anonymous protocol reader for one
caller-selected versioned object. Configuration owns endpoint, region, bucket,
addressing style, HTTP admission, and the complete operation budget. Construction
does not discover credentials, proxies, or endpoint settings from the environment.
Redirects, SDK retries, and reqwest's default protocol-NACK retry policy are
explicitly disabled. The maintained dependency owns S3 URL,
version, condition, range, and response parsing; Pumas adds no signer or XML parser.

Selection validates exact remote-key identity and the portable logical path,
requests the selected VersionId with HEAD, and requires matching returned version
evidence and a strong ETag. Mutable `null` versions are unavailable to this reader.
Addressing style and losslessly hex-encoded endpoint/bucket/key fields form the
existing stable manifest identity; VersionId is immutable revision evidence. SHA-256 is supplied
as declared evidence and is never inferred from ETag.

Range reads request VersionId and If-Match together. They check returned version,
validator, total size, and exact range before exposing bytes, then stream into
caller-owned staging under the operation budget. Short or excess bodies fail.
Dropping the future stops polling it; partial writes remain with their caller.
The adapter creates no runtime, acquisition tasks, store, verifier, retry policy,
or publisher. SDK/client connection internals remain dependency-owned.
Shared-service source dispatch and model import are not implemented in this slice.

## Dependency decision and attribution

Admitted direct dependency: `object_store = =0.12.4`, `default-features = false`,
features `aws`, optional in `pumas-library` only. Its declared license is
`MIT OR Apache-2.0`; the exact downloaded package supplies `LICENSE.txt` and
`NOTICE.txt` for the Apache license. The checksum is retained in `Cargo.lock`.

Comparison inspected exact upstream crate sources for 0.12.4 and 0.14.2.
0.12.4 uses the existing reqwest 0.12 family and ring backend; 0.14.2 moves to
reqwest 0.13 and its `aws` feature adds AWS-LC. Both fit the installed Rust 1.92
toolchain. The bounded optional reader uses 0.12.4 to preserve the existing HTTP
family and avoid another client/native-crypto family. This is a compatibility
decision for the observed reader contract, not a security audit or a promise that
all authenticated/refreshing-credential requirements have been qualified.
The core crate declares reqwest's minimum tested version, 0.12.28, locally because
this consumer uses the explicit retry-policy API. Its existing JSON/stream and
default TLS features remain enabled; other workspace consumers retain their
existing requirement. The locked reqwest version and dependency graph are unchanged.
The exact APIs support the admitted anonymous/version/range contract locally;
the broader Q3 credential and provider matrix remains open.

No existing locked package identity was removed or upgraded. Cargo's unrelated
compatible Windows-edge reselections were restored to their admitted lock edges;
locked metadata accepted the preserved graph. New optional dependencies and
feature-dependent dependency edges remain explicitly visible in the lock diff.

The canonical `scripts/release/generate-notices.py` ran unchanged and collected
374 default-release notice entries; its checker passed. Its first invocation
failed because this isolated worktree lacked frontend dependencies. Reusing the
existing installation after byte-comparing both package manifests and the pnpm
lock resolved that setup issue without an install or duplicate dependency tree.
The default release omits the optional SDK, so its notice text is unchanged.

The [optional license inventory](s3-reader-license-inventory.json) separately
records all 276 registry packages in the normal/build closure for the three
desktop target graphs. Existing notices are referenced by exact package identity;
15 additional entries retain their exact license/NOTICE texts in
[supplemental notices](s3-reader-third-party-notices.txt). Collection reused the
canonical generator's unchanged license-discovery function. The inventory records
locked crate checksums, exact text hashes, input hashes, and graph commands.
Graph collection establishes neither cross-platform compilation nor distribution
qualification. Sources: [pinned crate](https://crates.io/crates/object_store/0.12.4)
and [AWS GetObject contract](https://docs.aws.amazon.com/AmazonS3/latest/API/API_GetObject.html).

## Local evidence and limits

On Linux x86_64, `cargo test --locked --offline --manifest-path rust/Cargo.toml
-p pumas-library --no-default-features --features s3 --test s3_reader --
--test-threads=1` passed 11 tests. The local fixture checks both addressing styles,
literal encoded key/version requests, exact range bytes, selected manifest
evidence, missing/different version or validator, invalid selection/ranges,
conditional failure, changed size/range, truncated/excess/ignored-range bodies,
redirect/retry refusal, unfinished-body cancellation, and destination backpressure
within the complete operation budget, plus distinct addressing identities at the
same endpoint/key and stable identity across equivalent reader construction.
The source stays reachable until requests
finish, so extra retries or followed redirects are observable. Fixture tasks are
joined and aborted on early test failure.

The initial full unit-test link exhausted this environment's disk and returned a
linker bus error; it produced no test result. Removing only this task's failed
object outputs and using a separate integration-test target allowed the focused
suite to pass. Shared targets were reused; unrelated build caches and worktrees
were preserved. Root-package test debug/incremental output was disabled and
debuginfo stripped through command-local Cargo profile overrides to fit disk.
After PR37's separate qualification finished, its task-created executable cache
was retired (source commit/branch and saved evidence retained) to fit this resumed
reader build. No unrelated cache or worktree was removed.

Strict all-target `pumas-library` Clippy passed with `--no-default-features
--features s3,test-support -- -D warnings`; root dev debug/incremental output was
disabled with command-local profile overrides. Rustfmt and authored-source diff
checks passed. The unrestricted staged diff check flags CRLF, trailing whitespace,
and a final separator in the supplemental notices, which preserve exact upstream
license text. That text was not normalized to silence the check.
The headless compile check passed using `cargo check --locked --offline --manifest-path
rust/Cargo.toml -p pumas-library --no-default-features` with the same bounded root
dev profile overrides. Test-only reverse-range syntax was expressed as a `Range`
struct to satisfy Clippy while preserving the invalid-range oracle.

The repository dependency-feature checker passed all 12 default/headless/fixture
graphs across Linux, macOS ARM64, and Windows x86_64. Separate S3 graph checks
observed no SDK in the six default/headless core graphs. These are graph checks,
not native builds or full suites. No live AWS/non-AWS/MinIO, authenticated source,
Torch download, actual model import, default ONNX compilation, packaged consumer,
resource benchmark, hosted exact-head CI, or independent source review ran.
The local HTTP/1.1 fixture does not exercise HTTP/2 NACKs, TLS trust or public DNS;
the explicit reqwest retry policy was verified against the locked 0.12.28 API.
AQ-S3 remains not ready. The next admitted S3 slice must compose source dispatch
with the existing acquisition owner before claiming an import workflow.
