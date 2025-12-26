#!/usr/bin/env python3
"""
Unit tests for GitHub release collapse logic (latest patch per minor).
"""

import tempfile
import unittest
from pathlib import Path

from backend.github_integration import GitHubReleasesFetcher
from backend.metadata_manager import MetadataManager


def build_fetcher(tmp_path: Path) -> GitHubReleasesFetcher:
    """Create a fetcher with an isolated metadata directory."""
    metadata_mgr = MetadataManager(tmp_path / "launcher-data")
    return GitHubReleasesFetcher(metadata_mgr)


class GitHubReleaseCollapseTests(unittest.TestCase):
    def test_collapse_picks_latest_patch_per_minor(self):
        fetcher = build_fetcher(Path(tempfile.mkdtemp()))

        releases = [
            {"tag_name": "v0.5.1", "prerelease": False},
            {"tag_name": "v0.5.0", "prerelease": False},
            {"tag_name": "v0.4.0", "prerelease": False},
            {"tag_name": "v0.3.75", "prerelease": False},
            {"tag_name": "v0.3.74", "prerelease": False},
        ]

        collapsed = fetcher.collapse_latest_patch_per_minor(releases, include_prerelease=True)
        tags = [r["tag_name"] for r in collapsed]

        self.assertEqual(tags, ["v0.5.1", "v0.4.0", "v0.3.75"])

    def test_prerelease_excluded_when_not_requested(self):
        fetcher = build_fetcher(Path(tempfile.mkdtemp()))

        releases = [
            {"tag_name": "v0.3.1-rc1", "prerelease": True},
            {"tag_name": "v0.3.0", "prerelease": False},
        ]

        collapsed = fetcher.collapse_latest_patch_per_minor(releases, include_prerelease=False)
        tags = [r["tag_name"] for r in collapsed]

        self.assertEqual(tags, ["v0.3.0"])

    def test_invalid_versions_are_skipped_safely(self):
        fetcher = build_fetcher(Path(tempfile.mkdtemp()))

        releases = [
            {"tag_name": "v0.2.1", "prerelease": False},
            {"tag_name": "invalid-tag", "prerelease": False},
            {"tag_name": "v0.2.0", "prerelease": False},
        ]

        collapsed = fetcher.collapse_latest_patch_per_minor(releases, include_prerelease=True)
        tags = [r["tag_name"] for r in collapsed]

        # invalid tag should be ignored but not break others
        self.assertEqual(tags, ["v0.2.1"])


if __name__ == "__main__":
    unittest.main()
