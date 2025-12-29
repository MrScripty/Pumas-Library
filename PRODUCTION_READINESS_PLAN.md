# Implementation Plan: Production Readiness Improvements

## Prerequisites

### 0. **Unit Testing Framework Setup**
**Goal:** Establish comprehensive testing infrastructure before implementing other tasks

**Current State:**
- Minimal automated testing (1 unittest-based test, 5 manual interactive scripts)
- No pytest infrastructure
- No mocking framework
- Some existing tests may be outdated

**Testing Strategy:**
- **Framework:** pytest (industry standard, better fixtures and assertions)
- **Unit Tests:** Fast, isolated tests with mocked external dependencies
- **Integration Tests:** Tests with real file operations in temp directories
- **Coverage Target:** 80% overall, 90%+ for critical paths (version_manager, metadata_manager)
- **Real File I/O:** Use temp directories, don't mock file operations (more reliable)
- **Mock External APIs:** GitHub API, PyPI, subprocess calls (avoid network dependencies)

**What We'll Build:**
```
tests/
├── conftest.py              # Shared fixtures (temp_launcher_root, metadata_manager, mocks)
├── unit/                    # Fast, isolated unit tests
│   ├── test_metadata_manager.py
│   ├── test_github_integration.py
│   └── test_utils.py
├── integration/             # Integration tests with real resources
│   ├── test_full_installation.py
│   └── test_version_switching.py
└── fixtures/                # Sample data files
    └── sample_releases.json
```

**Implementation Plan:**

**Commit 1: Testing Infrastructure**
- Create `requirements-dev.txt` (pytest, pytest-cov, pytest-mock, pytest-timeout)
- Create `pytest.ini` with 80% coverage requirement
- Create `.coveragerc` for coverage exclusion rules
- Create `tests/conftest.py` with shared fixtures
- Update `.gitignore` for pytest artifacts
- Create `TESTING.md` documentation
- Functional: Can run `pytest` successfully

**Commit 2: Migrate Existing Tests**
- Convert `test_github_release_collapse.py` to pytest
- Convert practical tests (`test_github_integration.py`, `test_resource_manager.py`)
- Extract testable functions from interactive tests
- Remove outdated tests
- Functional: All migrated tests pass

**Commit 3: First New Unit Tests**
- Add `tests/unit/test_metadata_manager.py`
- Add `tests/unit/test_utils.py` (if applicable)
- Functional: Coverage report shows >70% for tested modules

**Notes:**
- Sets foundation for all other tasks
- Each task going forward will include unit tests
- Aim for 80% coverage to catch critical bugs without being overly verbose
- Do not create unit tests for the entire code base, they wil lbe created over time as the code is worked on

---

## High Priority Tasks

### 1. **Input Validation and Sanitization**
**Goal:** Prevent path traversal, command injection, and malformed data crashes

**Key Areas:**
- Version tags: Whitelist alphanumeric + dash/dot only (`^[a-zA-Z0-9.-]+$`)
- File paths: Validate against base directories, no `..` traversal
- URLs: Whitelist `http://` and `https://` schemes only
- User-provided strings: Sanitize before use in filesystem operations

**Implementation Details:**
```python
# Create backend/validators.py
- validate_version_tag(tag: str) -> bool
- validate_url(url: str) -> bool
- sanitize_path(path: Path, base_dir: Path) -> Path
- validate_package_name(name: str) -> bool
```

**Files to modify:**
- `backend/version_manager.py` - validate tags before use
- `backend/api/core.py` - validate all API inputs
- `backend/api/system_utils.py` - validate URLs and paths

---

### 2. **Structured Logging System**
**Goal:** Replace 456 `print()` statements with proper logging

**Implementation Details:**
```python
# Create backend/logging_config.py
import logging
from logging.handlers import RotatingFileHandler

# Configure loggers per module:
- Root logger: INFO level, writes to launcher.log (10MB max, 5 backups)
- Console handler: WARNING+ only (don't spam terminal)
- File handler: DEBUG+ (detailed logs for troubleshooting)
- Format: "%(asctime)s - %(name)s - %(levelname)s - %(message)s"
```

**Migration strategy:**
- Replace `print(f"Warning: {msg}")` → `logger.warning(msg)`
- Replace `print(f"Error: {msg}")` → `logger.error(msg)`
- Replace `print(f"✓ Success")` → `logger.info("Success")`
- Keep user-facing messages as print (installation progress, etc.)

**Files to modify:** All 23 backend files with print statements

---

### 3. **Refactor version_manager.py**
**Goal:** Split 2067-line god object into focused modules

**Proposed structure:**
```
backend/version_management/
├── __init__.py
├── installer.py              # Installation orchestration
├── dependency_resolver.py    # Constraint calculation, PyPI queries
├── launcher.py               # Process launching, health checks
├── venv_manager.py          # Virtual environment operations
└── progress.py              # Installation progress (if not using existing tracker)
```

**Key classes to extract:**
- `VersionInstaller` - install_version, download, extract
- `DependencyResolver` - _build_constraints_for_tag, _fetch_pypi_versions
- `VersionLauncher` - launch_version, _wait_for_server_ready
- `VenvManager` - _create_venv, _get_python_version

**Keep in VersionManager:**
- High-level coordination
- Metadata management
- Active version state

---

### 4. **Specific Exception Handling**
**Goal:** Stop masking bugs with `except Exception:`

**Implementation Details:**
```python
# Create backend/exceptions.py
class ComfyUILauncherError(Exception):
    """Base exception for launcher"""

class InstallationError(ComfyUILauncherError):
    """Installation failed"""

class DependencyError(ComfyUILauncherError):
    """Dependency resolution/installation failed"""

class NetworkError(ComfyUILauncherError):
    """Network operation failed"""

class ValidationError(ComfyUILauncherError):
    """Input validation failed"""

class MetadataError(ComfyUILauncherError):
    """Metadata corruption or read/write failure"""
```

**Refactoring strategy:**
- Identify what can go wrong in each try block
- Catch specific exceptions (FileNotFoundError, urllib.error.URLError, json.JSONDecodeError)
- Re-raise as custom exceptions with context
- Let unexpected exceptions bubble up (they indicate bugs)

**Example:**
```python
# Before:
try:
    releases = self._fetch_from_github()
except Exception as e:
    print(f"Error: {e}")
    return []

# After:
try:
    releases = self._fetch_from_github()
except urllib.error.HTTPError as e:
    if e.code == 403:
        raise NetworkError("GitHub API rate limit exceeded") from e
    raise NetworkError(f"GitHub API error: {e.code}") from e
except urllib.error.URLError as e:
    raise NetworkError("Network unavailable") from e
# Don't catch Exception - let unexpected errors crash with traceback
```

---

### 5. **Fix File Operation Race Conditions**
**Goal:** Prevent metadata corruption from non-atomic writes

**Implementation Details:**
```python
# Create backend/file_utils.py
def atomic_write_json(path: Path, data: dict, lock: threading.Lock = None):
    """Write JSON atomically with optional file locking"""
    temp_path = path.with_suffix('.tmp')

    if lock:
        lock.acquire()
    try:
        # Write to temp file
        with open(temp_path, 'w') as f:
            json.dump(data, f, indent=2)

        # Atomic rename
        temp_path.replace(path)
    finally:
        if lock:
            lock.release()
        if temp_path.exists():
            temp_path.unlink()
```

**Files to update:**
- `backend/metadata_manager.py` - Use atomic writes for all JSON
- `backend/installation_progress_tracker.py` - Add file locking
- `backend/version_manager.py` - Atomic constraint cache writes

**Add validation:**
- Verify JSON is valid before replacing file
- Keep backup of previous version

---

### 6. **Remove Browser Logs from Repo**
**Goal:** Clean git history, update .gitignore

**Implementation:**
```bash
# Update .gitignore (already has launcher-data/ but files were committed before)
echo "*.log" >> .gitignore
echo "launcher-data/profiles/" >> .gitignore

# Remove from git history using git-filter-repo
pip install git-filter-repo
git filter-repo --path launcher-data/profiles/ --invert-paths
git filter-repo --path '*.log' --path-glob '*.log' --invert-paths

# Force push (coordinate with any collaborators first)
git push --force
```

**Note:** Coordinate this with any active branches/PRs

---

## Medium Priority Tasks

### 7. **Pin Dependencies**
**Goal:** Reproducible builds, prevent surprise breakages

**Implementation:**
```bash
# Python - generate lock file
pip install pip-tools
pip-compile requirements.txt --output-file requirements-lock.txt

# Node - commit the lock file
git add frontend/package-lock.json
git rm frontend/.gitignore  # Remove package-lock.json from ignore

# Update install.sh to use lock file
pip install -r requirements-lock.txt  # Instead of requirements.txt
```

**Maintenance process:**
- Use `requirements-lock.txt` in production
- Update with `pip-compile --upgrade` when needed
- Test after dependency updates

---

### 8. **Consolidate Hardcoded Config**
**Goal:** All configuration in config.py

**Current hardcoded values to move:**
```python
# github_integration.py
GITHUB_API_BASE = "https://api.github.com"  → NetworkConfig
COMFYUI_REPO = "comfyanonymous/ComfyUI"    → AppConfig
PER_PAGE = 100                              → NetworkConfig
MAX_PAGES = 10                              → NetworkConfig
DEFAULT_TTL = 3600                          → NetworkConfig

# version_manager.py
PIP_CACHE_DIR naming                        → PathsConfig
CONSTRAINTS_DIR naming                      → PathsConfig
```

**Create new config class:**
```python
@dataclass(frozen=True)
class AppConfig:
    """Application-level configuration."""
    GITHUB_REPO: str = "comfyanonymous/ComfyUI"
    GITHUB_API_BASE: str = "https://api.github.com"
    APP_NAME: str = "ComfyUI Setup"
    LOG_FILE_MAX_BYTES: int = 10_485_760  # 10MB
    LOG_FILE_BACKUP_COUNT: int = 5
```

---

### 9. **Rate Limiting for Destructive Actions**
**Goal:** Prevent user from spamming install/remove/cancel

**Implementation:**
```python
# Create backend/rate_limiter.py
from collections import defaultdict
from time import time

class RateLimiter:
    def __init__(self, max_calls: int, period_seconds: int):
        self.max_calls = max_calls
        self.period = period_seconds
        self.calls = defaultdict(list)

    def is_allowed(self, key: str) -> bool:
        now = time()
        self.calls[key] = [t for t in self.calls[key] if now - t < self.period]

        if len(self.calls[key]) >= self.max_calls:
            return False

        self.calls[key].append(now)
        return True

# Usage in JavaScriptAPI:
rate_limiter = RateLimiter(max_calls=3, period_seconds=60)

def install_version(self, tag):
    if not rate_limiter.is_allowed('install'):
        return {"success": False, "error": "Rate limit exceeded. Please wait."}
    # ... proceed with installation
```

**Apply to:**
- `install_version()` - Max 3 installs per minute
- `remove_version()` - Max 5 removes per minute
- `cancel_installation()` - Max 10 cancels per minute

---

### 10. **Exponential Backoff with Jitter**
**Goal:** Better retry resilience for network operations

**What is exponential backoff with jitter:**
- Exponential backoff: Wait times increase exponentially (2s, 4s, 8s, 16s...)
- Jitter: Add random 0-1 second to prevent synchronized retries
- Prevents "thundering herd" when services recover
- Gives failed services time to stabilize

**Implementation:**
```python
# Update github_integration.py
import random

def download_with_retry(
    self,
    url: str,
    destination: Path,
    max_retries: int = 3,
    progress_callback = None
) -> bool:
    for attempt in range(max_retries):
        if attempt > 0:
            # Exponential backoff: 2^attempt seconds
            base_wait = 2 ** attempt  # 2s, 4s, 8s

            # Add jitter: random 0-1 seconds
            jitter = random.uniform(0, 1)
            wait_time = base_wait + jitter

            print(f"Retry attempt {attempt + 1}/{max_retries} in {wait_time:.1f}s...")
            time.sleep(wait_time)

        if self.download_file(url, destination, progress_callback):
            return True

    print(f"Download failed after {max_retries} attempts")
    return False
```

**Also apply to:**
- GitHub API fetches
- PyPI package resolution
- ComfyUI server health checks

---

### 11. **Security Audit Setup**
**Goal:** Automated vulnerability scanning

**What this does:**
- Scans dependencies for known CVEs (Common Vulnerabilities and Exposures)
- Checks both direct and transitive dependencies
- Alerts when vulnerable packages are found
- Different from runtime dependency checking (which checks if packages are installed)

**Implementation:**
```bash
# Add to CI/CD workflow or run manually

# Python dependencies
pip install pip-audit
pip-audit

# Node dependencies
npm audit

# Fix vulnerabilities automatically
pip-audit --fix
npm audit fix
```

**Create GitHub Action:**
```yaml
# .github/workflows/security-scan.yml
name: Security Scan
on: [push, pull_request]

jobs:
  scan:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Python Security Scan
        run: |
          pip install pip-audit
          pip-audit --desc

      - name: Node Security Scan
        run: |
          cd frontend
          npm audit --production
```

**Schedule:** Run weekly + on every PR

---

### 12. **Pre-commit Hooks Setup**
**Goal:** Enforce code quality automatically

**What this does:**
- **Black**: Auto-formats code to consistent style
- **isort**: Sorts and organizes imports
- **Flake8**: Catches bugs, style issues, unused variables
- **mypy**: Validates type hints

Runs automatically before each `git commit`, blocks commit if checks fail.

**Implementation:**
```bash
# Install pre-commit
pip install pre-commit

# Create .pre-commit-config.yaml
cat > .pre-commit-config.yaml << 'EOF'
repos:
  - repo: https://github.com/psf/black
    rev: 24.1.1
    hooks:
      - id: black
        language_version: python3.12
        args: [--line-length=100]

  - repo: https://github.com/pycqa/isort
    rev: 5.13.2
    hooks:
      - id: isort
        args: [--profile=black, --line-length=100]

  - repo: https://github.com/pycqa/flake8
    rev: 7.0.0
    hooks:
      - id: flake8
        args: [--max-line-length=100, --ignore=E203,W503]

  - repo: https://github.com/pre-commit/mirrors-mypy
    rev: v1.8.0
    hooks:
      - id: mypy
        additional_dependencies: [types-all]
        args: [--ignore-missing-imports, --allow-untyped-calls]
EOF

# Install hooks
pre-commit install

# Run on all files (first time)
pre-commit run --all-files
```

**Configuration notes:**
- `--line-length=100` - Modern standard, good for reading
- `--ignore=E203,W503` - Black compatibility
- `--ignore-missing-imports` - For mypy, gradual adoption

---

### 13. **Type Hint Enforcement**
**Goal:** Catch type errors before runtime

**What this does:**
- Validates type hints are correct
- Catches bugs like returning wrong types
- Improves IDE autocomplete
- Makes refactoring safer
- Acts as inline documentation

**Practical benefits:**
- Catches ~20% of bugs that would require debugging
- Better IDE support (autocomplete knows types)
- Refactoring safety (finds all affected call sites)

**Implementation:**
```bash
# Create mypy.ini
cat > mypy.ini << 'EOF'
[mypy]
python_version = 3.12
warn_return_any = True
warn_unused_configs = True
disallow_untyped_defs = False  # Start lenient, tighten later
ignore_missing_imports = True

# Gradually make stricter:
# [mypy-backend.version_manager]
# disallow_untyped_defs = True
EOF

# Run mypy
mypy backend/
```

**Incremental adoption strategy:**
1. Run mypy with lenient settings initially
2. Fix obvious type errors
3. Add type hints to new code
4. Gradually enable stricter checks per module
5. Eventually enable `--strict` mode

**Focus areas first:**
- Public API methods in `api/core.py`
- Data models in `models.py`
- New code in refactored modules

---

### 14. **SBOM Generation**
**Goal:** Track complete dependency inventory for security

**What is SBOM (Software Bill of Materials):**
- Complete list of ALL software components (not just direct dependencies)
- Includes transitive dependencies (dependencies of dependencies)
- Different from requirements.txt which only lists what YOU install
- Example: requirements.txt has 10 packages, SBOM shows 50+ (including their deps)

**Why it matters:**
- Security: When CVE is announced, check if vulnerable package is in your SBOM
- Compliance: Some industries/enterprises require SBOMs
- Transparency: Know exactly what's in your application

**requirements.txt vs SBOM:**
```
requirements.txt:          SBOM (complete tree):
- pywebview>=5.0          - pywebview==5.0.5
- click>=8.1                └── PyGObject==3.46.0
- psutil>=5.9                   └── pycairo==1.25.1
                           - click==8.1.7
                           - psutil==5.9.8
                           (50+ more packages)
```

**Implementation:**
```bash
# Install SBOM generators
pip install cyclonedx-bom
npm install -g @cyclonedx/cyclonedx-npm

# Generate Python SBOM
cyclonedx-py -o sbom-python.json

# Generate Node SBOM
cd frontend
npx @cyclonedx/cyclonedx-npm --output-file sbom-frontend.json

# Combine (manual or scripted)
# Store in release artifacts
```

**When to generate:**
- Before each release
- After dependency updates
- On security audit requests

**Storage:**
- Include in release assets
- Commit to repo in `docs/sbom/`
- Upload to GitHub releases

---

## Lower Priority Tasks

### 15. **Improve Async Handling**
**Goal:** Future-proof for complex operations

**Current state:** Already uses threading + progress polling - works well

**Potential improvements:**
- Consider `asyncio` for network operations
- Use `concurrent.futures` for parallel tasks
- Profile actual blocking operations before optimizing

**Not urgent unless:** UI freezing is reported by users

---

### 16. **SQLite Migration**
**Goal:** Better metadata management than JSON files

**Benefits:**
- ACID transactions
- Query capability
- Better performance with many versions
- No corruption risk

**Migration path:**
```python
# Create backend/database.py
import sqlite3

# Schema:
CREATE TABLE versions (
    tag TEXT PRIMARY KEY,
    path TEXT NOT NULL,
    installed_date TEXT NOT NULL,
    python_version TEXT,
    release_tag TEXT
);

CREATE TABLE metadata (
    key TEXT PRIMARY KEY,
    value TEXT
);

# Migration script:
- Read existing JSON files
- Insert into SQLite
- Keep JSON as backup
- Gradual migration: read from SQLite, fallback to JSON
```

**Effort:** 2-3 days
**Value:** High for scalability, medium for current scale

---

### 17. **Code Quality Issues**
**Goal:** Consistent style, no magic numbers

**Will be mostly solved by:**
- Black (auto-formatting)
- Flake8 (style enforcement)
- Pre-commit hooks (automation)

**Manual cleanup:**
- Extract magic numbers to constants
- Remove commented-out code
- Standardize error messages

---

## Implementation Order Recommendation

### Phase 0: Testing Foundation (FIRST - PREREQUISITE)
0. **Unit testing framework setup** - Task #0
   - Establish pytest infrastructure
   - Create shared fixtures and conftest.py
   - Migrate existing practical tests
   - Write first unit tests (metadata_manager, utils)
   - Documentation (TESTING.md)
   - **3 functional commits**

### Week 1: Quick Wins (Foundation Setup)
1. **Pre-commit hooks setup** (30 min) - Task #12
   - Automates code quality from now on
   - Easiest to set up first

2. **Pin dependencies** (1 hour) - Task #7
   - Prevents surprise breakages
   - Generate requirements-lock.txt

3. **Remove browser logs** (30 min) - Task #6
   - Clean up repository
   - Update .gitignore

4. **Set up pip-audit/npm audit** (30 min) - Task #11
   - Security baseline
   - Run initial scan

5. **Add exponential backoff** (1 hour) - Task #10
   - Quick win for reliability
   - Simple code change

**Total time: ~3.5 hours**

---

### Week 2: Foundation (Core Infrastructure)
6. **Structured logging** (4 hours) - Task #2
   - Create logging_config.py
   - Start migrating print() statements
   - Don't need to migrate all at once

7. **Custom exceptions** (2 hours) - Task #4
   - Create exceptions.py
   - Define exception hierarchy
   - Start using in new code

8. **Input validation** (3 hours) - Task #1
   - Create validators.py
   - Add validation to critical paths
   - Version tags, URLs, paths

9. **Consolidate config** (2 hours) - Task #8
   - Add AppConfig to config.py
   - Move hardcoded values
   - Update references

**Total time: ~11 hours**

---

### Week 3-4: Major Refactoring (Big Changes)
10. **Refactor version_manager.py** (2-3 days) - Task #3
    - Plan module structure
    - Extract classes incrementally
    - Test after each extraction
    - This is the biggest task

11. **Fix file race conditions** (1 day) - Task #5
    - Create file_utils.py
    - Implement atomic writes
    - Add file locking
    - Update all JSON writes

12. **Rate limiting** (2 hours) - Task #9
    - Create rate_limiter.py
    - Add to API methods
    - Test limits work

13. **Type hints + mypy** (ongoing) - Task #13
    - Set up mypy.ini
    - Fix initial errors
    - Add to new code going forward

**Total time: ~3-4 days**

---

### Week 5: Production Readiness (Polish)
14. **SBOM generation** (2 hours) - Task #14
    - Install generators
    - Generate SBOMs
    - Document process

15. **Security audit** (1 hour) - Task #11 continued
    - Review pip-audit results
    - Fix any vulnerabilities
    - Document findings

16. **Documentation updates** (2 hours)
    - Update README
    - Add CONTRIBUTING.md
    - Document new processes

**Total time: ~5 hours**

---

## Task Dependencies

Some tasks depend on others being completed first:

```
Unit Testing Framework (0) ← MUST BE FIRST
  ↓
Pre-commit hooks (12)
  ↓
Logging (2) + Exceptions (4) + Validation (1)
  ↓
Refactor version_manager.py (3)
  ↓
File race conditions (5)
  ↓
Rate limiting (9)

Independent tasks (can do anytime after testing framework):
- Pin dependencies (7)
- Remove browser logs (6)
- Exponential backoff (10)
- Security audit (11)
- Consolidate config (8)
- Type hints (13)
- SBOM (14)
```

---

## Success Metrics

Track progress with:
- [x] Unit testing framework established (pytest, 80% coverage target)
- [x] TESTING.md documentation complete
- [ ] All print() statements replaced with logging
- [ ] No `except Exception:` in codebase
- [ ] All user inputs validated
- [ ] version_manager.py under 500 lines
- [x] All JSON writes are atomic (MetadataManager already uses atomic writes)
- [ ] pip-audit shows 0 vulnerabilities
- [ ] Pre-commit hooks pass on all commits
- [ ] mypy passes with no errors
- [ ] SBOM generated for latest release

---

## Notes

- **Local-only app:** No need for authentication/multi-user support
- **GitHub API:** Read-only, minimal usage - no credentials needed
- **Early development:** Semantic versioning starts at first release
- **Test coverage:** 40 tests passing, 22.29% overall coverage baseline. Targeting 80% coverage.
- **UI freezing:** Not currently an issue, async improvements are low priority

---

## Implementation Notes & Decisions

### Clarifications Made:
1. **Testing Framework (Task #0):** Added as prerequisite before all other tasks
   - Using pytest with 80% coverage target (not 100% - avoiding overly verbose tests)
   - Existing tests may be outdated - will migrate practical ones, remove obsolete
   - Each commit must be functional (no broken intermediate states)
   - Large sections split into multiple functional commits

2. **Commit Strategy:**
   - Each commit must be runnable without errors
   - Large tasks can be split into multiple commits if each is functional
   - Testing framework will have 3 commits (infrastructure, migrations, new tests)

3. **CI/CD:**
   - No GitHub Actions workflow needed at this time
   - Manual testing sufficient for current scope

4. **Code Quality:**
   - Black line length: 100 characters (modern standard)
   - Coverage: 80% overall, 90%+ for critical modules
   - Logging format: Plain text (human-readable)

---

## Questions or Clarifications Needed

- ~~Should we set up CI/CD pipeline in parallel with these tasks?~~ **RESOLVED: No GitHub Actions needed**
- Any preference for logging format (JSON vs plain text)? **RESOLVED: Plain text**
- Target line length for Black/Flake8 (100 recommended)? **RESOLVED: 100 chars**
- Should SQLite migration happen before or after first release? **TBD: Later priority**

---

## Implementation Progress

### ✅ Phase 0: Testing Foundation - COMPLETED (2025-12-29)

**Task #0: Unit Testing Framework Setup** - 3 commits completed

**Commit 1: Testing Infrastructure** (cffdda3)
- Created requirements-dev.txt with pytest, pytest-cov, pytest-mock, pytest-timeout
- Set up pytest.ini with coverage reporting and markers
- Created .coveragerc with exclusion rules
- Built tests/conftest.py with shared fixtures
- Updated .gitignore for pytest artifacts
- Wrote comprehensive TESTING.md documentation
- Status: ✅ Infrastructure functional, pytest runs successfully

**Commit 2: Migrate Existing Tests** (051e6a9)
- Converted unittest tests to pytest format
- Created tests/unit/test_github_integration.py (9 tests - release collapse logic)
- Created tests/unit/test_file_opener.py (3 tests - file opening)
- Created tests/unit/test_installation_progress.py (8 tests - progress tracking)
- Moved 7 interactive tests to manual-tests/ directory
- Status: ✅ 17 migrated tests passing

**Commit 3: First New Unit Tests** (bbe965e)
- Created tests/unit/test_metadata_manager.py (23 comprehensive tests)
- Achieved 94.68% coverage on MetadataManager (critical module)
- Tests cover CRUD, error handling, atomic writes, edge cases
- Status: ✅ 40 total tests passing, 22.29% overall coverage

**Key Achievements:**
- 40 tests passing (0 failures)
- 22.29% overall backend coverage baseline
- Critical module coverage: MetadataManager 94.68%, models.py 100%
- Testing infrastructure ready for "test what you touch" approach
- All commits functional and properly tested

---

### 🚧 Next Up: Week 1 Quick Wins

Ready to implement:
1. Task #12: Pre-commit hooks setup (~30 min)
2. Task #7: Pin dependencies (~1 hour)
3. Task #6: Remove browser logs (~30 min)
4. Task #11: Security audit setup (~30 min)
5. Task #10: Exponential backoff (~1 hour)
