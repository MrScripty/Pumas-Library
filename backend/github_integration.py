#!/usr/bin/env python3
"""
GitHub Integration for ComfyUI Version Manager
Handles fetching releases, caching, and downloading
"""

import json
import threading
import time
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional

from cachetools import TTLCache
from packaging.version import InvalidVersion, Version

from backend.metadata_manager import MetadataManager
from backend.models import GitHubRelease, GitHubReleasesCache, Release, get_iso_timestamp


class GitHubReleasesFetcher:
    """Fetches and caches ComfyUI releases from GitHub"""

    GITHUB_API_BASE = "https://api.github.com"
    COMFYUI_REPO = "comfyanonymous/ComfyUI"
    PER_PAGE = 100
    MAX_PAGES = 10  # Safety cap for pagination
    DEFAULT_TTL = 3600  # 1 hour cache TTL

    def __init__(self, metadata_manager: MetadataManager, ttl: int = DEFAULT_TTL):
        """
        Initialize GitHub releases fetcher

        Args:
            metadata_manager: MetadataManager instance for caching
            ttl: Cache time-to-live in seconds
        """
        self.metadata_manager = metadata_manager
        self.ttl = ttl

        # In-memory cache with TTL and thread lock
        self._memory_cache = TTLCache(maxsize=1, ttl=ttl)
        self._cache_lock = threading.Lock()
        self._CACHE_KEY = "github_releases"

    def _fetch_page(self, page: int) -> List[GitHubRelease]:
        """
        Fetch a single page of releases from GitHub API.
        """
        url = f"{self.GITHUB_API_BASE}/repos/{self.COMFYUI_REPO}/releases?per_page={self.PER_PAGE}&page={page}"

        # Create request with User-Agent header (required by GitHub API)
        req = urllib.request.Request(url)
        req.add_header("User-Agent", "ComfyUI-Version-Manager/1.0")
        req.add_header("Accept", "application/vnd.github.v3+json")

        with urllib.request.urlopen(req, timeout=10) as response:
            return json.loads(response.read().decode("utf-8"))

    def _fetch_from_github(self) -> List[GitHubRelease]:
        """
        Fetch releases from GitHub API

        Returns:
            List of GitHubRelease objects

        Raises:
            urllib.error.URLError: If network request fails
        """
        releases: List[GitHubRelease] = []
        try:
            for page in range(1, self.MAX_PAGES + 1):
                page_data = self._fetch_page(page)
                if not page_data:
                    break
                releases.extend(page_data)
                if len(page_data) < self.PER_PAGE:
                    break
            return releases
        except urllib.error.HTTPError as e:
            if e.code == 403:
                print("GitHub API rate limit exceeded. Using cached data if available.")
                raise
            else:
                print(f"GitHub API error: {e.code} {e.reason}")
                raise
        except urllib.error.URLError as e:
            print(f"Network error fetching releases: {e}")
            raise

    def _is_cache_valid(self, cache: Optional[GitHubReleasesCache]) -> bool:
        """
        Check if cached data is still valid

        Args:
            cache: Cached releases data

        Returns:
            True if cache is valid and not expired
        """
        if cache is None:
            return False

        try:
            from backend.models import parse_iso_timestamp

            last_fetched = parse_iso_timestamp(cache["lastFetched"])
            now = parse_iso_timestamp(get_iso_timestamp())
            age_seconds = (now - last_fetched).total_seconds()
            return age_seconds < cache.get("ttl", self.ttl)
        except (KeyError, ValueError) as e:
            print(f"Error validating cache: {e}")
            return False

    def get_releases(self, force_refresh: bool = False) -> List[GitHubRelease]:
        """
        Get ComfyUI releases with offline-first strategy

        Strategy:
        1. Check in-memory cache (instant, ~0.1ms)
        2. Check disk cache (fast, ~5ms)
        3. If no valid cache: return stale cache or empty list
        4. Network fetch ONLY happens in background thread (never blocks)

        Args:
            force_refresh: If True, bypass cache and fetch from GitHub (blocking)

        Returns:
            List of GitHubRelease objects (may be empty if offline with no cache)
        """
        # FAST PATH: Check in-memory cache first (no lock needed for read)
        if not force_refresh and self._CACHE_KEY in self._memory_cache:
            print("Using in-memory cached releases data")
            return self._memory_cache[self._CACHE_KEY]

        # SLOW PATH: Coordinated cache check or fetch
        with self._cache_lock:
            # Double-check pattern: another thread might have cached while we waited
            if not force_refresh and self._CACHE_KEY in self._memory_cache:
                print("Using in-memory cached releases data (after lock)")
                return self._memory_cache[self._CACHE_KEY]

            # Check disk cache
            if not force_refresh:
                disk_cache = self.metadata_manager.load_github_cache()

                # Valid cache - load into memory
                if self._is_cache_valid(disk_cache):
                    print("Loading releases from disk cache")
                    releases = disk_cache["releases"]
                    self._memory_cache[self._CACHE_KEY] = releases
                    return releases

                # Stale cache exists - use it anyway (offline-first)
                if disk_cache and disk_cache.get("releases"):
                    print("Using stale disk cache (offline-first)")
                    stale_releases = disk_cache["releases"]
                    self._memory_cache[self._CACHE_KEY] = stale_releases
                    return stale_releases

                # No cache at all - return empty (background will fetch)
                print("No cache available - returning empty (background fetch will populate)")
                return []

            # force_refresh=True: Actually fetch from GitHub (blocking)
            print("Fetching releases from GitHub (forced refresh)...")
            try:
                releases = self._fetch_from_github()

                # Update both caches
                cache_data: GitHubReleasesCache = {
                    "lastFetched": get_iso_timestamp(),
                    "ttl": self.ttl,
                    "releases": releases,
                }
                self.metadata_manager.save_github_cache(cache_data)
                self._memory_cache[self._CACHE_KEY] = releases

                print(f"Fetched {len(releases)} releases from GitHub")
                return releases

            except urllib.error.URLError as e:
                # Network error (offline, timeout, DNS failure)
                if force_refresh:
                    print(f"⚠️  Cannot refresh: Network unavailable ({e})")
                    print("Returning cached data (if available)")
                else:
                    print(f"Network unavailable: {e}")

                # Return stale cache if available
                disk_cache = self.metadata_manager.load_github_cache()
                if disk_cache and disk_cache.get("releases"):
                    stale_releases = disk_cache["releases"]
                    self._memory_cache[self._CACHE_KEY] = stale_releases

                    if force_refresh:
                        print(f"Using stale cache ({len(stale_releases)} releases)")
                    else:
                        print("Using stale disk cache (network unavailable)")

                    return stale_releases

                if force_refresh:
                    print("❌ No cache available - cannot refresh while offline")
                else:
                    print("No cache available and network unavailable - returning empty")

                return []

            except Exception as e:
                # Other errors (rate limit, parse error, etc.)
                print(f"Error fetching from GitHub: {e}")

                # Try stale cache
                disk_cache = self.metadata_manager.load_github_cache()
                if disk_cache and disk_cache.get("releases"):
                    print("Using stale disk cache (fetch error)")
                    return disk_cache["releases"]

                print("No cache available and fetch failed - returning empty")
                return []

    def get_cache_status(self) -> Dict[str, Any]:
        """
        Get current cache status for UI display

        Returns:
            {
                'has_cache': bool,
                'is_valid': bool,
                'age_seconds': int,
                'last_fetched': str (ISO timestamp),
                'ttl': int,
                'releases_count': int,
                'is_fetching': bool
            }
        """
        status = {
            "has_cache": False,
            "is_valid": False,
            "age_seconds": None,
            "last_fetched": None,
            "ttl": self.ttl,
            "releases_count": 0,
            "is_fetching": False,
        }

        # Check if fetch is in progress
        status["is_fetching"] = self._cache_lock.locked()

        # Check in-memory cache first
        if self._CACHE_KEY in self._memory_cache:
            releases = self._memory_cache[self._CACHE_KEY]
            status["has_cache"] = True
            status["is_valid"] = True  # In-memory is always valid (TTL enforced)
            status["releases_count"] = len(releases)
            # Try to get age from disk cache for display
            try:
                disk_cache = self.metadata_manager.load_github_cache()
                if disk_cache and disk_cache.get("lastFetched"):
                    from backend.models import parse_iso_timestamp

                    last_fetched = parse_iso_timestamp(disk_cache["lastFetched"])
                    now = parse_iso_timestamp(get_iso_timestamp())
                    status["age_seconds"] = int((now - last_fetched).total_seconds())
                    status["last_fetched"] = disk_cache.get("lastFetched")
            except Exception as e:
                print(f"Error getting cache age: {e}")
            return status

        # Check disk cache
        disk_cache = self.metadata_manager.load_github_cache()
        if disk_cache and disk_cache.get("releases"):
            status["has_cache"] = True
            status["releases_count"] = len(disk_cache["releases"])
            status["last_fetched"] = disk_cache.get("lastFetched")

            # Check validity
            try:
                from backend.models import parse_iso_timestamp

                last_fetched = parse_iso_timestamp(disk_cache["lastFetched"])
                now = parse_iso_timestamp(get_iso_timestamp())
                age_seconds = (now - last_fetched).total_seconds()
                status["age_seconds"] = int(age_seconds)
                status["is_valid"] = age_seconds < disk_cache.get("ttl", self.ttl)
            except Exception as e:
                print(f"Error validating disk cache in get_cache_status: {e}")
                # If we can't parse the timestamp, assume it's invalid but exists
                status["is_valid"] = False

        return status

    def get_latest_release(self, include_prerelease: bool = False) -> Optional[GitHubRelease]:
        """
        Get the latest release

        Args:
            include_prerelease: If True, include pre-releases

        Returns:
            Latest GitHubRelease or None if no releases found
        """
        releases = self.get_releases()

        for release in releases:
            if not include_prerelease and release.get("prerelease", False):
                continue
            return release

        return None

    def get_release_by_tag(self, tag: str) -> Optional[GitHubRelease]:
        """
        Get a specific release by tag

        Args:
            tag: Release tag (e.g., "v0.2.0")

        Returns:
            GitHubRelease or None if not found
        """
        releases = self.get_releases()

        for release in releases:
            if release.get("tag_name") == tag:
                return release

        return None

    def format_releases_for_display(self, releases: List[GitHubRelease]) -> List[Release]:
        """
        Convert GitHub releases to simplified Release format

        Args:
            releases: List of GitHubRelease objects

        Returns:
            List of simplified Release objects
        """
        formatted = []
        for release in releases:
            formatted.append(
                {
                    "tag": release.get("tag_name", ""),
                    "name": release.get("name", ""),
                    "date": release.get("published_at", ""),
                    "notes": release.get("body", ""),
                    "url": release.get("zipball_url", ""),
                    "prerelease": release.get("prerelease", False),
                }
            )
        return formatted

    def collapse_latest_patch_per_minor(
        self, releases: List[GitHubRelease], include_prerelease: bool = True
    ) -> List[GitHubRelease]:
        """
        Reduce releases to the latest patch per minor (major.minor).
        """
        best_by_minor: Dict[str, GitHubRelease] = {}

        for release in releases:
            if release.get("prerelease", False) and not include_prerelease:
                continue

            tag = release.get("tag_name") or ""
            if not tag:
                continue

            # Strip leading 'v' or 'V' for parsing
            normalized = tag.lstrip("vV")

            try:
                parsed = Version(normalized)
            except InvalidVersion:
                # Skip unparseable tags to avoid breaking the list
                continue

            minor_key = f"{parsed.major}.{parsed.minor}"
            current_best = best_by_minor.get(minor_key)

            if not current_best:
                best_by_minor[minor_key] = release
                continue

            current_tag = (current_best.get("tag_name") or "").lstrip("vV")
            try:
                current_version = Version(current_tag)
            except InvalidVersion:
                best_by_minor[minor_key] = release
                continue

            if parsed > current_version:
                best_by_minor[minor_key] = release

        # Preserve original order based on appearance in the source list
        collapsed: List[GitHubRelease] = []
        seen = set()
        for release in releases:
            tag = release.get("tag_name")
            if not tag:
                continue
            normalized = tag.lstrip("vV")
            try:
                parsed = Version(normalized)
            except InvalidVersion:
                continue
            minor_key = f"{parsed.major}.{parsed.minor}"
            best = best_by_minor.get(minor_key)
            if best and best.get("tag_name") == tag and minor_key not in seen:
                collapsed.append(release)
                seen.add(minor_key)

        return collapsed


class DownloadManager:
    """Handles downloading files with progress tracking"""

    def __init__(self):
        """Initialize download manager"""
        self.last_progress_time = 0
        self.last_progress_bytes = 0
        self.progress_update_interval = 0.5  # Update progress every 500ms
        self._cancel_requested = False  # Cancellation flag

    def cancel(self):
        """Request cancellation of current download"""
        self._cancel_requested = True
        print("Download cancellation requested")

    def download_file(
        self,
        url: str,
        destination: Path,
        progress_callback: Optional[Callable[[int, int, Optional[float]], None]] = None,
    ) -> bool:
        """
        Download a file with progress tracking

        Args:
            url: URL to download from
            destination: Path to save file
            progress_callback: Optional callback function(bytes_downloaded, total_bytes, speed_bytes_per_sec)

        Returns:
            True if successful, False otherwise
        """
        # Reset cancellation flag at start of download
        self._cancel_requested = False
        self.last_progress_time = time.time()
        self.last_progress_bytes = 0

        try:
            # Create parent directory if needed
            destination.parent.mkdir(parents=True, exist_ok=True)

            # Create request with User-Agent
            req = urllib.request.Request(url)
            req.add_header("User-Agent", "ComfyUI-Version-Manager/1.0")

            # Download with progress tracking
            with urllib.request.urlopen(req, timeout=30) as response:
                total_size = int(response.headers.get("Content-Length", 0))
                downloaded = 0

                # Initial progress update so listeners know total size
                if progress_callback:
                    progress_callback(downloaded, total_size, None)

                with open(destination, "wb") as f:
                    while True:
                        # Check for cancellation before reading next chunk
                        if self._cancel_requested:
                            print("Download cancelled by user")
                            raise InterruptedError("Download cancelled")

                        chunk = response.read(8192)  # 8KB chunks
                        if not chunk:
                            break

                        f.write(chunk)
                        downloaded += len(chunk)

                        # Call progress callback if provided
                        if progress_callback:
                            current_time = time.time()
                            should_update = (
                                current_time - self.last_progress_time
                                >= self.progress_update_interval
                            )
                            if should_update or downloaded == total_size:
                                speed = None
                                elapsed = current_time - self.last_progress_time
                                if elapsed > 0:
                                    bytes_since_last = downloaded - self.last_progress_bytes
                                    speed = bytes_since_last / elapsed

                                progress_callback(downloaded, total_size, speed)
                                self.last_progress_time = current_time
                                self.last_progress_bytes = downloaded

                # Final progress update
                if progress_callback:
                    current_time = time.time()
                    elapsed = current_time - self.last_progress_time
                    speed = None
                    if elapsed > 0:
                        bytes_since_last = downloaded - self.last_progress_bytes
                        speed = bytes_since_last / elapsed

                    progress_callback(downloaded, total_size, speed)

                return True

        except urllib.error.URLError as e:
            print(f"Download error: {e}")
            # Clean up partial download
            if destination.exists():
                destination.unlink()
            return False
        except Exception as e:
            print(f"Unexpected error during download: {e}")
            # Clean up partial download
            if destination.exists():
                destination.unlink()
            return False

    def download_with_retry(
        self,
        url: str,
        destination: Path,
        max_retries: int = 3,
        progress_callback: Optional[Callable[[int, int, Optional[float]], None]] = None,
    ) -> bool:
        """
        Download with automatic retries on failure

        Args:
            url: URL to download from
            destination: Path to save file
            max_retries: Maximum number of retry attempts
            progress_callback: Optional progress callback

        Returns:
            True if successful, False otherwise
        """
        for attempt in range(max_retries):
            if attempt > 0:
                print(f"Retry attempt {attempt + 1}/{max_retries}...")
                time.sleep(2)  # Wait 2 seconds between retries

            if self.download_file(url, destination, progress_callback):
                return True

        print(f"Download failed after {max_retries} attempts")
        return False


def format_bytes(size: int) -> str:
    """
    Format bytes as human-readable string

    Args:
        size: Size in bytes

    Returns:
        Formatted string (e.g., "1.5 MB")
    """
    for unit in ["B", "KB", "MB", "GB", "TB"]:
        if size < 1024.0:
            return f"{size:.1f} {unit}"
        size /= 1024.0
    return f"{size:.1f} PB"


def print_progress(downloaded: int, total: int) -> None:
    """
    Print download progress to console

    Args:
        downloaded: Bytes downloaded so far
        total: Total bytes to download
    """
    if total > 0:
        percent = (downloaded / total) * 100
        print(
            f"\rDownloading: {format_bytes(downloaded)} / {format_bytes(total)} ({percent:.1f}%)",
            end="",
            flush=True,
        )
    else:
        print(f"\rDownloading: {format_bytes(downloaded)}", end="", flush=True)


if __name__ == "__main__":
    # For testing - demonstrate GitHub integration
    from backend.utils import get_launcher_root

    launcher_root = get_launcher_root()
    launcher_data_dir = launcher_root / "launcher-data"

    # Initialize metadata manager
    metadata_mgr = MetadataManager(launcher_data_dir)

    # Initialize GitHub fetcher
    github = GitHubReleasesFetcher(metadata_mgr)

    print("=== ComfyUI Releases ===\n")

    # Fetch releases
    releases = github.get_releases()

    if releases:
        print(f"Found {len(releases)} releases:\n")

        # Display first 5 releases
        for i, release in enumerate(releases[:5]):
            tag = release.get("tag_name", "unknown")
            name = release.get("name", "Unnamed")
            date = release.get("published_at", "unknown")
            prerelease = " (pre-release)" if release.get("prerelease") else ""

            print(f"{i+1}. {tag} - {name}{prerelease}")
            print(f"   Published: {date}")
            print()

        # Get latest stable release
        latest = github.get_latest_release(include_prerelease=False)
        if latest:
            print(f"\nLatest stable release: {latest.get('tag_name')}")
            print(f"Published: {latest.get('published_at')}")
    else:
        print("No releases found")

    print("\n=== Testing Download (sample file) ===\n")

    # Test download with a small file (GitHub API response as example)
    downloader = DownloadManager()
    test_url = "https://api.github.com/repos/comfyanonymous/ComfyUI/releases/latest"
    test_dest = launcher_root / "test-download.json"

    print(f"Downloading test file to {test_dest}...")
    success = downloader.download_file(test_url, test_dest, print_progress)

    if success:
        print(f"\n✓ Download successful! Size: {format_bytes(test_dest.stat().st_size)}")
        test_dest.unlink()  # Clean up
    else:
        print("\n✗ Download failed")
