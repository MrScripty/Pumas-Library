#!/usr/bin/env python3
"""
Release Size Calculator - Phase 6.2.5a
Calculates total size per release (archive + dependencies)
"""

import json
from pathlib import Path
from typing import Dict, List, Optional, Tuple
from datetime import datetime, timezone


class ReleaseSizeCalculator:
    """Calculates and caches total sizes for ComfyUI releases"""

    def __init__(
        self,
        cache_dir: Path,
        release_data_fetcher,
        package_size_resolver
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

        self._cache: Dict = self._load_cache()

    def _load_cache(self) -> Dict:
        """Load release sizes cache from disk"""
        if self.cache_file.exists():
            try:
                with open(self.cache_file, 'r') as f:
                    return json.load(f)
            except Exception as e:
                print(f"Warning: Failed to load release sizes cache: {e}")
        return {}

    def _save_cache(self):
        """Save release sizes cache to disk"""
        try:
            with open(self.cache_file, 'w') as f:
                json.dump(self._cache, f, indent=2)
        except Exception as e:
            print(f"Error saving release sizes cache: {e}")

    def _get_iso_timestamp(self) -> str:
        """Get current timestamp in ISO format"""
        return datetime.now(timezone.utc).isoformat()

    def calculate_release_size(
        self,
        tag: str,
        archive_size: int,
        force_refresh: bool = False
    ) -> Optional[Dict[str, any]]:
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
        requirements_data = self.release_data_fetcher.get_cached_requirements(tag)

        if not requirements_data:
            # Try to fetch if not cached
            requirements_data = self.release_data_fetcher.fetch_requirements_for_release(tag)

        if not requirements_data:
            return None

        # Check if we need to recalculate
        requirements_hash = requirements_data['requirements_hash']
        cache_key = tag

        if not force_refresh and cache_key in self._cache:
            cached = self._cache[cache_key]
            # Validate cache is for same requirements
            if cached.get('requirements_hash') == requirements_hash:
                return cached

        # Parse requirements
        requirements = self.release_data_fetcher.parse_requirements(
            requirements_data['requirements_txt']
        )

        # Get sizes for all dependencies
        print(f"Calculating sizes for {len(requirements)} dependencies of {tag}...")

        dependency_sizes = []
        total_dependencies_size = 0
        unknown_count = 0

        for package, version_spec in requirements.items():
            package_spec = f"{package}{version_spec}" if version_spec else package

            size = self.package_size_resolver.get_package_size(package_spec)

            if size is not None:
                dependency_sizes.append({
                    'package': package,
                    'version_spec': version_spec,
                    'size': size
                })
                total_dependencies_size += size
            else:
                unknown_count += 1
                dependency_sizes.append({
                    'package': package,
                    'version_spec': version_spec,
                    'size': None
                })

        # Sort by size (largest first), None values at end
        dependency_sizes.sort(
            key=lambda x: x['size'] if x['size'] is not None else -1,
            reverse=True
        )

        # Calculate total size
        total_size = archive_size + total_dependencies_size

        # Build result
        result = {
            'tag': tag,
            'total_size': total_size,
            'archive_size': archive_size,
            'dependencies_size': total_dependencies_size,
            'dependency_count': len(requirements),
            'unknown_size_count': unknown_count,
            'dependencies': dependency_sizes,
            'requirements_hash': requirements_hash,
            'calculated_at': self._get_iso_timestamp(),
            'platform': self.package_size_resolver.platform,
            'python_version': self.package_size_resolver.python_version
        }

        # Cache the result
        self._cache[cache_key] = result
        self._save_cache()

        print(f"✓ Calculated total size for {tag}: {self._format_size(total_size)}")
        return result

    def get_cached_size(self, tag: str) -> Optional[Dict[str, any]]:
        """
        Get cached size data for a release

        Args:
            tag: Release tag

        Returns:
            Cached size data or None
        """
        return self._cache.get(tag)

    def get_sorted_dependencies(
        self,
        tag: str,
        top_n: Optional[int] = None
    ) -> List[Dict[str, any]]:
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

        dependencies = cached.get('dependencies', [])

        if top_n:
            return dependencies[:top_n]

        return dependencies

    def get_size_breakdown(self, tag: str) -> Optional[Dict[str, any]]:
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

        total_size = cached['total_size']
        archive_size = cached['archive_size']
        dependencies_size = cached['dependencies_size']

        # Calculate percentages
        archive_pct = (archive_size / total_size * 100) if total_size > 0 else 0
        deps_pct = (dependencies_size / total_size * 100) if total_size > 0 else 0

        return {
            'total_size': total_size,
            'total_size_formatted': self._format_size(total_size),
            'archive_size': archive_size,
            'archive_size_formatted': self._format_size(archive_size),
            'archive_percentage': archive_pct,
            'dependencies_size': dependencies_size,
            'dependencies_size_formatted': self._format_size(dependencies_size),
            'dependencies_percentage': deps_pct,
            'dependency_count': cached['dependency_count'],
            'unknown_count': cached.get('unknown_size_count', 0)
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
            print(f"✓ Cache invalidated for {tag}")

    def clear_cache(self):
        """Clear all cached release sizes"""
        self._cache = {}
        self._save_cache()
        print("✓ Release sizes cache cleared")

    def calculate_multiple_releases(
        self,
        releases: List[Tuple[str, int]],
        progress_callback: Optional[callable] = None
    ) -> Dict[str, Dict]:
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
    from backend.release_data_fetcher import ReleaseDataFetcher
    from backend.package_size_resolver import PackageSizeResolver

    test_cache_dir = Path("./test-cache")

    # Initialize components
    fetcher = ReleaseDataFetcher(test_cache_dir)
    resolver = PackageSizeResolver(test_cache_dir)
    calculator = ReleaseSizeCalculator(test_cache_dir, fetcher, resolver)

    print("=== Testing ReleaseSizeCalculator ===\n")

    # Test with a release
    tag = "v0.2.7"
    archive_size = 125 * 1024 * 1024  # 125 MB estimate

    print(f"Calculating size for {tag}...")
    result = calculator.calculate_release_size(tag, archive_size)

    if result:
        print(f"\nTotal Size: {calculator._format_size(result['total_size'])}")
        print(f"Archive: {calculator._format_size(result['archive_size'])}")
        print(f"Dependencies: {calculator._format_size(result['dependencies_size'])}")
        print(f"Dependency Count: {result['dependency_count']}")

        print("\nTop 5 Dependencies:")
        top_deps = calculator.get_sorted_dependencies(tag, top_n=5)
        for i, dep in enumerate(top_deps, 1):
            if dep['size']:
                size_str = calculator._format_size(dep['size'])
                print(f"  {i}. {dep['package']}{dep['version_spec']} - {size_str}")

        print("\nSize Breakdown:")
        breakdown = calculator.get_size_breakdown(tag)
        if breakdown:
            print(f"  Archive: {breakdown['archive_size_formatted']} ({breakdown['archive_percentage']:.1f}%)")
            print(f"  Dependencies: {breakdown['dependencies_size_formatted']} ({breakdown['dependencies_percentage']:.1f}%)")

    # Cleanup
    import shutil
    if test_cache_dir.exists():
        shutil.rmtree(test_cache_dir)
        print("\n✓ Test cache cleaned up")
