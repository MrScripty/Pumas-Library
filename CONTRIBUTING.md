# Contributing to ComfyUI Linux Launcher

This document outlines the development standards, practices, and workflows for contributing to this project. All code must adhere to these standards.

---

## Table of Contents

1. [Development Setup](#development-setup)
2. [Code Standards](#code-standards)
3. [Testing Requirements](#testing-requirements)
4. [Type Hints and Type Safety](#type-hints-and-type-safety)
5. [Pre-commit Hooks](#pre-commit-hooks)
6. [Architecture Patterns](#architecture-patterns)
7. [Security Practices](#security-practices)
8. [Commit Guidelines](#commit-guidelines)

---

## Development Setup

### Prerequisites

- Python 3.12+
- Node.js 14+
- GTK/WebKit libraries (for PyWebView)

### Initial Setup

```bash
# Clone the repository
git clone <repository-url>
cd Linux-ComfyUI-Launcher

# Run installation script
./install.sh

# Activate virtual environment
source venv/bin/activate

# Install development dependencies
pip install -r requirements-dev.txt

# Install pre-commit hooks
pre-commit install
```

### Running the Application

```bash
# Normal mode
./launcher

# Developer mode (console visible)
./launcher dev

# Rebuild frontend
./launcher build
```

---

## Code Standards

### Formatting and Style

We use **Black** and **isort** for automatic code formatting, and **Ruff** for linting.

**Line Length:** 100 characters

**Black Configuration:**
- Automatically formats code to PEP 8 standards
- Enforced via pre-commit hooks

**isort Configuration:**
- Profile: black-compatible
- Sorts imports alphabetically within sections

**Ruff:**
- Replaces flake8 with faster, more granular control
- Currently checks for undefined names (F821, F822, F823)
- Run manually: `ruff check backend/`

### Code Organization

```
backend/
├── api/                    # Public API layer
├── resources/              # Resource management (models, custom nodes)
├── version_manager_components/  # Version management mixins
├── *.py                    # Core functionality modules
frontend/
├── src/                    # React components and utilities
tests/
├── unit/                   # Fast, isolated unit tests
├── integration/            # Integration tests with real I/O
├── conftest.py             # Shared fixtures
```

**Key Principles:**
- Separation of concerns (API layer vs business logic)
- Mixin-based composition for complex managers
- Type protocols for interface contracts

### Logging

**DO NOT use `print()` statements** in backend code. Use the centralized logging system instead.

```python
from backend.logging_config import get_logger

logger = get_logger(__name__)

# Use appropriate log levels
logger.debug("Detailed diagnostic information")
logger.info("General informational messages")
logger.warning("Warning messages for recoverable issues")
logger.error("Error messages for failures", exc_info=True)
```

**Exception:** User-facing output (installation progress, CLI output) can use `print()` with `# noqa: print` comment.

**Pre-commit Hook:** Automatically checks for print statements and enforces logging usage.

### Exception Handling

**DO NOT use generic exception handlers.** Always catch specific exception types.

```python
# ❌ BAD - Generic exception handling
try:
    do_something()
except Exception as e:
    logger.error(f"Error: {e}")

# ✅ GOOD - Specific exception handling
try:
    do_something()
except FileNotFoundError as e:
    raise MetadataError(f"Metadata file not found: {e}") from e
except json.JSONDecodeError as e:
    raise MetadataError(f"Invalid metadata format: {e}") from e
```

**Custom Exceptions:**
All custom exceptions are defined in `backend/exceptions.py`:
- `ComfyUILauncherError` - Base exception
- `InstallationError` - Installation failures
- `DependencyError` - Dependency issues
- `NetworkError` - Network operations
- `ValidationError` - Input validation failures
- `MetadataError` - Metadata corruption

**Pre-commit Hook:** Automatically detects bare `except:` and `except Exception:` handlers.

**Exception:** Use `# noqa: generic-exception` for cases where generic catching is truly necessary.

### Input Validation

**All user inputs and external data must be validated** using the validators in `backend/validators.py`.

```python
from backend.validators import (
    validate_version_tag,
    validate_url,
    sanitize_path,
    validate_package_name
)

# Validate version tags
if not validate_version_tag(tag):
    raise ValidationError(f"Invalid version tag: {tag}")

# Validate and sanitize file paths
safe_path = sanitize_path(user_path, base_directory)

# Validate URLs
if not validate_url(download_url):
    raise ValidationError(f"Invalid URL: {download_url}")
```

**Security Considerations:**
- Version tags: Alphanumeric + dash/dot only (`^[a-zA-Z0-9.-]+$`)
- File paths: No `..` traversal, must be within base directory
- URLs: `http://` and `https://` schemes only
- Package names: Standard Python package format

### Configuration

**All configuration values must be in `backend/config.py`.** Do not hardcode values.

```python
from backend.config import (
    UIConfig,
    AppConfig,
    NetworkConfig,
    PathsConfig
)

# Use configuration values
github_api_base = AppConfig.GITHUB_API_BASE
max_retries = NetworkConfig.MAX_RETRIES
```

---

## Testing Requirements

### Philosophy: "Test What You Touch"

We follow an **incremental testing approach**:
- New files: Must have ≥80% test coverage
- Modified files: Must maintain or improve coverage
- Untouched files: No coverage requirement (yet)

### Running Tests

```bash
# Run all tests
pytest

# Run unit tests only
pytest tests/unit/

# Run with coverage report
pytest --cov=backend --cov-report=html

# Run specific test file
pytest tests/unit/test_metadata_manager.py

# Run tests in parallel (faster)
pytest -n auto
```

### Writing Tests

**Test Structure:**
```python
import pytest
from backend.metadata_manager import MetadataManager

def test_metadata_creation(metadata_manager, temp_metadata_dir):
    """Test that metadata is created correctly."""
    # Arrange
    version_data = {"tag": "v1.0.0", "path": "/some/path"}

    # Act
    metadata_manager.save_version_metadata("v1.0.0", version_data)

    # Assert
    loaded = metadata_manager.get_version_metadata("v1.0.0")
    assert loaded == version_data
```

**Test Markers:**
```python
@pytest.mark.unit         # Fast, isolated unit tests
@pytest.mark.integration  # Integration tests with real I/O
@pytest.mark.slow         # Tests taking >1 second
@pytest.mark.network      # Tests requiring network (should mock)
```

**Fixtures:**
Shared fixtures are in `tests/conftest.py`:
- `temp_launcher_root` - Temporary test directory
- `temp_metadata_dir` - Temporary metadata storage
- `metadata_manager` - MetadataManager instance
- `sample_releases` - Mock GitHub release data

**Best Practices:**
- Use real file I/O in temp directories (don't mock filesystem)
- Mock external APIs (GitHub, PyPI, subprocess calls)
- Test edge cases and error conditions
- Keep tests fast (<1 second each for unit tests)

### Coverage Enforcement

**Pre-commit Hook:** Automatically runs full test suite before each commit.

**Coverage Goals:**
- Overall: 80% (not enforced yet, incremental approach)
- New files: 80% required (enforced via pre-commit)
- Critical modules: 90%+ (metadata_manager, version_manager, validators)

**Coverage Report:**
```bash
# View HTML coverage report
pytest --cov=backend --cov-report=html
open htmlcov/index.html
```

---

## Type Hints and Type Safety

### Requirements

**All new code must have complete type hints.**

```python
from typing import Optional, Dict, List, Any
from pathlib import Path

def install_version(
    self,
    tag: str,
    progress_callback: Optional[Callable[[str, float], None]] = None
) -> bool:
    """Install a specific ComfyUI version."""
    # Implementation
```

### mypy Configuration

We use **mypy** for static type checking. Configuration is in `mypy.ini`.

**Running mypy:**
```bash
# Check all backend code
mypy backend/

# Check specific module
mypy backend/api/core.py
```

**Pre-commit Hook:** mypy runs automatically on every commit (will fail if type errors exist).

**Type Checking Standards:**
- Use `from __future__ import annotations` for deferred evaluation
- Use `Optional[T]` for nullable types
- Use `Any` sparingly (only when truly dynamic)
- Define protocols for duck-typed interfaces (see `version_manager_components/protocols.py`)

### Common Patterns

```python
from __future__ import annotations
from typing import Optional, Callable, Protocol

# Type aliases for clarity
ProgressCallback = Callable[[str, float], None]
MetadataDict = Dict[str, Any]

# Protocol for duck typing
class HasMetadata(Protocol):
    def get_metadata(self) -> MetadataDict: ...
    def set_metadata(self, data: MetadataDict) -> None: ...
```

---

## Pre-commit Hooks

Pre-commit hooks **automatically enforce code quality** before each commit.

### Installed Hooks

1. **Black** - Auto-formats Python code
2. **isort** - Sorts imports
3. **check-print-statements** - Ensures logging system usage
4. **check-generic-exceptions** - Prevents bare exception handlers
5. **pytest** - Runs full test suite
6. **ruff-undefined** - Checks for undefined variables
7. **mypy** - Type checking
8. **General hooks** - trailing whitespace, EOF, YAML/JSON validation, large file detection, private key detection

### How They Work

```bash
# Hooks run automatically on git commit
git commit -m "Add feature"

# If hooks fail, commit is blocked
# Fix issues, then commit again

# Run hooks manually on all files
pre-commit run --all-files

# Skip hooks (NOT RECOMMENDED - only for emergencies)
git commit --no-verify
```

### Hook Behavior

**Auto-fixing hooks (Black, isort):**
- Automatically modify files
- You must `git add` the changes and commit again

**Validation hooks (pytest, mypy, ruff):**
- Block commit if checks fail
- Fix issues manually, then commit again

---

## Architecture Patterns

### API Layer

**Public API:** `backend/api/core.py` (`ComfyUISetupAPI` class)
- All JavaScript bindings go through this class
- Coordinates between subsystems (version manager, resource manager, etc.)
- Rate limiting for destructive actions
- Input validation on all entry points

### Manager Classes

**Responsibilities:**
- `MetadataManager` - JSON metadata persistence
- `VersionManager` - Version installation/launching/management
- `ResourceManager` - Model and custom node symlinks
- `GitHubReleasesFetcher` - GitHub API interactions

**Pattern: Mixin Composition**

The `VersionManager` uses mixins for modularity:
```python
# backend/version_manager_components/
installer.py      # Installation orchestration
launcher.py       # Process launching, health checks
dependencies.py   # Venv and dependency management
state.py          # Active/default version state
constraints.py    # Constraints cache and PyPI queries
protocols.py      # Type protocols for contracts
```

### File Operations

**Atomic Writes:** All JSON writes use `backend/file_utils.atomic_write_json()`

```python
from backend.file_utils import atomic_write_json

# Atomic write with automatic locking
atomic_write_json(path, data, lock=self._lock)
```

**Benefits:**
- No corruption from interrupted writes
- Thread-safe with file locking
- Automatic backup of previous version
- JSON validation before commit

### Network Operations

**Retry Logic:** Use `backend/retry_utils.py` for network operations

```python
from backend.retry_utils import retry_operation

success = retry_operation(
    operation=lambda: download_file(url, dest),
    max_retries=3,
    operation_name="download ComfyUI release"
)
```

**Features:**
- Exponential backoff with jitter (2s, 4s, 8s...)
- Automatic logging of retry attempts
- Configurable max delay and retries

---

## Security Practices

### Input Validation

**Always validate before use:**
- Version tags, URLs, file paths, package names
- See `backend/validators.py` for validation functions

### Dependency Scanning

**Run security audits regularly:**
```bash
# Python dependencies
pip-audit

# Node.js dependencies
cd frontend && npm audit
```

**Pre-release:** Always run security scans before releases.

**Vulnerability Handling:**
- Fix immediately if high/critical severity
- Update dependency pins in `requirements-lock.txt` and `frontend/package-lock.json`
- Regenerate SBOM after fixes

### Secrets

**Never commit:**
- API keys, tokens, credentials
- `.env` files, config with secrets
- Log files with sensitive data

**Pre-commit hook** detects private keys automatically.

---

## Commit Guidelines

### Commit Messages

**Format:**
```
<type>: <short summary>

<optional detailed description>

🤖 Generated with Claude Code
Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

**Types:**
- `feat`: New feature
- `fix`: Bug fix
- `refactor`: Code restructuring (no behavior change)
- `test`: Add/update tests
- `docs`: Documentation changes
- `chore`: Maintenance (deps, config)

**Examples:**
```
feat: Add exponential backoff to GitHub API calls

fix: Prevent metadata corruption with atomic writes

test: Add unit tests for MetadataManager (94% coverage)
```

### Pre-commit Checklist

Before committing, ensure:
- [ ] Code follows Black formatting (auto-fixed)
- [ ] Imports sorted with isort (auto-fixed)
- [ ] No print() statements in backend (except with `# noqa: print`)
- [ ] Specific exception handling (no bare `except:`)
- [ ] All tests pass (`pytest`)
- [ ] Type hints added and mypy passes
- [ ] New code has ≥80% test coverage
- [ ] Input validation on all external data

**Pre-commit hooks enforce most of these automatically.**

---

## Development Workflow

### Adding a New Feature

1. **Create a branch** (if using Git flow)
2. **Write tests first** (TDD approach recommended)
3. **Implement feature** with type hints
4. **Validate coverage** ≥80% for new code
5. **Run pre-commit** hooks (automatic on commit)
6. **Update documentation** if needed
7. **Commit** with descriptive message

### Modifying Existing Code

1. **Read existing tests** to understand behavior
2. **Add tests** for new behavior
3. **Modify code** with type hints
4. **Ensure tests pass** (including existing tests)
5. **Verify coverage** maintained or improved
6. **Commit** changes

### Refactoring

1. **Ensure existing tests pass** first
2. **Keep tests passing** throughout refactor
3. **Add new tests** if coverage drops
4. **Update type hints** if signatures change
5. **Update documentation** if architecture changes

---

## Additional Resources

- [docs/TESTING.md](docs/TESTING.md) - Comprehensive testing guide
- [docs/SECURITY.md](docs/SECURITY.md) - Security practices and vulnerability scanning
- [README.md](README.md) - Project overview and installation
- [docs/README.md](docs/README.md) - Documentation index

---

## Getting Help

For questions or clarification on these standards, please:
1. Check existing documentation
2. Review similar code patterns in the codebase
3. Open an issue for discussion

---

## Summary: Quick Reference

**Before writing code:**
- Set up pre-commit hooks (`pre-commit install`)
- Understand the architecture (see above)

**While writing code:**
- Use logging, not print()
- Add type hints to all functions
- Validate all external inputs
- Use specific exception handling

**Before committing:**
- Write tests (≥80% coverage for new code)
- Run `pytest` and `mypy`
- Ensure pre-commit hooks pass
- Write clear commit message

**The pre-commit hooks will catch most issues automatically!**
