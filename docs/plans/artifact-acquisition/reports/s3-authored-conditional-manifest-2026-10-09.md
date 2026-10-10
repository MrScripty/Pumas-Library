# Explicit authored conditional S3 file sets

S3 remains an arbitrary byte store. `S3Reader::select_conditional_manifest`
accepts 2–32 caller-authored `S3ConditionalManifestEntry` members containing exact
key, logical path, strong quoted HTTP ETag, expected size and whole-file SHA-256.
Neither bucket enumeration nor provider snapshot semantics are used. Existing
VersionId and single-object conditional entry points remain compatible.

Before copying/sorting the set or making HEAD requests, cardinality is bounded.
Complete structural preflight checks exact keys, strong ETags, nonnegative sizes
representable by the SDK, duplicate-key ETag consistency and the shared manifest
namespace, source size/digest, aggregate-size and 16-KiB revision limits. Raw keys
remain raw: encoding ETag into a source key must not hide inconsistent evidence.
Members and revision tuples are ordered by logical path. Every sequential HEAD
must match its authored ETag/size and non-versioned contract. Any missing/changed
member returns no selection. Success returns the original authored Weak manifest
with authority `s3.explicit_conditional_objects`, rather than a new observation.

Each later GET retains the existing reader's exact If-Match, range, version,
response size and actual byte-count checks. An empty member needs a fresh actual
conditional GET; versioned auxiliary HEAD shortcuts do not apply. The generic
acquisition owner verifies every full hash before consumer use/receipt issuance.
ETag/size never become a checksum or immutable revision. Mutable objects may
change between requests; failed conditions refuse the package, and no atomic
remote snapshot is claimed. Operation/retry budgets remain per object under the
existing scoped selection and transfer owners.

`PumasApi::import_s3_conditional_bundle` takes the additive native
`S3ConditionalBundleModelImportRequest`. It checks importer-owned paths and exact
selected primary membership before I/O, then routes the complete selection through
the existing shared package validator, held descriptors, issued receipt,
copied-byte verifier and atomic model publisher. Successful arbitrary-byte
acquisition, structural model-package qualification and backend/inference admission
remain separate. Unsafe formats/custom code retain existing policy; transfer
does not authorize execution. RPC and prefix discovery are unchanged.

Replay retains the exact operation, source authority, authored manifest and
workspace identity. Matching a new HEAD to changed authored facts cannot replace
an existing record or authorize append. Cancellation keeps retained transfer
custody but clears continuation proof. The same owner's paused transfer can resume
only its checked matching prefix; cold reopen has no live checkpoint and restarts
partial data at byte zero. Whole-file reuse still requires full digest proof.

The owned loopback integration suite uses the official SDK and genuine tiny F32
safetensors shards, config, tokenizer files and a shard index. It checks exact
ordered manifest/files, per-file hashes, import-spec receipt payload, Confirmed
publication/model ID, copied bytes, Ready metadata and direct index row. A GGUF
bundle additionally proves empty-member HEAD plus non-range conditional GET.
Malformed paths/keys/validators, invalid member counts, source-evidence conflicts,
SDK size overflow, aggregate overflow, revision overflow and namespace collisions
fail with zero HEADs. Late absent/changed/weak HEADs admit no acquisition. Changed
GET evidence/version/range and wrong payload hashes issue no receipt or model.
Missing config/tokenizer/index/shard, malformed config/shard/index and custom code
can acquire all valid declared bytes and retain an issued receipt, while the
importer refuses publication. Held HEAD/GET cancellation, matching-fact retries,
changed ETag/size/digest/bucket/endpoint replay, exact-owner warm pause and cold
reopen are tested independently.

Run with `ORT_SKIP_DOWNLOAD=1` and official cached dependencies:

```sh
cargo test --offline --locked --manifest-path rust/Cargo.toml -p pumas-library \
  --no-default-features --features s3 --test s3_conditional_bundle_workflow \
  -- --nocapture --test-threads=1
```

Qualification: the seven new bundle tests and four existing conditional
single-object tests pass. Independent review accepted the source and independently
reran all seven new tests successfully. Strict core Clippy with `-D warnings`
passes. All nine existing versioned S3 workflow tests pass with an owned
`XDG_CONFIG_HOME`. Their first run had eight registry setup failures because the
default home configuration directory is read-only; that failed log is retained
separately, and the old fixture source is unchanged.

These fixtures qualify structural package import and custody only. They establish
no inference, atomic bucket snapshot, live-provider campaign, conditional prefix
discovery, RPC/UI bundle admission or public catalog lookup acceptance. Public
catalog lookup visibility remains unresolved on the baseline and is excluded.
ONNX and other format support remain subject to the shared importer and separate integration
work; the selected transformer fixture requires no processor/external tensor.
No provider was launched for this slice.
