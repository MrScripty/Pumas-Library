"""
Unit tests for backend/github_integration.py

Tests for GitHubReleasesFetcher and DownloadManager classes with comprehensive coverage
of caching, error handling, retry logic, and download management.
"""

import json
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path
from unittest.mock import MagicMock, Mock, mock_open, patch

import pytest

from backend.config import NETWORK
from backend.exceptions import CancellationError
from backend.github_integration import DownloadManager, GitHubReleasesFetcher
from backend.models import GitHubRelease

# ============================================================================
# FIXTURES
# ============================================================================


@pytest.fixture
def mock_metadata_manager(mocker):
    """Create a mock MetadataManager for testing"""
    mock_mm = Mock()
    mock_mm.load_github_cache_for_repo.return_value = None
    mock_mm.save_github_cache_for_repo = Mock()
    return mock_mm


@pytest.fixture
def sample_github_release() -> GitHubRelease:
    """Create a sample GitHub release for testing"""
    return {
        "tag_name": "v0.1.0",
        "name": "ComfyUI v0.1.0",
        "published_at": "2024-01-01T00:00:00Z",
        "body": "Initial release",
        "assets": [
            {
                "name": "source.zip",
                "browser_download_url": "https://github.com/test/repo/releases/download/v0.1.0/source.zip",
                "size": 1024000,
            }
        ],
        "zipball_url": "https://api.github.com/repos/test/repo/zipball/v0.1.0",
        "tarball_url": "https://api.github.com/repos/test/repo/tarball/v0.1.0",
    }


@pytest.fixture
def sample_releases_list(sample_github_release) -> list:
    """Create a sample list of GitHub releases"""
    return [
        sample_github_release,
        {
            "tag_name": "v0.2.0",
            "name": "ComfyUI v0.2.0",
            "published_at": "2024-02-01T00:00:00Z",
            "body": "Second release",
            "assets": [],
            "zipball_url": "https://api.github.com/repos/test/repo/zipball/v0.2.0",
            "tarball_url": "https://api.github.com/repos/test/repo/tarball/v0.2.0",
        },
    ]


# ============================================================================
# GitHubReleasesFetcher TESTS
# ============================================================================


class TestGitHubReleasesFetcherInit:
    """Test GitHubReleasesFetcher initialization"""

    def test_init_sets_metadata_manager_and_ttl(self, mock_metadata_manager):
        """Test that initialization sets metadata_manager and ttl"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager, ttl=300)
        assert fetcher.metadata_manager == mock_metadata_manager
        assert fetcher.ttl == 300

    def test_init_sets_default_ttl(self, mock_metadata_manager):
        """Test that initialization uses default TTL from config"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)
        # Should use NETWORK.GITHUB_RELEASES_TTL_SEC (default config value)
        assert fetcher.ttl > 0

    def test_init_creates_memory_cache(self, mock_metadata_manager):
        """Test that initialization creates in-memory cache"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager, ttl=300)
        assert fetcher._memory_cache is not None
        assert fetcher._memory_cache.maxsize == 1
        assert fetcher._memory_cache.ttl == 300

    def test_init_creates_cache_lock(self, mock_metadata_manager):
        """Test that initialization creates thread lock"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)
        # Check that lock exists and has lock/unlock methods
        assert hasattr(fetcher._cache_lock, "acquire")
        assert hasattr(fetcher._cache_lock, "release")


class TestGitHubReleasesFetcherParsing:
    """Test release list parsing"""

    def test_parse_release_list_with_valid_data(self, mock_metadata_manager, sample_releases_list):
        """Test parsing valid release list"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)
        result = fetcher._parse_release_list(sample_releases_list)
        assert len(result) == 2
        assert result[0]["tag_name"] == "v0.1.0"
        assert result[1]["tag_name"] == "v0.2.0"

    def test_parse_release_list_with_non_list(self, mock_metadata_manager):
        """Test parsing non-list data returns empty list"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)
        result = fetcher._parse_release_list("not a list")
        assert result == []

    def test_parse_release_list_with_empty_list(self, mock_metadata_manager):
        """Test parsing empty list"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)
        result = fetcher._parse_release_list([])
        assert result == []

    def test_parse_release_list_filters_non_dict_items(self, mock_metadata_manager):
        """Test that non-dict items are filtered out"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)
        data = [{"tag_name": "v0.1.0"}, "invalid", 123, None, {"tag_name": "v0.2.0"}]
        result = fetcher._parse_release_list(data)
        assert len(result) == 2
        assert result[0]["tag_name"] == "v0.1.0"
        assert result[1]["tag_name"] == "v0.2.0"


class TestGitHubReleasesFetcherCaching:
    """Test caching functionality"""

    def test_get_releases_uses_memory_cache_when_valid(
        self, mock_metadata_manager, sample_releases_list
    ):
        """Test that get_releases uses in-memory cache when valid"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager, ttl=300)

        # Populate memory cache
        fetcher._memory_cache[fetcher._CACHE_KEY] = sample_releases_list

        # Mock _fetch_from_github to ensure it's not called
        with patch.object(fetcher, "_fetch_from_github") as mock_fetch:
            releases = fetcher.get_releases()

            assert len(releases) == 2
            mock_fetch.assert_not_called()

    def test_get_releases_from_disk_cache(
        self, mock_metadata_manager, sample_releases_list, mocker
    ):
        """Test loading from disk cache when memory cache is empty"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager, ttl=300)

        # Mock disk cache to return valid data
        from backend.models import get_iso_timestamp

        cache_data = {
            "lastFetched": get_iso_timestamp(),
            "ttl": 3600,
            "releases": sample_releases_list,
        }

        mocker.patch.object(
            fetcher.metadata_manager, "load_github_cache_for_repo", return_value=cache_data
        )

        # Mock _fetch_from_github to ensure it's not called
        with patch.object(fetcher, "_fetch_from_github") as mock_fetch:
            releases = fetcher.get_releases()

            # Should use disk cache
            assert len(releases) == 2
            mock_fetch.assert_not_called()

    def test_is_cache_valid_with_none(self, mock_metadata_manager):
        """Test that None cache is invalid"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)
        assert fetcher._is_cache_valid(None) is False

    def test_is_cache_valid_with_missing_fields(self, mock_metadata_manager):
        """Test that cache with missing fields is invalid"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)
        cache = {"releases": []}  # Missing lastFetched
        assert fetcher._is_cache_valid(cache) is False

    def test_is_cache_valid_with_expired_cache(self, mock_metadata_manager, mocker):
        """Test that expired cache is invalid"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager, ttl=10)

        # Mock timestamps to simulate expired cache
        from backend.models import get_iso_timestamp

        old_timestamp = "2024-01-01T00:00:00Z"
        current_timestamp = "2024-01-01T01:00:00Z"  # 1 hour later

        cache = {"lastFetched": old_timestamp, "ttl": 10, "releases": []}

        mocker.patch(
            "backend.github_integration.get_iso_timestamp",
            return_value=current_timestamp,
        )

        assert fetcher._is_cache_valid(cache) is False

    def test_is_cache_valid_with_valid_cache(self, mock_metadata_manager, mocker):
        """Test that valid cache is recognized"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager, ttl=3600)

        from backend.models import get_iso_timestamp

        current_timestamp = "2024-01-01T01:00:00Z"

        cache = {"lastFetched": current_timestamp, "ttl": 3600, "releases": []}

        mocker.patch(
            "backend.github_integration.get_iso_timestamp",
            return_value=current_timestamp,
        )

        assert fetcher._is_cache_valid(cache) is True


class TestGitHubReleasesFetcherNetworkFetch:
    """Test network fetching with retries"""

    def test_fetch_page_success_first_try(
        self, mock_metadata_manager, mocker, sample_releases_list
    ):
        """Test successful fetch on first attempt"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)

        # Mock urlopen to return sample data
        mock_response = Mock()
        mock_response.read.return_value = json.dumps(sample_releases_list).encode("utf-8")
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mocker.patch("urllib.request.urlopen", return_value=mock_response)

        result = fetcher._fetch_page(1)

        assert len(result) == 2
        assert result[0]["tag_name"] == "v0.1.0"

    def test_fetch_page_retries_on_network_error(
        self, mock_metadata_manager, mocker, sample_releases_list
    ):
        """Test retry logic on network errors"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)

        # First two attempts fail, third succeeds
        mock_response = Mock()
        mock_response.read.return_value = json.dumps(sample_releases_list).encode("utf-8")
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mock_urlopen = mocker.patch("urllib.request.urlopen")
        mock_urlopen.side_effect = [
            urllib.error.URLError("Network error"),
            urllib.error.URLError("Network error"),
            mock_response,
        ]

        # Mock time.sleep to avoid delays
        mocker.patch("time.sleep")

        result = fetcher._fetch_page(1, max_retries=3)

        assert len(result) == 2
        assert mock_urlopen.call_count == 3

    def test_fetch_page_raises_after_max_retries(self, mock_metadata_manager, mocker):
        """Test that URLError is raised after max retries"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)

        mock_urlopen = mocker.patch("urllib.request.urlopen")
        mock_urlopen.side_effect = urllib.error.URLError("Network error")

        mocker.patch("time.sleep")

        with pytest.raises(urllib.error.URLError):
            fetcher._fetch_page(1, max_retries=3)

        assert mock_urlopen.call_count == 3

    def test_fetch_page_no_retry_on_rate_limit(self, mock_metadata_manager, mocker):
        """Test that 403 rate limit error is not retried"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)

        http_error = urllib.error.HTTPError(
            url="http://test", code=403, msg="Rate limit", hdrs={}, fp=None
        )

        mock_urlopen = mocker.patch("urllib.request.urlopen")
        mock_urlopen.side_effect = http_error

        with pytest.raises(urllib.error.HTTPError) as exc_info:
            fetcher._fetch_page(1, max_retries=3)

        assert exc_info.value.code == 403
        assert mock_urlopen.call_count == 1  # No retries

    def test_fetch_page_no_retry_on_client_errors(self, mock_metadata_manager, mocker):
        """Test that 4xx client errors are not retried"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)

        for error_code in [400, 401, 404]:
            http_error = urllib.error.HTTPError(
                url="http://test", code=error_code, msg="Client error", hdrs={}, fp=None
            )

            mock_urlopen = mocker.patch("urllib.request.urlopen")
            mock_urlopen.side_effect = http_error

            with pytest.raises(urllib.error.HTTPError) as exc_info:
                fetcher._fetch_page(1, max_retries=3)

            assert exc_info.value.code == error_code
            assert mock_urlopen.call_count == 1

    def test_fetch_page_retries_on_500_errors(
        self, mock_metadata_manager, mocker, sample_releases_list
    ):
        """Test that 5xx server errors are retried"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)

        http_error = urllib.error.HTTPError(
            url="http://test", code=500, msg="Server error", hdrs={}, fp=None
        )

        mock_response = Mock()
        mock_response.read.return_value = json.dumps(sample_releases_list).encode("utf-8")
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mock_urlopen = mocker.patch("urllib.request.urlopen")
        mock_urlopen.side_effect = [http_error, mock_response]

        mocker.patch("time.sleep")

        result = fetcher._fetch_page(1, max_retries=3)

        assert len(result) == 2
        assert mock_urlopen.call_count == 2


class TestGitHubReleasesFetcherFromGitHub:
    """Test _fetch_from_github method"""

    def test_fetch_from_github_single_page(
        self, mock_metadata_manager, mocker, sample_releases_list
    ):
        """Test fetching when all releases fit in one page"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)
        fetcher.per_page = 100
        fetcher.max_pages = 10

        # Mock _fetch_page to return sample data
        mocker.patch.object(fetcher, "_fetch_page", return_value=sample_releases_list)

        result = fetcher._fetch_from_github()

        assert len(result) == 2
        assert result[0]["tag_name"] == "v0.1.0"

    def test_fetch_from_github_multiple_pages(
        self, mock_metadata_manager, mocker, sample_github_release
    ):
        """Test fetching across multiple pages"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)
        fetcher.per_page = 1
        fetcher.max_pages = 3

        # Create different releases for each page
        page1 = [{"tag_name": "v0.1.0"}]
        page2 = [{"tag_name": "v0.2.0"}]
        page3 = [{"tag_name": "v0.3.0"}]

        mock_fetch = mocker.patch.object(fetcher, "_fetch_page")
        mock_fetch.side_effect = [page1, page2, page3]

        result = fetcher._fetch_from_github()

        assert len(result) == 3
        assert mock_fetch.call_count == 3

    def test_fetch_from_github_stops_on_empty_page(self, mock_metadata_manager, mocker):
        """Test that fetching stops when empty page is returned"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)
        fetcher.per_page = 100
        fetcher.max_pages = 10

        page1 = [{"tag_name": "v0.1.0"}]
        page2 = []  # Empty page

        mock_fetch = mocker.patch.object(fetcher, "_fetch_page")
        mock_fetch.side_effect = [page1, page2]

        result = fetcher._fetch_from_github()

        assert len(result) == 1
        # Should call page 1, then page 2 (which is empty and stops iteration)
        # But the break happens immediately when empty list is detected
        assert mock_fetch.call_count == 1

    def test_fetch_from_github_stops_on_partial_page(self, mock_metadata_manager, mocker):
        """Test that fetching stops when partial page is returned"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)
        fetcher.per_page = 100
        fetcher.max_pages = 10

        # Create a partial page (less than per_page)
        page1 = [{"tag_name": f"v0.{i}.0"} for i in range(50)]

        mock_fetch = mocker.patch.object(fetcher, "_fetch_page")
        mock_fetch.return_value = page1

        result = fetcher._fetch_from_github()

        assert len(result) == 50
        assert mock_fetch.call_count == 1

    def test_fetch_from_github_handles_rate_limit(self, mock_metadata_manager, mocker):
        """Test that rate limit errors are handled properly"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)

        http_error = urllib.error.HTTPError(
            url="http://test", code=403, msg="Rate limit", hdrs={}, fp=None
        )

        mocker.patch.object(fetcher, "_fetch_page", side_effect=http_error)

        with pytest.raises(urllib.error.HTTPError) as exc_info:
            fetcher._fetch_from_github()

        assert exc_info.value.code == 403

    def test_fetch_from_github_handles_network_error(self, mock_metadata_manager, mocker):
        """Test that network errors are handled properly"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager)

        url_error = urllib.error.URLError("Network unreachable")

        mocker.patch.object(fetcher, "_fetch_page", side_effect=url_error)

        with pytest.raises(urllib.error.URLError):
            fetcher._fetch_from_github()


class TestGitHubReleasesFetcherGetReleases:
    """Test get_releases method with full integration"""

    def test_get_releases_force_refresh(self, mock_metadata_manager, mocker, sample_releases_list):
        """Test that force_refresh bypasses cache"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager, ttl=300)

        # Populate memory cache with old data
        fetcher._memory_cache[fetcher._CACHE_KEY] = [{"tag_name": "old"}]

        # Mock fetch to return new data
        mocker.patch.object(fetcher, "_fetch_from_github", return_value=sample_releases_list)
        mocker.patch.object(fetcher.metadata_manager, "save_github_cache_for_repo")

        result = fetcher.get_releases(force_refresh=True)

        # Should use fresh data, not cache
        assert len(result) == 2
        assert result[0]["tag_name"] == "v0.1.0"

    def test_get_releases_saves_to_disk_cache(
        self, mock_metadata_manager, mocker, sample_releases_list
    ):
        """Test that fetched releases are saved to disk cache"""
        fetcher = GitHubReleasesFetcher(mock_metadata_manager, ttl=300)

        mocker.patch.object(fetcher, "_fetch_from_github", return_value=sample_releases_list)
        mock_save = mocker.patch.object(fetcher.metadata_manager, "save_github_cache_for_repo")

        fetcher.get_releases(force_refresh=True)

        # Should save to disk cache
        mock_save.assert_called_once()
        args = mock_save.call_args[0]
        assert len(args[1]["releases"]) == 2  # Saved releases


# ============================================================================
# DownloadManager TESTS
# ============================================================================


class TestDownloadManagerInit:
    """Test DownloadManager initialization"""

    def test_init_creates_instance(self):
        """Test that DownloadManager can be instantiated"""
        manager = DownloadManager()
        assert manager is not None

    def test_init_sets_default_attributes(self):
        """Test that initialization sets default attributes"""
        manager = DownloadManager()
        assert manager.last_progress_time == 0
        assert manager.last_progress_bytes == 0
        assert manager.progress_update_interval == 0.5
        assert manager._cancel_requested is False

    def test_cancel_sets_flag(self):
        """Test that cancel() sets cancellation flag"""
        manager = DownloadManager()
        assert manager._cancel_requested is False
        manager.cancel()
        assert manager._cancel_requested is True


class TestDownloadManagerDownloadFile:
    """Test download_file method"""

    def test_download_file_success(self, tmp_path, mocker):
        """Test successful file download"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"
        test_content = b"Hello, World!"

        # Mock urllib.request.urlopen
        mock_response = Mock()
        mock_response.read.side_effect = [test_content, b""]
        mock_response.headers.get.return_value = str(len(test_content))
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mocker.patch("urllib.request.urlopen", return_value=mock_response)

        result = manager.download_file("http://test.com/file.txt", destination)

        assert result is True
        assert destination.exists()
        assert destination.read_bytes() == test_content

    def test_download_file_with_progress_callback(self, tmp_path, mocker):
        """Test download with progress callback"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"
        test_content = b"A" * 16384  # 16KB

        # Mock response
        mock_response = Mock()
        mock_response.read.side_effect = [test_content[:8192], test_content[8192:], b""]
        mock_response.headers.get.return_value = str(len(test_content))
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mocker.patch("urllib.request.urlopen", return_value=mock_response)

        # Track progress calls
        progress_calls = []

        def progress_callback(downloaded, total, speed):
            progress_calls.append((downloaded, total, speed))

        result = manager.download_file("http://test.com/file.txt", destination, progress_callback)

        assert result is True
        assert len(progress_calls) > 0
        # First call should be initial progress
        assert progress_calls[0] == (0, len(test_content), None)
        # Last call should have full download
        assert progress_calls[-1][0] == len(test_content)

    def test_download_file_calculates_speed(self, tmp_path, mocker):
        """Test that download calculates speed correctly"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"
        test_content = b"A" * 16384

        # Mock response
        mock_response = Mock()
        mock_response.read.side_effect = [test_content, b""]
        mock_response.headers.get.return_value = str(len(test_content))
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mocker.patch("urllib.request.urlopen", return_value=mock_response)

        # Mock time to control speed calculation
        mock_time = mocker.patch("time.time")
        mock_time.side_effect = [0.0, 0.6, 1.0]  # Start, during, end

        progress_calls = []

        def progress_callback(downloaded, total, speed):
            progress_calls.append((downloaded, total, speed))

        manager.download_file("http://test.com/file.txt", destination, progress_callback)

        # Speed should be calculated for at least one callback
        speeds = [call[2] for call in progress_calls if call[2] is not None]
        assert len(speeds) > 0

    def test_download_file_creates_parent_directory(self, tmp_path, mocker):
        """Test that download creates parent directories"""
        manager = DownloadManager()
        destination = tmp_path / "subdir" / "nested" / "test.txt"
        test_content = b"content"

        # Mock response
        mock_response = Mock()
        mock_response.read.side_effect = [test_content, b""]
        mock_response.headers.get.return_value = str(len(test_content))
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mocker.patch("urllib.request.urlopen", return_value=mock_response)

        result = manager.download_file("http://test.com/file.txt", destination)

        assert result is True
        assert destination.parent.exists()
        assert destination.exists()

    def test_download_file_handles_network_error(self, tmp_path, mocker):
        """Test that network errors are handled"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        mocker.patch("urllib.request.urlopen", side_effect=urllib.error.URLError("Network error"))

        result = manager.download_file("http://test.com/file.txt", destination)

        assert result is False
        assert not destination.exists()

    def test_download_file_rejects_invalid_url(self, tmp_path, mocker):
        """Test that invalid URLs are rejected before network calls"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        mock_urlopen = mocker.patch("urllib.request.urlopen")

        result = manager.download_file("not-a-url", destination)

        assert result is False
        assert mock_urlopen.call_count == 0
        assert not destination.exists()

    def test_download_file_handles_cancellation(self, tmp_path, mocker):
        """Test that download can be cancelled"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"
        test_content = b"A" * 32768  # 32KB

        # Mock response that returns data in chunks
        mock_response = Mock()
        chunks = [test_content[i : i + 8192] for i in range(0, len(test_content), 8192)]

        def read_side_effect(size):
            # Cancel after first chunk
            if len(chunks) == 4:
                manager.cancel()
            return chunks.pop(0) if chunks else b""

        mock_response.read.side_effect = read_side_effect
        mock_response.headers.get.return_value = str(len(test_content))
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mocker.patch("urllib.request.urlopen", return_value=mock_response)

        result = manager.download_file("http://test.com/file.txt", destination)

        assert result is False
        assert not destination.exists()  # Partial download should be cleaned up

    def test_download_file_preserves_existing_file_on_error(self, tmp_path, mocker):
        """Test that existing destination file is preserved on download error"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        # Create an existing file that should not be removed on failure
        destination.write_bytes(b"existing")

        temp_path = destination.with_name(destination.name + NETWORK.DOWNLOAD_TEMP_SUFFIX)
        temp_path.write_bytes(b"stale-partial")

        # Mock network error
        mocker.patch("urllib.request.urlopen", side_effect=urllib.error.URLError("Network error"))

        result = manager.download_file("http://test.com/file.txt", destination)

        assert result is False
        assert destination.exists()
        assert destination.read_bytes() == b"existing"
        assert not temp_path.exists()

    def test_download_file_handles_io_error(self, tmp_path, mocker):
        """Test that I/O errors are handled"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        # Mock response
        mock_response = Mock()
        mock_response.read.return_value = b"content"
        mock_response.headers.get.return_value = "7"
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mocker.patch("urllib.request.urlopen", return_value=mock_response)

        # Mock file write to raise IOError
        mock_open_func = mocker.patch("builtins.open", side_effect=IOError("Disk full"))

        result = manager.download_file("http://test.com/file.txt", destination)

        assert result is False

    def test_download_file_respects_pre_cancelled_state(self, tmp_path, mocker):
        """Test that download exits early when cancellation is already requested"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        manager.cancel()
        result = manager.download_file("http://test.com/file.txt", destination)

        assert result is False
        assert not destination.exists()

    def test_download_file_adds_user_agent(self, tmp_path, mocker):
        """Test that User-Agent header is added to request"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        # Mock response
        mock_response = Mock()
        mock_response.read.side_effect = [b"content", b""]
        mock_response.headers.get.return_value = "7"
        mock_response.__enter__ = Mock(return_value=mock_response)
        mock_response.__exit__ = Mock(return_value=False)

        mock_urlopen = mocker.patch("urllib.request.urlopen", return_value=mock_response)

        manager.download_file("http://test.com/file.txt", destination)

        # Check that request was created with User-Agent
        call_args = mock_urlopen.call_args
        request = call_args[0][0]
        assert request.headers.get("User-agent") == "ComfyUI-Version-Manager/1.0"


class TestDownloadManagerRetry:
    """Test download_with_retry method"""

    def test_download_with_retry_success_first_attempt(self, tmp_path, mocker):
        """Test successful download on first attempt"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        mock_download = mocker.patch.object(manager, "download_file", return_value=True)

        result = manager.download_with_retry("http://test.com/file.txt", destination, max_retries=3)

        assert result is True
        assert mock_download.call_count == 1

    def test_download_with_retry_succeeds_after_failure(self, tmp_path, mocker):
        """Test retry succeeds after initial failures"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        # Fail twice with retryable errors, then succeed
        attempts = {"count": 0}

        def side_effect(*_args, **_kwargs):
            attempts["count"] += 1
            if attempts["count"] <= 2:
                manager._last_error_retryable = True
                return False
            return True

        mock_download = mocker.patch.object(manager, "download_file", side_effect=side_effect)

        mocker.patch("time.sleep")  # Mock sleep to avoid delays

        result = manager.download_with_retry("http://test.com/file.txt", destination, max_retries=3)

        assert result is True
        assert mock_download.call_count >= 2
        assert mock_download.call_count <= 3

    def test_download_with_retry_fails_after_max_retries(self, tmp_path, mocker):
        """Test that retry fails after max attempts"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        def side_effect(*_args, **_kwargs):
            manager._last_error_retryable = True
            return False

        mock_download = mocker.patch.object(manager, "download_file", side_effect=side_effect)
        mocker.patch("time.sleep")

        result = manager.download_with_retry("http://test.com/file.txt", destination, max_retries=3)

        assert result is False
        assert mock_download.call_count == 3

    def test_download_with_retry_stops_on_non_retryable_error(self, tmp_path, mocker):
        """Test that retry stops on non-retryable errors"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        def fake_download(*_args, **_kwargs):
            manager._last_error_retryable = False
            manager._last_error = urllib.error.HTTPError(
                "http://test.com/file.txt", 404, "Not Found", None, None
            )
            return False

        mock_download = mocker.patch.object(manager, "download_file", side_effect=fake_download)

        result = manager.download_with_retry("http://test.com/file.txt", destination, max_retries=3)

        assert result is False
        assert mock_download.call_count == 1

    def test_download_with_retry_stops_on_cancel(self, tmp_path, mocker):
        """Test that retry stops after cancellation"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        def fake_download(*_args, **_kwargs):
            manager._last_error_cancelled = True
            manager._last_error = CancellationError("Download cancelled")
            return False

        mock_download = mocker.patch.object(manager, "download_file", side_effect=fake_download)

        result = manager.download_with_retry("http://test.com/file.txt", destination, max_retries=3)

        assert result is False
        assert mock_download.call_count == 1

    def test_download_with_retry_uses_backoff(self, tmp_path, mocker):
        """Test that retry uses exponential backoff"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        def side_effect(*_args, **_kwargs):
            manager._last_error_retryable = True
            return False

        mock_download = mocker.patch.object(manager, "download_file", side_effect=side_effect)
        mock_sleep = mocker.patch("time.sleep")

        manager.download_with_retry("http://test.com/file.txt", destination, max_retries=3)

        # Should have slept twice (before attempt 2 and 3)
        assert mock_sleep.call_count == 2
        # Verify backoff delay increases
        delays = [call[0][0] for call in mock_sleep.call_args_list]
        assert delays[0] >= 2.0  # First retry: base delay
        assert delays[1] > delays[0]  # Second retry: longer delay

    def test_download_with_retry_passes_progress_callback(self, tmp_path, mocker):
        """Test that progress callback is passed through"""
        manager = DownloadManager()
        destination = tmp_path / "test.txt"

        mock_callback = Mock()
        mock_download = mocker.patch.object(manager, "download_file", return_value=True)

        manager.download_with_retry(
            "http://test.com/file.txt", destination, progress_callback=mock_callback
        )

        # Verify callback was passed to download_file
        # Should be called with (url, destination, progress_callback)
        assert mock_download.call_count == 1
        args, kwargs = mock_download.call_args
        # Check positional args or keyword args
        if len(args) >= 3:
            assert args[2] == mock_callback
        else:
            assert kwargs.get("progress_callback") == mock_callback


# ============================================================================
# UTILITY FUNCTION TESTS
# ============================================================================


class TestUtilityFunctions:
    """Test utility functions"""

    def test_format_bytes_small_values(self):
        """Test formatting small byte values"""
        from backend.github_integration import format_bytes

        assert format_bytes(0) == "0.0 B"
        assert format_bytes(512) == "512.0 B"
        assert format_bytes(1023) == "1023.0 B"

    def test_format_bytes_kilobytes(self):
        """Test formatting kilobyte values"""
        from backend.github_integration import format_bytes

        assert format_bytes(1024) == "1.0 KB"
        assert format_bytes(2048) == "2.0 KB"
        assert format_bytes(5120) == "5.0 KB"

    def test_format_bytes_megabytes(self):
        """Test formatting megabyte values"""
        from backend.github_integration import format_bytes

        assert format_bytes(1024 * 1024) == "1.0 MB"
        assert format_bytes(5 * 1024 * 1024) == "5.0 MB"
        assert format_bytes(125 * 1024 * 1024) == "125.0 MB"

    def test_format_bytes_gigabytes(self):
        """Test formatting gigabyte values"""
        from backend.github_integration import format_bytes

        assert format_bytes(1024 * 1024 * 1024) == "1.0 GB"
        assert format_bytes(4 * 1024 * 1024 * 1024) == "4.0 GB"

    def test_format_bytes_terabytes(self):
        """Test formatting terabyte values"""
        from backend.github_integration import format_bytes

        assert format_bytes(1024 * 1024 * 1024 * 1024) == "1.0 TB"

    def test_format_bytes_fractional_values(self):
        """Test formatting fractional byte values"""
        from backend.github_integration import format_bytes

        result = format_bytes(1536)  # 1.5 KB
        assert "1.5 KB" == result
