# Hugging Face model-detail cache

**Status:** implemented in 0.8.0. Owner:
`rust/crates/pumas-core/src/model_library/hf/metadata/model_info_cache.rs` and
its `budget` and `local_discovery` modules. Direction and the parts not yet
built (shared online cache, bulk index, other catalogs) live in the
[HF cache brief](../breif/hf-cache.md).

Pumas keeps anonymous Hugging Face model-detail responses on disk so repeated
lookups and offline browsing do not have to call the Hub. A cached detail is a
dated observation, never authority: download revision, file selection,
package admission and inference compatibility keep their own live checks.

## Entry points

| Entry point | Behavior |
| --- | --- |
| `HuggingFaceClient::get_model_info_cached(repo_id)` | Advisory. A fresh anonymous observation returns without HTTP, also after reopening the client. A stale one is revalidated. |
| `HuggingFaceClient::get_model_info(repo_id)` | Always revalidates upstream (metadata **Refetch**), using stored validators when present. |
| `PumasApi::lookup_hf_metadata_for_file` | Hydrates only the selected match through `get_model_info_cached`. Other candidates get no detail request. |
| `PumasApi::search_hf_models[_with_hydration]` with a `cache:` query | Browses (`cache:`) or searches (`cache:acme multilingual`) retained details with no HTTP. Native `search_cached_model_details` also takes an offset and a format filter. |
| `HuggingFaceClient::model_detail_cache_usage()` | `(record_count, logical_bytes)` without HTTP. Limits: `MODEL_DETAIL_CACHE_RECORD_LIMIT`, `MODEL_DETAIL_CACHE_BYTE_LIMIT`. |

Ordinary interactive search passes `hydrate_limit = 0`; opening one result's
download menu requests that repository's details.

## Storage and freshness

- One versioned JSON envelope per exact anonymous model API URL, named
  `hf_<64 lowercase hex>_metadata_v1.json` after the URL hash. It holds the
  bounded response body, ETag/Last-Modified, validation time, freshness and any
  cooldown deadline.
- Body at most 4 MiB; envelope read at most 4 MiB + 64 KiB. Only regular
  files are read, without following symlinks.
- Freshness is at most 24 hours. `Cache-Control` can shorten it; `no-cache`
  forces revalidation; `private`/`no-store`, `Authorization` in `Vary` or a
  wildcard `Vary` prevent storage; `Age` reduces freshness. `Expires` is not
  used.
- Stale records revalidate with `If-None-Match`, else `If-Modified-Since`. A
  304 refreshes only the matching stored representation at the same URL;
  redirected responses are never stored.
- HTTP 401/403/404 delete the record. Other failures keep it but never return
  it as a fresh success; a failed stale refresh in file lookup is an error.
- Corrupt, oversized, wrong-source, future-version or future-dated records are
  misses and are left untouched.

## Rate limits and admission

- HTTP 429 returns `RateLimited` and stores a cooldown for that source and
  repository: `Retry-After` (seconds or HTTP date), else the HF rate-limit
  reset, else 5 minutes; capped at 24 hours. During a cooldown, calls that need
  revalidation return the remaining wait without HTTP; a fresh record is still
  served. There are no automatic retries.
- Concurrent anonymous refreshes of the same record are serialized and recheck
  freshness after admission. At most 64 distinct records and 32 participants
  per record are in flight; beyond that, `RateLimited` with service
  `hf-metadata-local-admission` and a 1-second retry hint.
- Authenticated calls never read or write this cache.

## Budget

- At most 256 records and 64 MiB of final-file bytes. A write evicts the
  oldest observations (by validation time, then filename) to fit; this is not
  access-order LRU, and reads never touch timestamps.
- Records in an active cooldown and records that cannot be classified are
  never evicted. If they leave no room, the new record is not stored and the
  live result is still returned.
- A scan covers at most 4096 directory entries; an incomplete scan refuses new
  persistence rather than evicting from a partial view.
- Coordination is per process and per lexical directory path; separate
  processes and path aliases are not coordinated.

## Offline discovery (`cache:`)

- Only records from the current HF API source whose body states
  `private: false` and `gated: false` (and not `disabled: true`) appear.
  Visibility is as observed, not as it is now.
- Query at most 256 bytes, kind and format at most 128 bytes each, limit at
  most 100, native offset at most 256. Results sort by repository ID; all query
  tokens must match.
- Each result's `model_card.pumas_discovery` is set by Pumas: `source`,
  `source_url`, `observed_at`, `freshness` (`fresh`/`stale`), `fresh_until`,
  optional `revision_observed`, `visibility_observed`, `discovery_only: true`.
  Download options are empty and compatibility hints are cleared.
- The desktop shows this as **Cached details (offline)** in Model Manager
  search, with source, observation time, freshness and revision on each row,
  and offers no download choices from it. Ordinary search never falls back to
  cached results.
- Discovery makes no HTTP request and writes nothing.

## Verification

Owned loopback HTTP tests in the `model_info_cache` modules and the public HF
metadata tests; the desktop path is covered by
`scripts/tests/qualify-hf-cached-search-ui.py` against a launched `pumas-rpc`.
