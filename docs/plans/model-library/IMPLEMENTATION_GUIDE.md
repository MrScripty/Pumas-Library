# Model Library Implementation Guide

**Version**: 1.0
**Status**: Active Development Guide

---

## Table of Contents

- [File Organization Strategy](#file-organization-strategy)
- [Commit Strategy](#commit-strategy)
- [Code Standards Compliance](#code-standards-compliance)
- [Testing Strategy](#testing-strategy)
- [Implementation Order](#implementation-order)
- [Session Workflow](#session-workflow)

---

## File Organization Strategy

### Problem: Large Files

Current `downloader.py` is **996 lines** - exceeds 700-line limit.

### Solution: Split into Focused Modules

**New Structure:**

```
backend/model_library/
├── __init__.py
├── library.py              (98 lines - OK)
├── importer.py             (153 lines - OK)
├── mapper.py               (204 lines - OK)
├── naming.py               (56 lines - OK)
├── index.py                (140 lines - OK)
│
├── hf/                     (NEW - HuggingFace operations)
│   ├── __init__.py
│   ├── client.py           (~200 lines - HTTP/2 client wrapper)
│   ├── metadata_lookup.py  (~250 lines - metadata search & verification)
│   ├── file_download.py    (~200 lines - download operations)
│   ├── cache.py            (~150 lines - LRU cache for API responses)
│   └── throttle.py         (~100 lines - rate limiting)
│
├── io/                     (NEW - I/O and filesystem operations)
│   ├── __init__.py
│   ├── manager.py          (~200 lines - drive-aware I/O queue)
│   ├── validator.py        (~150 lines - filesystem validation)
│   ├── hashing.py          (~150 lines - stream hashing utilities)
│   └── platform.py         (~200 lines - platform abstraction for links)
│
├── network/                (NEW - networking infrastructure)
│   ├── __init__.py
│   ├── manager.py          (~250 lines - NetworkManager + circuit breaker)
│   ├── circuit_breaker.py  (~150 lines - circuit breaker logic)
│   └── retry.py            (~100 lines - retry logic with backoff)
│
└── search/                 (NEW - search and indexing)
    ├── __init__.py
    ├── fts5.py             (~200 lines - FTS5 virtual table setup)
    └── query.py            (~150 lines - search query builder)
```

**Rationale:**
- **Each module < 700 lines**
- **Clear separation of concerns** (HuggingFace, I/O, networking, search)
- **Easy to test** (each module has focused responsibility)
- **Future-proof** (easy to add new capabilities)

### Module Responsibilities

#### `backend/model_library/hf/` - HuggingFace Operations

**Purpose:** All interactions with HuggingFace API

- `client.py` - HTTP/2 async client with timeout handling
- `metadata_lookup.py` - Search by hash/filename, verification
- `file_download.py` - Download models from HuggingFace
- `cache.py` - LRU cache for API responses (24h TTL)
- `throttle.py` - Rate limiting to avoid API bans

**Key Functions:**
```python
# hf/metadata_lookup.py
async def lookup_by_hash(file_hash: str) -> Optional[ModelMetadata]
async def lookup_by_filename(filename: str) -> list[ModelMetadata]
async def verify_hash_match(file_path: Path, candidate: dict) -> bool

# hf/client.py
async def request(method: str, url: str, **kwargs) -> httpx.Response
def is_rate_limited() -> bool
```

#### `backend/model_library/io/` - I/O Operations

**Purpose:** Drive-aware file operations

- `manager.py` - I/O queue with SSD/HDD detection
- `validator.py` - Filesystem validation (NTFS dirty bit, read-only mounts)
- `hashing.py` - Stream hashing (BLAKE3 + SHA256 during copy)
- `platform.py` - Platform abstraction (Linux symlinks now, Windows later)

**Key Functions:**
```python
# io/manager.py
async def copy_with_hashing(src: Path, dst: Path) -> tuple[Path, Hashes]
def get_drive_type(path: Path) -> Literal["ssd", "hdd", "unknown"]

# io/validator.py
def validate_import_source(path: Path) -> ValidationResult
def validate_mapping_target(path: Path) -> ValidationResult

# io/platform.py
def create_link(source: Path, target: Path, strategy: LinkStrategy) -> bool
```

#### `backend/model_library/network/` - Networking Infrastructure

**Purpose:** Robust network operations with circuit breaker

- `manager.py` - NetworkManager coordinating all network ops
- `circuit_breaker.py` - Circuit breaker state machine
- `retry.py` - Retry logic with exponential backoff

**Key Functions:**
```python
# network/manager.py
async def request(method: str, url: str, **kwargs) -> httpx.Response
def is_circuit_open(domain: str) -> bool

# network/circuit_breaker.py
def record_failure(domain: str) -> None
def record_success(domain: str) -> None
def should_open_circuit(domain: str) -> bool
```

#### `backend/model_library/search/` - Search and Indexing

**Purpose:** Fast full-text search

- `fts5.py` - FTS5 virtual table setup and migration
- `query.py` - Search query builder with prefix matching

**Key Functions:**
```python
# search/fts5.py
def migrate_to_fts5(db_path: Path) -> None
def create_fts5_triggers(conn: sqlite3.Connection) -> None

# search/query.py
def build_fts5_query(search_term: str) -> str
def search_models(query: str, limit: int) -> list[dict]
```

---

## Commit Strategy

### Atomic Commits: One Change at a Time

**Pre-commit hooks enforce:**
- ✅ Black formatting
- ✅ isort import sorting
- ✅ No print() statements
- ✅ Specific exception handling
- ✅ Full test suite passes
- ✅ mypy type checking
- ✅ ≥80% coverage for new files

**Each commit MUST:**
1. Pass all pre-commit hooks
2. Have tests for new code
3. Follow commit message format: `<type>: <summary>`

### Recommended Commit Sequence for Phase 1A

**Session 1: Refactor downloader.py (PREREQUISITE)**

```bash
# Commit 1: Create new directory structure
git checkout -b refactor/split-downloader
mkdir -p backend/model_library/{hf,io,network,search}
touch backend/model_library/{hf,io,network,search}/__init__.py
git add backend/model_library/{hf,io,network,search}
git commit -m "chore: create model_library subdirectories"

# Commit 2: Extract HF client (smallest, safest)
# - Create hf/client.py with httpx client wrapper
# - Import in downloader.py but don't use yet
# - Add tests for hf/client.py
git add backend/model_library/hf/client.py tests/unit/test_hf_client.py
git commit -m "feat(hf): add HTTP/2 client wrapper with timeout"

# Commit 3: Extract throttle logic
# - Create hf/throttle.py
# - Update downloader.py to import it
# - Add tests
git add backend/model_library/hf/throttle.py tests/unit/test_hf_throttle.py
git commit -m "feat(hf): add rate limiting throttle"

# Commit 4: Extract metadata lookup
# - Create hf/metadata_lookup.py (~250 lines from downloader.py)
# - Update downloader.py to import and delegate
# - Add tests
git add backend/model_library/hf/metadata_lookup.py tests/unit/test_hf_metadata_lookup.py
git commit -m "refactor(hf): extract metadata lookup to separate module"

# Commit 5: Extract file download
# - Create hf/file_download.py
# - Update downloader.py to import and delegate
# - Add tests
git add backend/model_library/hf/file_download.py tests/unit/test_hf_file_download.py
git commit -m "refactor(hf): extract file download to separate module"

# Commit 6: Extract cache
# - Create hf/cache.py
# - Update downloader.py to use it
# - Add tests
git add backend/model_library/hf/cache.py tests/unit/test_hf_cache.py
git commit -m "feat(hf): add LRU cache for API responses"

# Commit 7: Slim down downloader.py
# - downloader.py now just coordinates hf/* modules
# - Should be ~150 lines (under limit)
# - Update existing tests
git add backend/model_library/downloader.py tests/unit/test_downloader.py
git commit -m "refactor(hf): slim downloader.py to coordinator role"

# Verify line counts
wc -l backend/model_library/hf/*.py
# All should be < 300 lines
```

**Session 2: I/O Infrastructure**

```bash
git checkout -b feat/io-infrastructure

# Commit 8: I/O manager
git add backend/model_library/io/manager.py tests/unit/test_io_manager.py
git commit -m "feat(io): add drive-aware I/O queue manager"

# Commit 9: Filesystem validator
git add backend/model_library/io/validator.py tests/unit/test_io_validator.py
git commit -m "feat(io): add filesystem validation with NTFS checks"

# Commit 10: Stream hashing
git add backend/model_library/io/hashing.py tests/unit/test_io_hashing.py
git commit -m "feat(io): add stream hashing for BLAKE3/SHA256"

# Commit 11: Platform abstraction
git add backend/model_library/io/platform.py tests/unit/test_io_platform.py
git commit -m "feat(io): add platform abstraction for link creation"
```

**Session 3: Networking Infrastructure**

```bash
git checkout -b feat/networking-infrastructure

# Commit 12: Circuit breaker
git add backend/model_library/network/circuit_breaker.py tests/unit/test_circuit_breaker.py
git commit -m "feat(network): add circuit breaker for fail-fast"

# Commit 13: Retry logic
git add backend/model_library/network/retry.py tests/unit/test_network_retry.py
git commit -m "feat(network): add retry logic with exponential backoff"

# Commit 14: NetworkManager
git add backend/model_library/network/manager.py tests/unit/test_network_manager.py
git commit -m "feat(network): add NetworkManager with circuit breaker"
```

**Session 4: Update Existing Files to Use New Modules**

```bash
git checkout -b feat/integrate-new-modules

# Commit 15: Update importer.py to use io/* modules
git add backend/model_library/importer.py tests/unit/test_importer.py
git commit -m "refactor(importer): use io/manager for stream hashing"

# Commit 16: Update library.py for FTS5
git add backend/model_library/library.py tests/unit/test_library.py
git commit -m "feat(library): add FTS5 virtual table support"

# Commit 17: Update mapper.py for platform abstraction
git add backend/model_library/mapper.py tests/unit/test_mapper.py
git commit -m "refactor(mapper): use io/platform for link creation"
```

### Commit Message Examples

**Good:**
```
feat(hf): add async metadata lookup with hash verification

Implements lookup_by_hash() and verify_hash_match() functions
with HTTP/2 client. Includes 24h LRU cache and rate limiting.

Tests: 87% coverage
```

**Bad:**
```
wip stuff
```

**Bad:**
```
feat: add everything for phase 1a

- network manager
- io manager
- fts5
- frontend
- api updates
```

---

## Code Standards Compliance

### Type Hints (REQUIRED)

```python
from __future__ import annotations
from typing import Optional, Literal
from pathlib import Path

# ✅ GOOD - Complete type hints
async def lookup_by_hash(
    file_hash: str,
    cache_ttl: int = 86400
) -> Optional[ModelMetadata]:
    """Lookup model metadata by BLAKE3 hash."""
    pass

# ❌ BAD - Missing type hints
async def lookup_by_hash(file_hash, cache_ttl=86400):
    pass
```

### Logging (REQUIRED)

```python
from backend.logging_config import get_logger

logger = get_logger(__name__)

# ✅ GOOD - Use logger
logger.info("Starting metadata lookup for hash: %s", file_hash)
logger.error("Hash verification failed", exc_info=True)

# ❌ BAD - Use print
print(f"Starting metadata lookup for hash: {file_hash}")
```

### Exception Handling (REQUIRED)

```python
# ✅ GOOD - Specific exceptions, logging
try:
    response = await client.get(url)
except httpx.TimeoutException as e:
    logger.warning("Request timeout for %s", url)
    return None
except httpx.ConnectError as e:
    logger.error("Connection failed: %s", e)
    raise NetworkError(f"Failed to connect to {url}") from e

# ❌ BAD - Generic exception, no logging
try:
    response = await client.get(url)
except Exception as e:
    return None
```

### Configuration (REQUIRED)

```python
# ✅ GOOD - Use config.py
from backend.config import NetworkConfig

timeout = NetworkConfig.REQUEST_TIMEOUT

# ❌ BAD - Hardcoded values
timeout = 7.0
```

### Input Validation (REQUIRED)

```python
from backend.validators import validate_url, sanitize_path

# ✅ GOOD - Validate inputs
def download_file(url: str, dest: Path) -> bool:
    if not validate_url(url):
        raise ValidationError(f"Invalid URL: {url}")
    safe_dest = sanitize_path(dest, base_dir=library_root)
    # ...

# ❌ BAD - No validation
def download_file(url: str, dest: Path) -> bool:
    urllib.request.urlretrieve(url, dest)  # Path traversal risk!
```

---

## Testing Strategy

### Test Organization

```
tests/
├── unit/                          # Fast, isolated tests
│   ├── model_library/
│   │   ├── hf/
│   │   │   ├── test_client.py
│   │   │   ├── test_metadata_lookup.py
│   │   │   ├── test_cache.py
│   │   │   └── test_throttle.py
│   │   ├── io/
│   │   │   ├── test_manager.py
│   │   │   ├── test_validator.py
│   │   │   ├── test_hashing.py
│   │   │   └── test_platform.py
│   │   └── network/
│   │       ├── test_circuit_breaker.py
│   │       ├── test_retry.py
│   │       └── test_manager.py
│   └── ...
│
└── integration/                   # Integration tests
    ├── test_import_flow.py        # End-to-end import
    ├── test_mapping_flow.py       # End-to-end mapping
    └── test_network_resilience.py # Circuit breaker + offline
```

### Coverage Requirements

**Pre-commit hook enforces ≥80% for new files**

```bash
# After implementing hf/metadata_lookup.py
pytest tests/unit/model_library/hf/test_metadata_lookup.py --cov=backend/model_library/hf/metadata_lookup --cov-report=term

# Must show ≥80% coverage or commit blocked
```

### Test Patterns

```python
import pytest
from pathlib import Path
from backend.model_library.hf.metadata_lookup import lookup_by_hash

@pytest.mark.unit
async def test_lookup_by_hash_exact_match(mock_hf_api):
    """Test that exact hash match returns correct metadata."""
    # Arrange
    file_hash = "abc123..."
    mock_hf_api.set_response("/api/models", [
        {"id": "model/repo", "sha256": "abc123...", "modelId": "sdxl"}
    ])

    # Act
    result = await lookup_by_hash(file_hash)

    # Assert
    assert result is not None
    assert result["modelId"] == "sdxl"

@pytest.mark.unit
async def test_lookup_by_hash_no_match(mock_hf_api):
    """Test that non-existent hash returns None."""
    # Arrange
    mock_hf_api.set_response("/api/models", [])

    # Act
    result = await lookup_by_hash("nonexistent")

    # Assert
    assert result is None
```

### Running Tests

```bash
# Run unit tests only (fast)
pytest tests/unit/ -v

# Run with coverage
pytest tests/unit/ --cov=backend/model_library --cov-report=html

# Run specific module
pytest tests/unit/model_library/hf/test_metadata_lookup.py -v

# Run in parallel (faster)
pytest tests/unit/ -n auto
```

---

## Implementation Order

### Phase 1A: Core Infrastructure

**Prerequisite: Refactor downloader.py first**

1. **Week 0: Refactoring** (1-2 days)
   - Split `downloader.py` into `hf/*` modules
   - Verify all tests still pass
   - Merge refactoring branch

2. **Week 1: Backend Infrastructure** (Days 1-5)
   - Day 1-2: `io/*` modules (manager, validator, hashing, platform)
   - Day 3: `network/*` modules (circuit_breaker, retry, manager)
   - Day 4-5: Update `importer.py`, `library.py`, `mapper.py`

3. **Week 2: API & Frontend** (Days 6-10)
   - Day 6-7: Update `api/core.py` with new endpoints
   - Day 8: TypeScript types (`pywebview.d.ts`) **CRITICAL - blocks UI**
   - Day 9-10: Frontend components (drop zone, dialog)

### Phase 1B: Link Registry

- Depends on Phase 1A complete
- 1 week implementation
- See [04-implementation-phases.md](04-implementation-phases.md#part-b-link-registry-database)

### Phase 1C: Mapping System

- Depends on Phase 1A + 1B complete
- 1-2 weeks implementation
- See [04-implementation-phases.md](04-implementation-phases.md#part-c-basic-link-mapping-system)

---

## Session Workflow

### Starting a Session

1. **Read progress tracking**
   ```bash
   cat docs/plans/model-library/PROGRESS.md
   cat docs/plans/model-library/ACTIVE_SESSION.md
   ```

2. **Check git status**
   ```bash
   git status
   git log --oneline -10
   ```

3. **Create feature branch**
   ```bash
   git checkout -b feat/phase1a-io-manager
   ```

4. **Run baseline tests**
   ```bash
   pytest tests/unit/ -v
   # Ensure all pass before starting
   ```

### During Implementation

1. **Write tests first** (TDD approach)
   ```bash
   # Create test file
   touch tests/unit/model_library/io/test_manager.py

   # Write failing tests
   pytest tests/unit/model_library/io/test_manager.py -v
   # Expected: FAILED (module doesn't exist yet)
   ```

2. **Implement module**
   ```bash
   # Create implementation
   touch backend/model_library/io/manager.py

   # Add type hints, logging, validation
   # ...
   ```

3. **Run tests frequently**
   ```bash
   pytest tests/unit/model_library/io/test_manager.py -v
   # Iterate until all pass
   ```

4. **Check coverage**
   ```bash
   pytest tests/unit/model_library/io/test_manager.py \
     --cov=backend/model_library/io/manager \
     --cov-report=term

   # Must be ≥80%
   ```

### Before Committing

1. **Run full test suite**
   ```bash
   pytest
   # All tests must pass
   ```

2. **Check type hints**
   ```bash
   mypy backend/model_library/io/manager.py
   # No errors allowed
   ```

3. **Format code** (pre-commit will auto-fix)
   ```bash
   pre-commit run --files backend/model_library/io/manager.py
   ```

4. **Commit with clear message**
   ```bash
   git add backend/model_library/io/manager.py tests/unit/model_library/io/test_manager.py
   git commit -m "feat(io): add drive-aware I/O queue manager

   Implements IOManager class with SSD/HDD detection and
   drive-aware semaphores for optimal disk access patterns.

   Tests: 87% coverage"

   # Pre-commit hooks run automatically
   # If they fail, fix issues and commit again
   ```

5. **Update progress tracking**
   ```bash
   echo "✅ io/manager.py - Completed $(date)" >> docs/plans/model-library/PROGRESS.md
   git add docs/plans/model-library/PROGRESS.md
   git commit -m "docs: update progress tracking"
   ```

### Ending a Session

1. **Document current state**
   ```bash
   cat > docs/plans/model-library/ACTIVE_SESSION.md <<EOF
   # Active Session State

   **Last Updated:** $(date)
   **Branch:** $(git branch --show-current)
   **Status:** In progress

   ## Completed This Session
   - io/manager.py (87% coverage)
   - io/validator.py (92% coverage)

   ## Next Steps
   - [ ] Implement io/hashing.py
   - [ ] Implement io/platform.py
   - [ ] Update importer.py to use io/manager

   ## Blockers
   None
   EOF

   git add docs/plans/model-library/ACTIVE_SESSION.md
   git commit -m "docs: update session state"
   ```

2. **Push branch**
   ```bash
   git push origin feat/phase1a-io-manager
   ```

3. **If tests are failing, stash or commit WIP**
   ```bash
   # Option 1: Stash
   git stash -u -m "WIP: io/hashing.py partial implementation"

   # Option 2: Commit WIP (NOT to main!)
   git add .
   git commit -m "wip: io/hashing.py partial implementation (tests failing)"
   git push origin feat/phase1a-io-manager
   ```

---

## File Size Monitoring

**Keep files under 700 lines** - automate checking:

```bash
# Add to .pre-commit-config.yaml (optional)
- repo: local
  hooks:
    - id: check-file-size-lines
      name: Check Python file size (max 700 lines)
      entry: python3 .pre-commit-hooks/check_file_size.py
      language: system
      types: [python]
      args: [--max-lines=700]
```

```python
# .pre-commit-hooks/check_file_size.py
import sys
from pathlib import Path

MAX_LINES = 700

def check_file_size(file_path: str) -> bool:
    lines = Path(file_path).read_text().count('\n')
    if lines > MAX_LINES:
        print(f"❌ {file_path}: {lines} lines (max {MAX_LINES})")
        return False
    return True

if __name__ == "__main__":
    files = sys.argv[1:]
    failed = [f for f in files if not check_file_size(f)]
    sys.exit(1 if failed else 0)
```

---

## Summary: Key Principles

### Modularity
- ✅ Files < 700 lines (ideally < 300)
- ✅ Single responsibility per module
- ✅ Group related files in subdirectories
- ✅ Clear module boundaries

### Code Quality
- ✅ Type hints on all functions
- ✅ Logging (not print)
- ✅ Specific exception handling
- ✅ Input validation
- ✅ No hardcoded values

### Testing
- ✅ ≥80% coverage for new files
- ✅ Tests before implementation (TDD)
- ✅ Unit tests fast (<1s each)
- ✅ Integration tests for workflows

### Commits
- ✅ One feat/fix/chore per commit
- ✅ All pre-commit hooks pass
- ✅ Clear commit messages
- ✅ Update progress tracking

### Workflow
- ✅ Feature branches for each component
- ✅ Frequent commits (after each module)
- ✅ Document session state before context clear
- ✅ Run tests before every commit

---

**Next Steps:**
1. Create progress tracking files (PROGRESS.md, ACTIVE_SESSION.md)
2. Refactor downloader.py into hf/* modules
3. Begin Phase 1A implementation

---

**End of Implementation Guide**
