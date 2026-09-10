# Hugging Face Discovery Cache Brief

## Purpose

Pumas Library currently uses the Hugging Face Hub API for model discovery and repository metadata. Although Pumas already maintains a local SQLite cache for search results and repository details, normal interactive use can still generate enough upstream API traffic to encounter Hugging Face rate limits.

The goal of this work is to make Hugging Face discovery primarily cache-driven, using Hugging Face as the authoritative upstream source without requiring every user search to become an upstream API request.

The desired architecture is:

```text
Local Pumas cache
        ↓ miss/stale
Shared or bulk online cache
        ↓ miss/stale
Hugging Face API
```

Hugging Face remains authoritative, but should become the last discovery source rather than the first.

## Existing Local Cache

Pumas already maintains a SQLite-backed Hugging Face cache containing:

* exact cached search queries and result repository IDs;
* cached repository details;
* configurable search-result TTLs;
* repository-detail freshness checks;
* cache size limits and eviction;
* compatibility and extracted download information.

The current search cache is primarily an **exact request cache**. Query text, filters, pagination, and limits form part of the cache identity. As a result, a previously discovered repository does not necessarily help answer a different search query.

Cached search results may also be re-enriched with repository/download information, which can still cause Hugging Face API traffic when repository-detail cache entries are unavailable, stale, or considered incomplete.

The local cache should remain the fastest and most private cache layer, but its role should expand from exact-request reuse toward a local discovery catalog.

## Local Discovery Catalog

Repository records learned from any source should be retained in a locally searchable catalog.

Useful indexed fields may include:

* Hugging Face repository ID;
* author or organization;
* model name;
* task or pipeline type;
* tags;
* supported formats;
* known quantizations;
* parameter or artifact size information where available;
* download and popularity metadata;
* last-modified information;
* Pumas compatibility and package facts;
* source and observation timestamp.

SQLite FTS can provide immediate local search over previously discovered models.

An interactive search should therefore first search accumulated local repository records rather than only checking for an identical historical query.

## Search and Hydration Separation

Search discovery should be kept lightweight.

A search should not automatically require detailed hydration of every returned repository. Repository file trees, download options, artifact details, and other expensive metadata should normally be fetched only when required, such as when:

* the user opens model details;
* a particular model becomes relevant to an operation;
* cached detailed metadata must be refreshed;
* a download is requested.

This should reduce one visible search from potentially many Hugging Face requests to normally zero or one upstream discovery request.

## Shared Online Cache

Pumas may optionally use a shared Internet cache before contacting Hugging Face directly.

A small hosted service could contain normalized search observations and repository metadata contributed by Pumas clients or collected centrally.

Potential hosting options include lightweight services such as:

* Cloudflare D1;
* Vercel
* Turso/libSQL;
* another inexpensive SQL or key/value service;
* a small custom Pumas catalog API.

Clients should not connect directly to privileged database credentials. A narrow service API should validate submitted and returned records.

Example flow:

```text
Pumas client
    ↓ local miss
Pumas shared catalog/cache
    ↓ shared miss
Hugging Face
    ↓
local cache + optional shared observation
```

This allows one user's Hugging Face request to benefit later users.

Client-submitted data should be treated as observations rather than unquestioned authority. Records should include provenance and timestamps, and important metadata can later be refreshed from Hugging Face.

## Bulk Hugging Face Index Sources

Pumas should investigate existing bulk or semi-bulk sources that can bootstrap a large model catalog without individually searching through the Hub API.

One candidate is the Hugging Face `hub-stats` model dataset, which may provide a periodically updated index of a large portion of the Hub.

A bulk source could be transformed into a compact Pumas catalog:

```text
Hugging Face bulk dataset/index
        ↓
Pumas catalog builder
        ↓
SQLite or Parquet snapshot
        ↓
Pumas clients
```

Clients could periodically download a compressed catalog snapshot and perform most model discovery locally.

The bulk catalog does not need minute-by-minute freshness. Model selection or detailed inspection can still trigger an authoritative Hugging Face lookup when current information matters.

## Other Catalog Sources

Third-party curated catalogs may also be useful as supplementary sources.

For example, LM Studio maintains its own model catalog. If an officially supported machine-readable catalog interface is available, Pumas could consume it as an additional source of model discovery or compatibility information.

Undocumented private service endpoints should not become a required dependency.

Different sources should remain distinguishable through provenance rather than being silently merged into authoritative facts.

## Optional Peer-to-Peer Sharing

A peer-to-peer cache could potentially allow Pumas clients to exchange repository observations directly.

This should be considered optional and lower priority than a central shared cache because it introduces additional problems:

* peer discovery and NAT traversal;
* stale or poisoned records;
* trust and signature requirements;
* privacy of user searches;
* protocol/version compatibility;
* offline availability;
* Sybil and abuse resistance.

A future P2P layer may be useful for distributing signed catalog snapshots or cache records, but the initial design should not depend on it.

## Freshness Model

Different information should have different freshness requirements.

For example:

* interactive search results can tolerate relatively old cached data;
* popularity/download counts can be refreshed infrequently;
* repository last-modified metadata can determine whether deeper cached data needs inspection;
* artifact details required for download or execution should be refreshed when correctness depends on them.

Stale-but-valid search results should normally remain usable while a refresh is unavailable.

Freshness should not be confused with authority: cached data may be valid for discovery without being authoritative enough to permit a mutation or execution decision.

## Hugging Face Resolver Usage

Once Pumas knows the repository, revision, and file path it requires, file access should use Hugging Face's resolver/download infrastructure where appropriate rather than relying on general Hub API requests.

The Hub API should primarily serve:

* discovery misses;
* current repository metadata where required;
* operations that cannot be performed through resolver/file endpoints.

This preserves API quota for operations that actually require the API.

## Desired Outcome

The long-term discovery architecture should resemble:

```text
                  Hugging Face
                 /            \
        bulk catalog          API/details
             ↓                    ↓
       Pumas shared catalog/cache
                  ↓
           local Pumas catalog
                  ↓
             SQLite FTS
                  ↓
              user search
```

The system should provide:

* immediate local search;
* useful offline discovery from previously known models;
* substantially fewer Hugging Face API calls;
* shared benefit from searches performed by other Pumas users;
* clear provenance and freshness;
* graceful behavior during Hugging Face rate limiting or outages;
* no requirement for Pumas desktop clients to hold shared-service database credentials;
* continued use of Hugging Face as the authoritative upstream source.

## Development-Plan Questions

A future implementation plan should determine:

* whether the existing `repo_details` cache can evolve into the local discovery catalog or should be separated;
* which fields belong in the catalog versus detailed repository hydration;
* which current enrichment calls can be deferred;
* search debounce and request-coalescing policy;
* stale-while-revalidate behavior;
* rate-limit header tracking and shared request admission;
* bulk-index source quality, licensing, size, and update frequency;
* shared-cache hosting and authentication;
* contribution validation and poisoning resistance;
* privacy policy for shared search observations;
* catalog schema/version migration;
* snapshot distribution and incremental updates;
* how provenance and freshness are represented in the public Pumas API.
