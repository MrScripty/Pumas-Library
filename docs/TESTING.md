# Testing Guide for ComfyUI Setup Launcher

This document describes the testing infrastructure and how to write and run tests for the launcher.

## Table of Contents

- [Quick Start](#quick-start)
- [Test Structure](#test-structure)
- [Running Tests](#running-tests)
- [Writing Tests](#writing-tests)
- [Coverage Requirements](#coverage-requirements)
- [Best Practices](#best-practices)

---

## Quick Start

### Installation

Install development dependencies including pytest:

```bash
pip install -r requirements-dev.txt
```

### Running All Tests

```bash
# Run all tests with coverage
pytest

# Run with verbose output
pytest -v

# Run a specific test file
pytest tests/unit/test_metadata_manager.py

# Run a specific test function
pytest tests/unit/test_metadata_manager.py::test_save_metadata
```

### Check Coverage

```bash
# Generate coverage report
pytest --cov=backend --cov-report=term-missing

# Generate HTML coverage report
pytest --cov=backend --cov-report=html
# Then open htmlcov/index.html in a browser
```

---

## Test Structure

The test suite is organized as follows:

```
tests/
├── conftest.py              # Shared fixtures and pytest configuration
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

### Unit Tests

- Located in `tests/unit/`
- Test individual functions/methods in isolation
- Mock external dependencies (network, filesystem when appropriate)
- Should run in milliseconds
- Use the `@pytest.mark.unit` marker

### Integration Tests

- Located in `tests/integration/`
- Test multiple components working together
- Use real file I/O with temporary directories
- May take longer to run
- Use the `@pytest.mark.integration` marker

---

## Running Tests

### Run All Tests

```bash
pytest
```

### Run Only Unit Tests

```bash
pytest -m unit
```

### Run Only Integration Tests

```bash
pytest -m integration
```

### Run Tests in Parallel

```bash
pytest -n auto  # Uses all available CPU cores
```

### Run with Debugging

```bash
# Show print() statements
pytest -s

# Drop into debugger on failure
pytest --pdb

# Show local variables in traceback
pytest --showlocals
```

### Stop on First Failure

```bash
pytest -x
```

### Run Last Failed Tests

```bash
pytest --lf
```

---

## Writing Tests

### Basic Test Structure

```python
import pytest
from backend.metadata_manager import MetadataManager


@pytest.mark.unit
def test_metadata_creation(temp_metadata_dir):
    """Test that metadata manager creates required files."""
    manager = MetadataManager(temp_metadata_dir)

    # Assertions
    assert manager.metadata_file.exists()
    assert manager.get_active_version() is None
```

### Using Fixtures

Fixtures are reusable test components defined in [conftest.py](tests/conftest.py:1). Use them by adding parameters to your test functions:

```python
@pytest.mark.unit
def test_save_version(metadata_manager):
    """Test saving version metadata."""
    # metadata_manager fixture provides a ready-to-use MetadataManager
    metadata_manager.set_active_version("v0.5.0")
    assert metadata_manager.get_active_version() == "v0.5.0"
```

### Available Fixtures

- `temp_launcher_root`: Temporary launcher root directory
- `temp_metadata_dir`: Temporary metadata directory
- `temp_versions_dir`: Temporary versions directory
- `metadata_manager`: MetadataManager instance with temp storage
- `github_fetcher`: GitHubReleasesFetcher instance
- `sample_releases`: List of mock GitHub release data
- `sample_version_metadata`: Sample version metadata dictionary

See [conftest.py](tests/conftest.py:1) for full fixture documentation.

### Mocking External Dependencies

Use `pytest-mock` for mocking:

```python
@pytest.mark.unit
def test_github_fetch_with_mock(github_fetcher, mocker):
    """Test GitHub API fetch with mocked response."""
    # Mock the HTTP request
    mock_response = mocker.Mock()
    mock_response.json.return_value = [
        {"tag_name": "v0.5.0", "prerelease": False}
    ]
    mocker.patch('urllib.request.urlopen', return_value=mock_response)

    releases = github_fetcher.fetch_releases()
    assert len(releases) == 1
```

Use `responses` library for HTTP mocking:

```python
import responses

@pytest.mark.unit
@responses.activate
def test_download_file(github_fetcher):
    """Test file download with mocked HTTP."""
    responses.add(
        responses.GET,
        'https://example.com/file.zip',
        body=b'fake zip content',
        status=200
    )

    result = github_fetcher.download_file('https://example.com/file.zip', '/tmp/test.zip')
    assert result is True
```

### Testing Exceptions

```python
@pytest.mark.unit
def test_invalid_version_tag():
    """Test that invalid version tags raise ValidationError."""
    from backend.validators import validate_version_tag

    with pytest.raises(ValidationError, match="Invalid version tag"):
        validate_version_tag("../etc/passwd")
```

### Parametrized Tests

Test the same logic with multiple inputs:

```python
@pytest.mark.unit
@pytest.mark.parametrize("tag,expected", [
    ("v0.5.0", True),
    ("v1.2.3-rc1", True),
    ("invalid", False),
    ("../etc/passwd", False),
])
def test_version_validation(tag, expected):
    """Test version tag validation with various inputs."""
    from backend.validators import validate_version_tag
    assert validate_version_tag(tag) == expected
```

---

## Coverage Requirements

### Target Coverage

- **Overall**: 80% coverage across the backend
- **Critical modules**: 90%+ coverage
  - `metadata_manager.py`
  - `version_manager.py`
  - `validators.py`
  - `file_utils.py`

### Excluded from Coverage

The following are automatically excluded (see [.coveragerc](.coveragerc:1)):

- Test files themselves
- Configuration files
- Main entry points (tested via integration tests)
- Virtual environments
- Debug code (`def __repr__`, `def __str__`)
- Abstract methods
- Type checking blocks

### Viewing Coverage Report

```bash
# Terminal report
pytest --cov=backend --cov-report=term-missing

# HTML report (open htmlcov/index.html)
pytest --cov=backend --cov-report=html

# Generate both
pytest --cov=backend --cov-report=term-missing --cov-report=html
```

### Coverage Fails Below 80%

The test suite is configured to fail if coverage drops below 80%:

```bash
# This will exit with error code if coverage < 80%
pytest
```

---

## Best Practices

### 1. Test What You Touch

When modifying code, write tests for:
- New functions/methods you create
- Functions you modify
- Bug fixes (add regression test)

### 2. Keep Tests Fast

- Mock network calls
- Use temporary directories for file I/O
- Clean up resources in fixtures

### 3. Use Descriptive Names

```python
# Good
def test_metadata_manager_creates_missing_directory():
    """Test that MetadataManager creates metadata dir if missing."""
    pass

# Bad
def test_metadata():
    pass
```

### 4. One Assertion Per Test (Guideline)

While not a strict rule, prefer focused tests:

```python
# Good
def test_save_metadata_creates_file(metadata_manager):
    metadata_manager.save()
    assert metadata_manager.metadata_file.exists()

def test_save_metadata_writes_json(metadata_manager):
    metadata_manager.set_active_version("v0.5.0")
    metadata_manager.save()
    data = json.loads(metadata_manager.metadata_file.read_text())
    assert data["active_version"] == "v0.5.0"
```

### 5. Use Markers

Tag tests appropriately:

```python
@pytest.mark.unit  # Fast, isolated test
@pytest.mark.integration  # Integration test
@pytest.mark.slow  # Takes >1 second
@pytest.mark.network  # Requires network (should mock)
```

### 6. Clean Up Resources

Use fixtures with `yield` for cleanup:

```python
@pytest.fixture
def temp_database():
    db = create_database()
    yield db
    db.close()  # Cleanup happens after test
```

### 7. Test Edge Cases

Don't just test the happy path:

```python
def test_version_validation_edge_cases():
    # Empty string
    assert not validate_version_tag("")
    # Very long string
    assert not validate_version_tag("v" * 10000)
    # Unicode characters
    assert not validate_version_tag("v0.5.0\u0000")
```

### 8. Avoid Test Interdependence

Each test should be independent and not rely on other tests:

```python
# Bad - tests depend on order
def test_step1():
    global data
    data = setup()

def test_step2():
    assert data is not None  # Fails if test_step1 didn't run

# Good - each test is self-contained
def test_operation_a():
    data = setup()
    assert data is not None

def test_operation_b():
    data = setup()
    assert data.process() == expected
```

---

## Continuous Integration

Tests are run automatically on every commit (future: via GitHub Actions).

To ensure your changes pass CI:

```bash
# Run the full test suite locally
pytest

# Check that coverage meets requirements
pytest --cov=backend --cov-fail-under=80

# Run all pre-commit checks
pre-commit run --all-files
```

---

## Troubleshooting

### Tests Fail with Import Errors

Make sure you've installed dev dependencies:

```bash
pip install -r requirements-dev.txt
```

### Coverage Report Shows Missing Files

Ensure you're running pytest from the project root:

```bash
cd /path/to/Linux-ComfyUI-Launcher
pytest
```

### Tests Hang or Run Slowly

- Check for network calls that should be mocked
- Use `pytest -v` to see which test is hanging
- Set a timeout: `pytest --timeout=10`

### Fixtures Not Found

Ensure `conftest.py` is in the `tests/` directory and your test file is within the `tests/` tree.

---

## Additional Resources

- [pytest documentation](https://docs.pytest.org/)
- [pytest-cov documentation](https://pytest-cov.readthedocs.io/)
- [pytest-mock documentation](https://pytest-mock.readthedocs.io/)
- [Python testing best practices](https://docs.python-guide.org/writing/tests/)

---

## Questions?

For questions or issues with the test suite, please open an issue on GitHub.
