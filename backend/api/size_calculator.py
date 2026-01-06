#!/usr/bin/env python3
"""
Size Calculator for ComfyUI Releases
Handles calculation and caching of release download sizes
"""

import threading
import urllib.request
from pathlib import Path
from typing import TYPE_CHECKING, Any, Callable, Dict, List, Optional

from backend.logging_config import get_logger
from backend.models import GitHubRelease

if TYPE_CHECKING:
    from backend.github_integration import GitHubReleasesFetcher
    from backend.release_size_calculator import ReleaseSizeCalculator
    from backend.version_manager import VersionManager

logger = get_logger(__name__)


class SizeCalculator:
    """Manages release size calculation and caching"""

    def __init__(
        self,
        release_size_calculator: "ReleaseSizeCalculator",
        github_fetcher: "GitHubReleasesFetcher",
        version_manager: Optional["VersionManager"] = None,
    ):
        """
        Initialize size calculator

        Args:
            release_size_calculator: ReleaseSizeCalculator instance
            github_fetcher: GitHubReleasesFetcher instance
            version_manager: Optional VersionManager instance
        """
        self.release_size_calculator = release_size_calculator
        self.github_fetcher = github_fetcher
        self.version_manager = version_manager

    def _refresh_release_sizes_async(
        self,
        releases: List[GitHubRelease],
        installed_tags: set[str],
        force_refresh: bool = False,
    ):
        """
        Calculate release sizes in the background, prioritizing non-installed releases.
        """
        if not self.release_size_calculator:
            return

        # Build priority queue: non-installed first
        def sort_key(release: GitHubRelease):
            tag = release.get("tag_name", "")
            return 0 if tag not in installed_tags else 1

        pending = sorted(releases, key=sort_key)

        def _worker():
            for release in pending:
                tag = release.get("tag_name", "")
                if not tag:
                    continue
                # Skip if already cached and not forcing
                if not force_refresh and self.release_size_calculator.get_cached_size(tag):
                    continue
                try:
                    self.calculate_release_size(tag, force_refresh=force_refresh)
                except (urllib.error.URLError, OSError, ValueError, KeyError) as exc:
                    logger.error(f"Size refresh failed for {tag}: {exc}", exc_info=True)

        threading.Thread(target=_worker, daemon=True).start()

    def calculate_release_size(
        self, tag: str, force_refresh: bool = False
    ) -> Optional[Dict[str, Any]]:
        """
        Calculate total download size for a release (Phase 6.2.5c)

        Args:
            tag: Release tag to calculate size for
            force_refresh: Force recalculation even if cached

        Returns:
            Dict with size breakdown or None if calculation fails
        """
        try:
            # Get release from GitHub
            release = self.github_fetcher.get_release_by_tag(tag)
            if not release:
                logger.warning(f"Release {tag} not found")
                return None

            # Get archive size from zipball_url
            download_url = release.get("zipball_url") or release.get("tarball_url")
            archive_size = None

            if download_url:
                archive_size = self._get_content_length(download_url)

            # Fallback estimate if HEAD fails
            if not archive_size:
                archive_size = 125 * 1024 * 1024  # 125 MB estimate

            # Calculate total size including dependencies
            result = self.release_size_calculator.calculate_release_size(
                tag=tag, archive_size=archive_size, force_refresh=force_refresh
            )

            return result
        except (urllib.error.URLError, OSError, ValueError, KeyError) as e:
            logger.error(f"Error calculating release size for {tag}: {e}", exc_info=True)
            return None

    def calculate_all_release_sizes(
        self, progress_callback: Optional[Callable[[int, int, str], None]] = None
    ) -> Dict[str, Dict[str, Any]]:
        """
        Calculate sizes for all available releases (Phase 6.2.5c)

        Args:
            progress_callback: Optional callback(current, total, tag)

        Returns:
            Dict mapping tag to size data
        """
        if not self.version_manager:
            return {}

        releases = self.version_manager.get_available_releases()
        results = {}
        total = len(releases)

        for i, release in enumerate(releases):
            tag = release.get("tag_name", "")
            if progress_callback:
                progress_callback(i + 1, total, tag)

            result = self.calculate_release_size(tag)
            if result:
                results[tag] = result

        return results

    def _get_content_length(self, url: str) -> Optional[int]:
        """
        Perform a HEAD request to retrieve Content-Length for a URL.
        """
        try:
            req = urllib.request.Request(url, method="HEAD")
            req.add_header("User-Agent", "ComfyUI-Version-Manager/1.0")
            with urllib.request.urlopen(req, timeout=10) as resp:
                length = resp.headers.get("Content-Length")
                if length:
                    return int(length)
        except (urllib.error.URLError, OSError, ValueError) as e:
            logger.warning(f"Warning: Failed to fetch Content-Length for {url}: {e}")
        return None

    def get_release_size_info(self, tag: str, archive_size: int) -> Optional[Dict[str, Any]]:
        """
        Get size information for a release (Phase 6.2.5a/c)

        Args:
            tag: Release tag
            archive_size: Size of the archive in bytes

        Returns:
            Dict with size breakdown or None if not available
        """
        if not self.release_size_calculator:
            return None

        try:
            # Calculate release size (uses cache if available)
            result = self.release_size_calculator.calculate_release_size(tag, archive_size)
            return result
        except (ValueError, KeyError, TypeError) as e:
            logger.error(f"Error calculating release size: {e}", exc_info=True)
            return None

    def get_release_size_breakdown(self, tag: str) -> Optional[Dict[str, Any]]:
        """
        Get size breakdown for display (Phase 6.2.5c)

        Args:
            tag: Release tag

        Returns:
            Dict with formatted size breakdown or None if not available
        """
        if not self.release_size_calculator:
            return None

        try:
            return self.release_size_calculator.get_size_breakdown(tag)
        except (ValueError, KeyError, TypeError) as e:
            logger.error(f"Error getting size breakdown: {e}", exc_info=True)
            return None

    def get_release_dependencies(
        self, tag: str, top_n: Optional[int] = None
    ) -> List[Dict[str, Any]]:
        """
        Get dependencies for a release sorted by size (Phase 6.2.5c)

        Args:
            tag: Release tag
            top_n: Optional limit to top N packages

        Returns:
            List of dependency dicts sorted by size (largest first)
        """
        if not self.release_size_calculator:
            return []

        try:
            return self.release_size_calculator.get_sorted_dependencies(tag, top_n)
        except (ValueError, KeyError, TypeError) as e:
            logger.error(f"Error getting dependencies: {e}", exc_info=True)
            return []
