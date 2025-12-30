#!/usr/bin/env python3
"""
Release Size Calculator - Phase 6.2.5a
Calculates total size per release (archive + dependencies)
"""

import json
import os
import shutil
import subprocess
import sys
import urllib.error
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional, Set, Tuple

from backend.logging_config import get_logger

logger = get_logger(__name__)


class ReleaseSizeCalculator:
    """Calculates and caches total sizes for ComfyUI releases"""

    def __init__(
        self,
        cache_dir: Path,
        release_data_fetcher,
        package_size_resolver,
        pip_cache_dir: Optional[Path] = None,
    ):
        """
        Initialize ReleaseSizeCalculator

        Args:
            cache_dir: Directory for cache storage
            release_data_fetcher: ReleaseDataFetcher instance
            package_size_resolver: PackageSizeResolver instance
        """
        self.cache_dir = Path(cache_dir)
        self.cache_dir.mkdir(parents=True, exist_ok=True)

        self.cache_file = self.cache_dir / "release-sizes.json"
        self.release_data_fetcher = release_data_fetcher
        self.package_size_resolver = package_size_resolver

        self._cache: Dict[str, Dict[str, Any]] = self._load_cache()
        # Optional shared pip cache to reuse metadata downloads if present
        self.pip_cache_dir = Path(pip_cache_dir) if pip_cache_dir else None

    def _load_cache(self) -> Dict[str, Dict[str, Any]]:
        """Load release sizes cache from disk"""
        if self.cache_file.exists():
            try:
                with open(self.cache_file, "r") as f:
                    data = json.load(f)
                    # Drop old cache entries that don't include source info
                    cleaned = {}
                    for tag, entry in data.items():
                        if isinstance(entry, dict) and entry.get("dependencies_size_source"):
                            cleaned[tag] = entry
                    return cleaned
            except (json.JSONDecodeError, OSError, ValueError) as e:
                logger.warning(f"Warning: Failed to load release sizes cache: {e}")
        return {}

    def _save_cache(self):
        """Save release sizes cache to disk"""
        try:
            with open(self.cache_file, "w") as f:
                json.dump(self._cache, f, indent=2)
        except (OSError, TypeError, ValueError) as e:
            logger.error(f"Error saving release sizes cache: {e}", exc_info=True)

    def _get_iso_timestamp(self) -> str:
        """Get current timestamp in ISO format"""
        return datetime.now(timezone.utc).isoformat()

    def calculate_release_size(
        self, tag: str, archive_size: int, force_refresh: bool = False
    ) -> Optional[Dict[str, Any]]:
        """
        Calculate total size for a release

        Args:
            tag: Release tag
            archive_size: Size of the ComfyUI archive in bytes
            force_refresh: Force recalculation

        Returns:
            Dict with size breakdown or None if requirements not available
        """
        # Get requirements data
        requirements_data: Optional[Dict[str, Any]] = None
        if not force_refresh:
            requirements_data = self.release_data_fetcher.get_cached_requirements(tag)

        if not requirements_data:
            # Try to fetch if not cached or when forcing refresh
            requirements_data = self.release_data_fetcher.fetch_requirements_for_release(
                tag, force_refresh
            )

        if not requirements_data:
            return None

        # Check if we need to recalculate
        requirements_hash = requirements_data["requirements_hash"]
        cache_key = tag

        if not force_refresh and cache_key in self._cache:
            cached = self._cache[cache_key]
            # Validate cache is for same requirements
            if cached.get("requirements_hash") == requirements_hash:
                return cached

        # Parse requirements
        requirements = self.release_data_fetcher.parse_requirements(
            requirements_data["requirements_txt"]
        )

        # Fast total estimate using pip resolver (captures transitives)
        logger.info(f"Estimating total dependency download size for {tag} via resolver report...")
        pip_estimate = self._estimate_dependencies_size_via_pip(
            requirements_data["requirements_txt"], tag
        )

        deps_size = pip_estimate if pip_estimate is not None else 0
        deps_source = "pip_report" if pip_estimate is not None else "unknown"
        if pip_estimate is None:
            cached_entry = self._cache.get(cache_key)
            if cached_entry and cached_entry.get("requirements_hash") == requirements_hash:
                cached_deps = cached_entry.get("dependencies_size")
                if cached_deps:
                    deps_size = cached_deps
                    deps_source = "cache_fallback"
        dependency_sizes: List[Dict[str, Any]] = []
        unknown_count = 0

        # Calculate total size
        if deps_size == 0 and pip_estimate is None:
            logger.warning(f"Warning: No dependency size estimate available for {tag}")
        total_size = archive_size + deps_size

        # Build result
        result = {
            "tag": tag,
            "total_size": total_size,
            "archive_size": archive_size,
            "dependencies_size": deps_size,
            "dependency_count": len(requirements),
            "unknown_size_count": unknown_count,
            "dependencies": dependency_sizes,
            "requirements_hash": requirements_hash,
            "calculated_at": self._get_iso_timestamp(),
            "platform": self.package_size_resolver.platform,
            "python_version": self.package_size_resolver.python_version,
            "dependencies_size_source": deps_source,
        }

        # Cache the result
        self._cache[cache_key] = result
        self._save_cache()

        logger.info(f"✓ Calculated total size for {tag}: {self._format_size(total_size)}")
        return result

    def get_cached_size(self, tag: str) -> Optional[Dict[str, Any]]:
        """
        Get cached size data for a release

        Args:
            tag: Release tag

        Returns:
            Cached size data or None
        """
        return self._cache.get(tag)

    def get_sorted_dependencies(
        self, tag: str, top_n: Optional[int] = None
    ) -> List[Dict[str, Any]]:
        """
        Get dependencies sorted by size

        Args:
            tag: Release tag
            top_n: Optional limit to top N packages

        Returns:
            List of dependency dicts sorted by size (largest first)
        """
        cached = self.get_cached_size(tag)
        if not cached:
            return []

        dependencies = cached.get("dependencies", [])
        if not isinstance(dependencies, list):
            return []

        filtered: List[Dict[str, Any]] = []
        for entry in dependencies:
            if isinstance(entry, dict):
                filtered.append(entry)

        if top_n:
            return filtered[:top_n]

        return filtered

    def _estimate_dependencies_size_via_pip(self, requirements_txt: str, tag: str) -> Optional[int]:
        """
        Use pip --dry-run --report to estimate total download size including transitives.
        """
        temp_root = self.cache_dir / "pip-size-estimates" / tag
        report_file = temp_root / "report.json"
        try:
            if temp_root.exists():
                shutil.rmtree(temp_root)
            temp_root.mkdir(parents=True, exist_ok=True)

            req_file = temp_root / "requirements.txt"
            req_file.write_text(requirements_txt)

            cmd = [
                sys.executable,
                "-m",
                "pip",
                "install",
                "--dry-run",
                "--report",
                str(report_file),
                "--disable-pip-version-check",
                "--no-input",
                "-r",
                str(req_file),
            ]

            return self._estimate_dependencies_size_via_report(
                tag, "pip", cmd, temp_root, report_file
            )

        except (OSError, RuntimeError, TypeError, ValueError) as e:
            logger.error(f"Error estimating dependency size via pip for {tag}: {e}", exc_info=True)
            return None
        finally:
            # Clean up temp dir to avoid disk bloat
            try:
                if temp_root.exists():
                    shutil.rmtree(temp_root)
            except OSError:
                pass

    def _estimate_dependencies_size_via_report(
        self, tag: str, tool_name: str, cmd: List[str], temp_root: Path, report_file: Path
    ) -> Optional[int]:
        """
        Shared helper to run a resolver command that produces a pip-compatible report.
        """
        result = subprocess.run(
            cmd, capture_output=True, text=True, timeout=600, env=self._build_pip_env()
        )

        if result.returncode != 0:
            logger.error(f"{tool_name} dry-run failed for {tag}: {result.stderr}")
            return None

        if not report_file.exists():
            logger.error(f"{tool_name} dry-run did not produce report for {tag}")
            return None

        try:
            with open(report_file, "r") as f:
                report = json.load(f)
        except (json.JSONDecodeError, OSError, ValueError) as e:
            logger.error(f"{tool_name} report parse failed for {tag}: {e}", exc_info=True)
            return None

        total_size = 0
        install_items = report.get("install", [])
        seen: Set[str] = set()

        for item in install_items:
            # Avoid double counting the same file/name/version
            name = item.get("metadata", {}).get("name") or item.get("name")
            version = item.get("metadata", {}).get("version") or item.get("version")
            download_info = item.get("download_info") or {}
            url = download_info.get("url")
            size = download_info.get("size")

            key = f"{name or ''}:{version or ''}:{url or ''}"
            if key in seen:
                continue
            seen.add(key)

            if size:
                total_size += size
                continue

            # Try HEAD on the URL if present
            if url:
                head_size = self._head_content_length(url)
                if head_size:
                    total_size += head_size
                    continue

            # Fallback: query size by resolved name/version via package_size_resolver
            if name and version:
                pkg_size = self.package_size_resolver.get_package_size(f"{name}=={version}")
                if pkg_size:
                    total_size += pkg_size
                    continue

        return total_size if total_size > 0 else None

    def _head_content_length(self, url: str) -> Optional[int]:
        """
        Perform a HEAD request to get Content-Length for a URL.
        """
        try:
            req = urllib.request.Request(url, method="HEAD")
            req.add_header("User-Agent", "ComfyUI-Version-Manager/1.0")
            with urllib.request.urlopen(req, timeout=10) as resp:
                length = resp.headers.get("Content-Length")
                if length:
                    return int(length)
        except (urllib.error.URLError, OSError, ValueError) as e:
            logger.warning(f"Warning: HEAD failed for {url}: {e}")
        return None

    def _build_pip_env(self) -> Optional[Dict[str, str]]:
        """
        Build env for pip to reuse the shared pip cache if provided.
        """
        if not self.pip_cache_dir:
            return None

        try:
            self.pip_cache_dir.mkdir(parents=True, exist_ok=True)
        except OSError as e:
            logger.warning(
                f"Warning: Could not ensure pip cache directory {self.pip_cache_dir}: {e}"
            )
            return None

        env = os.environ.copy()
        env["PIP_CACHE_DIR"] = str(self.pip_cache_dir)
        return env

    def get_size_breakdown(self, tag: str) -> Optional[Dict[str, Any]]:
        """
        Get size breakdown for display

        Args:
            tag: Release tag

        Returns:
            Dict with formatted size breakdown
        """
        cached = self.get_cached_size(tag)
        if not cached:
            return None

        total_size = cached["total_size"]
        archive_size = cached["archive_size"]
        dependencies_size = cached["dependencies_size"]

        # Calculate percentages
        archive_pct = (archive_size / total_size * 100) if total_size > 0 else 0
        deps_pct = (dependencies_size / total_size * 100) if total_size > 0 else 0

        return {
            "total_size": total_size,
            "total_size_formatted": self._format_size(total_size),
            "archive_size": archive_size,
            "archive_size_formatted": self._format_size(archive_size),
            "archive_percentage": archive_pct,
            "dependencies_size": dependencies_size,
            "dependencies_size_formatted": self._format_size(dependencies_size),
            "dependencies_percentage": deps_pct,
            "dependency_count": cached["dependency_count"],
            "unknown_count": cached.get("unknown_size_count", 0),
        }

    def _format_size(self, size_bytes: int) -> str:
        """
        Format size in human-readable format

        Args:
            size_bytes: Size in bytes

        Returns:
            Formatted string (e.g., '4.5 GB')
        """
        if size_bytes < 1024:
            return f"{size_bytes} B"
        elif size_bytes < 1024 * 1024:
            return f"{size_bytes / 1024:.1f} KB"
        elif size_bytes < 1024 * 1024 * 1024:
            return f"{size_bytes / (1024 * 1024):.1f} MB"
        else:
            return f"{size_bytes / (1024 * 1024 * 1024):.2f} GB"

    def invalidate_cache(self, tag: str):
        """
        Invalidate cache for a specific release

        Args:
            tag: Release tag
        """
        if tag in self._cache:
            del self._cache[tag]
            self._save_cache()
            logger.info(f"✓ Cache invalidated for {tag}")

    def clear_cache(self):
        """Clear all cached release sizes"""
        self._cache = {}
        self._save_cache()
        logger.info("✓ Release sizes cache cleared")

    def calculate_multiple_releases(
        self,
        releases: List[Tuple[str, int]],
        progress_callback: Optional[Callable[[int, int, str], None]] = None,
    ) -> Dict[str, Dict[str, Any]]:
        """
        Calculate sizes for multiple releases

        Args:
            releases: List of (tag, archive_size) tuples
            progress_callback: Optional callback(current, total, tag)

        Returns:
            Dict mapping tag to size data
        """
        results = {}
        total = len(releases)

        for i, (tag, archive_size) in enumerate(releases):
            if progress_callback:
                progress_callback(i + 1, total, tag)

            result = self.calculate_release_size(tag, archive_size)
            if result:
                results[tag] = result

        return results


if __name__ == "__main__":
    # Test the ReleaseSizeCalculator
    from pathlib import Path

    from backend.package_size_resolver import PackageSizeResolver
    from backend.release_data_fetcher import ReleaseDataFetcher

    test_cache_dir = Path("./test-cache")

    # Initialize components
    fetcher = ReleaseDataFetcher(test_cache_dir)
    resolver = PackageSizeResolver(test_cache_dir)
    calculator = ReleaseSizeCalculator(test_cache_dir, fetcher, resolver)

    logger.info("=== Testing ReleaseSizeCalculator ===\n")

    # Test with a release
    tag = "v0.2.7"
    archive_size = 125 * 1024 * 1024  # 125 MB estimate

    logger.info(f"Calculating size for {tag}...")
    result = calculator.calculate_release_size(tag, archive_size)

    if result:
        logger.info(f"\nTotal Size: {calculator._format_size(result['total_size'])}")
        logger.info(f"Archive: {calculator._format_size(result['archive_size'])}")
        logger.info(f"Dependencies: {calculator._format_size(result['dependencies_size'])}")
        logger.info(f"Dependency Count: {result['dependency_count']}")

        logger.info("\nTop 5 Dependencies:")
        top_deps = calculator.get_sorted_dependencies(tag, top_n=5)
        for i, dep in enumerate(top_deps, 1):
            if dep["size"]:
                size_str = calculator._format_size(dep["size"])
                logger.info(f"  {i}. {dep['package']}{dep['version_spec']} - {size_str}")

        logger.info("\nSize Breakdown:")
        breakdown = calculator.get_size_breakdown(tag)
        if breakdown:
            logger.info(
                f"  Archive: {breakdown['archive_size_formatted']} ({breakdown['archive_percentage']:.1f}%)"
            )
            logger.info(
                f"  Dependencies: {breakdown['dependencies_size_formatted']} ({breakdown['dependencies_percentage']:.1f}%)"
            )

    # Cleanup
    import shutil

    if test_cache_dir.exists():
        shutil.rmtree(test_cache_dir)
        logger.info("\n✓ Test cache cleaned up")
