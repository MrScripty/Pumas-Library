# Production S3 SDK reader replacement qualification

The isolated replacement branch `feat/s3-sdk-reader-493b935c` starts at
`493b935c6a41d4d4aeca8e8a66f10b4aba114365`, tree
`ecb51f20c0740fa7d88e6d0eface06c45feb2e07`: the composed authenticated reader,
including accepted PR40 discovery/VersionId repairs. It preserves the reviewed
spike `ba1a9d908010f23f2391059cb93bfe61dcca6aa1`, tree
`fee4b11a76306bcf224abbd8b11811e329ca0483`, as an independent checkpoint.
HF and AC08 successors remain separate branches. Parent owns PRs, reviews and
integration; this is local synthetic qualification, not live-provider acceptance.

## Result and scope

Production uses one maintained AWS SDK protocol/credential owner over the
existing reqwest client pool. `object_store` and its connector are removed with
no fallback. Public `S3ReaderConfig`, `S3Reader::new`, `S3Credentials::new` and
`S3Reader::new_authenticated`, selection/range/manifest methods and consumer/RPC
contracts keep their signatures. Anonymous construction supplies no credentials;
authenticated construction consumes explicit in-memory credentials with optional
session token. HTTPS remains mandatory in production. Literal-loopback plaintext
credential transport exists only in private unit-test builds.

The fixed endpoint resolver checks the configured bucket, and transport checks
origin, GET/HEAD and empty request body. Existing no-proxy, no-redirect, no-HTTP-
retry configuration remains. SDK retries, identity caching, S3 Express session
authentication and multi-region access points are disabled. SDK stalled-stream
protection is also disabled so its default five-second grace period cannot
override the caller's sole operation/transfer budget. There is no ambient
provider, refresh, credential persistence or anonymous fallback.

HEAD checks exact VersionId, strong ETag and known nonnegative size. GET retains
VersionId plus If-Match, requires a partial response, checks full size and exact
half-open range before writes, and retains body length/destination/operation
budgets. GET carries the SDK-standard `x-id=GetObject` query marker alongside
the same immutable VersionId; it does not alter artifact or continuation identity.
The signature oracle sorts encoded query pairs as SigV4 requires rather than
assuming wire order. Empty members keep the existing HEAD-only protocol plus shared verifier
and receipt path. Source identities remain the same endpoint/addressing/bucket/
key/VersionId tuple and exclude credentials. The private key wrapper retains the
previous exact-key segment rules so frozen `acquisition/s3/manifest.rs` stays
byte-identical; no importer/watcher/native repair owner changes.

SDK construction, entire operation futures and each body poll use a scoped
NoSubscriber because upstream TRACE can expose access-key IDs before transport
header redaction. The embedding application's global subscriber and outer safe
status events remain active. Authorization/session-token headers are marked
sensitive before reqwest. Public errors retain fixed safe messages; remote error
bodies are capped at 1 MiB before SDK parsing and are never exposed in receipts,
cache or durable diagnostics.

The reviewed XML guard is for ListObjectsV2, not model bytes. This HEAD/range-GET
slice contains no listing route; XML/cardinality/exact-value guard integration
belongs to the next prefix enumeration slice. No namespace, pagination or prefix
snapshot acceptance is claimed here.

## Actual dependency delta

Rust 1.92 resolves the production lock offline from the original lock, retaining
existing versions unless required by the new owner. A discarded full-regeneration
attempt selected unrelated upgrades and encountered uncached tokio-macros; it is
not the delivered lock. Final lock: 482 to 511 package versions, 51 added and 22
removed (net +29). Existing-name replacements are md-5, proc-macro2 and quote.
The latter two satisfy the added Smithy macro dependency; md-5 belongs to the SDK
checksum implementation replacing the removed storage backend's version.

For core with no defaults and `s3,hf-client`, the normal/build closure changes
239 to 276 package versions: 53 added, 16 removed, 43 new names (net +37).
This is the actual production closure rather than the standalone spike's gross
count. The old reader's Rustls/native-certificate transport closure drops out;
reqwest continues its ordinary default TLS peer verification. There is no
`aws-config`, default AWS HTTP client, alternative production cloud/backend
owner, new XML DOM dependency or SDK SigV4a feature.

Direct optional dependencies are pinned `aws-sdk-s3` 1.137.0 (`rt-tokio`,
`http-1x`, defaults disabled), `aws-credential-types` 1.2.14,
`aws-smithy-runtime-api` 1.12.3 (`client`, `http-1x`), `aws-smithy-types` 1.5.0
(`http-body-1-x`), and the already locked HTTP bridge crates `http` 1.4.0,
`http-body` 1.0.1, `http-body-util` 0.1.3. The already locked tracing subscriber
is a test-only direct dependency for isolated global-TRACE regression evidence.

Internal evidence lives in `/workspace/scratch/s3-sdk-reader/`, including
`dependency-tree.log`, `base-dependency-tree.log` and `sdk-features.log`.

### Lock additions

- `allocator-api2` 0.2.21
- `arc-swap` 1.9.2
- `aws-credential-types` 1.2.14
- `aws-runtime` 1.7.5
- `aws-sdk-s3` 1.137.0
- `aws-sigv4` 1.4.5
- `aws-smithy-async` 1.2.14
- `aws-smithy-checksums` 0.64.8
- `aws-smithy-eventstream` 0.60.21
- `aws-smithy-http` 0.63.6
- `aws-smithy-json` 0.62.7
- `aws-smithy-observability` 0.2.6
- `aws-smithy-runtime` 1.11.3
- `aws-smithy-runtime-api` 1.12.3
- `aws-smithy-runtime-api-macros` 1.0.0
- `aws-smithy-schema` 0.1.0
- `aws-smithy-types` 1.5.0
- `aws-smithy-xml` 0.60.15
- `aws-types` 1.3.16
- `base64-simd` 0.8.0
- `block-buffer` 0.12.1
- `bytes-utils` 0.1.4
- `cmov` 0.5.4
- `const-oid` 0.10.2
- `crc-fast` 1.10.0
- `crypto-common` 0.2.2
- `ctutils` 0.4.2
- `deranged` 0.5.8
- `digest` 0.11.3
- `foldhash` 0.2.0
- `hmac` 0.13.0
- `http` 0.2.12
- `http-body` 0.4.6
- `hybrid-array` 0.4.10
- `lru` 0.16.4
- `md-5` 0.11.0
- `num-conv` 0.2.2
- `outref` 0.5.2
- `powerfmt` 0.2.1
- `proc-macro2` 1.0.107
- `quote` 1.0.47
- `regex-lite` 0.1.9
- `rustc_version` 0.4.1
- `sha1` 0.11.0
- `sha2` 0.11.0
- `spin` 0.10.1
- `time` 0.3.55
- `time-core` 0.1.9
- `time-macros` 0.2.32
- `vsimd` 0.8.0
- `xmlparser` 0.13.6

### Lock removals

- `chacha20` 0.10.2
- `core-foundation` 0.10.1
- `getrandom` 0.4.3
- `humantime` 2.4.0
- `lru-slab` 0.1.3
- `md-5` 0.10.6
- `object_store` 0.12.4
- `openssl-probe` 0.2.1
- `proc-macro2` 1.0.105
- `quick-xml` 0.38.4
- `quinn` 0.11.12
- `quinn-proto` 0.11.19
- `quinn-udp` 0.5.16
- `quote` 1.0.43
- `r-efi` 6.0.0
- `rand` 0.10.3
- `rand_core` 0.10.1
- `rand_pcg` 0.10.2
- `rustc-hash` 2.1.3
- `rustls-native-certs` 0.8.4
- `security-framework` 3.5.1
- `web-time` 1.1.0

## Qualification

Rust/Cargo 1.92, locked/offline, one worker, debug level zero and incremental
compilation disabled qualified the source with `s3,hf-client,test-support` and
no default inference features. Actual successful executions:

| Check | Result |
| --- | --- |
| Focused production S3 unit cases | 13 passed; isolated global-TRACE child executed by its passing parent (one separately ignored entry) |
| Broader acquisition unit filter | 156 passed; same child executed by its parent |
| S3 acquisition integrations | 44 passed |
| Native S3 model workflow | 7 passed; an additional cold-process child execution appears in the log |
| Anonymous/public reader integration | 14 passed, including a six-second body stall within the caller's 12-second budget |
| Inference-disabled S3 RPC | 202 passed; cold-process child executed |
| Strict core Clippy | All targets passed with SDK/consumer fixtures enabled |
| Strict RPC Clippy | Production binary passed with S3 enabled and inference disabled |
| Feature-off core | Locked/offline compilation passed |
| Formatting/diff | Passed |

The independent RFC-anchored signature oracle verifies HEAD/GET in both
addressing styles, no token/optional token, and tampered secret/key/version/
range/If-Match/token controls. Anonymous requests remain unsigned. Credential
validation/debug, mandatory production HTTPS despite HTTP opt-in, literal-loopback
unit-only credential fixtures, endpoint/proxy/redirect/retry constraints,
manifest/acquisition/receipt identity and persisted secret exclusion all pass.
The new fresh-process global-TRACE case checks construction, selection, range
body polls, acquisition body capability and reflected SDK errors while observing
outer safe selecting/verified statuses. The new GET error probe exceeds 1 MiB,
leaves its response unfinished and observes refusal plus connection closure
before the caller's five-second deadline, without destination writes. A different
SDK bucket is refused before source I/O.

Logs are under `/workspace/scratch/s3-sdk-reader/`: `unit-final.log`,
`acquisition-final.log`, `integration-final.log`, `rpc-final.log`,
`clippy-core-final.log`, `clippy-rpc-production-final.log`,
`feature-off-final.log` and `fmt-final.log`. Behavior tests preceded a final
field-shorthand-only Clippy cleanup; final all-target compilation/lint checks
use the delivered source. Intermediates are retained: the full-lock regeneration
and initial compile errors; two signature-oracle failures before encoded query
sorting; the body-bound probe's incorrect HEAD fixture before moving it to GET;
and consumer setup failures before the standard task-local XDG configuration
path replaced the read-only home registry location. No product registry or
sandbox exception was added.

An additional broader strict RPC test-target pass fails on 11 inference-disabled
Torch DTO dead-code warnings in unchanged `contract.rs`. The same warnings appear
in the otherwise passing 202-case run. Their source is byte-identical to the
base; no lint suppression or frozen RPC-contract edit is included. This broader
lint scope remains an integration limitation, recorded in
`clippy-rpc-test-target-limit.log`.

## Dependency and attribution evidence

The existing repository feature checker passes all 12 default/headless/fixture
contracts; workspace dependency ownership passes. Explicit S3 normal/build
graphs additionally pass for Linux (276 package versions), macOS (275) and
Windows (282), with no object_store, aws-config, default AWS HTTP client or
inference dependencies. These are dependency graphs, not cross-platform builds.
Logs: `feature-contracts.log`, `dependency-ownership.log`,
`sdk-target-closure.log`, `sdk-features.log` and the two dependency-tree logs.

The unchanged canonical notice generator produced and validated an isolated
374-entry default-release candidate at `attribution-candidate/`; its input hashes
match this branch. Cached frontend/Electron dependencies were linked only after
manifest/lock byte comparison with the base workspace, then those exact links
were retired. Shared canonical release outputs and generator remain untouched;
parent integration must regenerate attribution for the final merged release
profile.

Authoritative license texts cover all 51 added lock versions and all 311 external
versions in the three-platform optional S3 union. Internal inventories are
`sdk-added-license-inventory.json`, `sdk-full-closure-license-inventory.json` and
`sdk-added-third-party-notices.txt`. `base64-simd`/`vsimd` 0.8.0 omit license text
from their cached crates. Both declare MIT and identify upstream commit
`d74c030d9dc4f3cae02146d1f497ff62726ef09a`; its
[repository LICENSE](https://raw.githubusercontent.com/Nugine/simd/d74c030d9dc4f3cae02146d1f497ff62726ef09a/LICENSE)
was fetched anonymously over normal verified HTTPS, with SHA-256
`71674605ec4c087fe9eb534e3e4f9e26eb2e4aabcd76a29fd156c6a844d44b3d`.
No missing terms were invented. The older checked-in optional-reader inventory
belongs to its original object_store milestone; the new internal inventory is
review evidence for the parent's optional-S3 distribution attribution update.

## Remaining scope

No real credentials, account provisioning, paid services, external reviewers,
ONNX build downloads or browser access workarounds were used. Actual-provider,
installed/platform, inference, full desktop/assistive-technology and acquisition
gate acceptance remain separate. The next existing-plan feature is bounded
explicit S3 prefix enumeration with the accepted ListObjectsV2 XML/cardinality/
exact-value guard, after parent review/integration of this reader replacement.
