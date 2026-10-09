# Adopt persistent HF details in file-metadata discovery

## Selected existing caller

This follows the [HF cache brief](../breif/hf-cache.md) and [bounded observation protocol](hf-model-detail-cache-2026-10-08.md). The brief separates discovery from detailed hydration and from download/execution admission. The existing public `PumasApi::lookup_hf_metadata_for_file` and its IPC dispatch both use `HuggingFaceClient::lookup_metadata` to propose metadata for a local file. That advisory workflow is the adoption point.

Search and existing top-two file-tree matching remain unchanged. Only the selected LFS match, otherwise the selected filename fallback, is hydrated through `get_model_info_cached`. Empty search returns no match without detail hydration. Other candidates do not receive model-detail requests. Full model card, license, official name, pipeline/model type hint and release date come from the qualifying observation. Existing repository/match identity, confidence, expected hash, local fast hash and pending-full-verification state are preserved. A descriptive observation does not verify an artifact, import a package or establish backend compatibility.

A failed stale detail refresh propagates an error; it does not replace that failure with old descriptive metadata or return a result presented as a freshly hydrated match. This can make a lookup fail where it previously returned an incomplete filename/LFS suggestion. There is no offline stale-success policy in this slice.

Explicit public metadata Refetch retains `get_model_info` force revalidation. Download snapshots and revision resolution retain their live source/revision identity checks. Search ranking, catalog/SQLite logic, download preparation, acquisition, importer, runtime, vision and release production are not modified. The search request uses the existing `api_base_url` accessor; the default production URL is unchanged, and an existing test source override can now route the public discovery workflow entirely to owned loopback HTTP.

## Narrow bounded refresh admission

Simultaneous lookups can select the same repository before either detail response has been published. Anonymous detail reads therefore serialize by exact observation-file path, then recheck authentication and cache freshness after admission. An ordinary follower can reuse the newly published fresh detail without a duplicate detail GET. Explicit Refetch, no-cache/zero-freshness responses and unavailable disk persistence still require their own revalidation; this does not share stale or uncacheable payloads as fresh.

The process-local registry holds weak mutex references only. It retains neither clients, credentials, response bodies nor store leases. At most 64 distinct exact paths and 32 participants per path are admitted; exhausted local admission returns typed RateLimited for `hf-metadata-local-admission` with a one-second retry hint. There is no automatic retry, sleep loop, detached HTTP task, persistent queue, global HF bucket admission or new service.

Keys are lexical cache paths incorporating the exact source-URL hash. This is not physical filesystem identity: relative/absolute or symlink aliases need not coalesce, and separate processes are not coordinated by this registry. The existing physical store lifetime/custody owner remains responsible for its own exclusion; it is not replaced by cache locks.

Canceling a waiter or an HTTP-body reader drops its request/admission future. Once an atomic cache write or invalidation is handed to the existing owned blocking-effect primitive, its owned admission guard moves into the actual closure. It remains held until that effect settles even if the awaiting caller is canceled. A newer refresh or cooldown cannot overtake that admitted effect and then be overwritten or removed by it. No interruption of native filesystem effects is claimed.

## Authentication boundary

Authenticated detail requests take a live-only path and never read or mutate the anonymous observation envelope. A queued anonymous lookup rechecks authentication before cache reuse. If authentication disappears after selecting the live bypass, that invocation remains live-only; it cannot fall into anonymous publication without admission. Actual request credentials are still observed through the existing snapshot request path.

The privacy qualification is scoped to the new full-detail envelope. Existing search and repository-tree cache formats and credential policies are outside this slice. The tests do not claim that every legacy HF cache has been redesigned for private data. All test credentials are synthetic memory-only values scoped to owned HTTP fixtures; no token persistence or gated HF service is used.

## Public API qualification

The owned loopback suite calls the existing public `lookup_hf_metadata_for_file` API and public metadata Refetch. The local file is an actual 24-byte GGUF v3 header with empty metadata/tensor tables. It is a format fixture, not an inference-ready model. Fixture LFS identity/hash observations remain marked pending full verification, and `list_models` proves this read-only workflow publishes no library model.

Coverage includes cold/warm/reopened full card reuse; stale conditional 304 refresh; explicit Refetch with a new body; upstream failure without stale success; persisted detail cooldown; two credential contexts and authenticated denial without anonymous envelope mutation; simultaneous anonymous selection; canceled HTTP reader and waiter; canceled blocking publication followed by newer public Refetch; queued authentication changes; authentication cleared during live bypass; multiple-candidate filename fallback and empty search. Cache-module tests separately bound registry keys/participants and retain prior cache protocol coverage.

Request reduction is for selected full-detail hydration. Search can still run for each public lookup, and existing file-tree matching retains its own cache behavior. This does not claim a zero-network discovery call, RPC transport qualification, real HF validator availability or real inference.

## Recorded qualification

Qualification used locked offline dependencies with `ORT_SKIP_DOWNLOAD=1`, owned loopback sources and synthetic in-memory credentials. It did not require a real model download, gated HF access or a new RPC method.

At the selected-hydration stage, all 11 focused public API tests and all 20 cache-module tests passed. The broader no-default-features plus `hf-client` library suite passed 1,978 tests with 11 ignored and one environment baseline failure; all 262 HF tests and six SQLite cache tests passed within that run. The failure, `api::models::tests::get_inference_settings_batch_reports_per_model_errors`, attempted to create the default user registry on a read-only filesystem. The earlier observation-cache binary reproduced the same error at `api/models.rs:782`, and its 18 cache tests passed. Strict all-target Clippy, formatting and whitespace checks passed. These results qualify selected detail hydration and its admission behavior, not live HF, RPC transport or inference.
