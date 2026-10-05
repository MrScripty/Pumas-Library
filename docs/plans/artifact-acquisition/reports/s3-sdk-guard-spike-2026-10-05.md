# Single S3 SDK: bounded guard and diagnostic-scope follow-up

This is the coordinator-authorized isolated follow-up to
[the initial suitability spike](s3-single-sdk-spike-2026-10-05.md).
It preserves that checkpoint at `a6dbc3ffa745a2611fb5cd2f01b28b8c9bc1cd87`,
tree `e5726845b720cb1bf9deb7dedca747779006aefe`, and prefix decision
`62a32e12affe45fcaebfe01d74278e3fdce23fc7`.
Branch `spike/s3-sdk-guards-a6dbc3ff` changes only the independent spike and
this report. Production APIs, readers, manifests, lockfiles, credential policy,
retry/verification policy and receipt identities are unchanged.

The two original counterexamples have a small supported adaptation at this
scope. Recommend the single AWS SDK direction for a separately reviewed migration
decision. This does not approve or perform production dependency migration,
implement prefix acquisition, close AQ-S3, or establish real-provider acceptance.

## Listing guard: SDK output remains the protocol owner

The pinned Smithy runtime collects a nonstreaming response into `SdkBody`,
deserializes it, retains its bytes, then runs `read_after_deserialization`.
An interceptor error replaces successful output before `send()` returns. The
spike verifies that ordering using responses the original SDK decoded as complete.
`read_before_deserialization` would be too early for this buffered-body check.

`list_xml.rs` uses maintained `roxmltree =0.21.1`, with DTD parsing disabled,
no external entity resolver and a 4,096-node ceiling. The existing transport
bounds response bytes before the parser; the guard independently checks that
bound. It runs only for ListObjectsV2 successful deserialization, not HEAD or
streaming GetObject. It requires `ListBucketResult`, either the standard S3
namespace or the existing compatible fixture's absent namespace, exactly one
direct `IsTruncated`, at most one direct `NextContinuationToken`, and consistent
completion/token evidence. Its completion values must agree with SDK typed output.

Relevant direct selection fields are singleton, and each Contents entry has one
unambiguous Key and at most one Size/ETag. Nested element content and split text
in these exact-value fields are refused rather than interpreted differently from
the SDK. This is a bounded structural/completion guard, not a universal S3 schema
validator. The SDK still owns S3 deserialization, signing, request encoding,
typed output and one existing reqwest transport. There is no second protocol
stack, SDK fork, custom signer or network/security bypass.

Wire probes now reject the original unterminated and duplicate-IsTruncated bodies,
wrong root/namespace, duplicate tokens/prefix/key/size/ETag, entity-bearing DTD,
missing/contradictory completion and oversized/node-overflow responses. Valid
namespaced XML with an escaped opaque token preserves the SDK's decoded value.

## Diagnostic scope: access-key identifier versus secret material

The original reproduction observes an **access-key ID** in deliberately enabled
SDK TRACE signing output. It did not observe secret-key or session-token leakage
in that happy-path trace and is not evidence of credential compromise. However,
Pumas's request-scoped diagnostic contract still excludes credential identifiers
and source-reflected credential values from its logs.

Supported `WithSubscriber`/`NoSubscriber` wraps the complete SDK request future.
For ranges, its scope includes every body poll and the body verification future;
the operation timeout remains unchanged. Ordinary Pumas status events are emitted
outside that future. The adapter does not change process-global logging.
SDK errors are mapped inside the adapter to bounded static operation diagnostics;
raw SDK responses/errors are not emitted by the caller.

A fresh-process fixture deliberately installs global TRACE only in that disposable
test process. An unscoped HEAD first reproduces access-key-ID disclosure; the
captured memory buffer is then cleared. Scoped TLS HEAD, conditional range GET,
valid listing, reflected 403 errors for all three operations and malformed 200
listing errors leave access key, secret and session token absent from captured
logs while safe outer `selecting`/`settled` statuses remain visible. Captured
traces stay in memory and are never printed or written to an artifact. This
qualifies the tested construction/request/body/diagnostic path, not arbitrary
future call sites or independently spawned instrumentation.

## Actual dependency/build cost and verification

The candidate remains AWS SDK S3 1.137.0 on actual Rust 1.92.0, with default SDK
features disabled and `rt-tokio`/`http-1x` enabled. The follow-up lock adds only
roxmltree 0.21.1; its memchr dependency was already present. Its declared Rust
minimum is 1.60 and license is `MIT OR Apache-2.0`; registry checksum is
`f1964b10c76125c36f8afe190065a4bf9a87bf324842c05701330bba9f1cacbb`.

Under the isolated fixture's feature unification, the SDK plus parser normal/build
closure has 134 package versions and the complete host graph has 208. Against
the unchanged full production lock, that closure has 43 new package names and
52 different/new versions. These gross facts are not a resolved production
migration cost after removing the incumbent. No `aws-config`, `object_store`,
default AWS HTTP client, `ort` or `ort-sys` occurs in this spike graph. All closure
packages retain license expressions.

The original actually compiled SDK candidate's single-worker Linux observations
remain relevant: approximately 87–104 seconds and 1,330,984 KiB largest compiler
child RSS for the recorded builds. They are not platform-wide budgets. A final
warm follow-up test invocation took 1.188 seconds, largest child RSS 61,360 KiB;
this includes fixtures and is neither a cold compiler measurement nor production
acquisition-memory acceptance. The parser adds a bounded DOM while the SDK-owned
response remains retained; no exact peak-memory claim is made for that combination.

Final locked tests: **15 passed, 0 failed**, with two ignored child entry points
both explicitly executed and checked by their parent tests. Scoped all-target
Clippy with `-D warnings`, formatting and diff checks passed. The first guard
compile used the wrong BoxError import; it was corrected to the maintained
runtime API export before successful qualification. No protocol or policy was
relaxed to get a passing result.

Commands remain the standalone manifest's locked test, all-target Clippy and fmt
checks from the prior report. Evidence is under `/workspace/scratch/s3-sdk-spike/`:
`guards-build.log`, `guards-final-probes.log`, `guards-clippy.log`,
`guards-measured-final.log`, host metadata and dependency-impact inventories.
Final measured test-log SHA-256:
`33f89fdeadb2658c8283aa049eea38478f5b2c5e1af8a580a80a3db34119024f`.

## Proposed bounded migration sequence; no production change here

1. After coordinator review, replace the incumbent S3 protocol owner in one
   reader slice. Resolve and inspect the actual production lock/feature/target
   cost before committing dependencies; remove the incumbent rather than keep a
   fallback. Preserve `S3ReaderConfig`/anonymous construction and the distinct
   explicit-authenticated path. Map the existing operation budget into the pool,
   SDK settings and outer owner rather than promote this fixture's five-second
   value to production policy. Keep HTTPS, endpoint/origin/proxy/redirect controls,
   no ambient credentials, disabled SDK retries, exact range verification and
   stable source/receipt identity. Keep construction and all SDK/body polls in the
   proven diagnostic scope. Requalify the existing reader/authentication and
   actual consumer/receipt tests before proposing integration.
2. Implement Q3/AC13 prefix selection through that one owner. Use explicit page,
   object, XML-byte/node, total-work and elapsed budgets; read one page at a time;
   preserve the raw prefix. Reject empty/cyclic continuation tokens, duplicate
   keys and out-of-prefix entries. Sort/pin deterministic object identities,
   resolve each object through HEAD and immutable VersionId selection, and use
   the existing verifier and explicit file-set handoff. No listing or partial
   enumeration authorizes a complete package. Successful complete enumeration
   is not an atomic multi-object snapshot. ListObjectsV2 alone does not supply
   immutable object versions; that HEAD/pinning step must be implemented.
3. Qualify the composed prefix-to-model path and its exact receipts, then leave
   AWS/non-AWS/MinIO, supported native targets and real desktop acceptance with
   their existing coordinator-owned gates. Multi-file model-format capability
   must follow its actual importer contract rather than be inferred from listing.

The practical Q1 mixed regular/LFS HF feature proceeds independently on its own
branch. R1/R2 gate readiness and the publisher-owned PR41 gates are unaffected.
