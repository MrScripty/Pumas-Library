# Testing Guide

Complete guide for running and writing tests for the ComfyUI Launcher.

---

## Quick Start

### Run All Tests
```bash
cd <repo-root>
source venv/bin/activate
pytest backend/tests/ -v
```

### Run with Coverage
```bash
pytest backend/tests/ --cov=backend --cov-report=html
open htmlcov/index.html
```

### Run Specific Tests
```bash
# Single file
pytest backend/tests/test_process_manager.py -v

# Single test
pytest backend/tests/test_process_manager.py::test_init_creates_resource_tracker -v

# Pattern match
pytest backend/tests/ -k "github" -v
```

---

## Current Status

**Tests:** 219 passing
**Coverage:** 27.10% overall (Target: 80%+)
**Last Updated:** 2025-12-30

### Module Coverage

**Excellent (80%+):**
- process_manager.py: 98.19%
- system_utils.py: 92.91%
- process_resource_tracker.py: 86.07%
- patch_manager.py: 80.67%

**Complete (100%):**
- dependency_manager.py: 100%
- version_manager.py: 100%
- models.py: 100%

**In Progress (20-40%):**
- github_integration.py: 37.24%
- metadata_manager.py: 28.85%

**Needs Tests (0-20%):**
- See [COMPREHENSIVE_UNIT_TEST_PLAN.md](COMPREHENSIVE_UNIT_TEST_PLAN.md) for details

---

## Writing Tests

### Test File Template

```python
"""
Unit tests for backend/module_name.py
"""

import pytest
from unittest.mock import Mock, patch
from backend.module_name import ClassName


class TestClassName:
    """Tests for ClassName"""

    def test_method_success(self):
        """Test that method succeeds with valid input."""
        # Arrange
        instance = ClassName()

        # Act
        result = instance.method("valid_input")

        # Assert
        assert result == expected_value

    def test_method_raises_on_invalid_input(self):
        """Test that method raises ValueError on invalid input."""
        instance = ClassName()

        with pytest.raises(ValueError, match="Invalid input"):
            instance.method("invalid_input")

    def test_method_with_mock(self, mocker):
        """Test that method correctly uses dependency."""
        mock_dep = mocker.patch('backend.module_name.dependency')
        instance = ClassName()

        instance.method("input")

        mock_dep.assert_called_once()
```

### Common Patterns

**Parametrized Tests:**
```python
@pytest.mark.parametrize("input,expected", [
    ("v0.1.0", "v0-1-0"),
    ("v0.2.0-beta", "v0-2-0-beta"),
])
def test_slugify(input, expected):
    assert slugify(input) == expected
```

**Temporary Files:**
```python
def test_file_operation(tmp_path):
    file = tmp_path / "test.txt"
    write_function(file, "content")
    assert file.read_text() == "content"
```

**Mocking:**
```python
def test_with_mock(mocker):
    mock_subprocess = mocker.patch('subprocess.run')
    mock_subprocess.return_value.returncode = 0

    result = function_calling_subprocess()

    mock_subprocess.assert_called_once()
    assert result.success
```

---

## Test Organization

### Directory Structure
```
backend/tests/
├── conftest.py              # Shared fixtures
├── test_process_manager.py  # Process management
├── test_github_integration.py  # GitHub API
├── test_version_manager.py  # Version management
└── ...
```

### Naming Conventions

**Test Files:** `test_<module_name>.py`
**Test Classes:** `Test<ClassName>`
**Test Functions:** `test_<method>_<scenario>_<expected>()`

**Examples:**
- `test_init_sets_launcher_root()`
- `test_get_releases_force_refresh()`
- `test_fetch_page_raises_after_max_retries()`

---

## Best Practices

### DO ✅
- Test behavior, not implementation
- Mock external dependencies (network, filesystem, subprocess)
- Use fixtures for common setup
- Write clear test names
- Test error paths explicitly
- Keep tests fast (< 1s each)
- Use parametrize for variations

### DON'T ❌
- Test third-party libraries
- Share state between tests
- Ignore test failures
- Skip coverage gaps without reason
- Use real network/filesystem in unit tests
- Test multiple things in one test

---

## Common Fixtures

Located in [backend/tests/conftest.py](../backend/tests/conftest.py):

### Temporary Paths
- `tmp_path` - Pytest built-in temporary directory

### Mock Managers
- `mock_metadata_manager` - MetadataManager mock
- `mock_github_fetcher` - GitHubReleasesFetcher mock
- `mock_version_manager` - VersionManager mock

### Sample Data
- `sample_github_release` - GitHub release dict
- `sample_process_info` - Process info dict

---

## Coverage Analysis

### Generate Detailed Report
```bash
pytest --cov=backend --cov-report=html --cov-report=term-missing backend/tests/
open htmlcov/index.html
```

### Check Specific Module
```bash
pytest --cov=backend.api.process_manager --cov-report=term-missing
```

### Common Uncovered Areas
1. **Error handlers** - Exception paths not tested
2. **Edge cases** - Empty inputs, None values, boundary conditions
3. **Fallback logic** - Alternative code paths
4. **Cleanup code** - Finally blocks, context managers
5. **Conditional branches** - If/else not fully covered

---

## Troubleshooting

### Tests Failing Locally
```bash
# Clear pytest cache
pytest --cache-clear

# Run with verbose output
pytest -vv -s

# Run single failing test
pytest backend/tests/test_file.py::test_name -vv
```

### Import Errors
```bash
# Ensure backend is in Python path
export PYTHONPATH="${PYTHONPATH}:$(pwd)"

# Activate virtual environment
source venv/bin/activate
```

### Slow Tests
```bash
# Show slowest tests
pytest --durations=10

# Run in parallel (requires pytest-xdist)
pytest -n auto
```

---

## Implementation Plan

For detailed test implementation plans, see:
- **[COMPREHENSIVE_UNIT_TEST_PLAN.md](COMPREHENSIVE_UNIT_TEST_PLAN.md)** - Original module-by-module plan (450+ test specifications)
- **[UNDER_COVERED_CODE_TEST_PLAN.md](UNDER_COVERED_CODE_TEST_PLAN.md)** - Focused plan for under-covered modules (273 tests, +52.9% coverage)

---

## Progress Tracking

Current implementation status and next steps:

### Completed ✅
- Phase 1: Critical Path Modules (171 tests, 91.6% avg coverage)
  - process_manager.py: 98.19%
  - dependency_manager.py: 100%
  - system_utils.py: 92.91%
  - version_manager.py: 100%

### In Progress 🟡
- Phase 2: Core Integration
  - github_integration.py: 37.24% (29 tests, need +11)
  - metadata_manager.py: 28.85% (need 25 tests)

### Next Up 🔴
- Phase 3: API Layer (need ~150 tests)
  - core.py, main.py, shortcut_manager, metadata_manager

### Goal 🎯
**500+ tests, 80%+ coverage**

---

## Resources

### Documentation
- [pytest Documentation](https://docs.pytest.org/)
- [pytest-cov Documentation](https://pytest-cov.readthedocs.io/)
- [pytest-mock Documentation](https://pytest-mock.readthedocs.io/)

### Test Dependencies
```bash
pip install pytest pytest-cov pytest-mock pytest-timeout freezegun
```

---

**Last Updated:** 2025-12-30
**Status:** 219 tests, 27.10% coverage, 🟢 On Track
