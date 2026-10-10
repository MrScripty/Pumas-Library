# Bound anonymous HF model-detail cache storage

## Scope

The [HF cache brief](../breif/hf-cache.md) calls for local size limits and eviction. The [observation protocol](hf-model-detail-cache-2026-10-08.md) supplies full-detail JSON records; this extension adds aggregate final-file accounting without changing acquisition identity or model publication.

The shared acquired-model bridge supports bounded GGUF, safetensors/Transformers/Diffusers and static ONNX representations. Generic transfer success remains separate from package qualification and backend compatibility. This cache budget does not establish provider interoperability or inference.

## Resulting behavior

Anonymous full-detail observation writes are serialized per exact lexical cache-directory path and admitted against **256 complete records and 64 MiB of logical final-file bytes**. Before publishing a new/replacement record, the owner selects enough eligible victims to fit both limits. Eligible observations are ordered by validation timestamp, then filename for ties. This is oldest-observation eviction, not access-frequency LRU; fresh cache reads remain read-only and do not update disk timestamps.

Active admission cooldowns are protected. Unknown, corrupt, future-version, wrong-identity, oversized, invalid-validator, and future-timestamp records are counted but never evicted or overwritten by this policy. A valid refresh can replace its own eligible prior observation; accounting subtracts the old bytes once. If protected records leave insufficient capacity, no victims are removed and persistence is refused. Successful live lookup still returns the live metadata. Authenticated calls remain live-only and do not consume or mutate anonymous observation storage.

The native `HuggingFaceClient::model_detail_cache_usage()` returns `(record_count, logical_bytes)` without HTTP, credentials, source URLs, repository names or response bodies. Associated `MODEL_DETAIL_CACHE_RECORD_LIMIT` and `MODEL_DETAIL_CACHE_BYTE_LIMIT` constants expose the policy. Usage includes exact-family regular filenames even when their content cannot be admitted. The function does not claim SQLite/search/tree cache statistics or total filesystem allocated usage. No desktop/RPC method is added.

## Ownership and bounded work

Only names matching `hf_<64 lowercase hex characters>_metadata_v1.json` are accounted or considered for eviction. Other cache families, model outputs, downloads/acquisition documents, arbitrary filenames, symlinks, FIFOs and directories receive no cleanup authority. A matching nonregular member causes inventory/write refusal. Scans inspect at most 4096 directory entries; classification admits at most 64 MiB of reads plus one overflow-probe byte through non-following regular-file handles, with the existing per-record ceiling. Incomplete scans/classification refuse new persistence before eviction, rather than calling a partial ordering globally oldest-first.

Cache admission and eviction reuse the same structural observation predicate as ordinary detail reads. Eviction additionally requires a valid source/repository URL identity and a nonfuture observation time. Budget inventory/classification reads, eviction, invalidation and publication use held directory capabilities. The preserved ordinary lookup read uses its existing non-following ambient leaf open. The existing `AtomicJsonTarget` publishes the JSON through the held cache directory; no new publisher is introduced. Invalidation inspects only its exact known target, so an incomplete directory inventory cannot suppress no-store/denial cleanup of that target.

Existing per-URL admission precedes the directory admission. Both owned guards move into the existing `StoreLifetime` blocking effect. Canceling the outer lookup cannot release directory admission while its actual cache mutation is still running. Inventory holds only directory admission and cannot reverse the lock order. Registries retain weak guards, with the existing bounded active-path/participant policy, rather than retaining clients or payloads.

The budget is a cooperative current-process guarantee for complete final files at exact lexical directory identity. Relative/absolute/symlink aliases, old clients and separate processes are not coordinated by these locks. Existing store-lifetime ownership remains in force. Fully classified known legacy entries can be evicted during an ordinary write to reach the limits; there is no startup clearing/migration. Legacy directories exceeding scan/classification capacity freeze new persistence. Temporary atomic staging overhead, filesystem allocation overhead, foreign families and externally written bytes are outside the final-file accounting. Victim removal may complete before an eventual failed/uncertain new publication; cache preservation is optional and live results remain usable. This is not a multi-file atomic transaction or model-publication protocol.

## Recorded qualification

Focused owned fixtures exercise the public live lookup at default record pressure, actual disk eviction and reopened cache reuse, byte pressure and exact replacement accounting, protected/expired cooldowns, fully protected capacity, inadmissible target preservation, corrupt/future/wrong-source records, legacy classification byte capacity, directory scan capacity, nonregular member refusal, concurrent misses, and canceled outer lookup retaining directory admission until its real effect settles. Existing cache protocol and public selected-hydration tests must remain passing. All Cargo commands use locked offline official dependencies with `ORT_SKIP_DOWNLOAD=1`.

At the aggregate-budget stage, 32 detail-cache tests, including 12 budget cases, and 11 public selected-hydration tests passed, as did strict all-target Clippy, formatting and whitespace checks. The full `hf-client` library run passed 1,990 tests with 11 ignored and one environment baseline failure: `api::models::tests::get_inference_settings_batch_reports_per_model_errors`, creating the default user registry on a read-only filesystem. The failure also reproduced in an earlier accepted observation-cache binary; the failing API source blob matched the immediate pre-budget sources. This is not an all-green full-suite claim or an immediate-predecessor binary comparison. No baseline fixture was changed to mask the failure.

## Separate qualification requirements

1. Cold S3 resumption requires fresh reservation/recovery authority; persisted inspection alone does not authorize reopening.
2. Catalog-wide discovery and global HF API-bucket admission require separate public freshness and provenance policies.
3. Exact wheel closure and local-only package consumption require their own acquisition/runtime package qualification.
4. Actual-provider S3 interoperability and supported-platform deployment, migration and resource behavior require real service/platform evidence; synthetic fixtures cannot establish them.
