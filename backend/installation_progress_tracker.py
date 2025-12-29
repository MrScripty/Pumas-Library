#!/usr/bin/env python3
"""
Installation Progress Tracker - Phase 6.2.5b
Thread-safe progress tracking for version installations
"""

import json
import threading
from pathlib import Path
from typing import Dict, Optional, List
from datetime import datetime, timezone
from enum import Enum


class InstallationStage(Enum):
    """Installation stages"""
    DOWNLOAD = "download"
    EXTRACT = "extract"
    VENV = "venv"
    DEPENDENCIES = "dependencies"
    SETUP = "setup"


# Stage weights for overall progress calculation
STAGE_WEIGHTS = {
    InstallationStage.DOWNLOAD: 0.15,      # 15% - downloading archive
    InstallationStage.EXTRACT: 0.05,        # 5% - extracting archive
    InstallationStage.VENV: 0.05,           # 5% - creating venv
    InstallationStage.DEPENDENCIES: 0.70,   # 70% - installing dependencies (largest)
    InstallationStage.SETUP: 0.05           # 5% - final setup/symlinks
}

# Package weights for progress calculation
# Larger packages get higher weights to reflect actual download/install time
PACKAGE_WEIGHTS = {
    'torch': 15,              # ~2-3 GB depending on version/platform
    'torchvision': 5,         # ~500 MB
    'torchaudio': 3,          # ~300 MB
    'tensorflow': 12,         # ~500 MB - 2 GB
    'tensorflow-gpu': 15,     # ~2 GB
    'opencv-python': 4,       # ~80 MB
    'opencv-contrib-python': 6,  # ~120 MB
    'opencv-python-headless': 4,  # ~80 MB
    'scipy': 3,               # ~40 MB
    'pandas': 2,              # ~15 MB
    'matplotlib': 2,          # ~20 MB
    'scikit-learn': 3,        # ~30 MB
    'scikit-image': 3,        # ~35 MB
    'transformers': 3,        # ~30 MB
    'diffusers': 2,           # ~20 MB
    'accelerate': 2,          # ~15 MB
    'xformers': 4,            # ~50 MB
    'onnxruntime': 4,         # ~50 MB
    'onnxruntime-gpu': 5,     # ~100 MB
    '_default': 1             # Default weight for unknown packages
}


class InstallationProgressTracker:
    """Thread-safe installation progress tracker"""

    def __init__(self, cache_dir: Path):
        """
        Initialize InstallationProgressTracker

        Args:
            cache_dir: Directory for progress state storage
        """
        self.cache_dir = Path(cache_dir)
        self.cache_dir.mkdir(parents=True, exist_ok=True)

        self.state_file = self.cache_dir / "installation-state.json"
        self._lock = threading.Lock()
        self._current_state: Optional[Dict] = None

        # Weighted package tracking
        self._package_weights: Dict[str, int] = {}
        self._total_weight: int = 0
        self._completed_weight: int = 0

    def start_installation(
        self,
        tag: str,
        total_size: Optional[int] = None,
        dependency_count: Optional[int] = None,
        log_path: Optional[str] = None
    ):
        """
        Start tracking a new installation

        Args:
            tag: Version tag being installed
            total_size: Optional total download size in bytes
            dependency_count: Optional number of dependencies
        """
        with self._lock:
            self._current_state = {
                'tag': tag,
                'started_at': self._get_iso_timestamp(),
                'stage': InstallationStage.DOWNLOAD.value,
                'stage_progress': 0,
                'overall_progress': 0,
                'current_item': None,
                'download_speed': None,
                'eta_seconds': None,
                'total_size': total_size,
                'downloaded_bytes': 0,
                'dependency_count': dependency_count,
                'completed_dependencies': 0,
                'completed_items': [],
                'error': None,
                'pid': None,
                'log_path': log_path
            }
            self._save_state()

    def update_stage(
        self,
        stage: InstallationStage,
        progress: int = 0,
        current_item: Optional[str] = None
    ):
        """
        Update current installation stage

        Args:
            stage: New stage
            progress: Progress within stage (0-100)
            current_item: Optional current item being processed
        """
        with self._lock:
            if not self._current_state:
                return

            self._current_state['stage'] = stage.value
            self._current_state['stage_progress'] = progress
            self._current_state['current_item'] = current_item

            # Calculate overall progress
            self._current_state['overall_progress'] = self._calculate_overall_progress()

            self._save_state()

    def update_download_progress(
        self,
        downloaded_bytes: int,
        total_bytes: Optional[int] = None,
        speed_bytes_per_sec: Optional[float] = None
    ):
        """
        Update download progress

        Args:
            downloaded_bytes: Bytes downloaded so far
            total_bytes: Total bytes to download
            speed_bytes_per_sec: Download speed in bytes/second
        """
        with self._lock:
            if not self._current_state:
                return

            self._current_state['downloaded_bytes'] = downloaded_bytes

            if total_bytes:
                self._current_state['total_size'] = total_bytes
                progress = int((downloaded_bytes / total_bytes) * 100)
                self._current_state['stage_progress'] = progress

            if speed_bytes_per_sec is not None:
                self._current_state['download_speed'] = speed_bytes_per_sec

                # Calculate ETA
                if total_bytes and downloaded_bytes < total_bytes and speed_bytes_per_sec > 0:
                    remaining_bytes = total_bytes - downloaded_bytes
                    eta_seconds = remaining_bytes / speed_bytes_per_sec
                    self._current_state['eta_seconds'] = int(eta_seconds)
                elif total_bytes and speed_bytes_per_sec == 0:
                    # No progress yet, clear ETA
                    self._current_state['eta_seconds'] = None

            # Calculate overall progress
            self._current_state['overall_progress'] = self._calculate_overall_progress()

            self._save_state()

    def update_dependency_progress(
        self,
        current_package: str,
        completed_count: int,
        total_count: Optional[int] = None,
        package_size: Optional[int] = None
    ):
        """
        Update dependency installation progress

        Args:
            current_package: Package being installed
            completed_count: Number of packages completed
            total_count: Total number of packages
            package_size: Size of current package
        """
        with self._lock:
            if not self._current_state:
                return

            self._current_state['current_item'] = current_package
            self._current_state['completed_dependencies'] = completed_count

            if total_count:
                self._current_state['dependency_count'] = total_count
                progress = int((completed_count / total_count) * 100)
                self._current_state['stage_progress'] = progress

            # Calculate overall progress
            self._current_state['overall_progress'] = self._calculate_overall_progress()

            self._save_state()

    def add_completed_item(
        self,
        item_name: str,
        item_type: str,
        size: Optional[int] = None
    ):
        """
        Add an item to the completed list

        Args:
            item_name: Name of completed item
            item_type: Type (e.g., 'package', 'file')
            size: Optional size in bytes
        """
        with self._lock:
            if not self._current_state:
                return

            completed_item = {
                'name': item_name,
                'type': item_type,
                'size': size,
                'completed_at': self._get_iso_timestamp()
            }

            self._current_state['completed_items'].append(completed_item)
            self._save_state()

    def set_dependency_weights(self, packages: List[str]):
        """
        Calculate total weight from package list for weighted progress tracking

        Args:
            packages: List of package specifications (e.g., ['torch==2.1.0', 'numpy'])
        """
        with self._lock:
            self._package_weights = {}
            for pkg in packages:
                pkg_name = self._extract_package_name(pkg)
                weight = PACKAGE_WEIGHTS.get(pkg_name.lower(), PACKAGE_WEIGHTS['_default'])
                self._package_weights[pkg_name.lower()] = weight

            self._total_weight = sum(self._package_weights.values())
            self._completed_weight = 0

            # Store in state for visibility
            if self._current_state:
                self._current_state['total_weight'] = self._total_weight
                self._current_state['completed_weight'] = 0
                self._save_state()

    def complete_package(self, package_name: str):
        """
        Mark a package as completed and update weighted progress

        Args:
            package_name: Name of the package that completed
        """
        with self._lock:
            if not self._current_state:
                return

            pkg_name = self._extract_package_name(package_name)
            weight = self._package_weights.get(pkg_name.lower(), PACKAGE_WEIGHTS['_default'])
            self._completed_weight += weight

            # Calculate progress based on weight
            if self._total_weight > 0:
                progress = int((self._completed_weight / self._total_weight) * 100)
                self._current_state['stage_progress'] = min(progress, 100)
                self._current_state['completed_weight'] = self._completed_weight
                self._current_state['overall_progress'] = self._calculate_overall_progress()

            self._save_state()

    def _extract_package_name(self, package_spec: str) -> str:
        """
        Extract package name from specification

        Args:
            package_spec: Package spec (e.g., 'torch==2.1.0', 'numpy>=1.20')

        Returns:
            Package name (e.g., 'torch', 'numpy')
        """
        # Remove version specifiers
        for op in ['==', '>=', '<=', '~=', '!=', '>', '<', '@']:
            if op in package_spec:
                package_spec = package_spec.split(op)[0]
                break

        # Remove extras like [dev]
        if '[' in package_spec:
            package_spec = package_spec.split('[')[0]

        return package_spec.strip()

    def set_error(self, error_message: str):
        """
        Set an error state

        Args:
            error_message: Error message
        """
        with self._lock:
            if not self._current_state:
                return

            self._current_state['error'] = error_message
            self._save_state()

    def set_pid(self, pid: int):
        """
        Set the process ID for this installation

        Args:
            pid: Process ID
        """
        with self._lock:
            if not self._current_state:
                return

            self._current_state['pid'] = pid
            self._save_state()

    def complete_installation(self, success: bool = True):
        """
        Mark installation as complete

        Args:
            success: Whether installation succeeded
        """
        with self._lock:
            if not self._current_state:
                return

            self._current_state['completed_at'] = self._get_iso_timestamp()
            self._current_state['success'] = success
            self._current_state['overall_progress'] = 100 if success else self._current_state['overall_progress']

            self._save_state()

    def clear_state(self):
        """Clear current installation state"""
        with self._lock:
            self._current_state = None
            if self.state_file.exists():
                self.state_file.unlink()

    def get_current_state(self) -> Optional[Dict]:
        """
        Get current installation state

        Returns:
            Current state dict or None if no installation in progress
        """
        with self._lock:
            return self._current_state.copy() if self._current_state else None

    def _calculate_overall_progress(self) -> int:
        """
        Calculate overall progress across all stages

        Returns:
            Overall progress (0-100)
        """
        if not self._current_state:
            return 0

        current_stage_name = self._current_state['stage']
        current_stage = InstallationStage(current_stage_name)
        stage_progress = self._current_state['stage_progress']

        # Calculate cumulative progress from completed stages
        cumulative_progress = 0.0

        for stage in InstallationStage:
            stage_weight = STAGE_WEIGHTS[stage]

            if stage.value == current_stage_name:
                # Add partial progress for current stage
                cumulative_progress += stage_weight * (stage_progress / 100)
                break
            else:
                # This stage is before current, so it's completed
                # Check if we've passed this stage
                stage_order = list(InstallationStage)
                if stage_order.index(stage) < stage_order.index(current_stage):
                    cumulative_progress += stage_weight

        return int(cumulative_progress * 100)

    def _save_state(self):
        """Save current state to disk"""
        if not self._current_state:
            return

        try:
            with open(self.state_file, 'w') as f:
                json.dump(self._current_state, f, indent=2)
        except Exception as e:
            print(f"Error saving installation state: {e}")

    def _load_state(self) -> Optional[Dict]:
        """Load state from disk"""
        if not self.state_file.exists():
            return None

        try:
            with open(self.state_file, 'r') as f:
                return json.load(f)
        except Exception as e:
            print(f"Error loading installation state: {e}")
            return None

    def _get_iso_timestamp(self) -> str:
        """Get current timestamp in ISO format"""
        return datetime.now(timezone.utc).isoformat()


if __name__ == "__main__":
    # Test the InstallationProgressTracker
    import time
    from pathlib import Path

    test_cache_dir = Path("./test-cache")
    tracker = InstallationProgressTracker(test_cache_dir)

    print("=== Testing InstallationProgressTracker ===\n")

    # Start installation
    tracker.start_installation("v0.2.7", total_size=4_500_000_000, dependency_count=12)
    print("Started installation")

    # Simulate download progress
    tracker.update_stage(InstallationStage.DOWNLOAD, 0, "ComfyUI-v0.2.7.tar.gz")
    for i in range(0, 101, 20):
        downloaded = int(125_000_000 * i / 100)
        tracker.update_download_progress(
            downloaded,
            125_000_000,
            5_200_000  # 5.2 MB/s
        )
        time.sleep(0.1)
        state = tracker.get_current_state()
        print(f"Download: {state['stage_progress']}% - Overall: {state['overall_progress']}%")

    # Simulate extraction
    tracker.update_stage(InstallationStage.EXTRACT, 50, "Extracting...")
    time.sleep(0.1)
    state = tracker.get_current_state()
    print(f"\nExtract: {state['stage_progress']}% - Overall: {state['overall_progress']}%")

    # Simulate venv creation
    tracker.update_stage(InstallationStage.VENV, 100, "Creating venv...")
    time.sleep(0.1)
    state = tracker.get_current_state()
    print(f"Venv: {state['stage_progress']}% - Overall: {state['overall_progress']}%")

    # Simulate dependency installation
    tracker.update_stage(InstallationStage.DEPENDENCIES, 0)
    packages = ['pillow', 'numpy', 'torch', 'torchvision']
    for i, pkg in enumerate(packages):
        tracker.update_dependency_progress(pkg, i, len(packages))
        tracker.add_completed_item(pkg, 'package', 28_000_000)
        time.sleep(0.1)
        state = tracker.get_current_state()
        print(f"Installing {pkg}: Overall {state['overall_progress']}%")

    # Complete
    tracker.update_stage(InstallationStage.SETUP, 100)
    tracker.complete_installation(True)
    state = tracker.get_current_state()
    print(f"\nCompleted: Overall {state['overall_progress']}%")

    # Cleanup
    tracker.clear_state()
    import shutil
    if test_cache_dir.exists():
        shutil.rmtree(test_cache_dir)
    print("\n✓ Test cleanup complete")
