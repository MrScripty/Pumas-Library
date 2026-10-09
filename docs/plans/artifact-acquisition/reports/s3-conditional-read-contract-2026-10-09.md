# Conditional acquisition from non-versioned S3 objects

`S3Reader::select_conditional(key, logical_path, expected_sha256)` adds an opt-in
single-object acquisition capability for ordinary general-purpose buckets.
S3 carries arbitrary bytes. This selection establishes neither model-package
qualification nor backend compatibility. The existing VersionId model facade,
explicit version-pinned manifests and prefix discovery keep their existing contract.
The additive native conditional model facade is described below.

The caller supplies endpoint/bucket authority and a SHA-256 from its selected
artifact declaration. Credentials remain request-scoped, nonserialized and
excluded from identity. Authenticated sources still require HTTPS; redirects,
ambient credential discovery, proxies and automatic SDK retries remain disabled.

Selection sends HEAD without `versionId` and requires a known nonnegative size
and a strong quoted ETag. Missing or weak validators, malformed metadata, and
unexpected non-null immutable VersionId fail. An absent VersionId or `null`
identifies the new conditional mode; neither is admitted through `select`.
The manifest records **Weak** `s3.conditional_etag_size` revision evidence containing
the exact observed ETag and size. **Weak** describes mutable provenance, not an
HTTP weak validator: the `W/` prefix is refused, including by the native model
facade. A strong HTTP ETag still cannot supply content integrity. ETag is opaque
representation evidence, never
a digest or a claim that the source is immutable. The file requires whole-file
SHA-256 verification through the existing descriptor-owned acquisition lifecycle.

Every nonempty read sends the exact If-Match validator and requested range,
without a version query. Before writes, successful range responses must be206,
preserve the selected ETag and non-versioned mode, describe exactly the requested
range and total size, and declare exactly its byte count.412/404, ignored ranges,
changed metadata, absent response evidence, truncated or oversized bodies fail.
A provider that echoes stale ETags or ignores preconditions cannot authorize
publication of changed bytes: the mandatory full digest must still match before
verified-file handoff or any consumer receipt. Raw `read_range` output is unverified.
No remote atomic snapshot or proof of provider implementation correctness is claimed.

Fresh empty transfers use an explicitly authorized If-Match GET with no range.
Only200 with zero Content-Length, no Content-Range, matching ETag/non-versioned
metadata and zero actual body qualifies. A private SDK extension permits this
specific request; general whole-object GET authority remains absent. Existing
digest-verified file reuse can avoid a fresh GET, under the same held custody.

Warm continuation retains the current owner's verified prefix and exact source
resource, ETag and total. Cold recovery discards live continuation and restarts
incomplete transfers at byte zero. Reselection must reproduce the exact manifest
under the same demand; changed ETag, size or digest refuses reuse. Complete files
may be reused only after the existing full verifier and custody checks. Transfer
timeouts, finite retry admission, cancellation, registered-write draining,
consumer settlement and publication remain owned by the existing lifecycle.

Local in-memory SDK fixtures exercise both modes, strict responses, digest
verification and receipt absence on failure, empty reads, warm continuation,
cancellation and cold reselection. They do not establish live-provider acceptance
or model inference. No provider campaign or external object request is performed.

## Preserved campaign limitation

The earlier real Garage campaign passed a functional subset but exceeded a literal
128 MiB physical-transfer cap: observed S3 entities across both relay legs totaled
163,787,380 bytes, exceeding the cap by29,569,652 bytes before framing/internal RPC.
Its exact failed attempts and independent rejection remain preserved. This source
change does not relabel that campaign or rerun it. Separate synthetic harness tests
verify reserving both relay legs and checking the complete entity-plus-allowance
budget before explicit body reads and forwarding. Provider/header buffering can
precede these checks; such accounting is not a total wire-traffic proof.

## Integration scope

Use the new selection with existing `AcquisitionS3Request` and `acquire_s3` to
obtain digest-verified arbitrary bytes. For model publication, call the additive
`PumasApi::import_s3_conditional_model(S3ConditionalModelImportRequest, control)`.
Its request supplies `source_key`, mandatory `expected_sha256`, the logical file
path in `import.path`, explicit source/credential authority, UUID, held workspace
and finite retry policy. It resolves HEAD under the same bounded consumer task
scope, then uses the existing verified descriptor, receipt and shared importer
publication path. No empty/fake VersionId or immutable revision is manufactured.

Local loopback native tests qualify actual safetensors and GGUF publication,
wrong digests, malformed safetensors, unknown bytes, a shard missing its required
index/package companions, and cancellation before admission, during HEAD and
after GET headers before body bytes. They establish structural import only, not
inference. Existing unsafe format/custom-code policies continue to apply; byte transfer grants no execution.
Publication proof checks the durable Ready index row directly. For the tiny
safetensors fixture, on-demand public `get_model` lookup returned no row after
Confirmed/Ready publication; the same result reproduced on exact parent
`7f540efe65ff194057cd5582da09cd3cdbafc16d`. Its root cause remains unresolved,
and this slice does not qualify that catalog consumer or fix its reconciliation.

This slice exposes no non-versioned model RPC, conditional bundle resolver,
prefix discovery or UI. Existing VersionId callers need no source change.
No HF, modality, runtime ownership, release producer or old cold-reopen fixture
file is modified.
