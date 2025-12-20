#!/usr/bin/env python3
"""
Package Size Resolver - Phase 6.2.5a
Queries PyPI for package sizes with platform detection
"""

import json
import platform
import sys
import urllib.request
import subprocess
from pathlib import Path
from typing import Dict, Optional, Tuple
from datetime import datetime, timezone


class PackageSizeResolver:
    """Resolves package sizes from PyPI with platform-specific detection"""

    def __init__(self, cache_dir: Path):
        """
        Initialize PackageSizeResolver

        Args:
            cache_dir: Directory for cache storage
        """
        self.cache_dir = Path(cache_dir)
        self.cache_dir.mkdir(parents=True, exist_ok=True)

        self.cache_file = self.cache_dir / "package-sizes.json"
        self._cache: Dict = self._load_cache()

        # Detect platform
        self.platform = self._detect_platform()
        self.python_version = self._get_python_version()

    def _load_cache(self) -> Dict:
        """Load package sizes cache from disk"""
        if self.cache_file.exists():
            try:
                with open(self.cache_file, 'r') as f:
                    data = json.load(f)
                    return data.get('packages', {})
            except Exception as e:
                print(f"Warning: Failed to load package sizes cache: {e}")
        return {}

    def _save_cache(self):
        """Save package sizes cache to disk"""
        try:
            cache_data = {
                'packages': self._cache,
                'last_updated': self._get_iso_timestamp()
            }
            with open(self.cache_file, 'w') as f:
                json.dump(cache_data, f, indent=2)
        except Exception as e:
            print(f"Error saving package sizes cache: {e}")

    def _get_iso_timestamp(self) -> str:
        """Get current timestamp in ISO format"""
        return datetime.now(timezone.utc).isoformat()

    def _detect_platform(self) -> str:
        """
        Detect platform identifier for PyPI queries

        Returns:
            Platform string (e.g., 'linux_x86_64', 'win_amd64', 'macosx_11_0_arm64')
        """
        system = platform.system().lower()
        machine = platform.machine().lower()

        if system == 'linux':
            if 'x86_64' in machine or 'amd64' in machine:
                return 'linux_x86_64'
            elif 'aarch64' in machine or 'arm64' in machine:
                return 'linux_aarch64'
            else:
                return f'linux_{machine}'
        elif system == 'darwin':
            # macOS
            if 'arm' in machine or 'aarch64' in machine:
                return 'macosx_11_0_arm64'
            else:
                return 'macosx_10_9_x86_64'
        elif system == 'windows':
            if 'amd64' in machine or 'x86_64' in machine:
                return 'win_amd64'
            else:
                return 'win32'
        else:
            return 'any'

    def _get_python_version(self) -> str:
        """
        Get Python version string

        Returns:
            Version string (e.g., '3.11')
        """
        return f"{sys.version_info.major}.{sys.version_info.minor}"

    def _get_cache_key(self, package_spec: str) -> str:
        """
        Generate cache key for a package

        Args:
            package_spec: Package specification (e.g., 'torch==2.1.0')

        Returns:
            Cache key
        """
        return f"{package_spec}|{self.platform}|{self.python_version}"

    def query_pypi_package_size(
        self,
        package_name: str,
        version: Optional[str] = None
    ) -> Optional[Dict[str, any]]:
        """
        Query PyPI JSON API for package size

        Args:
            package_name: Package name (e.g., 'torch')
            version: Optional version (if None, uses latest)

        Returns:
            Dict with size info or None if not found
        """
        try:
            # Construct PyPI JSON API URL
            if version:
                url = f"https://pypi.org/pypi/{package_name}/{version}/json"
            else:
                url = f"https://pypi.org/pypi/{package_name}/json"

            req = urllib.request.Request(url)
            req.add_header('User-Agent', 'ComfyUI-Launcher')

            with urllib.request.urlopen(req, timeout=10) as response:
                data = json.load(response)

            # Get release info
            if version:
                release_files = data.get('urls', [])
            else:
                # Get latest version
                version = data.get('info', {}).get('version')
                releases = data.get('releases', {})
                release_files = releases.get(version, [])

            if not release_files:
                return None

            # Find best matching wheel for our platform
            best_match = self._find_best_wheel(release_files, package_name)

            if best_match:
                return {
                    'size': best_match['size'],
                    'platform': self.platform,
                    'python_version': self.python_version,
                    'checked_at': self._get_iso_timestamp(),
                    'wheel_filename': best_match['filename'],
                    'resolved_version': version
                }

            return None

        except urllib.error.HTTPError as e:
            if e.code == 404:
                print(f"Package {package_name} not found on PyPI (404)")
            else:
                print(f"HTTP error querying PyPI for {package_name}: {e}")
            return None
        except Exception as e:
            print(f"Error querying PyPI for {package_name}: {e}")
            return None

    def _find_best_wheel(
        self,
        release_files: list,
        package_name: str
    ) -> Optional[Dict]:
        """
        Find best matching wheel file for current platform

        Args:
            release_files: List of release files from PyPI
            package_name: Package name

        Returns:
            Best matching file dict or None
        """
        # Filter for wheels
        wheels = [f for f in release_files if f['packagetype'] == 'bdist_wheel']

        if not wheels:
            # Try source distribution as fallback
            sdists = [f for f in release_files if f['packagetype'] == 'sdist']
            if sdists:
                # Return the largest sdist (most complete)
                return max(sdists, key=lambda f: f['size'])
            return None

        # Try to find platform-specific wheel
        platform_wheels = [
            w for w in wheels
            if self.platform in w['filename'] or 'any' in w['filename']
        ]

        if platform_wheels:
            # Return the largest matching wheel
            return max(platform_wheels, key=lambda w: w['size'])

        # Fallback to any wheel
        return max(wheels, key=lambda w: w['size'])

    def get_package_size(
        self,
        package_spec: str,
        force_refresh: bool = False
    ) -> Optional[int]:
        """
        Get size of a package in bytes

        Args:
            package_spec: Package specification (e.g., 'torch==2.1.0' or 'torch>=2.0.0')
            force_refresh: Force re-query PyPI

        Returns:
            Size in bytes or None if not found
        """
        cache_key = self._get_cache_key(package_spec)

        # Check cache
        if not force_refresh and cache_key in self._cache:
            return self._cache[cache_key].get('size')

        # Parse package spec
        package_name, version = self._parse_package_spec(package_spec)

        # Query PyPI
        result = self.query_pypi_package_size(package_name, version)

        if result:
            self._cache[cache_key] = result
            self._save_cache()
            return result['size']

        # Fallback to UV dry-run if PyPI query fails
        return self._fallback_uv_dry_run(package_spec)

    def _parse_package_spec(self, package_spec: str) -> Tuple[str, Optional[str]]:
        """
        Parse package specification into name and version

        Args:
            package_spec: Package spec (e.g., 'torch==2.1.0')

        Returns:
            Tuple of (package_name, version)
        """
        # Handle various operators
        for op in ['==', '>=', '<=', '~=', '>', '<', '!=']:
            if op in package_spec:
                parts = package_spec.split(op, 1)
                package_name = parts[0].strip()

                # For exact version (==), use it; otherwise None for latest
                if op == '==':
                    version = parts[1].strip()
                else:
                    version = None

                return (package_name, version)

        # No version specified
        return (package_spec.strip(), None)

    def _fallback_uv_dry_run(self, package_spec: str) -> Optional[int]:
        """
        Fallback to UV dry-run to estimate package size

        Args:
            package_spec: Package specification

        Returns:
            Estimated size in bytes or None
        """
        try:
            # Use UV with --dry-run to see what would be installed
            result = subprocess.run(
                ['uv', 'pip', 'install', '--dry-run', package_spec],
                capture_output=True,
                text=True,
                timeout=30
            )

            if result.returncode == 0:
                # UV output doesn't directly show sizes, so this is limited
                # Return a conservative estimate
                print(f"Warning: Using UV dry-run fallback for {package_spec}")
                return None

        except Exception as e:
            print(f"UV dry-run fallback failed for {package_spec}: {e}")

        return None

    def get_multiple_package_sizes(
        self,
        package_specs: list,
        progress_callback: Optional[callable] = None
    ) -> Dict[str, Optional[int]]:
        """
        Get sizes for multiple packages

        Args:
            package_specs: List of package specifications
            progress_callback: Optional callback(current, total, package)

        Returns:
            Dict mapping package spec to size in bytes
        """
        results = {}
        total = len(package_specs)

        for i, spec in enumerate(package_specs):
            if progress_callback:
                progress_callback(i + 1, total, spec)

            size = self.get_package_size(spec)
            results[spec] = size

        return results

    def clear_cache(self):
        """Clear all cached package sizes"""
        self._cache = {}
        self._save_cache()
        print("✓ Package sizes cache cleared")


if __name__ == "__main__":
    # Test the PackageSizeResolver
    from pathlib import Path

    test_cache_dir = Path("./test-cache")
    resolver = PackageSizeResolver(test_cache_dir)

    print("=== Testing PackageSizeResolver ===\n")
    print(f"Platform: {resolver.platform}")
    print(f"Python: {resolver.python_version}\n")

    # Test querying a package
    test_packages = ['pillow', 'numpy', 'torch==2.1.0']

    for pkg in test_packages:
        print(f"Querying {pkg}...")
        size = resolver.get_package_size(pkg)
        if size:
            size_mb = size / (1024 * 1024)
            print(f"  ✓ Size: {size_mb:.2f} MB ({size:,} bytes)")
        else:
            print(f"  ✗ Size not found")
        print()

    # Cleanup
    import shutil
    if test_cache_dir.exists():
        shutil.rmtree(test_cache_dir)
        print("✓ Test cache cleaned up")
