#!/usr/bin/env python3
"""
Release Data Fetcher - Phase 6.2.5a
Fetches and caches requirements.txt files for GitHub releases
"""

import hashlib
import json
import urllib.request
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Dict, List, Optional

from backend.logging_config import get_logger

logger = get_logger(__name__)


class ReleaseDataFetcher:
    """Fetches and caches requirements.txt data for ComfyUI releases"""

    def __init__(self, cache_dir: Path):
        """
        Initialize ReleaseDataFetcher

        Args:
            cache_dir: Directory for cache storage
        """
        self.cache_dir = Path(cache_dir)
        self.cache_dir.mkdir(parents=True, exist_ok=True)

        self.requirements_cache_file = self.cache_dir / "release-requirements.json"
        self._cache: Dict[str, Dict[str, Any]] = self._load_cache()

    def _load_cache(self) -> Dict[str, Dict[str, Any]]:
        """Load requirements cache from disk"""
        if self.requirements_cache_file.exists():
            try:
                with open(self.requirements_cache_file, "r") as f:
                    data = json.load(f)
                    if isinstance(data, dict):
                        cleaned: Dict[str, Dict[str, Any]] = {}
                        for key, value in data.items():
                            if isinstance(key, str) and isinstance(value, dict):
                                cleaned[key] = value
                        return cleaned
            except OSError as e:
                logger.warning(f"Warning: Failed to load requirements cache: {e}")
            except json.JSONDecodeError as e:
                logger.warning(f"Warning: Failed to load requirements cache: {e}")
        return {}

    def _save_cache(self):
        """Save requirements cache to disk"""
        try:
            with open(self.requirements_cache_file, "w") as f:
                json.dump(self._cache, f, indent=2)
        except OSError as e:
            logger.error(f"Error saving requirements cache: {e}", exc_info=True)
        except TypeError as e:
            logger.error(f"Error saving requirements cache: {e}", exc_info=True)
        except ValueError as e:
            logger.error(f"Error saving requirements cache: {e}", exc_info=True)

    def _compute_hash(self, content: str) -> str:
        """Compute SHA256 hash of content"""
        return hashlib.sha256(content.encode("utf-8")).hexdigest()

    def _get_iso_timestamp(self) -> str:
        """Get current timestamp in ISO format"""
        return datetime.now(timezone.utc).isoformat()

    def fetch_requirements_for_release(
        self, tag: str, force_refresh: bool = False
    ) -> Optional[Dict[str, Any]]:
        """
        Fetch requirements.txt for a specific release

        Args:
            tag: Release tag (e.g., 'v0.2.0')
            force_refresh: Force re-fetch even if cached

        Returns:
            Dict with requirements data or None if not found
        """
        # Check cache first
        if not force_refresh and tag in self._cache:
            return self._cache[tag]

        # Construct GitHub raw URL for requirements.txt
        # Format: https://raw.githubusercontent.com/comfyanonymous/ComfyUI/{tag}/requirements.txt
        url = f"https://raw.githubusercontent.com/comfyanonymous/ComfyUI/{tag}/requirements.txt"

        try:
            logger.info(f"Fetching requirements.txt for {tag}...")
            req = urllib.request.Request(url)
            req.add_header("User-Agent", "ComfyUI-Launcher")

            with urllib.request.urlopen(req, timeout=10) as response:
                requirements_txt = response.read().decode("utf-8")

            # Compute hash
            requirements_hash = f"sha256:{self._compute_hash(requirements_txt)}"

            # Store in cache
            cache_entry = {
                "requirements_txt": requirements_txt,
                "requirements_hash": requirements_hash,
                "fetched_at": self._get_iso_timestamp(),
                "source_url": url,
            }

            self._cache[tag] = cache_entry
            self._save_cache()

            logger.info(f"✓ Cached requirements.txt for {tag}")
            return cache_entry

        except urllib.error.HTTPError as e:
            if e.code == 404:
                logger.warning(f"No requirements.txt found for {tag} (404)")
            else:
                logger.error(f"HTTP error fetching requirements for {tag}: {e}", exc_info=True)
            return None
        except urllib.error.URLError as e:
            logger.error(f"Error fetching requirements for {tag}: {e}", exc_info=True)
            return None
        except OSError as e:
            logger.error(f"Error fetching requirements for {tag}: {e}", exc_info=True)
            return None
        except UnicodeDecodeError as e:
            logger.error(f"Error fetching requirements for {tag}: {e}", exc_info=True)
            return None

    def fetch_requirements_for_releases(
        self,
        tags: List[str],
        progress_callback: Optional[Callable[[int, int, str], None]] = None,
    ) -> Dict[str, Dict[str, Any]]:
        """
        Fetch requirements.txt for multiple releases (background task)

        Args:
            tags: List of release tags, should be sorted newest → oldest
            progress_callback: Optional callback(current, total, tag)

        Returns:
            Dict mapping tag to requirements data
        """
        results = {}
        total = len(tags)

        for i, tag in enumerate(tags):
            if progress_callback:
                progress_callback(i + 1, total, tag)

            result = self.fetch_requirements_for_release(tag)
            if result:
                results[tag] = result

        return results

    def get_cached_requirements(self, tag: str) -> Optional[Dict[str, Any]]:
        """
        Get cached requirements without fetching

        Args:
            tag: Release tag

        Returns:
            Cached requirements data or None
        """
        return self._cache.get(tag)

    def parse_requirements(self, requirements_txt: str) -> Dict[str, str]:
        """
        Parse requirements.txt into package dictionary

        Args:
            requirements_txt: Contents of requirements.txt

        Returns:
            Dict mapping package name to version spec
        """
        requirements = {}

        for line in requirements_txt.split("\n"):
            line = line.strip()

            # Skip empty lines and comments
            if not line or line.startswith("#"):
                continue

            # Skip -r and other pip directives
            if line.startswith("-"):
                continue

            # Parse package spec (handle ==, >=, <=, >, <, ~=, etc.)
            for op in ["==", ">=", "<=", "~=", ">", "<"]:
                if op in line:
                    package, version = line.split(op, 1)
                    requirements[package.strip()] = f"{op}{version.strip()}"
                    break
            else:
                # No version specifier
                requirements[line.strip()] = ""

        return requirements

    def get_package_list(self, tag: str) -> List[str]:
        """
        Get list of package names for a release

        Args:
            tag: Release tag

        Returns:
            List of package names
        """
        cached = self.get_cached_requirements(tag)
        if not cached:
            return []

        requirements = self.parse_requirements(cached["requirements_txt"])
        return list(requirements.keys())

    def clear_cache(self):
        """Clear all cached requirements data"""
        self._cache = {}
        self._save_cache()
        logger.info("✓ Requirements cache cleared")


if __name__ == "__main__":
    # Test the ReleaseDataFetcher
    from pathlib import Path

    test_cache_dir = Path("./test-cache")
    fetcher = ReleaseDataFetcher(test_cache_dir)

    # Test fetching a single release
    logger.info("=== Testing ReleaseDataFetcher ===\n")

    result = fetcher.fetch_requirements_for_release("v0.2.7")
    if result:
        logger.info(f"\nFetched requirements for v0.2.7:")
        logger.info(f"Hash: {result['requirements_hash']}")
        logger.info(f"Fetched at: {result['fetched_at']}")

        packages = fetcher.parse_requirements(result["requirements_txt"])
        logger.info(f"\nPackages ({len(packages)}):")
        for pkg, version in list(packages.items())[:5]:
            logger.info(f"  - {pkg}{version}")
        if len(packages) > 5:
            logger.info(f"  ... and {len(packages) - 5} more")

    # Test cached retrieval
    logger.info("\n\nTesting cached retrieval...")
    cached = fetcher.get_cached_requirements("v0.2.7")
    if cached:
        logger.info("✓ Successfully retrieved from cache")

    # Cleanup
    import shutil

    if test_cache_dir.exists():
        shutil.rmtree(test_cache_dir)
        logger.info("\n✓ Test cache cleaned up")
