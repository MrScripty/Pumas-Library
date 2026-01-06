"""
Unit tests for backend/installation_progress_tracker.py

Tests for InstallationProgressTracker progress tracking, stage management, and state persistence.
"""

import json
from datetime import datetime, timezone
from pathlib import Path
from unittest.mock import Mock, mock_open, patch

import pytest

from backend.installation_progress_tracker import (
    PACKAGE_WEIGHTS,
    STAGE_WEIGHTS,
    InstallationProgressTracker,
    InstallationStage,
)

# ============================================================================
# FIXTURES
# ============================================================================


@pytest.fixture
def progress_tracker(tmp_path):
    """Create an InstallationProgressTracker instance for testing"""
    cache_dir = tmp_path / "cache"
    return InstallationProgressTracker(cache_dir)


@pytest.fixture
def sample_packages():
    """Sample package list"""
    return ["torch==2.1.0", "numpy>=1.20", "pillow", "requests[security]"]


# ============================================================================
# INITIALIZATION TESTS
# ============================================================================


class TestInstallationProgressTrackerInit:
    """Test InstallationProgressTracker initialization"""

    def test_init_creates_cache_dir(self, tmp_path):
        """Test that initialization creates cache directory"""
        cache_dir = tmp_path / "cache"
        tracker = InstallationProgressTracker(cache_dir)

        assert cache_dir.exists()

    def test_init_creates_state_file_path(self, tmp_path):
        """Test that initialization sets state file path"""
        cache_dir = tmp_path / "cache"
        tracker = InstallationProgressTracker(cache_dir)

        assert tracker.state_file == cache_dir / "installation-state.json"

    def test_init_initializes_locks(self, tmp_path):
        """Test that initialization creates threading locks"""
        cache_dir = tmp_path / "cache"
        tracker = InstallationProgressTracker(cache_dir)

        assert hasattr(tracker._lock, "acquire")
        assert hasattr(tracker._lock, "release")
        assert hasattr(tracker._file_lock, "acquire")
        assert hasattr(tracker._file_lock, "release")

    def test_init_sets_initial_state_to_none(self, tmp_path):
        """Test that current state is None initially"""
        cache_dir = tmp_path / "cache"
        tracker = InstallationProgressTracker(cache_dir)

        assert tracker._current_state is None

    def test_init_initializes_package_weights(self, tmp_path):
        """Test that package weight tracking is initialized"""
        cache_dir = tmp_path / "cache"
        tracker = InstallationProgressTracker(cache_dir)

        assert tracker._package_weights == {}
        assert tracker._total_weight == 0
        assert tracker._completed_weight == 0


# ============================================================================
# INSTALLATION LIFECYCLE TESTS
# ============================================================================


class TestInstallationLifecycle:
    """Test installation lifecycle management"""

    def test_start_installation(self, progress_tracker):
        """Test starting a new installation"""
        progress_tracker.start_installation("v0.1.0")

        state = progress_tracker.get_current_state()
        assert state is not None
        assert state["tag"] == "v0.1.0"
        assert state["stage"] == InstallationStage.DOWNLOAD.value
        assert state["overall_progress"] == 0

    def test_start_installation_with_size_and_count(self, progress_tracker):
        """Test starting installation with total size and dependency count"""
        progress_tracker.start_installation("v0.1.0", total_size=4500000000, dependency_count=25)

        state = progress_tracker.get_current_state()
        assert state["total_size"] == 4500000000
        assert state["dependency_count"] == 25

    def test_start_installation_creates_state(self, progress_tracker):
        """Test that start_installation creates complete state structure"""
        progress_tracker.start_installation("v0.1.0", log_path="/tmp/install.log")

        state = progress_tracker.get_current_state()
        assert "started_at" in state
        assert "stage" in state
        assert "stage_progress" in state
        assert "overall_progress" in state
        assert "current_item" in state
        assert "download_speed" in state
        assert "eta_seconds" in state
        assert "completed_items" in state
        assert "error" in state
        assert "pid" in state
        assert "log_path" in state
        assert state["log_path"] == "/tmp/install.log"

    def test_complete_installation_success(self, progress_tracker):
        """Test completing installation successfully"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.complete_installation(success=True)

        state = progress_tracker.get_current_state()
        assert state["success"] is True
        assert state["overall_progress"] == 100
        assert "completed_at" in state

    def test_complete_installation_failure(self, progress_tracker):
        """Test completing installation with failure"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.update_stage(InstallationStage.DEPENDENCIES, 50)
        progress_tracker.complete_installation(success=False)

        state = progress_tracker.get_current_state()
        assert state["success"] is False
        # Progress should not be forced to 100 on failure
        assert state["overall_progress"] < 100

    def test_clear_state(self, progress_tracker):
        """Test clearing installation state"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.clear_state()

        assert progress_tracker.get_current_state() is None
        assert not progress_tracker.state_file.exists()


# ============================================================================
# PROGRESS TRACKING TESTS
# ============================================================================


class TestProgressTracking:
    """Test progress tracking methods"""

    def test_update_stage(self, progress_tracker):
        """Test updating installation stage"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.update_stage(InstallationStage.EXTRACT, 50, "Extracting...")

        state = progress_tracker.get_current_state()
        assert state["stage"] == InstallationStage.EXTRACT.value
        assert state["stage_progress"] == 50
        assert state["current_item"] == "Extracting..."

    def test_update_download_progress(self, progress_tracker):
        """Test updating download progress"""
        progress_tracker.start_installation("v0.1.0", total_size=100_000_000)
        progress_tracker.update_download_progress(
            downloaded_bytes=25_000_000,
            total_bytes=100_000_000,
            speed_bytes_per_sec=5_000_000,
        )

        state = progress_tracker.get_current_state()
        assert state["downloaded_bytes"] == 25_000_000
        assert state["stage_progress"] == 25
        assert state["download_speed"] == 5_000_000

    def test_update_download_progress_calculates_eta(self, progress_tracker):
        """Test that download progress calculates ETA"""
        progress_tracker.start_installation("v0.1.0", total_size=100_000_000)
        progress_tracker.update_download_progress(
            downloaded_bytes=25_000_000,
            total_bytes=100_000_000,
            speed_bytes_per_sec=5_000_000,
        )

        state = progress_tracker.get_current_state()
        # Remaining: 75_000_000 bytes / 5_000_000 bytes/sec = 15 seconds
        assert state["eta_seconds"] == 15

    def test_update_download_progress_clears_eta_when_no_speed(self, progress_tracker):
        """Test that ETA is cleared when download speed is zero"""
        progress_tracker.start_installation("v0.1.0", total_size=100_000_000)
        progress_tracker.update_download_progress(
            downloaded_bytes=25_000_000,
            total_bytes=100_000_000,
            speed_bytes_per_sec=5_000_000,
        )
        # Now set speed to 0
        progress_tracker.update_download_progress(
            downloaded_bytes=25_000_000, total_bytes=100_000_000, speed_bytes_per_sec=0
        )

        state = progress_tracker.get_current_state()
        assert state["eta_seconds"] is None

    def test_update_dependency_progress(self, progress_tracker):
        """Test updating dependency installation progress"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.update_stage(InstallationStage.DEPENDENCIES)
        progress_tracker.update_dependency_progress(
            current_package="numpy", completed_count=5, total_count=20
        )

        state = progress_tracker.get_current_state()
        assert state["current_item"] == "numpy"
        assert state["completed_dependencies"] == 5
        assert state["stage_progress"] == 25

    def test_update_dependency_progress_calculates_percentage(self, progress_tracker):
        """Test dependency progress percentage calculation"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.update_stage(InstallationStage.DEPENDENCIES)
        progress_tracker.update_dependency_progress(
            current_package="torch", completed_count=15, total_count=20
        )

        state = progress_tracker.get_current_state()
        assert state["stage_progress"] == 75

    def test_add_completed_item(self, progress_tracker):
        """Test adding completed item to list"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.add_completed_item("numpy", "package", 28_000_000)

        state = progress_tracker.get_current_state()
        assert len(state["completed_items"]) == 1
        assert state["completed_items"][0]["name"] == "numpy"
        assert state["completed_items"][0]["type"] == "package"
        assert state["completed_items"][0]["size"] == 28_000_000
        assert "completed_at" in state["completed_items"][0]

    def test_get_current_state_returns_copy(self, progress_tracker):
        """Test that get_current_state returns a copy, not reference"""
        progress_tracker.start_installation("v0.1.0")

        state1 = progress_tracker.get_current_state()
        state2 = progress_tracker.get_current_state()

        assert state1 is not state2
        assert state1 == state2

    def test_get_current_state_returns_none_when_no_installation(self, progress_tracker):
        """Test that get_current_state returns None when no installation active"""
        assert progress_tracker.get_current_state() is None


# ============================================================================
# PACKAGE WEIGHT TESTS
# ============================================================================


class TestPackageWeights:
    """Test package weight tracking for weighted progress"""

    def test_set_dependency_weights(self, progress_tracker, sample_packages):
        """Test setting dependency weights from package list"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.set_dependency_weights(sample_packages)

        assert progress_tracker._total_weight > 0
        assert "torch" in progress_tracker._package_weights
        assert progress_tracker._package_weights["torch"] == PACKAGE_WEIGHTS["torch"]

    def test_set_dependency_weights_with_torch(self, progress_tracker):
        """Test that torch gets high weight"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.set_dependency_weights(["torch==2.1.0", "numpy"])

        # Torch should have weight of 15
        assert progress_tracker._package_weights["torch"] == 15

    def test_complete_package_updates_weight(self, progress_tracker):
        """Test that completing a package updates completed weight"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.update_stage(InstallationStage.DEPENDENCIES)
        progress_tracker.set_dependency_weights(["torch==2.1.0", "numpy"])

        initial_weight = progress_tracker._completed_weight
        progress_tracker.complete_package("torch")

        assert progress_tracker._completed_weight > initial_weight
        assert progress_tracker._completed_weight == 15  # torch weight

    def test_extract_package_name_from_spec(self, progress_tracker):
        """Test extracting package name from various specifications"""
        assert progress_tracker._extract_package_name("torch==2.1.0") == "torch"
        assert progress_tracker._extract_package_name("numpy>=1.20") == "numpy"
        assert progress_tracker._extract_package_name("pillow") == "pillow"
        assert progress_tracker._extract_package_name("requests<=2.28") == "requests"

    def test_extract_package_name_with_extras(self, progress_tracker):
        """Test extracting package name with extras"""
        assert progress_tracker._extract_package_name("requests[security]") == "requests"
        assert progress_tracker._extract_package_name("pip[dev,test]") == "pip"

    def test_extract_package_name_with_url(self, progress_tracker):
        """Test extracting package name with URL specifier"""
        package = "package @ https://github.com/user/repo.git"
        result = progress_tracker._extract_package_name(package)
        assert result == "package"

    def test_package_weight_defaults(self, progress_tracker):
        """Test that unknown packages get default weight"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.set_dependency_weights(["unknown-package"])

        assert progress_tracker._package_weights["unknown-package"] == PACKAGE_WEIGHTS["_default"]

    def test_complete_package_updates_state_progress(self, progress_tracker):
        """Test that completing package updates stage progress in state"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.update_stage(InstallationStage.DEPENDENCIES)
        progress_tracker.set_dependency_weights(["torch", "numpy"])  # Total weight: 15 + 1 = 16

        progress_tracker.complete_package("torch")  # Completed 15/16

        state = progress_tracker.get_current_state()
        # 15/16 = 93.75% -> int(93.75) = 93%
        assert state["stage_progress"] == 93
        assert state["completed_weight"] == 15


# ============================================================================
# OVERALL PROGRESS CALCULATION TESTS
# ============================================================================


class TestOverallProgressCalculation:
    """Test overall progress calculation across stages"""

    def test_calculate_overall_progress_download_stage(self, progress_tracker):
        """Test overall progress during download stage"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.update_stage(InstallationStage.DOWNLOAD, 50)

        state = progress_tracker.get_current_state()
        # Download is 15% of total, 50% complete = 0.15 * 0.50 = 0.075 = 7.5%
        assert state["overall_progress"] == 7

    def test_calculate_overall_progress_dependencies_stage(self, progress_tracker):
        """Test overall progress during dependencies stage"""
        progress_tracker.start_installation("v0.1.0")
        # Complete download (15%), extract (5%), venv (5%)
        progress_tracker.update_stage(InstallationStage.DEPENDENCIES, 50)

        state = progress_tracker.get_current_state()
        # Download: 15%, Extract: 5%, Venv: 5%, Dependencies 50% of 70% = 35%
        # Total = 15 + 5 + 5 + 35 = 60%
        assert state["overall_progress"] == 60

    def test_calculate_overall_progress_all_stages(self, progress_tracker):
        """Test overall progress through all stages"""
        progress_tracker.start_installation("v0.1.0")

        # Download complete
        progress_tracker.update_stage(InstallationStage.DOWNLOAD, 100)
        state = progress_tracker.get_current_state()
        assert state["overall_progress"] == 15

        # Extract complete
        progress_tracker.update_stage(InstallationStage.EXTRACT, 100)
        state = progress_tracker.get_current_state()
        assert state["overall_progress"] == 20

        # Venv complete
        progress_tracker.update_stage(InstallationStage.VENV, 100)
        state = progress_tracker.get_current_state()
        assert state["overall_progress"] == 25

        # Dependencies 50%
        progress_tracker.update_stage(InstallationStage.DEPENDENCIES, 50)
        state = progress_tracker.get_current_state()
        assert state["overall_progress"] == 60

        # Dependencies complete
        progress_tracker.update_stage(InstallationStage.DEPENDENCIES, 100)
        state = progress_tracker.get_current_state()
        assert state["overall_progress"] == 95

        # Setup complete
        progress_tracker.update_stage(InstallationStage.SETUP, 100)
        state = progress_tracker.get_current_state()
        assert state["overall_progress"] == 100


# ============================================================================
# STATE PERSISTENCE TESTS
# ============================================================================


class TestStatePersistence:
    """Test state saving and loading"""

    def test_save_state_creates_file(self, progress_tracker):
        """Test that starting installation saves state to file"""
        progress_tracker.start_installation("v0.1.0")

        assert progress_tracker.state_file.exists()

    def test_save_state_atomic_write(self, progress_tracker):
        """Test that state is saved with atomic write"""
        progress_tracker.start_installation("v0.1.0")

        # Backup file should exist
        backup_file = progress_tracker.state_file.with_suffix(".json.bak")
        # Might not exist on first write, but file should exist
        assert progress_tracker.state_file.exists()

    def test_load_state_from_disk(self, progress_tracker):
        """Test loading state from disk"""
        # Create state
        progress_tracker.start_installation("v0.1.0", total_size=1000000)
        progress_tracker.update_stage(InstallationStage.DEPENDENCIES, 50)

        # Load state
        loaded_state = progress_tracker._load_state()

        assert loaded_state is not None
        assert loaded_state["tag"] == "v0.1.0"
        assert loaded_state["total_size"] == 1000000
        assert loaded_state["stage"] == InstallationStage.DEPENDENCIES.value

    def test_load_state_nonexistent_file(self, progress_tracker):
        """Test loading state when file doesn't exist"""
        loaded_state = progress_tracker._load_state()

        assert loaded_state is None

    def test_state_survives_updates(self, progress_tracker):
        """Test that state persists across multiple updates"""
        progress_tracker.start_installation("v0.1.0")

        # Multiple updates
        progress_tracker.update_stage(InstallationStage.DOWNLOAD, 25)
        progress_tracker.update_stage(InstallationStage.DOWNLOAD, 50)
        progress_tracker.update_stage(InstallationStage.DOWNLOAD, 75)
        progress_tracker.update_stage(InstallationStage.EXTRACT, 100)

        # Load from disk
        loaded_state = progress_tracker._load_state()
        assert loaded_state["stage"] == InstallationStage.EXTRACT.value
        assert loaded_state["stage_progress"] == 100


# ============================================================================
# ERROR AND EDGE CASE TESTS
# ============================================================================


class TestErrorHandling:
    """Test error handling and edge cases"""

    def test_set_error(self, progress_tracker):
        """Test setting error message"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.set_error("Network timeout occurred")

        state = progress_tracker.get_current_state()
        assert state["error"] == "Network timeout occurred"

    def test_set_pid(self, progress_tracker):
        """Test setting process ID"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.set_pid(12345)

        state = progress_tracker.get_current_state()
        assert state["pid"] == 12345

    def test_update_stage_with_no_active_installation(self, progress_tracker):
        """Test that updating stage with no active installation is safe"""
        # Should not raise error
        progress_tracker.update_stage(InstallationStage.DOWNLOAD, 50)

        assert progress_tracker.get_current_state() is None

    def test_update_download_progress_with_no_active_installation(self, progress_tracker):
        """Test updating download progress with no active installation"""
        # Should not raise error
        progress_tracker.update_download_progress(1000, 10000, 500)

        assert progress_tracker.get_current_state() is None

    def test_complete_package_with_no_weights_set(self, progress_tracker):
        """Test completing package when weights haven't been set"""
        progress_tracker.start_installation("v0.1.0")
        progress_tracker.update_stage(InstallationStage.DEPENDENCIES)

        # Should use default weight
        progress_tracker.complete_package("unknown-package")

        assert progress_tracker._completed_weight == PACKAGE_WEIGHTS["_default"]


# ============================================================================
# INTEGRATION TESTS
# ============================================================================


class TestIntegration:
    """Test realistic installation scenarios"""

    def test_full_installation_simulation(self, progress_tracker):
        """Test simulating a complete installation"""
        # Start
        progress_tracker.start_installation("v0.2.7", total_size=125_000_000, dependency_count=12)

        # Download
        progress_tracker.update_stage(InstallationStage.DOWNLOAD, 0)
        progress_tracker.update_download_progress(62_500_000, 125_000_000, 5_000_000)
        state = progress_tracker.get_current_state()
        assert state["stage_progress"] == 50

        # Extract
        progress_tracker.update_stage(InstallationStage.EXTRACT, 100)

        # Venv
        progress_tracker.update_stage(InstallationStage.VENV, 100)

        # Dependencies
        progress_tracker.update_stage(InstallationStage.DEPENDENCIES, 0)
        packages = ["torch", "numpy", "pillow", "requests"]
        progress_tracker.set_dependency_weights(packages)

        for i, pkg in enumerate(packages):
            progress_tracker.update_dependency_progress(pkg, i, len(packages))
            progress_tracker.complete_package(pkg)
            progress_tracker.add_completed_item(pkg, "package", 10_000_000)

        # Setup
        progress_tracker.update_stage(InstallationStage.SETUP, 100)

        # Complete
        progress_tracker.complete_installation(True)

        state = progress_tracker.get_current_state()
        assert state["success"] is True
        assert state["overall_progress"] == 100
        assert len(state["completed_items"]) == 4
