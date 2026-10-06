# Bounded S3 prefix enumeration

Successor `feat/s3-prefix-main95` retains integrated checkpoint
`7cf17383001d582056ac4299803c83d0f9ae68a7`, tree
`046771b37c9a9f563936376539301b3225b48656`, and attribution prerequisite
`a39121b4cc7667f901959497f71368179341cfd1`, tree
`35e24f3265b6064d16516493378e89f695609a4b`. The prerequisite qualified default
plus S3 RPC compilation and 302 tests, eight attribution regressions, eleven
release contracts and 33 dependency graphs. The final source milestone and
review comparison are bound in the successor review packet after qualification.

## Owner and authority

One AWS SDK client remains the protocol/signing owner over the existing explicit
reqwest transport. There is no object_store fallback, AWS config discovery,
default SDK HTTP client or second listing backend. The promoted XML/SDK agreement
guard comes from reviewed spike `ba1a9d908010f23f2391059cb93bfe61dcca6aa1`;
its exact scalar/namespace/completion/selection comparisons remain intact.
Added operation-local annotations bound the listing body before SDK buffering;
an observed byte counter supplies the aggregate budget. This guard checks evidence
after the maintained SDK decodes S3; it does not replace the SDK protocol owner.
Optional `roxmltree = =0.21.1` uses its authoritative MIT/Apache-2.0 crate texts.

`S3Reader::enumerate_prefix` uses a caller-authorized raw prefix, one page at a
time, no delimiter or encoding conversion, and explicit positive page/object/XML
capacity. Each page is at most 1 MiB and retains the reviewed 4096-node/no-DTD
guard. The existing reader timeout covers the entire call, including all pages
and HEAD pins. The maximum source-call count is `max_pages + max_objects`.
These capacity bounds are not a production RSS or wire-header overhead claim.
The API does not impose a new fixed service deadline or SDK retry policy.

Completion must be explicit and unambiguous. All scope/count echoes must match;
next tokens must be nonempty and noncyclic; keys must be exact, in scope, unique
and in the general-purpose service's lexicographic order. Grouped or encoded
results, unsupported keys, failed pages and contradictions return no partial
success. Limits report typed `S3PrefixError::Incomplete`; other errors wrap the
unchanged `S3ReaderError`. The existing constructors/config/error enum remain
source-compatible. Directory buckets remain refused by existing construction.

No HEAD pins are resolved until listing pagination finishes. Every current HEAD
uses the listed opaque ETag as If-Match, must match size/ETag and must return a
valid immutable, non-null VersionId. The returned listing has ordered version
observations, no credential/read capability and no serialization. Debug shows
counts; object Debug is redacted. Opaque tokens are scoped local state. All SDK
and body/guard polls retain the same NoSubscriber boundary without changing the
global subscriber; caller-owned safe status remains visible.

The underlying [ListObjectsV2 API](https://docs.aws.amazon.com/AmazonS3/latest/API/API_ListObjectsV2.html)
specifies 200 responses, opaque continuation tokens, a maximum page size of 1000,
raw prefix selection and lexicographical ordering for general-purpose buckets.
The production API is a deliberately bounded subset of these semantics. Multiple
pages and sequential HEADs do not create an atomic multi-object snapshot. Caller
paths and authoritative SHA-256 evidence remain required before the unchanged
explicit manifest/acquisition/import path can admit and verify content.

## Boundary and receipt qualification

The focused wire fixtures cover anonymous, access-key/secret and session-token
requests in both addressing styles; exact query/signature bytes are checked by
the existing RFC-anchored independent HMAC oracle. They qualify malformed and
duplicate completion fields, XML/SDK CDATA disagreement, namespace/DTD/node
limits, scope/count mismatches, grouped/encoded results, duplicate/out-of-order
keys, cycles, empty advancing pages and empty complete results. Byte/page/object
limits and exact response-size boundaries are distinguished from successful
completion. Changed size/ETag, missing/null versions and source failure never
produce a shorter returned set. Redirects, response status, SDK retries, reflected
provider errors, fresh-process ambient/proxy rejection, global TRACE redaction,
unfinished body overflow, cancellation and total elapsed budget are exercised.

The public native workflow fixture maps two discovered keys to caller-chosen
logical paths and SHA-256 evidence. Distinct config/model versions pass through
the unchanged importer, publication and exact receipts. Before explicit import,
discovery creates neither acquisition records nor workspace files. GetModel and
Ready metadata, payload bytes, version-qualified source keys, adopted custody
and exact receipt pairing are checked; cold reopen requires no source replay.
Continuation tokens are absent from persisted acquisition state.

The source write set excludes `s3/manifest.rs`, watcher, importer/recovery,
stores, consumer/receipt formats, transfer retries/verification and ONNX source.
Their protected bytes are compared to integrated 7cf17383. New listing transport
authority leaves HEAD and conditional range GET behavior intact; existing reader,
acquisition and public workflow regressions remain required. No prefix RPC/UI,
automatic import, credential refresh or native runtime acquisition is added.

## Evidence and limits

Execution and source/check logs live under `/workspace/scratch/s3-prefix-main95/`.
Completed checks before the final fixture-module cleanup:

| Check | Result | Log in evidence directory |
| --- | --- | --- |
| Core S3 units | 28 passed, 2 subprocess helpers ignored; helpers exercised by parent tests | `prefix-unit-final.log` |
| S3 reader/acquisition/model workflow integrations | 44 + 14 + 8 passed | `prefix-integration-final.log` |
| Attribution and release-contract Node tests | 8 + 11 passed | `prefix-release-contracts-final.log` |
| Dependency feature graphs | 33 passed | `prefix-feature-contracts.log` |
| Dependency ownership | passed | `prefix-dependency-ownership.log` |
| Formatting | passed | `prefix-format-final.log` |
| Default/S3 attribution | 365 / 415 entries validated | attribution generation/check logs |

Strict core Clippy found a duplicate test fixture module. The fixture was moved
to the shared S3 test parent, retaining its original visibility and bytes. An
intermediate sibling import failed visibility checks; that compiler evidence is
retained in `prefix-clippy-core-fixture-reuse.log`. Post-cleanup unit/Clippy
qualification is recorded separately when complete. Post-prefix RPC tests,
production RPC Clippy and hosted checks remain pending at this checkpoint.
The 302 RPC tests above qualified the attribution prerequisite before prefix
source changes; they are not a post-prefix result.
The actual default-plus-S3 graph retains approved-main dynamic ONNX loading and
forbids build download/copy/TLS features. Final S3 attribution includes 415 notice
entries; default remains 365. Platform graphs are not platform builds; separately
provisioned runtime/inference, actual desktop/accessibility, hosted checks and
AWS/non-AWS/MinIO provider acceptance are not claimed. AQ-S3/Q4 remain pending.

Intermediate evidence is retained. The first unit build failed because String
fixtures used slice `repeat`, which requires Copy; cloning repeat_n fixes it.
The next link exhausted disk, failed with linker SIGBUS and temporarily prevented
normal sandbox startup (`No space left on device`). Managed automatic approval
allowed narrow cache cleanup; no sandbox/network/security settings changed.
Source, logs and packaged frozen evidence remain intact; paths/hashes of retired
recomputable caches are recorded. A subsequent actual run passed 26 units and
caught two fixture expectations: existing construction already refuses directory
bucket selectors, and the reqwest budget may win the outer timeout race. Tests
now check the existing contained error behavior and connection drainage, without
changing reader error mapping/retry policy. A workflow fixture also initially attempted serialization of the entire UUID-keyed
record map, whose key has no serde feature; it now serializes the individual
existing record without changing UUID dependencies. The diagnostic is retained
in `prefix-integration-key-serialization-intermediate.log`.
The saved source checkpoint records successful checks and explicitly pending
qualification; it does not claim all release gates have passed. No external reviewer was contacted and no
PR mutation occurred; parent owns draft publication, review and integration.
