# Authored conditional S3 bundles through the existing RPC owner

The existing `start_s3_model_bundle_import` and
`start_authenticated_s3_model_bundle_import` calls now expose the bounded native
authored-manifest capability. S3 transfers arbitrary bytes; the shared importer
qualifies the selected model package after whole-file integrity checks. No bucket
listing, atomic remote snapshot or new model parser/publisher is introduced.

Omitted `read_mode` or `"version_id"` preserves the existing 2–32 pinned members:
`key`, `version_id`, `logical_path`, `sha256`. Explicit `"conditional"` requires
2–32 uniformly authored members: `key`, `logical_path`, `sha256`, `expected_etag`,
`expected_size`. Conditional members forbid every supplied VersionId, including
empty/null/fake values. Both member DTOs deny unknown fields; mixed member kinds
and missing primary membership are invalid. The original pinned member contract
is unchanged.

`expected_etag` uses the native quoted HTTP opaque-tag grammar, preserving quoted
empty and Unicode tags and refusing weak/unquoted/control/embedded-quote forms.
`expected_size` is a canonical decimal **string**, not a JavaScript number:
`"0"` through `"9223372036854775807"`. Leading zeros, signs, whitespace, exponents,
numbers, null and overflow are invalid. Per-member paths retain the 1024-byte wire
bound. Shared namespace, source-size/hash consistency, aggregate overflow,
revision-size and importer-owned destination rules apply to the complete set
before workspace or source I/O. Duplicate raw keys must have consistent ETags.
The S3-enabled facade also calls the exact native reader preflight before job
admission. Builds without S3 validate the same DTO/shared manifest and return the
existing `Unavailable` capability result for valid requests.

Generated bundle schemas use closed top-level mode alternatives with uniformly
pinned or authored files, including nested authenticated definitions. Standard
JSON Schema patterns preserve exact size bounds without numeric coercion. Both
desktop consumers regenerate from this branch's Rust schema; unrelated schemas
are preserved. At integration, compose source changes with other branches and
regenerate the combined clients once rather than replacing another branch's
generated schema projection.

Jobs retain a typed selector for versioned objects, a conditional single object,
or the entire conditional manifest. That exact selector is cloned into retained
transfer custody and selects the matching existing native API. The existing
worker, reservation, cancellation, progress, retry observation, receipt and
cleanup owners remain in place. Fresh credentials are consumed by each
authenticated attempt and never serialized into source identity or retained work.
Cancelled partial retry restarts at zero; unchanged complete files require the
existing custody/full-digest checks. Changed source HEAD facts refuse before GET
without replacing the retained record or partial bytes. Cold reopen can inspect
the retained operation but cannot manufacture same-process retry authority or
overwrite an existing demand.

The actual-process fixture launches the production RPC with its official SDK and
owned HTTPS objects, explicit credentials where selected, a deny proxy for other
origins, isolated configuration/cache and bounded cleanup. Genuine tiny F32
safetensors shards, config, tokenizer files and index prove complete package
qualification. A GGUF header plus an empty auxiliary proves an actual non-range
conditional GET for the empty member. Full sorted source tuples, hashes, receipt
payload/files, Confirmed publication, copied bytes, Ready metadata and direct
index row are checked. All HEADs precede the first GET.

The 32 process cases include anonymous/authenticated and unchanged VersionId
success, missing/changed/weak HEADs, unexpected HEAD/GET versions, 412, changed GET
ETag, ignored ranges, modified bodies and late wrong digest. Missing required
config/tokenizer/index/shard, malformed config/shard/index and selected custom
code can acquire exact bytes and retain the issued receipt while model publication
is refused. Held HEAD/GET cancellation and shutdown drain. Exact same-ID retries
retain authored facts; authenticated retry requires fresh credentials; changed
ETag/size refuses before GET; cold process reopening preserves retained bytes and
has no live retry capability. Forty-seven malformed preflight variants are
checked through both anonymous and authenticated starts with zero source calls,
reservations or admitted acquisitions. Conditional prefix discovery is explicitly
refused by both starts.

Run with `ORT_SKIP_DOWNLOAD=1`, official dependencies and locked offline builds:

```sh
cargo test --offline --locked --manifest-path rust/Cargo.toml -p pumas-rpc \
  --no-default-features --features s3,export-contract \
  --test s3_conditional_bundle_process -- --nocapture
node --test scripts/tests/s3-authored-conditional-contract.test.mjs \
  scripts/tests/s3-conditional-contract.test.mjs
```

These tests establish structural import, integrity and custody. Tiny fixture
weights are not a runnable Llama model, and no inference/backend acceptance is
claimed. The selected transformer package requires no processor or external
tensor. Other formats remain governed by the shared importer and separate
qualification. Public catalog reconciliation/lookup is excluded; direct Ready
publication/index evidence does not qualify that consumer. No live provider,
external bucket, resource-envelope campaign, conditional prefix discovery or
conditional dialog workflow is established. The native per-object budgets do not
claim a complete-set hard wall clock. Unsafe/custom-code policy remains unchanged;
acquiring bytes grants no execution authority.

Qualification passed: 32 owned production-process cases, an independent rerun of
all 32, the Cargo process-test wrapper, the existing 18 single-object process
cases, 47 malformed preflight variants through each anonymous/authenticated start,
and valid/unavailable plus malformed/no-effect requests in an actual no-S3
process. Generated validator tests pass for both consumers; Rust boundary tests
pass with and without S3. Both TypeScript checks and generated-file checking pass.
All 82 non-S3 schemas and 139 non-S3 generated types are unchanged. RPC Clippy
passes in both feature profiles with `-D warnings -A dead_code`; the exception is
for the existing dead-code baseline (11 production / 8 test warnings), not a new
warning category. Format and diff checks pass.

The additional legacy
`source_bundle_structural_preflight_refuses_entire_set_before_admission` test
fails in its blanket rejection table on both this candidate and exact native
parent `766f4784b97cde38dac897985a41c21eb4b1a5b9`, at the same line/assertion.
Both failed logs are retained separately from the passing qualification. Its
expectation table is unchanged; this slice does not repair that baseline test or
claim that the full old RPC suite is green.
