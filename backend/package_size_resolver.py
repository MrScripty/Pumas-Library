#!/usr/bin/env python3
"""
Package Size Resolver - Phase 6.2.5a
Queries PyPI for package sizes with platform detection
"""

import json
import platform
import sys
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Dict, List, Optional, Set, Tuple

from packaging.markers import default_environment
from packaging.requirements import Requirement
from packaging.version import InvalidVersion, Version

from backend.logging_config import get_logger

logger = get_logger(__name__)


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
                with open(self.cache_file, "r") as f:
                    data = json.load(f)
                    return data.get("packages", {})
            except Exception as e:
                logger.warning(f"Warning: Failed to load package sizes cache: {e}")
        return {}

    def _save_cache(self):
        """Save package sizes cache to disk"""
        try:
            cache_data = {"packages": self._cache, "last_updated": self._get_iso_timestamp()}
            with open(self.cache_file, "w") as f:
                json.dump(cache_data, f, indent=2)
        except Exception as e:
            logger.error(f"Error saving package sizes cache: {e}", exc_info=True)

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

        if system == "linux":
            if "x86_64" in machine or "amd64" in machine:
                return "linux_x86_64"
            elif "aarch64" in machine or "arm64" in machine:
                return "linux_aarch64"
            else:
                return f"linux_{machine}"
        elif system == "darwin":
            # macOS
            if "arm" in machine or "aarch64" in machine:
                return "macosx_11_0_arm64"
            else:
                return "macosx_10_9_x86_64"
        elif system == "windows":
            if "amd64" in machine or "x86_64" in machine:
                return "win_amd64"
            else:
                return "win32"
        else:
            return "any"

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
        normalized_spec = self._normalize_package_spec(package_spec)
        return f"{normalized_spec}|{self.platform}|{self.python_version}"

    def query_pypi_package_size(
        self, package_name: str, version: Optional[str] = None, specifier: Optional[str] = None
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
            req.add_header("User-Agent", "ComfyUI-Launcher")

            with urllib.request.urlopen(req, timeout=10) as response:
                data = json.load(response)

            # Get release info
            if version:
                release_files = data.get("urls", [])
            else:
                releases = data.get("releases", {})

                if specifier:
                    # Select the latest version that satisfies the specifier
                    requirement = Requirement(f"{package_name}{specifier}")
                    spec_set = requirement.specifier
                    matching_versions = []
                    for version_str in releases.keys():
                        try:
                            parsed_version = Version(version_str)
                        except InvalidVersion:
                            continue
                        if parsed_version in spec_set:
                            matching_versions.append(parsed_version)

                    if matching_versions:
                        selected_version = str(max(matching_versions))
                        version = selected_version
                        release_files = releases.get(version, [])
                    else:
                        # Fall back to the latest if no matching versions found
                        version = data.get("info", {}).get("version")
                        release_files = releases.get(version, [])
                else:
                    # Get latest version
                    version = data.get("info", {}).get("version")
                    release_files = releases.get(version, [])

            if not release_files:
                return None

            # Ensure we use correct metadata for a non-latest version
            requires_dist = data.get("info", {}).get("requires_dist") or []
            if version and version != data.get("info", {}).get("version"):
                version_data = self._fetch_pypi_version_data(package_name, version)
                if version_data:
                    requires_dist = version_data.get("info", {}).get("requires_dist") or []
                    release_files = version_data.get("urls", release_files)

            # Find best matching wheel for our platform
            best_match = self._find_best_wheel(release_files, package_name)

            if best_match:
                dependencies = self._parse_requires_dist(requires_dist)
                return {
                    "size": best_match["size"],
                    "dependencies": dependencies,
                    "platform": self.platform,
                    "python_version": self.python_version,
                    "checked_at": self._get_iso_timestamp(),
                    "wheel_filename": best_match["filename"],
                    "resolved_version": version,
                }

            return None

        except urllib.error.HTTPError as e:
            if e.code == 404:
                logger.warning(f"Package {package_name} not found on PyPI (404)")
            else:
                logger.error(f"HTTP error querying PyPI for {package_name}: {e}", exc_info=True)
            return None
        except Exception as e:
            logger.error(f"Error querying PyPI for {package_name}: {e}", exc_info=True)
            return None

    def _find_best_wheel(self, release_files: list, package_name: str) -> Optional[Dict]:
        """
        Find best matching wheel file for current platform

        Args:
            release_files: List of release files from PyPI
            package_name: Package name

        Returns:
            Best matching file dict or None
        """
        # Filter for wheels
        wheels = [f for f in release_files if f["packagetype"] == "bdist_wheel"]

        if not wheels:
            # Try source distribution as fallback
            sdists = [f for f in release_files if f["packagetype"] == "sdist"]
            if sdists:
                # Return the largest sdist (most complete)
                return max(sdists, key=lambda f: f["size"])
            return None

        # Try to find platform-specific wheel
        platform_wheels = [
            w for w in wheels if self.platform in w["filename"] or "any" in w["filename"]
        ]

        if platform_wheels:
            # Return the largest matching wheel
            return max(platform_wheels, key=lambda w: w["size"])

        # Fallback to any wheel
        return max(wheels, key=lambda w: w["size"])

    def _fetch_pypi_version_data(self, package_name: str, version: str) -> Optional[Dict[str, any]]:
        """
        Fetch PyPI JSON metadata for a specific version.
        """
        try:
            url = f"https://pypi.org/pypi/{package_name}/{version}/json"
            req = urllib.request.Request(url)
            req.add_header("User-Agent", "ComfyUI-Launcher")
            with urllib.request.urlopen(req, timeout=10) as response:
                return json.load(response)
        except Exception as e:
            logger.warning(
                f"Warning: Failed to fetch PyPI metadata for {package_name} {version}: {e}"
            )
            return None

    def get_package_metadata(
        self, package_spec: str, force_refresh: bool = False
    ) -> Optional[Dict[str, any]]:
        """
        Get cached or fresh package metadata including size and dependencies.

        Args:
            package_spec: Package specification (e.g., 'torch==2.1.0')
            force_refresh: Force re-query PyPI

        Returns:
            Metadata dict or None if not found
        """
        cache_key = self._get_cache_key(package_spec)

        # Check cache
        if not force_refresh and cache_key in self._cache:
            cached = self._cache[cache_key]
            if cached and "dependencies" in cached:
                return cached
            # Refresh stale cache entries that are missing dependency data
            force_refresh = True

        # Parse package spec
        package_name, version = self._parse_package_spec(package_spec)
        specifier = None
        try:
            requirement = Requirement(package_spec)
            specifier = str(requirement.specifier) if requirement.specifier else None
        except Exception:
            pass

        # Query PyPI
        result = self.query_pypi_package_size(package_name, version, specifier)

        if result:
            self._cache[cache_key] = result
            self._save_cache()
            return result

        return None

    def get_package_size(self, package_spec: str, force_refresh: bool = False) -> Optional[int]:
        """
        Get size of a package in bytes (no dependency expansion).
        """
        metadata = self.get_package_metadata(package_spec, force_refresh)
        if metadata:
            return metadata.get("size")
        return None

    def get_package_total_size(
        self, package_spec: str, seen: Optional[Set[str]] = None, force_refresh: bool = False
    ) -> Optional[int]:
        """
        Get total download size for a package including its transitive dependencies.

        Args:
            package_spec: Package specification
            seen: Set used to avoid double-counting packages
            force_refresh: Force re-query PyPI metadata

        Returns:
            Total size in bytes or None if unresolved
        """
        if seen is None:
            seen = set()

        key = self._normalize_package_key(package_spec)
        if key in seen:
            return 0

        metadata = self.get_package_metadata(package_spec, force_refresh)
        if not metadata or metadata.get("size") is None:
            return None

        seen.add(key)
        total = metadata["size"]

        for dep_spec in metadata.get("dependencies", []):
            dep_size = self.get_package_total_size(dep_spec, seen, force_refresh)
            if dep_size is not None:
                total += dep_size

        return total

    def _parse_package_spec(self, package_spec: str) -> Tuple[str, Optional[str]]:
        """
        Parse package specification into name and version.

        Args:
            package_spec: Package spec (e.g., 'torch==2.1.0')

        Returns:
            Tuple of (package_name, version)
        """
        try:
            requirement = Requirement(package_spec)
            package_name = requirement.name
            version = None
            specifiers = list(requirement.specifier)
            if (
                len(specifiers) == 1
                and specifiers[0].operator == "=="
                and "*" not in specifiers[0].version
            ):
                version = specifiers[0].version
            return (package_name, version)
        except Exception:
            # Handle various operators in raw specs as a fallback
            for op in ["==", ">=", "<=", "~=", ">", "<", "!="]:
                if op in package_spec:
                    parts = package_spec.split(op, 1)
                    package_name = parts[0].strip()

                    # For exact version (==), use it; otherwise None for latest
                    if op == "==":
                        version = parts[1].strip()
                    else:
                        version = None

                    return (package_name, version)

            # No version specified
            return (package_spec.strip(), None)

    def _normalize_package_spec(self, package_spec: str) -> str:
        """
        Normalize a package spec for consistent caching.
        """
        try:
            requirement = Requirement(package_spec)
            extras = f"[{','.join(sorted(requirement.extras))}]" if requirement.extras else ""
            spec = str(requirement.specifier) if requirement.specifier else ""
            return f"{requirement.name}{extras}{spec}"
        except Exception:
            return package_spec.strip()

    def _normalize_package_key(self, package_spec: str) -> str:
        """
        Normalize a package spec for de-duplication in dependency graphs.

        Args:
            package_spec: Package specification string

        Returns:
            Lowercase package name for use as a key
        """
        name, _version = self._parse_package_spec(package_spec)
        return name.lower()

    def _parse_requires_dist(self, requires_dist: List[str]) -> List[str]:
        """
        Parse requires_dist entries from PyPI metadata into package specs.
        Environment markers are evaluated against the current platform and skipped when not matched.

        Args:
            requires_dist: Raw requires_dist list from PyPI JSON

        Returns:
            List of package specs (e.g., ['torch==2.3.1', 'numpy>=1.25.0'])
        """
        dependencies: List[str] = []
        env = default_environment()

        for entry in requires_dist:
            try:
                req = Requirement(entry)
                if req.marker and not req.marker.evaluate(env):
                    continue

                spec = str(req.specifier) if req.specifier else ""
                dependencies.append(f"{req.name}{spec}")
            except Exception as e:
                logger.warning(f"Warning: Failed to parse requires_dist entry '{entry}': {e}")
                continue

        return dependencies

    def get_multiple_package_sizes(
        self, package_specs: list, progress_callback: Optional[callable] = None
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
        logger.info("✓ Package sizes cache cleared")


if __name__ == "__main__":
    # Test the PackageSizeResolver
    from pathlib import Path

    test_cache_dir = Path("./test-cache")
    resolver = PackageSizeResolver(test_cache_dir)

    logger.info("=== Testing PackageSizeResolver ===\n")
    logger.info(f"Platform: {resolver.platform}")
    logger.info(f"Python: {resolver.python_version}\n")

    # Test querying a package
    test_packages = ["pillow", "numpy", "torch==2.1.0"]

    for pkg in test_packages:
        logger.info(f"Querying {pkg}...")
        size = resolver.get_package_size(pkg)
        if size:
            size_mb = size / (1024 * 1024)
            logger.info(f"  ✓ Size: {size_mb:.2f} MB ({size:,} bytes)")
        else:
            logger.info(f"  ✗ Size not found")
        logger.info("")

    # Cleanup
    import shutil

    if test_cache_dir.exists():
        shutil.rmtree(test_cache_dir)
        logger.info("✓ Test cache cleaned up")
