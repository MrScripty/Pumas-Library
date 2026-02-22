"""Device Manager - GPU/CPU detection and memory reporting."""

from dataclasses import dataclass

import psutil
import torch


@dataclass
class DeviceInfo:
    device_id: str
    name: str
    memory_total: int
    memory_available: int
    is_available: bool


class DeviceManager:
    """Detects and reports on available compute devices."""

    def list_devices(self) -> list[DeviceInfo]:
        """List all available compute devices with memory info."""
        devices: list[DeviceInfo] = []

        # CPU is always available
        mem = psutil.virtual_memory()
        devices.append(DeviceInfo(
            device_id="cpu",
            name="CPU",
            memory_total=mem.total,
            memory_available=mem.available,
            is_available=True,
        ))

        # CUDA devices
        if torch.cuda.is_available():
            for i in range(torch.cuda.device_count()):
                props = torch.cuda.get_device_properties(i)
                mem_info = torch.cuda.mem_get_info(i)
                devices.append(DeviceInfo(
                    device_id=f"cuda:{i}",
                    name=props.name,
                    memory_total=props.total_mem,
                    memory_available=mem_info[0],
                    is_available=True,
                ))

        # MPS (Apple Silicon)
        if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
            # MPS doesn't expose detailed memory info
            mem = psutil.virtual_memory()
            devices.append(DeviceInfo(
                device_id="mps",
                name="Apple Silicon GPU",
                memory_total=mem.total,
                memory_available=mem.available,
                is_available=True,
            ))

        return devices

    def resolve_device(self, device_str: str) -> torch.device:
        """Resolve a device string to a torch.device, with 'auto' detection."""
        if device_str == "auto":
            if torch.cuda.is_available():
                return torch.device("cuda:0")
            if hasattr(torch.backends, "mps") and torch.backends.mps.is_available():
                return torch.device("mps")
            return torch.device("cpu")
        return torch.device(device_str)

    def get_device_memory_used(self, device: torch.device) -> int:
        """Get memory used on a specific device in bytes."""
        if device.type == "cuda":
            return torch.cuda.memory_allocated(device)
        return 0
