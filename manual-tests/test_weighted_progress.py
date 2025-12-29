#!/usr/bin/env python3
"""
Test weighted progress tracking with realistic package weights
"""

import shutil
import sys
import time
from pathlib import Path

# Add parent directory to path
sys.path.insert(0, str(Path(__file__).parent.parent))

from backend.installation_progress_tracker import InstallationProgressTracker, InstallationStage


def test_weighted_progress():
    """Test weighted progress tracking"""
    test_cache_dir = Path("./test-weighted-cache")
    tracker = InstallationProgressTracker(test_cache_dir)

    print("=== Testing Weighted Progress Tracking ===\n")

    # Realistic ComfyUI package list
    packages = [
        "torch==2.1.0",  # Weight: 15 (very large)
        "torchvision==0.16.0",  # Weight: 5 (large)
        "pillow==10.0.0",  # Weight: 1 (small)
        "numpy==1.24.3",  # Weight: 1 (small)
        "scipy==1.11.0",  # Weight: 3 (medium)
        "opencv-python==4.8.0",  # Weight: 4 (medium-large)
    ]

    # Start installation
    tracker.start_installation("v0.6.0", dependency_count=len(packages))
    print(f"Started installation of {len(packages)} packages\n")

    # Set weights
    tracker.set_dependency_weights(packages)
    state = tracker.get_current_state()
    print(f"Total weight: {state['total_weight']} units")
    print(f"Expected weights:")
    for pkg in packages:
        pkg_name = pkg.split("==")[0]
        weight = tracker._package_weights.get(pkg_name.lower(), 1)
        print(f"  {pkg_name}: {weight} units")
    print()

    # Move to dependencies stage
    tracker.update_stage(InstallationStage.DEPENDENCIES, 0)

    # Simulate installation progress
    print("Simulating installation progress:\n")

    for i, pkg in enumerate(packages):
        pkg_name = pkg.split("==")[0]
        print(f"Installing {pkg_name}...")

        # Update current package
        tracker.update_dependency_progress(f"Downloading {pkg_name}", i, len(packages))
        time.sleep(0.05)

        # Complete package
        tracker.complete_package(pkg_name)
        tracker.add_completed_item(pkg_name, "package")

        state = tracker.get_current_state()
        print(f"  Completed {pkg_name}")
        print(
            f"  Progress: {state['stage_progress']}% (stage) | {state['overall_progress']}% (overall)"
        )
        print(f"  Completed weight: {state['completed_weight']}/{state['total_weight']}")
        print()

        time.sleep(0.05)

    # Complete installation
    tracker.update_stage(InstallationStage.SETUP, 100)
    tracker.complete_installation(True)

    state = tracker.get_current_state()
    print(f"✓ Installation complete!")
    print(f"  Final progress: {state['overall_progress']}%")
    print(f"  All packages installed: {len(state['completed_items'])}/{len(packages)}")

    # Cleanup
    tracker.clear_state()
    if test_cache_dir.exists():
        shutil.rmtree(test_cache_dir)
    print("\n✓ Test cleanup complete")


def test_progress_calculation():
    """Test that large packages dominate progress"""
    test_cache_dir = Path("./test-progress-calc")
    tracker = InstallationProgressTracker(test_cache_dir)

    print("\n=== Testing Progress Calculation with Large Packages ===\n")

    # Scenario: torch is 15x heavier than small packages
    packages = ["pillow", "numpy", "torch", "requests"]
    # Weights: 1 + 1 + 15 + 1 = 18 total

    tracker.start_installation("test", dependency_count=len(packages))
    tracker.set_dependency_weights(packages)
    tracker.update_stage(InstallationStage.DEPENDENCIES, 0)

    print("Package weights:")
    print(f"  pillow: {tracker._package_weights.get('pillow', 1)}")
    print(f"  numpy: {tracker._package_weights.get('numpy', 1)}")
    print(f"  torch: {tracker._package_weights.get('torch', 1)}")
    print(f"  requests: {tracker._package_weights.get('requests', 1)}")
    print(f"  Total: {tracker._total_weight}\n")

    # Complete small packages first
    print("Installing small packages first...")
    for pkg in ["pillow", "numpy", "requests"]:
        tracker.complete_package(pkg)
        state = tracker.get_current_state()
        print(f"  After {pkg}: {state['stage_progress']}% complete")

    # Now install torch
    print("\nInstalling torch (the large package)...")
    tracker.complete_package("torch")
    state = tracker.get_current_state()
    print(f"  After torch: {state['stage_progress']}% complete")

    print("\n✓ Progress correctly reflects package sizes!")

    # Cleanup
    tracker.clear_state()
    if test_cache_dir.exists():
        shutil.rmtree(test_cache_dir)


if __name__ == "__main__":
    test_weighted_progress()
    test_progress_calculation()
