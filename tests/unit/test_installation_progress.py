"""
Unit tests for installation progress tracking functionality.
"""

import pytest
from pathlib import Path

from backend.installation_progress_tracker import (
    InstallationProgressTracker,
    InstallationStage
)


@pytest.fixture
def tracker(tmp_path):
    """Create an InstallationProgressTracker with temporary storage."""
    return InstallationProgressTracker(tmp_path / "test-cache")


@pytest.mark.unit
class TestInstallationProgressTracker:
    """Tests for installation progress tracking."""

    def test_start_installation(self, tracker):
        """Test starting a new installation."""
        tracker.start_installation("v0.6.0", dependency_count=5)
        state = tracker.get_current_state()

        assert state["tag"] == "v0.6.0"
        assert state["dependency_count"] == 5
        assert state["stage"] == InstallationStage.DOWNLOAD.value
        assert state["overall_progress"] == 0

    def test_weighted_progress_calculation(self, tracker):
        """Test that large packages correctly dominate progress calculation."""
        # Scenario: torch is 15x heavier than small packages
        packages = ['pillow', 'numpy', 'torch', 'requests']
        # Expected weights: pillow=1, numpy=1, torch=15, requests=1 => total=18

        tracker.start_installation("test", dependency_count=len(packages))
        tracker.set_dependency_weights(packages)
        tracker.update_stage(InstallationStage.DEPENDENCIES, 0)

        state = tracker.get_current_state()
        total_weight = state['total_weight']

        # Complete small packages first (3 units out of 18)
        for pkg in ['pillow', 'numpy', 'requests']:
            tracker.complete_package(pkg)

        state = tracker.get_current_state()
        # 3/18 = ~16.67% of stage progress (stage is 70% of overall, so ~11.67% overall)
        # But we care more about the stage progress here
        assert state['completed_weight'] == 3
        assert state['total_weight'] == total_weight

        # Now install torch (15 units)
        tracker.complete_package('torch')
        state = tracker.get_current_state()

        # All packages complete: 18/18
        assert state['completed_weight'] == 18
        assert state['stage_progress'] == 100

    def test_set_dependency_weights(self, tracker):
        """Test that dependency weights are set correctly for known packages."""
        packages = [
            'torch',      # Should be weight 15
            'torchvision',  # Should be weight 5
            'pillow',     # Should be weight 1
            'unknown-pkg',  # Should default to weight 1
        ]

        tracker.start_installation("v0.6.0", dependency_count=len(packages))
        tracker.set_dependency_weights(packages)

        state = tracker.get_current_state()

        # Check total weight: 15 + 5 + 1 + 1 = 22
        expected_weight = 15 + 5 + 1 + 1
        assert state['total_weight'] == expected_weight

    def test_complete_package_increments_progress(self, tracker):
        """Test that completing packages increments completed weight."""
        packages = ['pillow', 'numpy']  # Both weight 1

        tracker.start_installation("test", dependency_count=2)
        tracker.set_dependency_weights(packages)
        tracker.update_stage(InstallationStage.DEPENDENCIES, 0)

        # Initially no packages complete
        state = tracker.get_current_state()
        assert state['completed_weight'] == 0

        # Complete first package
        tracker.complete_package('pillow')
        state = tracker.get_current_state()
        assert state['completed_weight'] == 1

        # Complete second package
        tracker.complete_package('numpy')
        state = tracker.get_current_state()
        assert state['completed_weight'] == 2
        assert state['stage_progress'] == 100

    def test_update_stage(self, tracker):
        """Test updating installation stage."""
        tracker.start_installation("v0.5.0", dependency_count=1)

        # Initially at DOWNLOAD stage
        state = tracker.get_current_state()
        assert state['stage'] == InstallationStage.DOWNLOAD.value

        # Move to DEPENDENCIES stage
        tracker.update_stage(InstallationStage.DEPENDENCIES, 50)
        state = tracker.get_current_state()
        assert state['stage'] == InstallationStage.DEPENDENCIES.value

    def test_clear_state(self, tracker):
        """Test clearing installation state."""
        tracker.start_installation("v0.6.0", dependency_count=5)
        tracker.clear_state()

        state = tracker.get_current_state()
        # After clear_state, get_current_state returns None
        assert state is None or state.get('tag') is None

    def test_complete_installation(self, tracker):
        """Test marking installation as complete."""
        tracker.start_installation("v0.6.0", dependency_count=1)
        tracker.complete_installation(success=True)

        state = tracker.get_current_state()
        assert state['overall_progress'] == 100

    def test_realistic_package_weights(self, tracker):
        """Test with realistic ComfyUI package list and weights."""
        packages = [
            'torch',         # Weight: 15
            'torchvision',   # Weight: 5
            'pillow',        # Weight: 1
            'numpy',         # Weight: 1
            'scipy',         # Weight: 3
            'opencv-python', # Weight: 4
        ]

        tracker.start_installation("v0.6.0", dependency_count=len(packages))
        tracker.set_dependency_weights(packages)

        state = tracker.get_current_state()

        # Total weight: 15 + 5 + 1 + 1 + 3 + 4 = 29
        expected_total = 15 + 5 + 1 + 1 + 3 + 4
        assert state['total_weight'] == expected_total
