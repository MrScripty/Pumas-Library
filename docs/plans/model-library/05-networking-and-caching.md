# Networking & Caching Strategy

**Version**: 1.0
**Status**: Planning Phase

---

## Table of Contents

- [Overview](#overview)
- [Integration with Model Library](#integration-with-model-library)
- [Unified Network Stack](#unified-network-stack)
- [Search Coordination](#search-coordination)
- [SQLite FTS5 for Search](#sqlite-fts5-for-search)
- [Dual-Path Search Strategy](#dual-path-search-strategy)
- [Database Design](#database-design)
- [Implementation Roadmap](#implementation-roadmap)
- [Success Criteria](#success-criteria)
- [Related Documents](#related-documents)

---

## Overview

This document integrates networking, caching, and search functionality into the Model Library System. It implements a **Unified Network Stack** using HTTPX with HTTP/2 multiplexing, **SQLite FTS5** for sub-millisecond search, and a **Dual-Path Strategy (Stale-While-Revalidate)** for instant UI updates.

### Key Differences from Base Plan

The model library plan already includes:
- Basic metadata caching (24-hour TTL for HuggingFace repo lists)
- SQLite WAL mode for concurrent access
- 5-second timeouts for API calls
- Offline-first import strategy

This document **extends** those foundations with:
- HTTP/2 multiplexing for concurrent metadata requests
- Circuit breaker pattern for fail-fast offline detection
- FTS5 full-text search for instant model filtering
- Stale-while-revalidate pattern for UI responsiveness
- Request sequence guarding to prevent race conditions
- Proactive rate-limit detection

---

## Integration with Model Library

### Compatibility Analysis

The search coordinator plan and model library plan are **complementary**:

| Feature | Model Library Plan | Search Coordinator Plan | Integration Strategy |
|---------|-------------------|------------------------|---------------------|
| **Database** | SQLite WAL for models.db | FTS5 for search | Extend models.db with FTS5 virtual table |
| **Networking** | Basic requests with 5s timeout | HTTPX HTTP/2 + Circuit Breaker | Replace requests with HTTPX in downloader.py |
| **Caching** | 24h LRU cache for repo lists | Aggressive metadata caching | Keep existing cache, add SWR pattern |
| **Offline Mode** | Offline-first import | Circuit breaker fail-fast | Unified: Circuit breaker enhances offline detection |
| **Search** | Basic filter in useModels hook | FTS5 + Debouncing | Add FTS5 index, client-side debouncing |

### Integration Points

1. **models.db Schema Extension**
   - Add FTS5 virtual table alongside existing models table
   - Sync FTS5 index when models are imported/updated

2. **Downloader Replacement**
   - Replace `requests` with `httpx.AsyncClient` in `backend/model_library/downloader.py`
   - Add circuit breaker wrapper around API calls
   - Keep existing 24h cache, add SWR refresh logic

3. **Search Enhancement**
   - Add FTS5 search endpoint to `backend/api/core.py`
   - Add debouncing (300ms) in `frontend/src/hooks/useModels.ts`
   - Add sequence ID tracking to prevent race conditions

4. **UI Updates**
   - Add "Using Cached Data" indicator when offline
   - Add "Refreshing..." indicator during background revalidation
   - Add "Rate Limit Warning" when approaching limits

---

## Unified Network Stack

### HTTPX Client Configuration

**File**: `backend/model_library/network_manager.py` (NEW)

```python
import httpx
import time
from typing import Optional, Dict, Any
from threading import Lock
from datetime import datetime, timedelta

class NetworkManager:
    """
    Centralized network client with HTTP/2 multiplexing and circuit breaker.

    Features:
    - HTTP/2 multiplexing for concurrent requests over single TCP connection
    - Circuit breaker pattern (3 failures → 60s blackout)
    - Request timeout (7 seconds)
    - Rate limit detection via X-RateLimit-Remaining header
    - Thread-safe singleton
    """

    _instance: Optional['NetworkManager'] = None
    _lock = Lock()

    def __new__(cls):
        if cls._instance is None:
            with cls._lock:
                if cls._instance is None:
                    cls._instance = super().__new__(cls)
        return cls._instance

    def __init__(self):
        if hasattr(self, '_initialized'):
            return

        self._initialized = True
        self._client: Optional[httpx.AsyncClient] = None
        self._circuit_breaker: Dict[str, 'CircuitBreakerState'] = {}
        self._rate_limit_warning = False

    async def get_client(self) -> httpx.AsyncClient:
        """Get or create HTTPX client with HTTP/2."""
        if self._client is None:
            self._client = httpx.AsyncClient(
                http2=True,
                timeout=httpx.Timeout(7.0),  # 7-second timeout
                limits=httpx.Limits(
                    max_keepalive_connections=10,
                    max_connections=20
                )
            )
        return self._client

    async def request(
        self,
        method: str,
        url: str,
        **kwargs
    ) -> httpx.Response:
        """
        Make HTTP request with circuit breaker protection.

        Args:
            method: HTTP method
            url: Request URL
            **kwargs: Additional request parameters

        Returns:
            httpx.Response

        Raises:
            CircuitBreakerOpen: If domain is in blackout period
            httpx.TimeoutException: If request times out
        """
        domain = self._extract_domain(url)

        # Check circuit breaker
        if self._is_circuit_open(domain):
            raise CircuitBreakerOpen(
                f"Circuit breaker OPEN for {domain}. "
                f"Retry after {self._get_retry_after(domain)}s"
            )

        try:
            client = await self.get_client()
            response = await client.request(method, url, **kwargs)

            # Check rate limit headers
            self._check_rate_limit(response)

            # Reset circuit breaker on success
            self._record_success(domain)

            return response

        except (httpx.TimeoutException, httpx.ConnectError) as e:
            # Record failure
            self._record_failure(domain)
            raise

    def _is_circuit_open(self, domain: str) -> bool:
        """Check if circuit breaker is open for domain."""
        if domain not in self._circuit_breaker:
            return False

        state = self._circuit_breaker[domain]

        # Check if blackout period has expired
        if datetime.now() > state.blackout_until:
            # Reset circuit breaker
            del self._circuit_breaker[domain]
            return False

        return True

    def _record_failure(self, domain: str):
        """Record request failure and potentially open circuit."""
        if domain not in self._circuit_breaker:
            self._circuit_breaker[domain] = CircuitBreakerState(
                consecutive_failures=1,
                blackout_until=None
            )
        else:
            state = self._circuit_breaker[domain]
            state.consecutive_failures += 1

            # Open circuit after 3 consecutive failures
            if state.consecutive_failures >= 3:
                state.blackout_until = datetime.now() + timedelta(seconds=60)
                logger.warning(
                    f"Circuit breaker OPENED for {domain} "
                    f"(60s blackout after {state.consecutive_failures} failures)"
                )

    def _record_success(self, domain: str):
        """Record successful request and reset circuit breaker."""
        if domain in self._circuit_breaker:
            del self._circuit_breaker[domain]

    def _check_rate_limit(self, response: httpx.Response):
        """Check for rate limit headers and warn if close to limit."""
        remaining = response.headers.get('X-RateLimit-Remaining')
        limit = response.headers.get('X-RateLimit-Limit')

        if remaining and limit:
            try:
                remaining_int = int(remaining)
                limit_int = int(limit)

                # Warn if below 10% of limit
                threshold = limit_int * 0.1

                if remaining_int < threshold:
                    if not self._rate_limit_warning:
                        self._rate_limit_warning = True
                        logger.warning(
                            f"API rate limit approaching: {remaining_int}/{limit_int} requests remaining"
                        )
            except ValueError:
                pass

    def _get_retry_after(self, domain: str) -> int:
        """Get seconds until circuit breaker closes."""
        if domain not in self._circuit_breaker:
            return 0

        state = self._circuit_breaker[domain]
        if state.blackout_until is None:
            return 0

        delta = state.blackout_until - datetime.now()
        return max(0, int(delta.total_seconds()))

    def _extract_domain(self, url: str) -> str:
        """Extract domain from URL."""
        from urllib.parse import urlparse
        parsed = urlparse(url)
        return parsed.netloc

    async def close(self):
        """Close client and cleanup resources."""
        if self._client is not None:
            await self._client.aclose()
            self._client = None


class CircuitBreakerState:
    """State tracking for circuit breaker."""
    def __init__(self, consecutive_failures: int, blackout_until: Optional[datetime]):
        self.consecutive_failures = consecutive_failures
        self.blackout_until = blackout_until


class CircuitBreakerOpen(Exception):
    """Raised when circuit breaker is open."""
    pass


# Global singleton
network_manager = NetworkManager()
```

### Integration with Downloader

**File**: `backend/model_library/downloader.py` (UPDATE)

```python
# Replace requests imports
import httpx
from .network_manager import network_manager, CircuitBreakerOpen

class ModelDownloader:
    def __init__(self):
        # ... existing init ...
        self._session = None  # Remove requests.Session

    async def lookup_model_metadata_by_filename(
        self,
        filename: str,
        file_path: Optional[Path] = None,
        timeout: float = 5.0
    ) -> Optional[dict]:
        """
        Lookup HF metadata using NetworkManager.

        Circuit breaker automatically handles offline scenarios.
        """
        try:
            # Use NetworkManager for API calls
            response = await network_manager.request(
                'GET',
                f'https://huggingface.co/api/models?search={filename}',
            )

            if response.status_code == 200:
                # Process response...
                return self._process_search_results(response.json(), filename)

        except CircuitBreakerOpen as e:
            # Circuit breaker is open - return cached data
            logger.info(f"Using cached data: {e}")
            return self._get_cached_metadata(filename)

        except httpx.TimeoutException:
            logger.warning(f"Request timeout for {filename}")
            return self._get_cached_metadata(filename)

        return None
```

---

## Search Coordination

### Debouncing Logic

**File**: `frontend/src/hooks/useModels.ts` (UPDATE)

```typescript
import { useState, useEffect, useRef } from 'react';
import { debounce } from 'lodash'; // or implement custom debounce

export function useModels() {
  const [searchQuery, setSearchQuery] = useState('');
  const [sequenceId, setSequenceId] = useState(0);
  const lastRenderedId = useRef(0);

  // Debounced search with 300ms delay
  const debouncedSearch = useRef(
    debounce(async (query: string, seqId: number) => {
      try {
        const results = await api.searchModels(query);

        // Sequence guard: Discard if stale
        if (seqId > lastRenderedId.current) {
          lastRenderedId.current = seqId;
          setModels(results);
        } else {
          console.debug(`Discarded stale results for sequence ${seqId}`);
        }
      } catch (error) {
        console.error('Search failed:', error);
      }
    }, 300)
  ).current;

  const handleSearch = (query: string) => {
    setSearchQuery(query);

    // Increment sequence ID
    const newSeqId = sequenceId + 1;
    setSequenceId(newSeqId);

    // Trigger debounced search
    debouncedSearch(query, newSeqId);
  };

  return {
    searchQuery,
    handleSearch,
    models,
    // ... other state
  };
}
```

### Sequence Guard Implementation

The sequence ID system prevents race conditions where typing "LLAMA" quickly might result in search results arriving out of order:

1. User types "L" → `sequence_id = 1` → API call
2. User types "LL" → `sequence_id = 2` → API call
3. User types "LLA" → `sequence_id = 3` → API call
4. Results arrive: **3, 1, 2** (out of order)

With sequence guard:
- Result 3 arrives → `lastRenderedId = 3` → Render
- Result 1 arrives → `1 < 3` → **Discard** (stale)
- Result 2 arrives → `2 < 3` → **Discard** (stale)

Only the most recent search results are displayed.

---

## SQLite FTS5 for Search

### FTS5 Virtual Table

**File**: `backend/model_library/library.py` (UPDATE)

```python
def _initialize_database(self):
    """Initialize SQLite database with FTS5 for search."""
    # ... existing initialization ...

    # Create FTS5 virtual table for full-text search
    cursor.execute("""
        CREATE VIRTUAL TABLE IF NOT EXISTS model_search USING fts5(
            model_id UNINDEXED,
            repo_id,
            official_name,
            family,
            tags,
            description,
            tokenize='unicode61 remove_diacritics 1 tokenchars "-_"'
        )
    """)

    # Trigger to keep FTS5 in sync with models table
    cursor.execute("""
        CREATE TRIGGER IF NOT EXISTS models_ai AFTER INSERT ON models BEGIN
            INSERT INTO model_search(
                model_id, repo_id, official_name, family, tags, description
            )
            VALUES (
                NEW.model_id,
                NEW.repo_id,
                NEW.official_name,
                NEW.family,
                NEW.tags,
                NEW.description
            );
        END
    """)

    cursor.execute("""
        CREATE TRIGGER IF NOT EXISTS models_au AFTER UPDATE ON models BEGIN
            DELETE FROM model_search WHERE model_id = OLD.model_id;
            INSERT INTO model_search(
                model_id, repo_id, official_name, family, tags, description
            )
            VALUES (
                NEW.model_id,
                NEW.repo_id,
                NEW.official_name,
                NEW.family,
                NEW.tags,
                NEW.description
            );
        END
    """)

    cursor.execute("""
        CREATE TRIGGER IF NOT EXISTS models_ad AFTER DELETE ON models BEGIN
            DELETE FROM model_search WHERE model_id = OLD.model_id;
        END
    """)
```

### FTS5 Search Query

**File**: `backend/api/core.py` (UPDATE)

```python
def search_models_fts(
    self,
    query: str,
    limit: int = 100,
    offset: int = 0
) -> dict:
    """
    Search models using FTS5 for sub-millisecond results.

    Args:
        query: Search query (supports prefix matching)
        limit: Maximum results
        offset: Result offset for pagination

    Returns:
        {
            'models': List[dict],
            'total': int,
            'query_time_ms': float
        }
    """
    import time
    start = time.time()

    try:
        conn = sqlite3.connect(str(self.library.db_path))
        conn.row_factory = sqlite3.Row
        cursor = conn.cursor()

        # FTS5 prefix query
        fts_query = ' OR '.join(f'{term}*' for term in query.split())

        cursor.execute("""
            SELECT m.*
            FROM model_search ms
            JOIN models m ON ms.model_id = m.model_id
            WHERE model_search MATCH ?
            ORDER BY rank
            LIMIT ? OFFSET ?
        """, (fts_query, limit, offset))

        rows = cursor.fetchall()
        models = [dict(row) for row in rows]

        # Get total count
        cursor.execute("""
            SELECT COUNT(*) FROM model_search WHERE model_search MATCH ?
        """, (fts_query,))
        total = cursor.fetchone()[0]

        query_time_ms = (time.time() - start) * 1000

        return {
            'models': models,
            'total': total,
            'query_time_ms': query_time_ms
        }

    except Exception as e:
        logger.error(f"FTS5 search failed: {e}")
        # Fallback to basic LIKE query
        return self._fallback_search(query, limit, offset)
    finally:
        conn.close()
```

---

## Dual-Path Search Strategy

### Stale-While-Revalidate Pattern

The dual-path strategy provides instant UI feedback while fetching fresh data in the background:

```
User types "Llama"
    ↓
Path 1 (Local - Instant):
    Query FTS5 local index
    Return cached results in <20ms
    Render UI immediately
    ↓
Path 2 (Network - Background):
    Check if data is stale (>10 mins)
    If stale:
        Fetch from HuggingFace API
        Update local cache
        Trigger UI refresh (append-only or toast)
```

### Anti-Jitter UI Update

To prevent the "jumping cursor" problem where new results push items around:

**Strategy 1: Append-Only** (Default)
- New results are added to the **end** of the list
- Existing items don't move
- User can scroll down to see new results

**Strategy 2: Toast Notification** (For significant changes)
```tsx
{hasNewResults && (
  <div className="fixed top-4 right-4 bg-blue-600 text-white px-4 py-2 rounded shadow-lg">
    New results available
    <button onClick={loadNewResults} className="ml-3 underline">
      Update View
    </button>
  </div>
)}
```

### Implementation

**File**: `frontend/src/hooks/useModels.ts` (UPDATE)

```typescript
export function useModels() {
  const [models, setModels] = useState<Model[]>([]);
  const [cachedModels, setCachedModels] = useState<Model[]>([]);
  const [hasNewResults, setHasNewResults] = useState(false);
  const [isRefreshing, setIsRefreshing] = useState(false);

  const searchModels = async (query: string) => {
    // Path 1: Local FTS5 search (instant)
    const localResults = await api.searchModelsFTS(query);
    setModels(localResults.models);

    // Path 2: Check if stale, fetch fresh data
    const cacheAge = getCacheAge(query);
    if (cacheAge > 10 * 60 * 1000) { // 10 minutes
      setIsRefreshing(true);

      try {
        const freshResults = await api.searchModelsHuggingFace(query);

        // Update cache
        await api.updateCache(query, freshResults);

        // Check if results differ significantly
        if (resultsChangedSignificantly(localResults, freshResults)) {
          setCachedModels(freshResults.models);
          setHasNewResults(true);
        } else {
          // Silently update in background
          setModels(freshResults.models);
        }
      } catch (error) {
        console.error('Background refresh failed:', error);
        // Continue using cached data
      } finally {
        setIsRefreshing(false);
      }
    }
  };

  const loadNewResults = () => {
    setModels(cachedModels);
    setHasNewResults(false);
  };

  return {
    models,
    isRefreshing,
    hasNewResults,
    loadNewResults,
    searchModels,
  };
}
```

---

## Database Design

### Registry Database Integration

The existing `registry.db` already tracks symlinks. We extend `models.db` with FTS5 for search:

```
models.db (SQLite WAL + FTS5):
├── models (main table)
│   ├── model_id PRIMARY KEY
│   ├── family
│   ├── official_name
│   ├── tags
│   └── ... (existing columns)
├── model_search (FTS5 virtual table)
│   ├── model_id UNINDEXED
│   ├── repo_id
│   ├── official_name
│   ├── family
│   ├── tags
│   └── description
└── Triggers (keep FTS5 in sync)
    ├── models_ai (AFTER INSERT)
    ├── models_au (AFTER UPDATE)
    └── models_ad (AFTER DELETE)
```

### Migration Strategy

**File**: `backend/model_library/library.py` (UPDATE)

```python
def _migrate_to_fts5(self):
    """Migrate existing database to include FTS5 search."""
    conn = sqlite3.connect(str(self.db_path))
    cursor = conn.cursor()

    # Check if FTS5 table exists
    cursor.execute("""
        SELECT name FROM sqlite_master
        WHERE type='table' AND name='model_search'
    """)

    if cursor.fetchone():
        logger.info("FTS5 table already exists")
        conn.close()
        return

    logger.info("Creating FTS5 search index...")

    # Create FTS5 table
    cursor.execute("""
        CREATE VIRTUAL TABLE model_search USING fts5(
            model_id UNINDEXED,
            repo_id,
            official_name,
            family,
            tags,
            description,
            tokenize='unicode61 remove_diacritics 1 tokenchars "-_"'
        )
    """)

    # Populate from existing models table
    cursor.execute("""
        INSERT INTO model_search(model_id, repo_id, official_name, family, tags, description)
        SELECT model_id, repo_id, official_name, family, tags, description
        FROM models
    """)

    # Create triggers for future updates
    # ... (triggers from _initialize_database) ...

    conn.commit()
    conn.close()

    logger.info("FTS5 migration complete")
```

---

## Implementation Roadmap

### Phase 1: Networking & Circuit Breaker (Foundation)

**Goal**: Replace basic requests with HTTPX HTTP/2 and add circuit breaker

**Dependencies**: None (can start immediately)

**Tasks**:
1. Create `backend/model_library/network_manager.py`
   - [ ] Implement `NetworkManager` class
   - [ ] Add HTTP/2 client initialization
   - [ ] Add circuit breaker logic (3 failures → 60s blackout)
   - [ ] Add rate limit detection
   - [ ] Create global singleton

2. Update `backend/model_library/downloader.py`
   - [ ] Replace `requests` with `network_manager`
   - [ ] Make methods async
   - [ ] Add CircuitBreakerOpen exception handling
   - [ ] Return cached data when circuit is open

3. Add logging
   - [ ] Create `backend/logs/network.log`
   - [ ] Implement session rotation (network.log → network_prev.log)
   - [ ] Log all circuit breaker events

**Testing**:
- [ ] Test HTTP/2 multiplexing with concurrent requests
- [ ] Test circuit breaker opens after 3 failures
- [ ] Test circuit breaker closes after 60s
- [ ] Test rate limit warning triggers correctly

**Completion Criteria**:
- [ ] All HuggingFace API calls use NetworkManager
- [ ] Circuit breaker prevents hanging on network failures
- [ ] Offline mode detected within 7 seconds (3 failures × ~2s each)

---

### Phase 2: FTS5 Persistence (The Cache)

**Goal**: Add FTS5 full-text search to models.db

**Dependencies**: Phase 1 (networking foundation)

**Tasks**:
1. Update `backend/model_library/library.py`
   - [ ] Add `_migrate_to_fts5()` method
   - [ ] Create FTS5 virtual table with unicode61 tokenizer
   - [ ] Add triggers to keep FTS5 in sync
   - [ ] Run migration on first startup

2. Update `backend/api/core.py`
   - [ ] Add `search_models_fts()` method
   - [ ] Add fallback to LIKE query if FTS5 fails
   - [ ] Return query time for performance monitoring

3. Update `frontend/src/api/models.ts`
   - [ ] Add `searchModelsFTS()` method
   - [ ] Add types for FTS5 response

**Testing**:
- [ ] Test FTS5 prefix matching ("Llama" matches "Llama-3-7b")
- [ ] Test token character handling ("Llama-3" found by "Llama", "3")
- [ ] Test diacritic removal ("café" matches "cafe")
- [ ] Test query performance (<20ms for 1000+ models)
- [ ] Test triggers keep FTS5 in sync with models table

**Completion Criteria**:
- [ ] FTS5 search returns results in <20ms
- [ ] Prefix matching works correctly
- [ ] Triggers maintain index consistency

---

### Phase 3: Coordination & UX (The Polish)

**Goal**: Add debouncing, sequence guarding, and stale-while-revalidate

**Dependencies**: Phase 2 (FTS5 search)

**Tasks**:
1. Update `frontend/src/hooks/useModels.ts`
   - [ ] Add 300ms debouncing with lodash or custom implementation
   - [ ] Add sequence ID tracking
   - [ ] Add stale-while-revalidate pattern
   - [ ] Add "append-only" merge strategy
   - [ ] Add toast notification for significant changes

2. Add UI indicators
   - [ ] "Using Cached Data" badge when offline
   - [ ] "Refreshing..." spinner during background fetch
   - [ ] "New results available [Update View]" toast
   - [ ] "Rate Limit Warning" banner

3. Update `frontend/src/components/ModelManager.tsx`
   - [ ] Display offline indicator
   - [ ] Display refresh indicator
   - [ ] Wire up "Update View" button

**Testing**:
- [ ] Test debouncing: Type "LLAMA" → only 1 API call
- [ ] Test sequence guard: Out-of-order responses discarded
- [ ] Test offline indicator appears when circuit is open
- [ ] Test background refresh updates cache
- [ ] Test append-only merge doesn't jump cursor

**Completion Criteria**:
- [ ] Typing "Meta-Llama" generates only 1 API call
- [ ] UI updates in <20ms using local FTS5 data
- [ ] Background refresh works without blocking UI
- [ ] Offline mode shows "Using Cached Data" indicator

---

## Success Criteria

### Performance Metrics

- [ ] **Instant Perceived Latency**: Local search results (Path 1) appear in <20ms
- [ ] **Zero UI Freezes**: Networking and DB writes occur on background threads; main thread never blocks
- [ ] **API Efficiency**: Typing "Meta-Llama" generates only 1 API call (debouncing)
- [ ] **Resiliency**: Disconnecting internet mid-session shows "Using Cached Data" notification without crashing

### User Experience

- [ ] Search results appear instantly (<20ms) using local FTS5 index
- [ ] Background refresh updates cache without blocking UI
- [ ] Offline mode detected quickly (within 7 seconds)
- [ ] UI never freezes on network failures
- [ ] Rate limit warnings prevent API quota exhaustion

### Technical Requirements

- [ ] HTTP/2 multiplexing used for concurrent requests
- [ ] Circuit breaker prevents ghost hangs (fail-fast in 7s)
- [ ] FTS5 tokenizer handles hyphens and underscores correctly
- [ ] Sequence guard prevents race conditions
- [ ] WAL checkpointing maintains database integrity

---

## Related Documents

- [00-overview.md](00-overview.md) - High-level architecture and goals
- [01-performance-and-integrity.md](01-performance-and-integrity.md) - I/O optimization, hashing, SQLite tuning
- [02-model-import.md](02-model-import.md) - Drag-and-drop import with HuggingFace lookup
- [03-mapping-system.md](03-mapping-system.md) - Configuration-based model mapping
- [04-implementation-phases.md](04-implementation-phases.md) - Concrete implementation steps

---

**End of Networking & Caching Document**
