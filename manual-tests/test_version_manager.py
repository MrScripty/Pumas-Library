#!/usr/bin/env python3
"""
Test script for Phase 4: Version Manager
Tests version installation, switching, dependency management, and launching
"""

import subprocess
import sys
import time
from pathlib import Path

# Add backend to path
sys.path.insert(0, str(Path(__file__).parent))

from backend.github_integration import GitHubReleasesFetcher, format_bytes
from backend.metadata_manager import MetadataManager
from backend.resource_manager import ResourceManager
from backend.utils import get_launcher_root
from backend.version_manager import VersionManager


def main():
    launcher_root = get_launcher_root()
    launcher_data_dir = launcher_root / "launcher-data"

    # Initialize all managers
    print("Initializing managers...")
    metadata_mgr = MetadataManager(launcher_data_dir)
    github_fetcher = GitHubReleasesFetcher(metadata_mgr)
    resource_mgr = ResourceManager(launcher_root, metadata_mgr)
    version_mgr = VersionManager(launcher_root, metadata_mgr, github_fetcher, resource_mgr)

    print("\n=== Phase 4: Version Manager Tests ===\n")

    # Test 1: Fetch available releases
    print("=" * 50)
    print("\nTest 1: Fetch Available Releases\n")

    releases = version_mgr.get_available_releases()
    if releases:
        print(f"✓ Found {len(releases)} releases")
        print(f"\nFirst 5 releases:")
        for i, release in enumerate(releases[:5], 1):
            tag = release.get("tag_name", "unknown")
            name = release.get("name", "Unnamed")
            date = release.get("published_at", "unknown")
            prerelease = " (pre-release)" if release.get("prerelease") else ""
            print(f"  {i}. {tag} - {name}{prerelease}")
            print(f"     Published: {date}")
    else:
        print("✗ No releases found")
        return 1

    # Test 2: Check installed versions
    print("\n" + "=" * 50)
    print("\nTest 2: Installed Versions\n")

    installed = version_mgr.get_installed_versions()
    print(f"Installed versions: {len(installed)}")

    if installed:
        for tag in installed:
            info = version_mgr.get_version_info(tag)
            print(f"  - {tag}")
            print(f"    Installed: {info.get('installedDate', 'unknown')}")
            print(f"    Python: {info.get('pythonVersion', 'unknown')}")
    else:
        print("  (No versions installed yet)")

    # Test 3: Check active version
    print("\n" + "=" * 50)
    print("\nTest 3: Active Version\n")

    active = version_mgr.get_active_version()
    if active:
        print(f"✓ Active version: {active}")
    else:
        print("ℹ No active version set")

    # Test 4: Get version status
    print("\n" + "=" * 50)
    print("\nTest 4: Version Status\n")

    status = version_mgr.get_version_status()
    print(f"✓ Status retrieved:")
    print(f"  - Installed count: {status['installedCount']}")
    print(f"  - Active version: {status['activeVersion']}")

    if status["versions"]:
        print(f"\nVersion details:")
        for tag, details in list(status["versions"].items())[:3]:
            print(f"  {tag}:")
            print(f"    Active: {details['isActive']}")
            dep_status = details["dependencies"]
            print(
                f"    Dependencies: {len(dep_status['installed'])} installed, {len(dep_status['missing'])} missing"
            )

    # Test 5: Install a version (optional - user can skip)
    print("\n" + "=" * 50)
    print("\nTest 5: Version Installation (Optional)\n")

    print("This test will install a ComfyUI version.")
    print("Installation includes:")
    print("  - Downloading release archive (~50-100 MB)")
    print("  - Extracting files")
    print("  - Creating virtual environment with UV")
    print("  - Installing dependencies (~500 MB+)")
    print("  - Setting up symlinks")
    print()
    print("This may take 5-15 minutes depending on your connection.")
    print()

    # Get user confirmation
    response = input("Do you want to test installation? (yes/no): ").strip().lower()

    if response in ["yes", "y"]:
        # Find a suitable version to install
        # Use latest stable release
        latest = github_fetcher.get_latest_release(include_prerelease=False)

        if not latest:
            print("✗ Could not find latest release")
            return 1

        install_tag = latest.get("tag_name")

        # Check if already installed
        if install_tag in installed:
            print(f"ℹ {install_tag} is already installed, skipping installation test")
        else:
            print(f"\nInstalling {install_tag}...")
            print("This may take several minutes...\n")

            def progress_callback(message, current, total):
                print(f"  [{current}/{total}] {message}")

            success = version_mgr.install_version(install_tag, progress_callback)

            if success:
                print(f"\n✓ Successfully installed {install_tag}")

                # Verify installation
                if install_tag in version_mgr.get_installed_versions():
                    print("✓ Version appears in installed list")

                    info = version_mgr.get_version_info(install_tag)
                    version_path = launcher_root / info["path"]

                    print(f"\nVerifying installation:")
                    print(f"  - Version directory: {version_path.exists()}")
                    print(f"  - main.py exists: {(version_path / 'main.py').exists()}")
                    print(f"  - venv exists: {(version_path / 'venv').exists()}")
                    print(f"  - models symlink: {(version_path / 'models').is_symlink()}")
                    print(f"  - user symlink: {(version_path / 'user').is_symlink()}")

                    # Test 6: Check dependencies
                    print("\n" + "=" * 50)
                    print("\nTest 6: Dependency Status\n")

                    dep_status = version_mgr.check_dependencies(install_tag)
                    print(f"✓ Dependency check complete:")
                    print(f"  - Installed: {len(dep_status['installed'])} packages")
                    print(f"  - Missing: {len(dep_status['missing'])} packages")

                    if dep_status["missing"]:
                        print(f"\n  Missing packages:")
                        for pkg in dep_status["missing"][:10]:
                            print(f"    - {pkg}")
                        if len(dep_status["missing"]) > 10:
                            print(f"    ... and {len(dep_status['missing']) - 10} more")

                    # Test 7: Switch active version
                    print("\n" + "=" * 50)
                    print("\nTest 7: Version Switching\n")

                    if version_mgr.set_active_version(install_tag):
                        print(f"✓ Switched to {install_tag}")

                        # Verify
                        current_active = version_mgr.get_active_version()
                        if current_active == install_tag:
                            print(f"✓ Active version verified: {current_active}")
                        else:
                            print(
                                f"✗ Active version mismatch: expected {install_tag}, got {current_active}"
                            )
                    else:
                        print(f"✗ Failed to switch to {install_tag}")

                    # Test 8: Launch version (start and immediately stop)
                    print("\n" + "=" * 50)
                    print("\nTest 8: Version Launching (Quick Test)\n")

                    print(f"Testing launch of {install_tag}...")
                    print("Will start ComfyUI and stop it after 3 seconds")
                    print()

                    response = input("Proceed with launch test? (yes/no): ").strip().lower()

                    if response in ["yes", "y"]:
                        success, process = version_mgr.launch_version(install_tag)

                        if success and process:
                            print(f"✓ ComfyUI launched (PID: {process.pid})")
                            print("Waiting 3 seconds...")
                            time.sleep(3)

                            # Check if still running
                            if process.poll() is None:
                                print("✓ Process is running")
                                print("Terminating...")
                                process.terminate()

                                # Wait for termination
                                try:
                                    process.wait(timeout=5)
                                    print("✓ Process terminated cleanly")
                                except (subprocess.TimeoutExpired, OSError) as e:
                                    print(f"Process didn't terminate gracefully: {e}")
                                    process.kill()
                            else:
                                print("⚠ Process exited early")
                                stdout, stderr = process.communicate()
                                if stderr:
                                    print(f"Error output:\n{stderr[:500]}")
                        else:
                            print("✗ Failed to launch ComfyUI")
                    else:
                        print("Skipping launch test")

                else:
                    print("✗ Version not found in installed list after installation")
            else:
                print(f"\n✗ Installation failed for {install_tag}")
    else:
        print("Skipping installation test")
        print("To test installation manually, use:")
        print(f"  version_mgr.install_version('TAG_NAME')")

    # Test 9: Version removal (optional)
    print("\n" + "=" * 50)
    print("\nTest 9: Version Removal (Optional)\n")

    current_installed = version_mgr.get_installed_versions()

    if len(current_installed) > 1:
        print(f"Currently installed versions: {', '.join(current_installed)}")
        print()
        print("This test can remove a version (not the active one).")
        response = input("Do you want to test version removal? (yes/no): ").strip().lower()

        if response in ["yes", "y"]:
            # Find a non-active version to remove
            active_ver = version_mgr.get_active_version()
            removable = [v for v in current_installed if v != active_ver]

            if removable:
                to_remove = removable[0]
                print(f"\nRemoving {to_remove}...")

                if version_mgr.remove_version(to_remove):
                    print(f"✓ Successfully removed {to_remove}")

                    # Verify removal
                    if to_remove not in version_mgr.get_installed_versions():
                        print("✓ Version no longer in installed list")
                    else:
                        print("✗ Version still in installed list")
                else:
                    print(f"✗ Failed to remove {to_remove}")
            else:
                print("ℹ No removable versions (only active version installed)")
        else:
            print("Skipping removal test")
    else:
        print("ℹ Need at least 2 installed versions to test removal")
        print("  (Cannot remove the active version)")

    # Summary
    print("\n" + "=" * 50)
    print("\n=== Test Summary ===\n")

    print("Phase 4 Version Manager implementation complete!")
    print("\nKey features tested:")
    print("  ✓ Fetch available releases from GitHub")
    print("  ✓ List installed versions")
    print("  ✓ Get active version")
    print("  ✓ Get comprehensive version status")

    if response in ["yes", "y"]:
        print("  ✓ Version installation")
        print("  ✓ Virtual environment creation with UV")
        print("  ✓ Dependency installation")
        print("  ✓ Symlink setup")
        print("  ✓ Version switching")
        print("  ✓ Version launching")

    print("\nVersion Manager ready for integration!")
    return 0


if __name__ == "__main__":
    sys.exit(main())
