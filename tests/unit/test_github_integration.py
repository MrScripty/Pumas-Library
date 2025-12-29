"""
Unit tests for GitHub release fetching and collapse logic.
"""

from pathlib import Path

import pytest

from backend.github_integration import GitHubReleasesFetcher
from backend.metadata_manager import MetadataManager


@pytest.fixture
def fetcher(temp_metadata_dir):
    """Create a GitHubReleasesFetcher with temporary storage."""
    metadata_mgr = MetadataManager(temp_metadata_dir)
    return GitHubReleasesFetcher(metadata_mgr)


@pytest.mark.unit
class TestGitHubReleaseCollapse:
    """Tests for release collapse functionality (latest patch per minor)."""

    def test_collapse_picks_latest_patch_per_minor(self, fetcher):
        """Test that collapse_latest_patch_per_minor picks latest patch for each minor version."""
        releases = [
            {"tag_name": "v0.5.1", "prerelease": False},
            {"tag_name": "v0.5.0", "prerelease": False},
            {"tag_name": "v0.4.0", "prerelease": False},
            {"tag_name": "v0.3.75", "prerelease": False},
            {"tag_name": "v0.3.74", "prerelease": False},
        ]

        collapsed = fetcher.collapse_latest_patch_per_minor(releases, include_prerelease=True)
        tags = [r["tag_name"] for r in collapsed]

        assert tags == ["v0.5.1", "v0.4.0", "v0.3.75"]

    def test_prerelease_excluded_when_not_requested(self, fetcher):
        """Test that prerelease versions are excluded when include_prerelease=False."""
        releases = [
            {"tag_name": "v0.3.1-rc1", "prerelease": True},
            {"tag_name": "v0.3.0", "prerelease": False},
        ]

        collapsed = fetcher.collapse_latest_patch_per_minor(releases, include_prerelease=False)
        tags = [r["tag_name"] for r in collapsed]

        assert tags == ["v0.3.0"]

    def test_invalid_versions_are_skipped_safely(self, fetcher):
        """Test that invalid version tags are skipped without breaking processing."""
        releases = [
            {"tag_name": "v0.2.1", "prerelease": False},
            {"tag_name": "invalid-tag", "prerelease": False},
            {"tag_name": "v0.2.0", "prerelease": False},
        ]

        collapsed = fetcher.collapse_latest_patch_per_minor(releases, include_prerelease=True)
        tags = [r["tag_name"] for r in collapsed]

        # invalid tag should be ignored but not break others
        assert tags == ["v0.2.1"]

    def test_empty_releases_list(self, fetcher):
        """Test that empty releases list returns empty result."""
        collapsed = fetcher.collapse_latest_patch_per_minor([], include_prerelease=True)
        assert collapsed == []

    def test_single_release(self, fetcher):
        """Test with a single release."""
        releases = [{"tag_name": "v1.0.0", "prerelease": False}]
        collapsed = fetcher.collapse_latest_patch_per_minor(releases, include_prerelease=True)
        assert len(collapsed) == 1
        assert collapsed[0]["tag_name"] == "v1.0.0"

    def test_only_prereleases(self, fetcher):
        """Test behavior when all releases are prereleases."""
        releases = [
            {"tag_name": "v0.2.0-beta1", "prerelease": True},
            {"tag_name": "v0.1.0-alpha", "prerelease": True},
        ]

        # With prerelease included
        collapsed_with = fetcher.collapse_latest_patch_per_minor(releases, include_prerelease=True)
        assert len(collapsed_with) == 2

        # Without prerelease - should return empty
        collapsed_without = fetcher.collapse_latest_patch_per_minor(
            releases, include_prerelease=False
        )
        assert collapsed_without == []
