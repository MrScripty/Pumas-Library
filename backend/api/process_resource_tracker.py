"""
Process Resource Tracker

Tracks CPU and GPU resource usage for individual processes and their child processes.
Supports NVIDIA GPUs via nvidia-smi with caching to minimize performance impact.
"""

import subprocess
import time
from typing import Dict, Optional

try:
    import psutil
except ImportError:
    psutil = None  # type: ignore

from backend.logging_config import get_logger

logger = get_logger(__name__)


class ProcessResourceTracker:
    """Track CPU and GPU resource usage for processes."""

    def __init__(self, cache_ttl: float = 2.0):
        """
        Initialize resource tracker with optional caching.

        Args:
            cache_ttl: Time-to-live for cached metrics in seconds (default: 2.0)
        """
        self._cache: Dict[int, Dict[str, float]] = {}
        self._cache_ttl = cache_ttl
        self._gpu_cache: Optional[Dict[int, float]] = None
        self._gpu_cache_time: float = 0.0

    def get_process_resources(self, pid: int, include_children: bool = True) -> Dict[str, float]:
        """
        Get CPU and GPU resource usage for a process and its children.

        Args:
            pid: Process ID to track
            include_children: Whether to include child processes in aggregation

        Returns:
            Dictionary with keys:
                - cpu: CPU usage percentage (0-100+)
                - ram_memory: RAM memory usage in GB
                - gpu_memory: GPU memory usage in GB (0 if not using GPU)
        """
        if not psutil:
            logger.warning("psutil not available, cannot track process resources")
            return {"cpu": 0.0, "ram_memory": 0.0, "gpu_memory": 0.0}

        # Check cache
        now = time.time()
        if pid in self._cache:
            cached = self._cache[pid]
            if now - cached.get("timestamp", 0.0) < self._cache_ttl:
                return {
                    "cpu": cached["cpu"],
                    "ram_memory": cached["ram_memory"],
                    "gpu_memory": cached["gpu_memory"],
                }

        # Get CPU usage
        cpu_usage = self._get_process_cpu(pid, include_children)

        # Get RAM memory
        ram_memory = self._get_process_ram_memory(pid, include_children)

        # Get GPU memory
        gpu_memory = self._get_process_gpu_memory(pid, include_children)

        # Cache result
        self._cache[pid] = {
            "cpu": cpu_usage,
            "ram_memory": ram_memory,
            "gpu_memory": gpu_memory,
            "timestamp": now,
        }

        return {"cpu": cpu_usage, "ram_memory": ram_memory, "gpu_memory": gpu_memory}

    def _get_process_cpu(self, pid: int, include_children: bool = True) -> float:
        """
        Get CPU usage percentage for process and optionally its children.

        Args:
            pid: Process ID
            include_children: Whether to include child processes

        Returns:
            CPU usage percentage (0-100+, can exceed 100 on multi-core systems)
        """
        if not psutil:
            return 0.0

        try:
            proc = psutil.Process(pid)
            procs = [proc]

            if include_children:
                try:
                    procs += proc.children(recursive=True)
                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    pass

            total_cpu = 0.0
            for proc_item in procs:
                try:
                    # cpu_percent() returns usage since last call or process start
                    # Using interval=None for non-blocking, instantaneous reading
                    cpu = proc_item.cpu_percent(interval=None)
                    total_cpu += cpu
                except (psutil.NoSuchProcess, psutil.AccessDenied, AttributeError):
                    continue

            return round(total_cpu, 1)

        except (psutil.NoSuchProcess, psutil.AccessDenied, OSError) as e:
            logger.debug(f"Failed to get CPU usage for PID {pid}: {e}")
            return 0.0

    def _get_process_ram_memory(self, pid: int, include_children: bool = True) -> float:
        """
        Get RAM memory usage in GB for process and optionally its children.

        Args:
            pid: Process ID
            include_children: Whether to include child processes

        Returns:
            RAM memory usage in GB
        """
        if not psutil:
            return 0.0

        try:
            proc = psutil.Process(pid)
            procs = [proc]

            if include_children:
                try:
                    procs += proc.children(recursive=True)
                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    pass

            total_ram_bytes = 0
            for proc_item in procs:
                try:
                    # memory_info() returns memory usage in bytes
                    mem_info = proc_item.memory_info()
                    total_ram_bytes += mem_info.rss  # Resident Set Size (physical RAM)
                except (psutil.NoSuchProcess, psutil.AccessDenied, AttributeError):
                    continue

            # Convert bytes to GB
            total_ram_gb = total_ram_bytes / (1024**3)
            return round(total_ram_gb, 2)

        except (psutil.NoSuchProcess, psutil.AccessDenied, OSError) as e:
            logger.debug(f"Failed to get RAM usage for PID {pid}: {e}")
            return 0.0

    def _get_process_gpu_memory(self, pid: int, include_children: bool = True) -> float:
        """
        Get GPU memory usage in GB for process and optionally its children.

        Queries nvidia-smi for per-process GPU memory usage.

        Args:
            pid: Process ID
            include_children: Whether to include child processes

        Returns:
            GPU memory usage in GB
        """
        if not psutil:
            return 0.0

        try:
            # Collect PIDs to check
            pids_to_check = {pid}

            if include_children:
                try:
                    proc = psutil.Process(pid)
                    children = proc.children(recursive=True)
                    pids_to_check.update(child.pid for child in children)
                except (psutil.NoSuchProcess, psutil.AccessDenied):
                    pass

            # Get GPU process data (cached)
            gpu_processes = self._get_nvidia_gpu_processes()

            # Aggregate GPU memory for this process and children
            total_gpu_memory_mb = 0.0
            for proc_pid in pids_to_check:
                if proc_pid in gpu_processes:
                    total_gpu_memory_mb += gpu_processes[proc_pid]

            # Convert MB to GB
            return round(total_gpu_memory_mb / 1024.0, 2)

        except (OSError, RuntimeError, ValueError, psutil.Error) as e:
            logger.debug(f"Failed to get GPU memory for PID {pid}: {e}")
            return 0.0

    def _get_nvidia_gpu_processes(self) -> Dict[int, float]:
        """
        Query nvidia-smi for all processes using GPU.

        Returns a cached mapping of PID -> GPU memory (MB).
        Cache is shared across all process queries to minimize nvidia-smi calls.

        Returns:
            Dictionary mapping PID to GPU memory usage in MB
        """
        now = time.time()

        # Return cached data if still valid
        if self._gpu_cache is not None and (now - self._gpu_cache_time) < self._cache_ttl:
            return self._gpu_cache

        # Query nvidia-smi
        gpu_processes: Dict[int, float] = {}

        try:
            result = subprocess.run(
                [
                    "nvidia-smi",
                    "--query-compute-apps=pid,used_memory",
                    "--format=csv,noheader,nounits",
                ],
                capture_output=True,
                text=True,
                timeout=2,
            )

            if result.returncode == 0:
                for line in result.stdout.strip().split("\n"):
                    if not line.strip():
                        continue

                    try:
                        parts = line.split(",")
                        if len(parts) >= 2:
                            proc_pid = int(parts[0].strip())
                            memory_mb = float(parts[1].strip())
                            gpu_processes[proc_pid] = memory_mb
                    except (ValueError, IndexError):
                        continue

        except (subprocess.SubprocessError, FileNotFoundError) as e:
            # nvidia-smi not available or failed - this is fine for CPU-only systems
            logger.debug(f"nvidia-smi not available or failed: {e}")

        # Cache result
        self._gpu_cache = gpu_processes
        self._gpu_cache_time = now

        return gpu_processes

    def clear_cache(self, pid: Optional[int] = None) -> None:
        """
        Clear cached resource data.

        Args:
            pid: If provided, clear cache only for this PID. Otherwise clear all.
        """
        if pid is not None:
            self._cache.pop(pid, None)
        else:
            self._cache.clear()
            self._gpu_cache = None
            self._gpu_cache_time = 0.0
