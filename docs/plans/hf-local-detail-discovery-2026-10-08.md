# Local discovery from retained anonymous HF details

## Existing boundary and selected workflow

The [HF cache brief](../breif/hf-cache.md) calls for useful local search across previously learned repository records while upstream is unavailable. `HuggingFaceClient::search` reuses exact SQLite query identities and may rehydrate them online; new ordinary queries go online. Full-detail observations retain source, validation time and observed commit. The workflow below makes these retained observations explicitly browsable without changing local library FTS, which searches imported models.

The existing public `PumasApi::search_hf_models` and `search_hf_models_with_hydration` now accept the reserved query prefix **`cache:`**. `cache:` browses retained detail records; `cache:acme multilingual` searches them with case-insensitive conjunctive whitespace tokens. The same existing IPC/RPC search delegation uses that consumer. Ordinary queries retain their existing behavior; upstream errors, including authorization denials or rate limits, do not silently turn into local successes.

Examples:

```rust,ignore
// Zero upstream requests, even when disconnected. No detail/tree hydration.
api.search_hf_models("cache:", None, 25).await?;
api.search_hf_models_with_hydration("cache:acme multilingual", Some("llm"), 25, 0).await?;
```

This is bounded browse/search of retained details, not a full HF catalog or FTS producer. The native `HuggingFaceClient::search_cached_model_details(&HfSearchParams)` additionally accepts bounded offset and format filtering. Only records learned by anonymous detail hydration qualify; search-only SQLite records are outside this workflow. No new SQLite index, background producer, external service, peer distribution, new RPC method or frontend contract generator is added.

## Public result semantics

The existing `HuggingFaceModel` result type is preserved. Search considers repository ID, model name, developer, inferred task and tags; all query tokens must match the combined projection. Kind filters preserve the existing `llm`, `reranker`, `diffusion` and `audio` aliases. Native format filters match exact supported-format tags. Results sort by repository ID before pagination. Empty query browses, zero limit returns an empty result, no match returns empty, and capacity/admission failure returns an error instead of a fabricated empty or partial catalog.

Each returned `model_card.pumas_discovery` is replaced wholesale with owner-derived `source`, exact `source_url`, `observed_at`, `freshness` (`fresh` or `stale`), `fresh_until`, optional `revision_observed`, `visibility_observed` and `discovery_only: true`. Provider card data cannot spoof that reserved member. Freshness uses the persisted conservative TTL/Age/no-cache policy; stale entries stay discoverable in this explicit mode and are never relabeled fresh. Commit observations do not become immutable download revision authorization. Download options remain empty; compatibility hints are cleared because this projection does not qualify a package or backend.

Discovery reads perform no HTTP, authentication-header lookup, tree hydration, observation/LRU timestamp refresh, cache write or eviction. Actual snapshot/revision/file selection and acquisition retain their live admission checks; an offline discovery result cannot make an unavailable download succeed. Existing Refetch/detail getters retain their prior fresh/stale/error policies.

## Visibility, provenance and bounds

Only known valid observations from the current exact HF API source are considered. Their body must explicitly declare `private: false` and `gated: false`; missing, null, wrong-type, duplicate, private or gated declarations in those fields are excluded. `disabled: true`, wrong-type and duplicate disabled declarations are also excluded; absent/null disabled is unspecified. Typed decoding rejects duplicate visibility fields. Authenticated calls do not populate this anonymous store; current authentication does not expose authenticated records through this workflow. Anonymous retained public bytes describe **visibility at observation time**, not current online visibility: offline browsing cannot establish that a formerly public repository is still public. Unknown legacy visibility is excluded until a normal permitted anonymous detail refresh records it.

The reader reuses existing detail-family filename/identity checks, held-directory/non-following regular-file reads, source URL hashing, bounded observation/body validation and StoreLifetime ownership. Directory admission remains held by the actual read effect after caller cancellation. Whole-directory accounting must fit 256 exact-family regular records and 64 MiB logical final bytes, with at most 4096 directory entries; incomplete/nonregular/over-capacity inventories fail closed before returning results. Reads add at most one overflow probe per admitted entry. Query length is at most 256 bytes, kind/format at most 128 bytes each, result limit at most 100 and native offset at most 256. Unknown/corrupt/future/wrong-identity records remain unmodified and are skipped. Foreign cache families receive no discovery or cleanup authority.

Up to 256 candidate projections can be materialized within the 64 MiB input budget before sorting; at most 100 are returned. The existing cooperative exact-lexical-directory/current-process coordination boundary is unchanged. This feature does not add cross-process coordination, catalog completeness, semantic ranking, strict access LRU, stale-success to ordinary search or ongoing knowledge of private/gated status. The source indicator prevents custom endpoints being silently represented as observations from the default authority. The [cached search presentation](hf-cached-search-ui-2026-10-08.md) adds an explicit UI source chooser using this same existing search contract.

## Recorded qualification

Owned loopback fixtures exercise actual public `cache:` callers after shutting down upstream, new-query discovery without a historical exact query, browse/reopen, deterministic limits/filtering, explicit fresh/stale/revision/source projection, reserved provenance spoof replacement, authenticated/private/gated/unknown/duplicate visibility exclusion, current-source isolation, corrupt/future records, bounded/nonregular inventory and unchanged live acquisition checks. Cargo uses locked offline official dependencies and `ORT_SKIP_DOWNLOAD=1`; no large model or native ORT download is used.

At the local-discovery stage, 19 public metadata tests, including eight local-discovery workflows, and 32 cache tests passed, as did strict all-target Clippy, formatting and whitespace checks. The full `hf-client` library run passed 1,998 tests with 11 ignored and one environment baseline failure: `api::models::tests::get_inference_settings_batch_reports_per_model_errors`, creating the default user registry on a read-only filesystem. The failure reproduced in an earlier accepted observation-cache binary with the same failing API source blob as the immediate pre-discovery sources. This is not an all-green full-suite claim or an immediate-predecessor binary comparison.

This workflow depends on the anonymous observation protocol, per-path admission and aggregate detail-cache budget. Real-provider behavior, native desktop transport and inference remain separate qualification requirements. Ordinary search and live acquisition must continue requiring their own source checks.
