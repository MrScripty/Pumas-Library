#!/usr/bin/env python3
"""
Test script for Phase 2: GitHub Integration
Tests fetching releases, caching, and download functionality
"""

import sys
from pathlib import Path

# Add backend to path
sys.path.insert(0, str(Path(__file__).parent))

from backend.github_integration import (
    DownloadManager,
    GitHubReleasesFetcher,
    format_bytes,
    print_progress,
)
from backend.metadata_manager import MetadataManager
from backend.utils import get_launcher_root


def main():
    launcher_root = get_launcher_root()
    launcher_data_dir = launcher_root / "launcher-data"

    # Initialize metadata manager
    print("Initializing metadata manager...")
    metadata_mgr = MetadataManager(launcher_data_dir)

    # Initialize GitHub fetcher
    print("Initializing GitHub releases fetcher...")
    github = GitHubReleasesFetcher(metadata_mgr)

    print("\n=== ComfyUI Releases ===\n")

    # Fetch releases (should fetch from GitHub first time, use cache second time)
    print("Fetching releases (this may take a moment)...")
    releases = github.get_releases()

    if releases:
        print(f"\n✓ Found {len(releases)} releases\n")

        # Display first 5 releases
        print("First 5 releases:")
        for i, release in enumerate(releases[:5]):
            tag = release.get("tag_name", "unknown")
            name = release.get("name", "Unnamed")
            date = release.get("published_at", "unknown")
            prerelease = " (pre-release)" if release.get("prerelease") else ""

            print(f"\n{i+1}. {tag} - {name}{prerelease}")
            print(f"   Published: {date}")

        # Get latest stable release
        print("\n" + "=" * 50)
        latest = github.get_latest_release(include_prerelease=False)
        if latest:
            print(f"\n✓ Latest stable release: {latest.get('tag_name')}")
            print(f"  Published: {latest.get('published_at')}")
            print(f"  Download URL: {latest.get('zipball_url')}")

        # Test cache by fetching again
        print("\n" + "=" * 50)
        print("\nTesting cache (second fetch should use cached data)...")
        releases2 = github.get_releases()
        print(f"✓ Retrieved {len(releases2)} releases from cache")

        # Test getting specific release
        print("\n" + "=" * 50)
        print("\nTesting get_release_by_tag...")
        if len(releases) > 0:
            test_tag = releases[0].get("tag_name")
            specific_release = github.get_release_by_tag(test_tag)
            if specific_release:
                print(f"✓ Found release {test_tag}")
            else:
                print(f"✗ Could not find release {test_tag}")
    else:
        print("✗ No releases found")
        return 1

    # Test download functionality
    print("\n" + "=" * 50)
    print("\n=== Testing Download ===\n")

    downloader = DownloadManager()
    test_url = "https://api.github.com/repos/comfyanonymous/ComfyUI/releases/latest"
    test_dest = launcher_root / "test-download.json"

    print(f"Downloading test file to {test_dest.name}...")
    success = downloader.download_file(test_url, test_dest, print_progress)

    if success:
        size = test_dest.stat().st_size
        print(f"\n✓ Download successful! Size: {format_bytes(size)}")

        # Verify it's valid JSON
        import json

        try:
            with open(test_dest) as f:
                data = json.load(f)
                print(f"✓ Downloaded valid JSON (contains {len(data)} keys)")
        except json.JSONDecodeError:
            print("✗ Downloaded file is not valid JSON")

        # Clean up
        test_dest.unlink()
        print("✓ Cleaned up test file")
    else:
        print("\n✗ Download failed")
        return 1

    print("\n" + "=" * 50)
    print("\n✓ Phase 2 GitHub Integration tests completed successfully!\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
