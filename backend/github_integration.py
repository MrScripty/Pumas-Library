#!/usr/bin/env python3
"""
GitHub Integration for ComfyUI Version Manager
Handles fetching releases, caching, and downloading
"""

import json
import time
import urllib.request
import urllib.error
from pathlib import Path
from typing import Optional, List, Callable
from backend.models import GitHubRelease, GitHubReleasesCache, Release, get_iso_timestamp
from backend.metadata_manager import MetadataManager


class GitHubReleasesFetcher:
    """Fetches and caches ComfyUI releases from GitHub"""

    GITHUB_API_BASE = "https://api.github.com"
    COMFYUI_REPO = "comfyanonymous/ComfyUI"
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

    def _fetch_from_github(self) -> List[GitHubRelease]:
        """
        Fetch releases from GitHub API

        Returns:
            List of GitHubRelease objects

        Raises:
            urllib.error.URLError: If network request fails
        """
        url = f"{self.GITHUB_API_BASE}/repos/{self.COMFYUI_REPO}/releases"

        # Create request with User-Agent header (required by GitHub API)
        req = urllib.request.Request(url)
        req.add_header('User-Agent', 'ComfyUI-Version-Manager/1.0')
        req.add_header('Accept', 'application/vnd.github.v3+json')

        try:
            with urllib.request.urlopen(req, timeout=10) as response:
                data = json.loads(response.read().decode('utf-8'))
                return data
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
            last_fetched = parse_iso_timestamp(cache['lastFetched'])
            now = parse_iso_timestamp(get_iso_timestamp())
            age_seconds = (now - last_fetched).total_seconds()
            return age_seconds < cache.get('ttl', self.ttl)
        except (KeyError, ValueError) as e:
            print(f"Error validating cache: {e}")
            return False

    def get_releases(self, force_refresh: bool = False) -> List[GitHubRelease]:
        """
        Get ComfyUI releases (from cache or GitHub)

        Args:
            force_refresh: If True, bypass cache and fetch from GitHub

        Returns:
            List of GitHubRelease objects
        """
        # Check cache first unless forced refresh
        if not force_refresh:
            cache = self.metadata_manager.load_github_cache()
            if self._is_cache_valid(cache):
                print("Using cached releases data")
                return cache['releases']

        # Fetch from GitHub
        print("Fetching releases from GitHub...")
        try:
            releases = self._fetch_from_github()

            # Update cache
            cache_data: GitHubReleasesCache = {
                'lastFetched': get_iso_timestamp(),
                'ttl': self.ttl,
                'releases': releases
            }
            self.metadata_manager.save_github_cache(cache_data)

            print(f"Fetched {len(releases)} releases from GitHub")
            return releases
        except Exception as e:
            # On error, try to return stale cache if available
            print(f"Error fetching from GitHub: {e}")
            cache = self.metadata_manager.load_github_cache()
            if cache:
                print("Using stale cached data due to fetch error")
                return cache['releases']
            else:
                print("No cache available and fetch failed")
                return []

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
            if not include_prerelease and release.get('prerelease', False):
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
            if release.get('tag_name') == tag:
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
            formatted.append({
                'tag': release.get('tag_name', ''),
                'name': release.get('name', ''),
                'date': release.get('published_at', ''),
                'notes': release.get('body', ''),
                'url': release.get('zipball_url', ''),
                'prerelease': release.get('prerelease', False)
            })
        return formatted


class DownloadManager:
    """Handles downloading files with progress tracking"""

    def __init__(self):
        """Initialize download manager"""
        self.last_progress_time = 0
        self.progress_update_interval = 0.5  # Update progress every 500ms

    def download_file(
        self,
        url: str,
        destination: Path,
        progress_callback: Optional[Callable[[int, int], None]] = None
    ) -> bool:
        """
        Download a file with progress tracking

        Args:
            url: URL to download from
            destination: Path to save file
            progress_callback: Optional callback function(bytes_downloaded, total_bytes)

        Returns:
            True if successful, False otherwise
        """
        try:
            # Create parent directory if needed
            destination.parent.mkdir(parents=True, exist_ok=True)

            # Create request with User-Agent
            req = urllib.request.Request(url)
            req.add_header('User-Agent', 'ComfyUI-Version-Manager/1.0')

            # Download with progress tracking
            with urllib.request.urlopen(req, timeout=30) as response:
                total_size = int(response.headers.get('Content-Length', 0))
                downloaded = 0

                with open(destination, 'wb') as f:
                    while True:
                        chunk = response.read(8192)  # 8KB chunks
                        if not chunk:
                            break

                        f.write(chunk)
                        downloaded += len(chunk)

                        # Call progress callback if provided
                        if progress_callback:
                            current_time = time.time()
                            if current_time - self.last_progress_time >= self.progress_update_interval:
                                progress_callback(downloaded, total_size)
                                self.last_progress_time = current_time

                # Final progress update
                if progress_callback and total_size > 0:
                    progress_callback(downloaded, total_size)

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
        progress_callback: Optional[Callable[[int, int], None]] = None
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
    for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
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
        print(f"\rDownloading: {format_bytes(downloaded)} / {format_bytes(total)} ({percent:.1f}%)", end='', flush=True)
    else:
        print(f"\rDownloading: {format_bytes(downloaded)}", end='', flush=True)


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
            tag = release.get('tag_name', 'unknown')
            name = release.get('name', 'Unnamed')
            date = release.get('published_at', 'unknown')
            prerelease = " (pre-release)" if release.get('prerelease') else ""

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
