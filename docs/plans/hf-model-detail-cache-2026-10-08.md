# Hugging Face model-detail observation cache

## Scope and caller contract

This slice addresses an unfinished part of `docs/breif/hf-cache.md`: persistent full model-detail observations and conditional refresh. Existing SQLite search/repository records omit some wire detail and do not retain source URL, commit observation and HTTP validators together. The new module uses the existing HF file-cache directory, store lifetime and atomic JSON writer; it introduces no database, remote cache, credential requirement or download transport.

`HuggingFaceClient::get_model_info_cached(repo_id)` is an explicit advisory discovery/detail API. Fresh anonymous observations can return without HTTP, including after reopening a client. Existing `get_model_info(repo_id)` always revalidates upstream, using persisted conditional validators when available. That preserves explicit public API/RPC metadata Refetch behavior. Integrators can opt ordinary detail consumers into `get_model_info_cached`; no existing search/catalog/UI consumer is silently converted to cache-first behavior in this slice.

Both methods respect persisted admission cooldown before issuing HTTP; a still-fresh advisory hit may be returned without HTTP during cooldown. Authenticated calls bypass anonymous disk observations and retain the existing live snapshot request. Download snapshot retrieval and revision resolution remain separate live identity checks. A cached `sha` is an observation, never a newly authorized immutable download revision, package admission or inference compatibility claim.

## Observation identity and persistence

Each versioned JSON envelope contains the exact anonymous model API URL, original bounded JSON body, ETag/Last-Modified validators, validation timestamp, bounded freshness and optional cooldown deadline. The filename hashes the full URL, avoiding legacy slash/underscore aliases and separating repository case and source endpoints. Read admission checks the envelope version and exact URL, requested repository, optional commit syntax, validators and body bounds. Corrupt, oversized, wrong-source or future-version records are misses. Future validation timestamps do not produce fresh hits.

The body is capped at 4 MiB; the envelope read is capped at that plus 64 KiB. Unix reads use nonblocking, no-follow opens and admit only regular files, so a FIFO or symlink cache entry cannot hold the worker waiting for a writer. Existing blocking-effect ownership and atomic JSON publication are reused. Cache read/write/invalidation failures do not turn a successful live response into a model-detail failure. No partial downloaded response is published.

Freshness defaults to at most 24 hours. Bounded Cache-Control fields can shorten it; no-cache forces revalidation and private/no-store prevents persistence. All repeated Cache-Control and Vary fields are inspected; Authorization or wildcard Vary prevents storage, and malformed policy fields are conservative. Age reduces freshness; repeated or invalid Age forces revalidation. This is a narrow native-client observation policy, not a general HTTP proxy cache: Expires and configurable discovery TTL are not implemented.

## Refresh and failure behavior

A stale or explicitly refreshed observation sends If-None-Match, otherwise If-Modified-Since. A 200 response replaces the validated body/validators atomically. A 304 requires an actual conditional request and the matching stored representation at the same source URL. Contradictory ETags or a contradictory Last-Modified fallback cannot extend freshness or pair old bytes with a different validator. A 304 without new Cache-Control cannot relax the existing freshness policy.

HTTP 401/403/404 invalidates the anonymous record. Other upstream failures retain the old observation without returning it as a successful fresh lookup. Redirected responses are not persisted; redirected 304 responses cannot refresh the original record.

HTTP 429 returns typed RateLimited and persists a per-source/repository admission deadline. Retry-After accepts strict integer seconds or an HTTP date. Otherwise the documented HF RateLimit API bucket reset is used; missing or unusable reset information uses a five-minute cooldown. Valid reset values are capped at 24 hours to prevent arithmetic overflow and unbounded suppression. Calls needing revalidation, including explicit Refetch and reopened clients, return the remaining cooldown without sending HTTP. A still-fresh advisory observation remains usable without HTTP. There are no automatic retries or sleeping request tasks.

Official protocol references: [HF rate-limit headers](https://huggingface.co/docs/hub/rate-limits) and [HTTP semantics for conditional requests, 304 and Retry-After](https://www.rfc-editor.org/rfc/rfc9110.html). Conditional reuse depends on the actual upstream supplying usable validators; local fixtures establish behavior, not a claim that live HF always sends these headers.

## Boundaries and integration

The initial observation-cache protocol did not serialize concurrent refreshes. The [selected metadata caller and admission extension](hf-metadata-caller-adoption-2026-10-08.md) adds bounded per-path serialization and a freshness recheck after admission. A cooldown prevents subsequent requests after publication, not requests already in flight; it remains repository-URL scoped rather than global HF API-bucket admission.

Per-record resource bounds are independent of the SQLite cache's 4 GB LRU. The [aggregate detail-cache budget](hf-model-detail-cache-budget-2026-10-08.md) additionally bounds this JSON family to 256 final records and 64 MiB. The [explicit local discovery workflow](hf-local-detail-discovery-2026-10-08.md) and [cached search presentation](hf-cached-search-ui-2026-10-08.md) expose retained public observations without changing ordinary refresh errors into stale successes. Authenticated/private persistence, a shared online cache and global API-bucket admission remain outside this protocol.

## Verification

The deterministic owned loopback HTTP suite covers restart reuse with full card/license preservation, exact endpoint/repository isolation, ETag and Last-Modified refresh, no-cache across 304, explicit existing Refetch getter behavior, auth bypass, live snapshot/revision isolation, rate-limit reopen/expiry, unavailable storage, corrupt/future/wrong-source records, malformed identity/commit/oversized bodies, repeated policy fields, contradictory 304 validators, huge reset values and nonblocking FIFO admission. It uses a synthetic local bearer value only for auth-isolation assertions; it does not access gated HF repositories or require a token.

Recorded qualification used locked offline official dependencies and `ORT_SKIP_DOWNLOAD=1`; no live HF scraping, model download or native ONNX execution was required.

At the observation-cache stage, 18 focused cache tests passed. The broader no-default-features plus `hf-client` library run passed 1,965 tests with 11 ignored and one environment baseline failure: `api::models::tests::get_inference_settings_batch_reports_per_model_errors`, creating the default user registry on a read-only filesystem. The earlier baseline binary reproduced the same error at `api/models.rs:782`; no registry fixture was changed to mask it. All 260 HF tests and six SQLite cache tests passed in that run; one additional HF resource measurement remained ignored. Baseline filtered HF tests passed 242 with one ignored, and baseline SQLite cache tests passed six. Strict all-target Clippy, formatting and whitespace checks passed. These counts describe that protocol stage rather than a new full-suite run of subsequent extensions.
