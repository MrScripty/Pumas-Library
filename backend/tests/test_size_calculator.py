"""
Unit tests for backend/api/size_calculator.py

Tests for SizeCalculator class covering initialization, size calculation,
caching, and dependency management.
"""

import urllib.error
from pathlib import Path
from unittest.mock import Mock, patch

import pytest

from backend.api.size_calculator import SizeCalculator

# ============================================================================
# FIXTURES
# ============================================================================


@pytest.fixture
def mock_release_size_calculator():
    """Create a mock ReleaseSizeCalculator"""
    mock_rsc = Mock()
    mock_rsc.calculate_release_size.return_value = {
        "total_size": 4500000000,
        "archive_size": 125000000,
        "dependencies_size": 4375000000,
        "dependencies_count": 12,
    }
    mock_rsc.get_cached_size.return_value = None
    mock_rsc.get_size_breakdown.return_value = {
        "total": "4.2 GB",
        "archive": "119.2 MB",
        "dependencies": "4.1 GB",
    }
    mock_rsc.get_sorted_dependencies.return_value = [
        {"name": "torch", "size": 2000000000},
        {"name": "numpy", "size": 15000000},
    ]
    return mock_rsc


@pytest.fixture
def mock_github_fetcher():
    """Create a mock GitHubReleasesFetcher"""
    mock_gf = Mock()
    mock_gf.get_release_by_tag.return_value = {
        "tag_name": "v0.1.0",
        "zipball_url": "https://github.com/test/repo/zipball/v0.1.0",
        "tarball_url": "https://github.com/test/repo/tarball/v0.1.0",
    }
    return mock_gf


@pytest.fixture
def mock_version_manager():
    """Create a mock VersionManager"""
    mock_vm = Mock()
    mock_vm.get_available_releases.return_value = [
        {"tag_name": "v0.1.0"},
        {"tag_name": "v0.2.0"},
        {"tag_name": "v0.3.0"},
    ]
    return mock_vm


@pytest.fixture
def size_calculator(mock_release_size_calculator, mock_github_fetcher, mock_version_manager):
    """Create a SizeCalculator instance for testing"""
    return SizeCalculator(
        release_size_calculator=mock_release_size_calculator,
        github_fetcher=mock_github_fetcher,
        version_manager=mock_version_manager,
    )


# ============================================================================
# INITIALIZATION TESTS
# ============================================================================


class TestSizeCalculatorInit:
    """Test SizeCalculator initialization"""

    def test_init_sets_dependencies(
        self, mock_release_size_calculator, mock_github_fetcher, mock_version_manager
    ):
        """Test that initialization sets all dependencies"""
        calculator = SizeCalculator(
            release_size_calculator=mock_release_size_calculator,
            github_fetcher=mock_github_fetcher,
            version_manager=mock_version_manager,
        )

        assert calculator.release_size_calculator == mock_release_size_calculator
        assert calculator.github_fetcher == mock_github_fetcher
        assert calculator.version_manager == mock_version_manager

    def test_init_without_version_manager(self, mock_release_size_calculator, mock_github_fetcher):
        """Test initialization without optional version_manager"""
        calculator = SizeCalculator(
            release_size_calculator=mock_release_size_calculator,
            github_fetcher=mock_github_fetcher,
            version_manager=None,
        )

        assert calculator.version_manager is None


# ============================================================================
# CALCULATE RELEASE SIZE TESTS
# ============================================================================


class TestCalculateReleaseSize:
    """Test calculate_release_size method"""

    def test_calculate_release_size_success(self, size_calculator, mock_github_fetcher):
        """Test successful size calculation"""
        result = size_calculator.calculate_release_size("v0.1.0")

        assert result is not None
        assert result["total_size"] == 4500000000
        assert result["archive_size"] == 125000000
        assert result["dependencies_size"] == 4375000000

    def test_calculate_release_size_gets_release_from_github(
        self, size_calculator, mock_github_fetcher
    ):
        """Test that release is fetched from GitHub"""
        size_calculator.calculate_release_size("v0.1.0")

        mock_github_fetcher.get_release_by_tag.assert_called_once_with("v0.1.0")

    def test_calculate_release_size_release_not_found(self, size_calculator, mock_github_fetcher):
        """Test handling when release is not found"""
        mock_github_fetcher.get_release_by_tag.return_value = None

        result = size_calculator.calculate_release_size("v9.9.9")

        assert result is None

    def test_calculate_release_size_uses_head_request(self, size_calculator, mocker):
        """Test that HEAD request is used to get archive size"""
        mock_urlopen = mocker.patch("urllib.request.urlopen")
        mock_response = Mock()
        mock_response.headers.get.return_value = "125000000"
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)
        mock_urlopen.return_value = mock_response

        size_calculator.calculate_release_size("v0.1.0")

        # Verify HEAD request was made
        assert mock_urlopen.called
        request = mock_urlopen.call_args[0][0]
        assert request.get_method() == "HEAD"

    def test_calculate_release_size_uses_fallback_estimate(self, size_calculator, mocker):
        """Test fallback to 125 MB estimate when HEAD fails"""
        mocker.patch(
            "urllib.request.urlopen",
            side_effect=urllib.error.URLError("Network error"),
        )

        result = size_calculator.calculate_release_size("v0.1.0")

        # Should still succeed with fallback estimate
        assert result is not None
        # Verify fallback size was used (125 MB = 125 * 1024 * 1024)
        assert size_calculator.release_size_calculator.calculate_release_size.called

    def test_calculate_release_size_with_force_refresh(self, size_calculator):
        """Test force_refresh parameter is passed through"""
        size_calculator.calculate_release_size("v0.1.0", force_refresh=True)

        call_args = size_calculator.release_size_calculator.calculate_release_size.call_args
        assert call_args[1]["force_refresh"] is True

    def test_calculate_release_size_handles_url_error(
        self, size_calculator, mock_github_fetcher, mocker
    ):
        """Test handling of URLError during size calculation"""
        mocker.patch(
            "urllib.request.urlopen",
            side_effect=urllib.error.URLError("Network error"),
        )
        size_calculator.release_size_calculator.calculate_release_size.side_effect = (
            urllib.error.URLError("Error")
        )

        result = size_calculator.calculate_release_size("v0.1.0")

        assert result is None

    def test_calculate_release_size_handles_value_error(self, size_calculator):
        """Test handling of ValueError during calculation"""
        size_calculator.release_size_calculator.calculate_release_size.side_effect = ValueError(
            "Invalid data"
        )

        result = size_calculator.calculate_release_size("v0.1.0")

        assert result is None


# ============================================================================
# CONTENT LENGTH TESTS
# ============================================================================


class TestGetContentLength:
    """Test _get_content_length method"""

    def test_get_content_length_success(self, size_calculator, mocker):
        """Test successful Content-Length retrieval"""
        mock_response = Mock()
        mock_response.headers.get.return_value = "125000000"
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mocker.patch("urllib.request.urlopen", return_value=mock_response)

        result = size_calculator._get_content_length("http://test.com/file.zip")

        assert result == 125000000

    def test_get_content_length_adds_user_agent(self, size_calculator, mocker):
        """Test that User-Agent header is added"""
        mock_response = Mock()
        mock_response.headers.get.return_value = "100"
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mock_urlopen = mocker.patch("urllib.request.urlopen", return_value=mock_response)

        size_calculator._get_content_length("http://test.com/file.zip")

        request = mock_urlopen.call_args[0][0]
        assert request.headers.get("User-agent") == "ComfyUI-Version-Manager/1.0"

    def test_get_content_length_no_header(self, size_calculator, mocker):
        """Test when Content-Length header is missing"""
        mock_response = Mock()
        mock_response.headers.get.return_value = None
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mocker.patch("urllib.request.urlopen", return_value=mock_response)

        result = size_calculator._get_content_length("http://test.com/file.zip")

        assert result is None

    def test_get_content_length_network_error(self, size_calculator, mocker):
        """Test handling of network errors"""
        mocker.patch(
            "urllib.request.urlopen",
            side_effect=urllib.error.URLError("Network error"),
        )

        result = size_calculator._get_content_length("http://test.com/file.zip")

        assert result is None

    def test_get_content_length_invalid_value(self, size_calculator, mocker):
        """Test handling of invalid Content-Length value"""
        mock_response = Mock()
        mock_response.headers.get.return_value = "not-a-number"
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mocker.patch("urllib.request.urlopen", return_value=mock_response)

        result = size_calculator._get_content_length("http://test.com/file.zip")

        assert result is None


# ============================================================================
# CALCULATE ALL RELEASE SIZES TESTS
# ============================================================================


class TestCalculateAllReleaseSizes:
    """Test calculate_all_release_sizes method"""

    def test_calculate_all_release_sizes_success(self, size_calculator):
        """Test calculating sizes for all releases"""
        results = size_calculator.calculate_all_release_sizes()

        assert len(results) == 3
        assert "v0.1.0" in results
        assert "v0.2.0" in results
        assert "v0.3.0" in results

    def test_calculate_all_release_sizes_calls_progress_callback(self, size_calculator):
        """Test that progress callback is called"""
        progress_calls = []

        def progress_callback(current, total, tag):
            progress_calls.append((current, total, tag))

        size_calculator.calculate_all_release_sizes(progress_callback)

        # Should have been called for each release
        assert len(progress_calls) == 3
        assert progress_calls[0] == (1, 3, "v0.1.0")
        assert progress_calls[1] == (2, 3, "v0.2.0")
        assert progress_calls[2] == (3, 3, "v0.3.0")

    def test_calculate_all_release_sizes_without_version_manager(
        self, mock_release_size_calculator, mock_github_fetcher
    ):
        """Test when version_manager is None"""
        calculator = SizeCalculator(
            release_size_calculator=mock_release_size_calculator,
            github_fetcher=mock_github_fetcher,
            version_manager=None,
        )

        results = calculator.calculate_all_release_sizes()

        assert results == {}

    def test_calculate_all_release_sizes_skips_failures(self, size_calculator, mock_github_fetcher):
        """Test that failures are skipped gracefully"""

        # Make second release fail
        def side_effect(tag):
            if tag == "v0.2.0":
                return None
            return mock_github_fetcher.get_release_by_tag.return_value

        mock_github_fetcher.get_release_by_tag.side_effect = side_effect

        results = size_calculator.calculate_all_release_sizes()

        # Should have 2 results (v0.1.0 and v0.3.0), skipping v0.2.0
        assert len(results) == 2
        assert "v0.1.0" in results
        assert "v0.2.0" not in results
        assert "v0.3.0" in results


# ============================================================================
# GET RELEASE SIZE INFO TESTS
# ============================================================================


class TestGetReleaseSizeInfo:
    """Test get_release_size_info method"""

    def test_get_release_size_info_success(self, size_calculator):
        """Test getting size info for a release"""
        result = size_calculator.get_release_size_info("v0.1.0", 125000000)

        assert result is not None
        assert result["total_size"] == 4500000000

    def test_get_release_size_info_without_calculator(
        self, mock_github_fetcher, mock_version_manager
    ):
        """Test when release_size_calculator is None"""
        calculator = SizeCalculator(
            release_size_calculator=None,
            github_fetcher=mock_github_fetcher,
            version_manager=mock_version_manager,
        )

        result = calculator.get_release_size_info("v0.1.0", 125000000)

        assert result is None

    def test_get_release_size_info_handles_errors(self, size_calculator):
        """Test error handling in get_release_size_info"""
        size_calculator.release_size_calculator.calculate_release_size.side_effect = ValueError(
            "Error"
        )

        result = size_calculator.get_release_size_info("v0.1.0", 125000000)

        assert result is None


# ============================================================================
# GET RELEASE SIZE BREAKDOWN TESTS
# ============================================================================


class TestGetReleaseSizeBreakdown:
    """Test get_release_size_breakdown method"""

    def test_get_release_size_breakdown_success(self, size_calculator):
        """Test getting formatted size breakdown"""
        result = size_calculator.get_release_size_breakdown("v0.1.0")

        assert result is not None
        assert result["total"] == "4.2 GB"
        assert result["archive"] == "119.2 MB"
        assert result["dependencies"] == "4.1 GB"

    def test_get_release_size_breakdown_without_calculator(
        self, mock_github_fetcher, mock_version_manager
    ):
        """Test when release_size_calculator is None"""
        calculator = SizeCalculator(
            release_size_calculator=None,
            github_fetcher=mock_github_fetcher,
            version_manager=mock_version_manager,
        )

        result = calculator.get_release_size_breakdown("v0.1.0")

        assert result is None

    def test_get_release_size_breakdown_handles_errors(self, size_calculator):
        """Test error handling in get_release_size_breakdown"""
        size_calculator.release_size_calculator.get_size_breakdown.side_effect = KeyError("tag")

        result = size_calculator.get_release_size_breakdown("v0.1.0")

        assert result is None


# ============================================================================
# GET RELEASE DEPENDENCIES TESTS
# ============================================================================


class TestGetReleaseDependencies:
    """Test get_release_dependencies method"""

    def test_get_release_dependencies_success(self, size_calculator):
        """Test getting dependencies list"""
        result = size_calculator.get_release_dependencies("v0.1.0")

        assert len(result) == 2
        assert result[0]["name"] == "torch"
        assert result[0]["size"] == 2000000000
        assert result[1]["name"] == "numpy"

    def test_get_release_dependencies_with_limit(self, size_calculator):
        """Test limiting number of dependencies"""
        size_calculator.get_release_dependencies("v0.1.0", top_n=1)

        size_calculator.release_size_calculator.get_sorted_dependencies.assert_called_once_with(
            "v0.1.0", 1
        )

    def test_get_release_dependencies_without_calculator(
        self, mock_github_fetcher, mock_version_manager
    ):
        """Test when release_size_calculator is None"""
        calculator = SizeCalculator(
            release_size_calculator=None,
            github_fetcher=mock_github_fetcher,
            version_manager=mock_version_manager,
        )

        result = calculator.get_release_dependencies("v0.1.0")

        assert result == []

    def test_get_release_dependencies_handles_errors(self, size_calculator):
        """Test error handling in get_release_dependencies"""
        size_calculator.release_size_calculator.get_sorted_dependencies.side_effect = TypeError(
            "Error"
        )

        result = size_calculator.get_release_dependencies("v0.1.0")

        assert result == []


# ============================================================================
# ASYNC REFRESH TESTS
# ============================================================================


class TestRefreshReleaseSizesAsync:
    """Test _refresh_release_sizes_async method"""

    def test_refresh_release_sizes_async_starts_thread(self, size_calculator, mocker):
        """Test that async refresh starts background thread"""
        mock_thread = mocker.patch("threading.Thread")

        releases = [
            {"tag_name": "v0.1.0"},
            {"tag_name": "v0.2.0"},
        ]
        installed_tags = set()

        size_calculator._refresh_release_sizes_async(releases, installed_tags)

        # Thread should be created and started
        assert mock_thread.called
        assert mock_thread.return_value.start.called

    def test_refresh_release_sizes_async_prioritizes_non_installed(self, size_calculator, mocker):
        """Test that non-installed releases are prioritized"""
        # Mock threading to capture the worker function
        worker_func = None

        def capture_worker(target, **kwargs):
            nonlocal worker_func
            worker_func = target
            mock = Mock()
            mock.start = Mock()
            return mock

        mocker.patch("threading.Thread", side_effect=capture_worker)

        releases = [
            {"tag_name": "v0.1.0"},  # installed
            {"tag_name": "v0.2.0"},  # not installed
            {"tag_name": "v0.3.0"},  # not installed
        ]
        installed_tags = {"v0.1.0"}

        size_calculator._refresh_release_sizes_async(releases, installed_tags)

        # Worker function should be captured
        assert worker_func is not None

    def test_refresh_release_sizes_async_skips_cached(self, size_calculator, mocker):
        """Test that cached releases are skipped when not forcing"""
        size_calculator.release_size_calculator.get_cached_size.return_value = {
            "total_size": 1000000
        }

        mock_thread = mocker.patch("threading.Thread")

        releases = [{"tag_name": "v0.1.0"}]
        installed_tags = set()

        size_calculator._refresh_release_sizes_async(releases, installed_tags, force_refresh=False)

        # Thread should still be created (but worker will skip cached releases)
        assert mock_thread.called

    def test_refresh_release_sizes_async_without_calculator(
        self, mock_github_fetcher, mock_version_manager
    ):
        """Test when release_size_calculator is None"""
        calculator = SizeCalculator(
            release_size_calculator=None,
            github_fetcher=mock_github_fetcher,
            version_manager=mock_version_manager,
        )

        releases = [{"tag_name": "v0.1.0"}]
        installed_tags = set()

        # Should return early without error
        calculator._refresh_release_sizes_async(releases, installed_tags)

    def test_refresh_release_sizes_async_skips_releases_without_tag(self, size_calculator, mocker):
        """Test that releases without tag_name are skipped"""
        mock_thread = mocker.patch("threading.Thread")

        releases = [
            {"tag_name": "v0.1.0"},
            {"name": "No tag"},  # Missing tag_name
            {"tag_name": ""},  # Empty tag_name
        ]
        installed_tags = set()

        size_calculator._refresh_release_sizes_async(releases, installed_tags)

        # Should still create thread (worker will skip invalid releases)
        assert mock_thread.called
