# HuggingFace Search Cache

This document describes the SQLite-based caching system for HuggingFace model searches.

## Overview

The caching system minimizes HuggingFace API calls while keeping data fresh. It uses a two-table design that separates search result ordering from model details, allowing cached model data to be reused across different searches.

**Cache location**: `/shared-resources/cache/search.sqlite`

## Database Schema

### search_cache

Stores search queries and their result ordering (repo IDs only, not full details).

| Column | Type | Description |
|--------|------|-------------|
| query_normalized | TEXT | Lowercase, trimmed search query |
| kind | TEXT | Optional filter (null = all) |
| result_limit | INTEGER | Number of results requested |
| result_offset | INTEGER | Pagination offset |
| result_repo_ids | TEXT | JSON array of repo IDs in order |
| searched_at | TEXT | ISO 8601 timestamp |

**Primary Key**: (query_normalized, kind, result_limit, result_offset)

### repo_details

Stores full model details including download sizes.

| Column | Type | Description |
|--------|------|-------------|
| repo_id | TEXT | HuggingFace repository ID (primary key) |
| last_modified | TEXT | From HF API, used for invalidation |
| name | TEXT | Model name |
| developer | TEXT | Developer/organization |
| kind | TEXT | Model type (e.g., "text-generation") |
| formats | TEXT | JSON array of formats |
| quants | TEXT | JSON array of quantizations |
| download_options | TEXT | JSON array of {quant, size_bytes} |
| url | TEXT | URL to model page |
| downloads | INTEGER | Download count |
| total_size_bytes | INTEGER | Total size |
| cached_at | TEXT | When this was cached |
| last_accessed | TEXT | For LRU eviction |
| data_size_bytes | INTEGER | For cache size tracking |

### cache_config

Stores configurable settings.

| Key | Default | Description |
|-----|---------|-------------|
| max_size_bytes | 4294967296 (4GB) | Maximum cache size |
| search_ttl_seconds | 86400 (24h) | How long search results are valid |
| last_modified_check_threshold | 86400 (24h) | Age before checking lastModified |
| background_refresh_enabled | true | Enable background refresh |
| rate_limit_window_seconds | 300 (5min) | HuggingFace rate limit window |

## Search Flow

```
User searches "Llama GGUF"
         │
         ▼
Normalize query: "llama gguf"
         │
         ▼
Check search_cache for (query, kind, limit, offset)
         │
         ├─────────────────────────────────────────┐
         │                                         │
    Cache HIT                                 Cache MISS
    (< 24 hours)                              (or stale)
         │                                         │
         ▼                                         ▼
Get repo_ids from cache                    Make HF API search call
         │                                         │
         ▼                                         ▼
For each repo_id:                          For each returned model:
  Get details from repo_details              Check repo_details cache
         │                                         │
         ▼                                    ┌────┴────┐
Update last_accessed timestamps              │         │
         │                                Cached    Not cached
         ▼                                   │         │
Return enriched results                      ▼         ▼
(0 API calls)                           Check if    Fetch repo
                                        needs       tree API
                                        refresh          │
                                           │             │
                                      ┌────┴────┐        │
                                      │         │        │
                                   Fresh     Stale       │
                                      │         │        │
                                      ▼         ▼        ▼
                                Use cached   Fetch    Insert
                                details      fresh    cache
                                      │         │        │
                                      └────┬────┴────────┘
                                           │
                                           ▼
                                Cache search results
                                           │
                                           ▼
                                Return enriched results
```

## Cache Invalidation

The cache uses **lastModified-based invalidation** rather than simple time-based TTL:

1. **Search results**: Invalidate after `search_ttl_seconds` (default 24 hours)

2. **Model details**: Invalidate when ALL of these are true:
   - Cache age > `last_modified_check_threshold` (default 24 hours)
   - Search result has a newer `lastModified` than cached value

This means:
- If a repo hasn't changed on HuggingFace, cached data is never refetched
- Only models that were actually updated trigger API calls
- Fresh cache (< 24 hours) is always used without checking

## LRU Eviction

When the cache exceeds `max_size_bytes` (default 4GB):

1. Calculate current size from `SUM(data_size_bytes)`
2. Query entries ordered by `last_accessed ASC`
3. Delete oldest entries until size is under limit
4. Clean up orphaned `search_cache` entries

The eviction is triggered after each `cache_repo_details()` call but runs asynchronously to avoid blocking.

## Background Refresh

Background refresh is designed for **local library models only** (models the user has downloaded):

- **Never refreshes** random search results
- **Rate-limit aware**: After one refresh, waits until near end of rate limit window (~4.5 min of 5 min)
- **User-priority**: Pauses when user is actively making API calls
- **Resumes** when user activity subsides

This ensures background refresh never interferes with the user's active usage.

## Configuration

Configuration can be modified via the `HfCacheConfig` struct:

```rust
pub struct HfCacheConfig {
    pub max_size_bytes: u64,                    // default: 4GB
    pub search_ttl_seconds: u64,                // default: 24 hours
    pub last_modified_check_threshold: u64,     // default: 24 hours
    pub background_refresh_enabled: bool,       // default: true
    pub rate_limit_window_seconds: u64,         // default: 300 (5 min)
}
```

Example:

```rust
let cache = HfSearchCache::new("/shared-resources/cache/search.sqlite")?;
let mut config = cache.get_config()?;
config.max_size_bytes = 2 * 1024 * 1024 * 1024; // 2GB
cache.set_config(&config)?;
```

## Graceful Degradation

If the SQLite database is unavailable or corrupted:

1. Cache operations return errors but don't panic
2. The HuggingFace client falls back to direct API calls
3. Search functionality continues to work (without caching)
4. Errors are logged for debugging

The system is designed to be resilient - caching is an optimization, not a requirement.

## API Methods

### HfSearchCache

| Method | Description |
|--------|-------------|
| `new(path)` | Create cache at path |
| `with_config(path, config)` | Create with custom config |
| `get_config()` | Get current configuration |
| `set_config(&config)` | Update configuration |
| `get_search_results(query, kind, limit, offset)` | Get cached search |
| `cache_search_results(...)` | Store search results |
| `get_repo_details(repo_id)` | Get cached model details |
| `cache_repo_details(&model)` | Store model details |
| `needs_refresh(repo_id, last_modified)` | Check if refresh needed |
| `check_and_evict()` | Run LRU eviction |
| `get_stats()` | Get cache statistics |
| `clear()` | Clear all cached data |
