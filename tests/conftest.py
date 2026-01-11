"""
Pytest configuration and shared fixtures for ComfyUI Launcher tests.

This file is automatically loaded by pytest and provides reusable fixtures
for all tests in the test suite.
"""

import tempfile
from pathlib import Path
from typing import Generator

import pytest

# ==================== Path Fixtures ====================


@pytest.fixture
def temp_launcher_root(tmp_path: Path) -> Path:
    """
    Create a temporary launcher root directory for isolated testing.

    This fixture creates a clean temporary directory that mimics the launcher's
    root directory structure. Use this for integration tests that need real
    file I/O without affecting the actual launcher data.

    Args:
        tmp_path: Pytest built-in fixture that provides a temporary directory

    Returns:
        Path to the temporary launcher root directory

    Example:
        def test_metadata_creation(temp_launcher_root):
            metadata_path = temp_launcher_root / "launcher-data"
            metadata_path.mkdir()
            assert metadata_path.exists()
    """
    launcher_root = tmp_path / "launcher-test"
    launcher_root.mkdir(parents=True, exist_ok=True)
    return launcher_root


@pytest.fixture
def temp_metadata_dir(temp_launcher_root: Path) -> Path:
    """
    Create a temporary metadata directory for testing metadata operations.

    Returns:
        Path to the temporary launcher-data directory
    """
    metadata_dir = temp_launcher_root / "launcher-data"
    metadata_dir.mkdir(parents=True, exist_ok=True)
    return metadata_dir


@pytest.fixture
def temp_versions_dir(temp_launcher_root: Path) -> Path:
    """
    Create a temporary versions directory for testing version installations.

    Returns:
        Path to the temporary comfyui-versions directory
    """
    versions_dir = temp_launcher_root / "comfyui-versions"
    versions_dir.mkdir(parents=True, exist_ok=True)
    return versions_dir


# ==================== Manager Fixtures ====================


@pytest.fixture
def metadata_manager(temp_metadata_dir: Path):
    """
    Create a MetadataManager instance with isolated temporary storage.

    This fixture provides a fully functional MetadataManager that writes to
    a temporary directory, preventing test pollution of real metadata files.

    Returns:
        MetadataManager instance configured for testing

    Example:
        def test_save_metadata(metadata_manager):
            metadata_manager.set_active_version("v0.5.0")
            assert metadata_manager.get_active_version() == "v0.5.0"
    """
    from backend.metadata_manager import MetadataManager

    return MetadataManager(temp_metadata_dir)


@pytest.fixture
def github_fetcher(metadata_manager):
    """
    Create a GitHubReleasesFetcher instance for testing.

    Note: This fetcher will make real network requests to GitHub API unless
    mocked. For tests that need to avoid network calls, use responses or
    pytest-mock to mock the HTTP requests.

    Returns:
        GitHubReleasesFetcher instance

    Example:
        def test_fetch_releases(github_fetcher, responses):
            # Mock the GitHub API response
            responses.add(...)
            releases = github_fetcher.fetch_releases()
    """
    from backend.github_integration import GitHubReleasesFetcher

    return GitHubReleasesFetcher(metadata_manager)


# ==================== Sample Data Fixtures ====================


@pytest.fixture
def sample_releases() -> list[dict]:
    """
    Provide sample GitHub release data for testing.

    Returns:
        List of mock GitHub release dictionaries matching the GitHub API format

    Example:
        def test_release_filtering(sample_releases):
            stable_releases = [r for r in sample_releases if not r['prerelease']]
            assert len(stable_releases) == 3
    """
    return [
        {
            "tag_name": "v0.6.0",
            "name": "Release v0.6.0",
            "prerelease": False,
            "published_at": "2024-01-15T10:00:00Z",
            "zipball_url": "https://github.com/comfyanonymous/ComfyUI/archive/refs/tags/v0.6.0.zip",
        },
        {
            "tag_name": "v0.5.1",
            "name": "Release v0.5.1",
            "prerelease": False,
            "published_at": "2024-01-10T10:00:00Z",
            "zipball_url": "https://github.com/comfyanonymous/ComfyUI/archive/refs/tags/v0.5.1.zip",
        },
        {
            "tag_name": "v0.5.0",
            "name": "Release v0.5.0",
            "prerelease": False,
            "published_at": "2024-01-05T10:00:00Z",
            "zipball_url": "https://github.com/comfyanonymous/ComfyUI/archive/refs/tags/v0.5.0.zip",
        },
        {
            "tag_name": "v0.5.0-rc1",
            "name": "Release v0.5.0-rc1",
            "prerelease": True,
            "published_at": "2024-01-01T10:00:00Z",
            "zipball_url": "https://github.com/comfyanonymous/ComfyUI/archive/refs/tags/v0.5.0-rc1.zip",
        },
    ]


@pytest.fixture
def sample_version_metadata() -> dict:
    """
    Provide sample version metadata for testing.

    Returns:
        Dictionary matching the metadata format stored in launcher-data/metadata.json
    """
    return {
        "v0.5.1": {
            "path": "comfyui-versions/v0.5.1",
            "installed_date": "2024-01-15T12:30:00",
            "python_version": "3.12.0",
            "release_tag": "v0.5.1",
        },
        "v0.6.0": {
            "path": "comfyui-versions/v0.6.0",
            "installed_date": "2024-01-16T09:00:00",
            "python_version": "3.12.0",
            "release_tag": "v0.6.0",
        },
    }


# ==================== Markers ====================


def pytest_configure(config):
    """
    Register custom pytest markers.

    This is called by pytest during initialization and allows us to define
    custom markers that can be used to categorize tests.
    """
    config.addinivalue_line("markers", "unit: Unit tests with mocked external dependencies")
    config.addinivalue_line("markers", "integration: Integration tests with real file I/O")
    config.addinivalue_line("markers", "slow: Tests that take longer than 1 second")
    config.addinivalue_line(
        "markers", "network: Tests that require network access (should be mocked)"
    )


# ==================== Cache Fixtures ====================


@pytest.fixture(autouse=True)
def clear_hf_metadata_cache():
    """
    Clear the HuggingFace metadata cache before each test.

    This ensures tests don't pollute each other with cached search results.
    The fixture runs automatically (autouse=True) for all tests.
    """
    from backend.model_library.hf.cache import hf_metadata_cache

    hf_metadata_cache.clear()
    yield
    hf_metadata_cache.clear()


# ==================== Test Hooks ====================


@pytest.hookimpl(tryfirst=True, hookwrapper=True)
def pytest_runtest_makereport(item, call):
    """
    Make test results available to fixtures for cleanup or logging.

    This hook allows fixtures to access whether a test passed or failed,
    which can be useful for conditional cleanup or debugging.
    """
    # Execute all other hooks to obtain the report object
    outcome = yield
    rep = outcome.get_result()

    # Set a report attribute for each phase (setup, call, teardown)
    setattr(item, f"rep_{rep.when}", rep)
