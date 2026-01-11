# Model Library Dependency Map

**Version:** 1.0
**Purpose:** Visualize dependencies to understand what can be parallelized

---

## Critical Path (MUST be linear)

```
┌─────────────────────────────────────────────────────────────────────┐
│ Phase 1A: Core Infrastructure - LINEAR DEPENDENCY CHAIN            │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  WEEK 0: PREREQUISITE REFACTORING (BLOCKING)                       │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │ downloader.py (996 lines) ──> MUST SPLIT FIRST             │    │
│  │   └─> hf/client.py                                         │    │
│  │   └─> hf/throttle.py                                       │    │
│  │   └─> hf/metadata_lookup.py                                │    │
│  │   └─> hf/file_download.py                                  │    │
│  │   └─> hf/cache.py                                          │    │
│  │   └─> downloader.py (slim, ~150 lines)                     │    │
│  └────────────────────────────────────────────────────────────┘    │
│                            │                                        │
│                            ▼                                        │
│  WEEK 1: BACKEND INFRASTRUCTURE                                    │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │ ┌─────────────────────┐  ┌─────────────────────┐          │    │
│  │ │ io/manager.py       │  │ network/             │          │    │
│  │ │ io/validator.py     │  │ circuit_breaker.py   │          │    │
│  │ │ io/hashing.py       │  │ retry.py             │          │    │
│  │ │ io/platform.py      │  │ manager.py           │          │    │
│  │ └─────────────────────┘  └─────────────────────┘          │    │
│  │         │                          │                       │    │
│  │         └──────────┬───────────────┘                       │    │
│  │                    ▼                                       │    │
│  │            UPDATE EXISTING FILES                           │    │
│  │         ┌──────────────────────────┐                       │    │
│  │         │ importer.py (use io/*)   │                       │    │
│  │         │ library.py (add FTS5)    │                       │    │
│  │         │ mapper.py (use platform) │                       │    │
│  │         └──────────────────────────┘                       │    │
│  └────────────────────────────────────────────────────────────┘    │
│                            │                                        │
│                            ▼                                        │
│  WEEK 2: API & FRONTEND                                            │
│  ┌────────────────────────────────────────────────────────────┐    │
│  │ api/core.py (add endpoints)                                │    │
│  │         │                                                   │    │
│  │         ▼                                                   │    │
│  │ pywebview.d.ts ◄── CRITICAL BLOCKING POINT                 │    │
│  │         │                                                   │    │
│  │         └──────────┬─────────────────────┬─────────────┐   │    │
│  │                    ▼                     ▼             ▼   │    │
│  │         ModelImportDropZone.tsx  ModelImportDialog  useModels│   │
│  │                                                              │    │
│  └────────────────────────────────────────────────────────────┘    │
│                                                                     │
└─────────────────────────────────────────────────────────────────────┘
```

---

## Module Dependencies (Import Graph)

### Backend Modules

```
hf/client.py
├─> httpx (external)
└─> backend.logging_config

hf/throttle.py
├─> backend.logging_config
└─> No internal dependencies ✅

hf/metadata_lookup.py
├─> hf/client.py ◄── DEPENDS ON
├─> hf/cache.py ◄── DEPENDS ON
└─> backend.logging_config

hf/file_download.py
├─> hf/client.py ◄── DEPENDS ON
└─> backend.logging_config

hf/cache.py
├─> functools (lru_cache)
└─> No internal dependencies ✅

downloader.py (slim)
├─> hf/metadata_lookup.py ◄── DEPENDS ON
├─> hf/file_download.py ◄── DEPENDS ON
└─> hf/cache.py ◄── DEPENDS ON

─────────────────────────────────────────

io/manager.py
├─> psutil (drive detection)
├─> backend.logging_config
└─> No internal dependencies ✅

io/validator.py
├─> backend.logging_config
└─> No internal dependencies ✅

io/hashing.py
├─> blake3 (external)
├─> hashlib (standard library)
└─> No internal dependencies ✅

io/platform.py
├─> sys (OS detection)
├─> backend.logging_config
└─> No internal dependencies ✅

─────────────────────────────────────────

network/circuit_breaker.py
├─> dataclasses
├─> backend.logging_config
└─> No internal dependencies ✅

network/retry.py
├─> backend.logging_config
└─> No internal dependencies ✅

network/manager.py
├─> httpx (external)
├─> network/circuit_breaker.py ◄── DEPENDS ON
├─> backend.logging_config
└─> No internal dependencies ✅

─────────────────────────────────────────

importer.py
├─> io/manager.py ◄── DEPENDS ON
├─> io/hashing.py ◄── DEPENDS ON
├─> naming.py
└─> library.py

library.py
├─> search/fts5.py ◄── DEPENDS ON
├─> index.py
└─> naming.py

mapper.py
├─> io/platform.py ◄── DEPENDS ON
└─> library.py
```

---

## Safe Parallelization Windows

### Window 1: HF Module Extraction (Week 0)

**Can work in parallel (independent extractions):**

```
Agent 1: Extract hf/client.py + tests
Agent 2: Extract hf/throttle.py + tests
Agent 3: Extract hf/cache.py + tests

✅ SAFE - No dependencies between these
```

**Must be sequential after:**

```
hf/metadata_lookup.py  ◄── Depends on client, cache
hf/file_download.py    ◄── Depends on client
downloader.py (slim)   ◄── Depends on all hf/* modules
```

### Window 2: I/O and Network Modules (Week 1, Days 1-3)

**Can work in parallel:**

```
Agent 1: io/manager.py + tests
Agent 2: io/validator.py + tests
Agent 3: io/hashing.py + tests
Agent 4: io/platform.py + tests

Agent 5: network/circuit_breaker.py + tests
Agent 6: network/retry.py + tests

✅ SAFE - No dependencies between modules
```

**Must wait for completion:**

```
network/manager.py ◄── Depends on circuit_breaker.py
```

### Window 3: Update Existing Files (Week 1, Days 4-5)

**MUST be sequential:**

```
1. importer.py  (uses io/manager, io/hashing)
2. library.py   (uses search/fts5)
3. mapper.py    (uses io/platform)

❌ CANNOT parallelize - may have conflicts
```

### Window 4: Frontend Components (Week 2, Days 9-10)

**Can work in parallel ONLY after pywebview.d.ts complete:**

```
BLOCKER: pywebview.d.ts ◄── MUST complete first

After TypeScript types exist:
  Agent 1: ModelImportDropZone.tsx
  Agent 2: ModelImportDialog.tsx
  Agent 3: Update ModelManager.tsx
  Agent 4: Update useModels.ts

✅ SAFE - TypeScript will catch type conflicts
```

---

## What CANNOT Be Parallelized

### Hard Blockers

1. **Cannot update importer.py until io/* exists**
   - importer.py imports io/manager, io/hashing
   - Must wait for io/* modules to be committed

2. **Cannot update library.py until search/fts5.py exists**
   - library.py uses FTS5 migration functions
   - Must wait for search/* modules

3. **Cannot work on frontend until pywebview.d.ts complete**
   - All frontend components import types
   - Missing types = TypeScript errors

4. **Cannot integrate downloader.py until all hf/* exist**
   - downloader.py coordinates hf/* modules
   - Missing modules = import errors

5. **Phase 1B cannot start until Phase 1A complete**
   - Link registry needs import system working
   - Hard dependency

6. **Phase 1C cannot start until Phase 1A + 1B complete**
   - Mapping system needs both import + link registry
   - Hard dependency

---

## Recommended Implementation Strategy

### Strategy: "Small Batch Sequential"

**Don't spawn multiple agents for code implementation.**

Instead, use **small sequential batches** with frequent commits:

```
Session 1: HF Extraction (4-6 hours)
├─ Extract hf/client.py + tests
├─ Commit: "feat(hf): add HTTP/2 client wrapper"
├─ Extract hf/throttle.py + tests
├─ Commit: "feat(hf): add rate limiting throttle"
├─ Extract hf/cache.py + tests
└─ Commit: "feat(hf): add LRU cache"

Session 2: HF Integration (3-4 hours)
├─ Extract hf/metadata_lookup.py + tests
├─ Commit: "refactor(hf): extract metadata lookup"
├─ Extract hf/file_download.py + tests
├─ Commit: "refactor(hf): extract file download"
├─ Slim downloader.py + update tests
└─ Commit: "refactor(hf): slim downloader to coordinator"

Session 3: I/O Modules (4-5 hours)
├─ Create io/manager.py + tests
├─ Commit: "feat(io): add drive-aware I/O manager"
├─ Create io/validator.py + tests
├─ Commit: "feat(io): add filesystem validator"
├─ Create io/hashing.py + tests
├─ Commit: "feat(io): add stream hashing"
├─ Create io/platform.py + tests
└─ Commit: "feat(io): add platform abstraction"

[Continue pattern...]
```

### Why Sequential is Better

1. **Pre-commit hooks enforce quality** - can't commit broken code
2. **No merge conflicts** - one person, one branch, linear history
3. **Easy to rollback** - each commit is atomic and tested
4. **Context preservation** - PROGRESS.md tracks exactly what's done
5. **Faster overall** - no coordination overhead, no conflict resolution

---

## Testing Parallelization

**This is where multiple agents help:**

```bash
# After implementation, run tests in parallel
Agent 1 (Bash): pytest tests/unit/model_library/hf/ -v
Agent 2 (Bash): pytest tests/unit/model_library/io/ -v
Agent 3 (Bash): pytest tests/unit/model_library/network/ -v
Agent 4 (Bash): pytest tests/integration/ -v

# Wait for all to complete, then commit if all pass
```

---

## Phase Boundaries (HARD BLOCKS)

```
Phase 1A Complete
       │
       ├─ All backend modules implemented
       ├─ All frontend components implemented
       ├─ All tests passing
       ├─ Coverage ≥80% on new files
       │
       ▼
Phase 1B Start ◄── Can now begin link registry
       │
       ├─ Link registry implemented
       ├─ Cascade delete working
       ├─ Health checks working
       │
       ▼
Phase 1C Start ◄── Can now begin mapping system
       │
       ├─ Default configs created
       ├─ Auto-sync working
       ├─ Version constraints working
       │
       ▼
Phase 2 Start ◄── Can now focus on reliability
```

---

## Summary

### Can Parallelize (Limited Windows)

✅ HF module extraction (client, throttle, cache only)
✅ I/O module creation (all independent)
✅ Network module creation (circuit_breaker, retry only)
✅ Frontend components (ONLY after TypeScript types)
✅ Test execution (always safe)

### Cannot Parallelize (Most of the work)

❌ HF modules that depend on client/cache
❌ Updating existing files (importer, library, mapper)
❌ API layer updates (depends on backend)
❌ Frontend before TypeScript types
❌ Phase boundaries (1A → 1B → 1C → 2)

### Recommendation

**Use single-agent sequential approach with:**
- Small batches (one module at a time)
- Frequent commits (after each module passes tests)
- Progress tracking (update PROGRESS.md after each commit)
- Test parallelization (only for running tests, not writing code)

This avoids merge conflicts, ensures quality, and makes progress tracking easier across context clears.

---

**End of Dependency Map**
