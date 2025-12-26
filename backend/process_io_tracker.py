"""
Process I/O Tracker for monitoring download progress.

This module provides reusable process I/O monitoring for tracking download
speeds and progress during package installations.
"""

import time
from pathlib import Path
from typing import Optional, Tuple, Callable

from backend.utils import get_directory_size


class ProcessIOTracker:
    """
    Reusable process I/O monitoring for progress tracking.

    Tracks download progress by monitoring:
    1. Process I/O bytes (primary method via /proc filesystem)
    2. Cache directory size growth (fallback method)

    This class eliminates code duplication in dependency installation
    monitoring.
    """

    def __init__(
        self,
        pid: Optional[int],
        cache_dir: Optional[Path],
        io_bytes_getter: Optional[Callable[[int, bool], Optional[int]]] = None
    ):
        """
        Initialize the process I/O tracker.

        Args:
            pid: Process ID to monitor (can be None)
            cache_dir: Cache directory to monitor for size changes (can be None)
            io_bytes_getter: Function to get I/O bytes for a PID
                           (signature: (pid: int, include_children: bool) -> Optional[int])
        """
        self.pid = pid
        self.cache_dir = cache_dir
        self.io_bytes_getter = io_bytes_getter

        # I/O baseline tracking
        self.io_baseline: Optional[int] = None
        self.io_last_bytes: Optional[int] = None
        self.io_last_time: Optional[float] = None

        if pid and io_bytes_getter:
            self.io_baseline = io_bytes_getter(pid, include_children=True)
            self.io_last_bytes = self.io_baseline
            self.io_last_time = time.time()

        # Cache size tracking
        self.cache_start_size = get_directory_size(cache_dir) if cache_dir and cache_dir.exists() else 0
        self.last_cache_size = self.cache_start_size
        self.last_sample_time = time.time()

    def get_download_metrics(self) -> Tuple[Optional[int], Optional[float]]:
        """
        Get current download metrics.

        Returns:
            Tuple of (downloaded_bytes, speed_bytes_per_sec)
            Both can be None if metrics are unavailable
        """
        now = time.time()
        speed: Optional[float] = None
        downloaded: Optional[int] = None

        # Try I/O-based tracking first (more accurate)
        if self.pid and self.io_bytes_getter:
            current_io = self.io_bytes_getter(self.pid, include_children=True)
            if (current_io is not None and
                self.io_baseline is not None and
                self.io_last_bytes is not None and
                self.io_last_time is not None):

                elapsed_io = now - self.io_last_time
                delta_io = current_io - self.io_last_bytes

                if elapsed_io > 0 and delta_io >= 0:
                    speed = delta_io / elapsed_io

                downloaded = max(0, current_io - self.io_baseline)
                self.io_last_bytes = current_io
                self.io_last_time = now

        # Fallback to cache growth if I/O metrics unavailable
        if speed is None and self.cache_dir and self.cache_dir.exists():
            current_cache_size = get_directory_size(self.cache_dir)
            bytes_since_last = current_cache_size - self.last_cache_size
            elapsed = now - self.last_sample_time

            if elapsed > 0:
                speed = bytes_since_last / elapsed

            downloaded = max(current_cache_size - self.cache_start_size, 0)
            self.last_cache_size = current_cache_size

        self.last_sample_time = now

        return downloaded, speed

    def should_update(self, min_interval_sec: float = 0.75) -> bool:
        """
        Check if enough time has passed to update metrics.

        Args:
            min_interval_sec: Minimum interval between updates in seconds

        Returns:
            True if enough time has passed since last sample
        """
        return (time.time() - self.last_sample_time) >= min_interval_sec
