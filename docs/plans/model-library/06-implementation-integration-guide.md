# Implementation Integration Guide

**Version**: 1.0
**Status**: Planning Phase

---

## Table of Contents

- [Overview](#overview)
- [Compatibility Matrix](#compatibility-matrix)
- [Integration Points](#integration-points)
- [Updated Implementation Timeline](#updated-implementation-timeline)
- [File Dependencies](#file-dependencies)
- [Testing Integration](#testing-integration)
- [Migration Path](#migration-path)

---

## Overview

This document provides a comprehensive guide for integrating the networking and caching features (from [05-networking-and-caching.md](05-networking-and-caching.md)) into the existing implementation plan (from [04-implementation-phases.md](04-implementation-phases.md)).

### Key Principle

The networking features are designed to **layer on top** of the existing model library infrastructure without requiring major refactoring. Most integration work happens in Phase 1, with networking enhancements added incrementally.

### How to Use This Document

**For Implementation**: This document is **self-contained** for coding. Each task includes:
- ✅ **Quick Reference code** - Skeleton/example code you can adapt
- ✅ **Checklist** - Step-by-step implementation tasks
- ✅ **Time estimates** - How long each task should take
- ✅ **Test criteria** - How to verify it works
- 📖 **Full implementation link** - Reference to complete code in 05-networking-and-caching.md (if you need more details)

**You should NOT need to jump between documents during implementation.** The Quick Reference code provides everything you need. Only consult [05-networking-and-caching.md](05-networking-and-caching.md) if you want deeper architectural context or the complete implementation.

---

## Compatibility Matrix

### Database Integration

| Component | Current Plan | Networking Plan | Integration Strategy |
|-----------|-------------|-----------------|---------------------|
| **models.db** | SQLite WAL mode | Add FTS5 virtual table | Extend schema with migration |
| **registry.db** | Track symlinks | No change needed | Use as-is |
| **Schema version** | PRAGMA user_version | Same mechanism | Unified versioning |

**Action**: Extend `models.db` with FTS5 virtual table and triggers in Phase 1 Part A.

### Networking Integration

| Component | Current Plan | Networking Plan | Integration Strategy |
|-----------|-------------|-----------------|---------------------|
| **HTTP client** | requests with 5s timeout | httpx with HTTP/2 | Replace in downloader.py |
| **Circuit breaker** | None | 3 failures → 60s blackout | Add NetworkManager wrapper |
| **Offline handling** | Offline-first import | Circuit breaker fail-fast | Unified: Circuit breaker enhances detection |

**Action**: Create `network_manager.py` and update `downloader.py` to use it in Phase 1 Part A.

### Caching Integration

| Component | Current Plan | Networking Plan | Integration Strategy |
|-----------|-------------|-----------------|---------------------|
| **Metadata cache** | 24h LRU cache | Stale-while-revalidate | Keep existing cache, add SWR refresh |
| **Cache invalidation** | Time-based (24h) | Time-based (10min for search) | Separate TTLs for different data types |

**Action**: Add SWR pattern to `useModels.ts` hook in Phase 1 Part A.

### Search Integration

| Component | Current Plan | Networking Plan | Integration Strategy |
|-----------|-------------|-----------------|---------------------|
| **Search method** | Basic filter in React hook | FTS5 full-text search | Add FTS5 endpoint, keep hook interface |
| **Debouncing** | None | 300ms debounce | Add to frontend hook |
| **Race condition handling** | None | Sequence guard | Add sequence ID tracking |

**Action**: Add FTS5 search endpoint and update `useModels.ts` with debouncing in Phase 1 Part A.

---

## Integration Points

### 1. Database Schema Extension (Phase 1 Part A)

**When**: During initial `models.db` setup in `backend/model_library/library.py`

**Changes**:
1. Keep existing `models` table
2. Add FTS5 virtual table `model_search`
3. Add triggers to keep FTS5 in sync
4. Add migration function for existing databases

**Files Modified**:
- `backend/model_library/library.py` - `_initialize_database()` method
- `backend/model_library/library.py` - Add `_migrate_to_fts5()` method

**Dependencies**: None (can be added immediately)

---

### 2. Network Manager Integration (Phase 1 Part A)

**When**: Before implementing HuggingFace metadata lookup

**Changes**:
1. Create `NetworkManager` class with HTTP/2 client
2. Add circuit breaker logic
3. Replace `requests` with `network_manager` in downloader
4. Make downloader methods async

**Files Created**:
- `backend/model_library/network_manager.py` (NEW)

**Files Modified**:
- `backend/model_library/downloader.py` - Replace requests with network_manager
- `backend/model_library/downloader.py` - Add async/await
- `backend/model_library/downloader.py` - Handle CircuitBreakerOpen exceptions

**Dependencies**:
- Requires `httpx` package: `pip install httpx[http2]`
- Async methods require async context in callers

---

### 3. Search Endpoint Addition (Phase 1 Part A)

**When**: After FTS5 migration, before frontend search implementation

**Changes**:
1. Add `search_models_fts()` method to API core
2. Add fallback to LIKE query if FTS5 fails
3. Return query time for performance monitoring

**Files Modified**:
- `backend/api/core.py` - Add `search_models_fts()` method

**Dependencies**:
- FTS5 virtual table must exist in `models.db`

---

### 4. Frontend Search Enhancement (Phase 1 Part A)

**When**: After backend search endpoint is ready

**Changes**:
1. Add debouncing (300ms) to search input
2. Add sequence ID tracking to prevent race conditions
3. Add stale-while-revalidate pattern
4. Add UI indicators (offline, refreshing, new results)

**Files Modified**:
- `frontend/src/hooks/useModels.ts` - Add debouncing and SWR
- `frontend/src/components/ModelManager.tsx` - Add UI indicators
- `frontend/src/types/pywebview.d.ts` - Add FTS search types

**Dependencies**:
- Optional: Install `lodash` for debounce: `npm install lodash @types/lodash`
- Or implement custom debounce function

---

## Updated Implementation Timeline

This integrates the networking phases into the existing implementation plan.

### Phase 1: Core Infrastructure + Networking Foundation

**Goal**: Core import system + HTTP/2 networking + FTS5 search

**Estimated Time**: 3-4 weeks

#### Part 1A: Model Import System (Weeks 1-2)
*From [04-implementation-phases.md](04-implementation-phases.md#part-a-model-import-system)*

**Focus**: File handling, hashing, metadata, atomic operations

**Core Tasks**:
- Create I/O Manager with drive detection
- Create Filesystem Validator
- Update NTFS filename normalization
- Update Importer with stream hashing
- Update Library Manager with WAL mode
- ✨ **NEW**: Create NetworkManager with HTTP/2 and circuit breaker
- ✨ **NEW**: Update Downloader to use NetworkManager (make async)
- ✨ **NEW**: Add FTS5 virtual table to models.db
- ✨ **NEW**: Add FTS5 migration function
- Update API Core with import batch methods
- Update TypeScript types (PRIORITY #1)
- Create ImportAPI class
- Create Drop Zone component
- Create Import Dialog component
- Update CSS animations
- Update useModels hook with debouncing and SWR
- Update Model Manager with virtualization
- Update Settings component

**Networking-Specific Tasks**:

#### Task 1: Create NetworkManager (2-3 hours)

**File**: `backend/model_library/network_manager.py` (NEW)

**Full implementation in [05-networking-and-caching.md#unified-network-stack](05-networking-and-caching.md#unified-network-stack)**

**Quick Reference**:
```python
# Key components to implement:
class NetworkManager:
    def __init__(self):
        self._client: Optional[httpx.AsyncClient] = None
        self._circuit_breaker: Dict[str, CircuitBreakerState] = {}
        self._rate_limit_warning = False

    async def request(self, method: str, url: str, **kwargs) -> httpx.Response:
        # 1. Check circuit breaker (is domain blocked?)
        # 2. Make request with 7s timeout
        # 3. Check rate limit headers
        # 4. Record success/failure for circuit breaker
        pass

    def _record_failure(self, domain: str):
        # Track failures, open circuit after 3 consecutive failures
        pass

# Global singleton
network_manager = NetworkManager()
```

**Checklist**:
- [ ] Create file with `NetworkManager` class
- [ ] Add HTTP/2 client: `httpx.AsyncClient(http2=True, timeout=7.0)`
- [ ] Add circuit breaker logic (3 failures → 60s blackout)
- [ ] Add `CircuitBreakerState` dataclass
- [ ] Add `CircuitBreakerOpen` exception
- [ ] Add rate limit detection (X-RateLimit-Remaining < 10%)
- [ ] Create global `network_manager` singleton
- [ ] Test: Circuit opens after 3 timeouts, closes after 60s

---

#### Task 2: Update Downloader to Use NetworkManager (3-4 hours)

**File**: `backend/model_library/downloader.py` (UPDATE)

**Full implementation in [05-networking-and-caching.md#integration-with-downloader](05-networking-and-caching.md#integration-with-downloader)**

**Quick Reference**:
```python
# BEFORE (requests):
def lookup_model_metadata_by_filename(self, filename: str) -> Optional[dict]:
    response = requests.get(f'https://huggingface.co/api/models?search={filename}', timeout=5)
    return response.json()

# AFTER (httpx + NetworkManager):
async def lookup_model_metadata_by_filename(self, filename: str) -> Optional[dict]:
    try:
        response = await network_manager.request(
            'GET', f'https://huggingface.co/api/models?search={filename}'
        )
        return response.json()
    except CircuitBreakerOpen:
        logger.info("Circuit breaker open, using cached data")
        return self._get_cached_metadata(filename)
    except httpx.TimeoutException:
        logger.warning(f"Request timeout for {filename}")
        return self._get_cached_metadata(filename)
```

**Checklist**:
- [ ] Import `network_manager` and `CircuitBreakerOpen`
- [ ] Add `async` keyword to all lookup methods
- [ ] Replace `requests.get()` with `await network_manager.request()`
- [ ] Add `CircuitBreakerOpen` exception handling → return cached data
- [ ] Add `httpx.TimeoutException` handling → return cached data
- [ ] Update all callers to use `await` (import system, retry lookups)
- [ ] Test: Offline mode returns cached data within 7s

---

#### Task 3: Add FTS5 to models.db (2 hours)

**File**: `backend/model_library/library.py` (UPDATE)

**Full implementation in [05-networking-and-caching.md#fts5-virtual-table](05-networking-and-caching.md#fts5-virtual-table)**

**Quick Reference**:
```python
def _initialize_database(self):
    # ... existing models table creation ...

    # Add FTS5 virtual table
    cursor.execute("""
        CREATE VIRTUAL TABLE IF NOT EXISTS model_search USING fts5(
            model_id UNINDEXED,
            repo_id, official_name, family, tags, description,
            tokenize='unicode61 remove_diacritics 1 tokenchars "-_"'
        )
    """)

    # Add triggers to keep FTS5 in sync
    cursor.execute("""
        CREATE TRIGGER IF NOT EXISTS models_ai AFTER INSERT ON models BEGIN
            INSERT INTO model_search(model_id, repo_id, official_name, family, tags, description)
            VALUES (NEW.model_id, NEW.repo_id, NEW.official_name, NEW.family, NEW.tags, NEW.description);
        END
    """)
    # ... similar for UPDATE and DELETE triggers ...
```

**Checklist**:
- [ ] Add FTS5 virtual table in `_initialize_database()`
- [ ] Configure tokenizer: `unicode61 remove_diacritics 1 tokenchars "-_"`
- [ ] Add AFTER INSERT trigger
- [ ] Add AFTER UPDATE trigger (delete old, insert new)
- [ ] Add AFTER DELETE trigger
- [ ] Add `_migrate_to_fts5()` for existing databases
- [ ] Call migration on startup if FTS5 table doesn't exist
- [ ] Test: Insert model → verify FTS5 entry created

---

#### Task 4: Add FTS5 Search Endpoint (1-2 hours)

**File**: `backend/api/core.py` (UPDATE)

**Full implementation in [05-networking-and-caching.md#fts5-search-query](05-networking-and-caching.md#fts5-search-query)**

**Quick Reference**:
```python
def search_models_fts(self, query: str, limit: int = 100) -> dict:
    """Search using FTS5 for sub-millisecond results."""
    start = time.time()

    # Build FTS5 prefix query
    fts_query = ' OR '.join(f'{term}*' for term in query.split())

    cursor.execute("""
        SELECT m.* FROM model_search ms
        JOIN models m ON ms.model_id = m.model_id
        WHERE model_search MATCH ?
        ORDER BY rank LIMIT ?
    """, (fts_query, limit))

    models = [dict(row) for row in cursor.fetchall()]
    query_time_ms = (time.time() - start) * 1000

    return {'models': models, 'query_time_ms': query_time_ms}
```

**Checklist**:
- [ ] Add `search_models_fts()` method
- [ ] Build FTS5 query with prefix matching (`term*`)
- [ ] Join `model_search` with `models` table
- [ ] Return query time for monitoring
- [ ] Add fallback to LIKE query if FTS5 fails
- [ ] Test: Query "Llama" finds "Llama-3-7b" in <20ms

---

#### Task 5: Add Frontend Debouncing and SWR (2-3 hours)

**File**: `frontend/src/hooks/useModels.ts` (UPDATE)

**Full implementation in [05-networking-and-caching.md#dual-path-search-strategy](05-networking-and-caching.md#dual-path-search-strategy)**

**Quick Reference**:
```typescript
export function useModels() {
  const [sequenceId, setSequenceId] = useState(0);
  const lastRenderedId = useRef(0);

  // 300ms debounce
  const debouncedSearch = useRef(
    debounce(async (query: string, seqId: number) => {
      const results = await api.searchModelsFTS(query);

      // Sequence guard: discard stale responses
      if (seqId > lastRenderedId.current) {
        lastRenderedId.current = seqId;
        setModels(results.models);
      }
    }, 300)
  ).current;

  const handleSearch = (query: string) => {
    const newSeqId = sequenceId + 1;
    setSequenceId(newSeqId);
    debouncedSearch(query, newSeqId);
  };
}
```

**Checklist**:
- [ ] Add `sequenceId` state and `lastRenderedId` ref
- [ ] Add 300ms debouncing (lodash or custom)
- [ ] Add sequence guard: `if (seqId > lastRenderedId.current)`
- [ ] Add stale-while-revalidate pattern (optional)
- [ ] Test: Type "LLAMA" → only 1 API call made
- [ ] Test: Out-of-order responses discarded

---

#### Task 6: Add UI Indicators (1 hour)

**File**: `frontend/src/components/ModelManager.tsx` (UPDATE)

**Quick Reference**:
```tsx
{isOffline && (
  <div className="bg-yellow-50 border-l-4 border-yellow-400 p-3">
    <p className="text-sm text-yellow-700">
      Using Cached Data (offline)
    </p>
  </div>
)}

{isRefreshing && (
  <div className="flex items-center gap-2">
    <Spinner className="w-4 h-4" />
    <span className="text-sm">Refreshing...</span>
  </div>
)}
```

**Checklist**:
- [ ] Add "Using Cached Data" banner when circuit breaker is open
- [ ] Add "Refreshing..." spinner during background refresh
- [ ] Add "New results available" toast (optional SWR)
- [ ] Test: Disconnect network → banner appears within 7s

**Testing**:
- [ ] Test HTTP/2 multiplexing (concurrent requests)
- [ ] Test circuit breaker opens after 3 failures
- [ ] Test circuit breaker closes after 60s
- [ ] Test FTS5 prefix matching (<20ms results)
- [ ] Test debouncing (type "LLAMA" → 1 API call)
- [ ] Test sequence guard (discard out-of-order responses)
- [ ] Test offline indicator shows when circuit open

**Completion Criteria**:
- All from [04-implementation-phases.md](04-implementation-phases.md#completion-criteria-part-a)
- **Plus**: HTTP/2 multiplexing working
- **Plus**: Circuit breaker prevents UI hangs
- **Plus**: FTS5 search returns results in <20ms
- **Plus**: Debouncing limits API calls
- **Plus**: Offline indicator works

---

#### Part 1B: Link Registry Database (Week 2)
*From [04-implementation-phases.md](04-implementation-phases.md#part-b-link-registry-database)*

**Focus**: Symlink tracking, cascade delete, health checks

**No networking changes needed** - this part is independent of networking features.

**Completion Criteria**: As defined in existing plan

---

#### Part 1C: Basic Link Mapping System (Weeks 3-4)
*From [04-implementation-phases.md](04-implementation-phases.md#part-c-basic-link-mapping-system)*

**Focus**: ComfyUI mapping configs, auto-sync

**No networking changes needed** - this part is independent of networking features.

**Completion Criteria**: As defined in existing plan

---

### Phase 2: Data Integrity & Reliability

**Goal**: File verification, health checks

**Estimated Time**: 1-2 weeks

**No networking changes needed** - Phase 2 is entirely focused on data integrity.

**Completion Criteria**: As defined in existing plan

---

### Phase 3: User Experience

**Goal**: Sharded sets, mapping preview, drive relocation

**Estimated Time**: 2-3 weeks

**No networking changes needed** - Phase 3 focuses on UX improvements.

**Completion Criteria**: As defined in existing plan

---

## File Dependencies

### New Files Required

**Phase 1 (Networking)**:
```
backend/model_library/
├── network_manager.py         # NEW - HTTP/2 client + circuit breaker
└── logs/
    └── network.log            # NEW - Network event log (auto-created)
```

**Phase 1 (Existing Plan)**:
```
backend/model_library/
├── io_manager.py              # NEW - Drive-aware I/O
├── fs_validator.py            # NEW - Filesystem validation

frontend/src/
├── api/import.ts              # NEW - ImportAPI class
└── components/
    ├── ModelImportDropZone.tsx  # NEW - Drop zone overlay
    └── ModelImportDialog.tsx    # NEW - Import wizard
```

### Modified Files

**Phase 1 (Networking + Existing)**:
```
backend/
├── model_library/
│   ├── library.py             # UPDATE - Add FTS5 migration
│   ├── downloader.py          # UPDATE - Use NetworkManager, async
│   └── api/core.py            # UPDATE - Add FTS5 search endpoint

frontend/src/
├── types/pywebview.d.ts       # UPDATE - Add types (PRIORITY #1)
├── hooks/useModels.ts         # UPDATE - Add debouncing + SWR
├── components/
│   ├── ModelManager.tsx       # UPDATE - Add drop zone + indicators
│   └── Settings.tsx           # UPDATE - Add rebuild index button
└── index.css                  # UPDATE - Add animations
```

### Dependency Graph

```
network_manager.py
    ↓
downloader.py (async)
    ↓
library.py (FTS5 migration)
    ↓
api/core.py (FTS5 search endpoint)
    ↓
pywebview.d.ts (types)
    ↓
useModels.ts (debouncing + SWR)
    ↓
ModelManager.tsx (UI indicators)
```

**Critical Path**: Start with `network_manager.py`, then update `downloader.py`, then add FTS5, then frontend.

---

## Testing Integration

### Unit Tests (Phase 1)

**Networking Tests** (NEW):
- [ ] NetworkManager: HTTP/2 client initialization
- [ ] NetworkManager: Circuit breaker opens after 3 failures
- [ ] NetworkManager: Circuit breaker closes after 60s
- [ ] NetworkManager: Rate limit warning triggers
- [ ] FTS5: Prefix matching works ("Llama" finds "Llama-3-7b")
- [ ] FTS5: Token character handling (hyphens, underscores)
- [ ] FTS5: Diacritic removal ("café" matches "cafe")
- [ ] Debouncing: Multiple keystrokes result in single API call
- [ ] Sequence guard: Out-of-order responses discarded

**Existing Tests**:
- All unit tests from [04-implementation-phases.md](04-implementation-phases.md#testing)

---

### Integration Tests (Phase 1)

**Networking Tests** (NEW):
- [ ] Offline import: Circuit breaker opens → "Using Cached Data" shown
- [ ] Background refresh: Stale data triggers API call in background
- [ ] Search performance: 1000+ models → Results in <20ms
- [ ] Race condition prevention: Type "LLAMA" fast → Only final results shown
- [ ] HTTP/2 multiplexing: Multiple concurrent requests use single connection

**Existing Tests**:
- All integration tests from [04-implementation-phases.md](04-implementation-phases.md#testing)

---

### System Tests

**Networking Tests** (NEW):
- [ ] **Circuit Breaker Test**: Disconnect network → 3 failures → Circuit opens → "Using Cached Data" shown → Reconnect → Circuit closes after 60s
- [ ] **Search Performance Test**: Import 1000 models → Search "Llama" → Results appear in <20ms
- [ ] **Race Condition Test**: Type "Meta-Llama" quickly → Only 1 API call made → Only final results rendered
- [ ] **HTTP/2 Test**: Make 10 concurrent HF API calls → Verify single TCP connection used

**Existing Tests**:
- All system tests from [04-implementation-phases.md](04-implementation-phases.md#testing)

---

## Migration Path

### For Existing Implementations

If you've already started implementing the model library plan, here's how to integrate networking features:

#### Step 1: Add NetworkManager (Low Risk)

**When**: Anytime after Phase 1 Part A starts

**Steps**:
1. Install httpx: `pip install httpx[http2]`
2. Create `backend/model_library/network_manager.py`
3. Test in isolation (no changes to existing code yet)

**Risk**: None - NetworkManager is self-contained

---

#### Step 2: Update Downloader (Medium Risk)

**When**: After NetworkManager is tested

**Steps**:
1. Make downloader methods async (add `async` keyword)
2. Replace `requests.get()` with `await network_manager.request()`
3. Add CircuitBreakerOpen exception handling
4. Update all callers to use `await`

**Risk**: Breaking changes to async signatures - test thoroughly

**Rollback**: Keep old `requests`-based methods alongside new async methods temporarily

---

#### Step 3: Add FTS5 Migration (Low Risk)

**When**: After models.db is stable

**Steps**:
1. Add `_migrate_to_fts5()` function
2. Call on startup (checks if already migrated)
3. Verify triggers keep FTS5 in sync

**Risk**: Low - migration is idempotent and backward-compatible

**Rollback**: No rollback needed - FTS5 table is additive

---

#### Step 4: Add Frontend Search (Low Risk)

**When**: After FTS5 search endpoint exists

**Steps**:
1. Add debouncing to search input
2. Add sequence ID tracking
3. Add UI indicators
4. Test with existing backend

**Risk**: Low - all changes are client-side enhancements

**Rollback**: Remove debouncing and sequence tracking (search still works)

---

## Summary

### Integration Strategy

1. ✅ **Additive, not disruptive**: Networking features layer on top of existing plan
2. ✅ **No major refactoring**: Most changes are isolated to new files
3. ✅ **Incremental adoption**: Can add NetworkManager, FTS5, debouncing separately
4. ✅ **Backward compatible**: Existing code works while networking features are added

### Key Takeaways

- **NetworkManager** wraps HTTP/2 and circuit breaker logic → Use in downloader.py
- **FTS5** extends models.db with search → Migration is additive
- **Debouncing** and **sequence guard** enhance frontend → Client-side only
- **Stale-while-revalidate** improves UX → No backend changes needed

### Implementation Order

1. Create NetworkManager (isolated, testable)
2. Update Downloader to use NetworkManager (breaking change, requires testing)
3. Add FTS5 migration (additive, low risk)
4. Add FTS5 search endpoint (new API, no breaking changes)
5. Add frontend debouncing and SWR (client-side enhancement)
6. Add UI indicators (polish, no backend changes)

---

**End of Implementation Integration Guide**
