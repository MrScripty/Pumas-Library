"""Boundary validation helpers for the Torch sidecar."""

from __future__ import annotations

import ipaddress
import os
import re
from pathlib import Path
from typing import Iterable

MODEL_NAME_MAX_LENGTH = 128
MAX_LOADED_MODELS_LIMIT = 16
LAN_ALLOW_ENV = "PUMAS_TORCH_ALLOW_LAN"
MODEL_ROOTS_ENV = "PUMAS_TORCH_MODEL_ROOTS"

_MODEL_NAME_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._:/ -]{0,127}$")
_CUDA_DEVICE_RE = re.compile(r"^cuda(?::[0-9]+)?$")


def validate_model_name(value: str) -> str:
    """Validate a user-visible model name."""
    model_name = value.strip()
    if not model_name:
        raise ValueError("model_name must not be empty")
    if len(model_name) > MODEL_NAME_MAX_LENGTH:
        raise ValueError(f"model_name must be {MODEL_NAME_MAX_LENGTH} characters or fewer")
    if not _MODEL_NAME_RE.fullmatch(model_name):
        raise ValueError(
            "model_name may contain letters, numbers, spaces, '.', '_', '-', ':', and '/'"
        )
    return model_name


def validate_model_path(value: str) -> str:
    """Resolve and validate a model path supplied at the API boundary."""
    raw_path = value.strip()
    if not raw_path:
        raise ValueError("model_path must not be empty")

    path = Path(raw_path).expanduser().resolve(strict=False)
    if not path.exists():
        raise ValueError(f"model_path does not exist: {path}")

    approved_roots = list(_configured_model_roots())
    if approved_roots and not any(_is_relative_to(path, root) for root in approved_roots):
        roots = ", ".join(str(root) for root in approved_roots)
        raise ValueError(f"model_path must be inside an approved model root: {roots}")

    return str(path)


def validate_device(value: str) -> str:
    """Validate a torch device selector before DeviceManager resolves it."""
    device = value.strip().lower()
    if device in {"auto", "cpu", "mps"} or _CUDA_DEVICE_RE.fullmatch(device):
        return device
    raise ValueError("device must be one of auto, cpu, mps, cuda, or cuda:<index>")


def validate_bind_host(value: str, *, lan_access: bool = False) -> str:
    """Validate the server bind host against the LAN exposure policy."""
    host = value.strip()
    if not host:
        raise ValueError("host must not be empty")

    if host == "localhost":
        return host

    try:
        ip = ipaddress.ip_address(host)
    except ValueError as exc:
        raise ValueError("host must be an IP address or localhost") from exc

    if ip.is_loopback:
        return host

    if not lan_access:
        raise ValueError("non-loopback host requires lan_access=true")

    require_lan_policy()
    return host


def is_loopback_host(value: str) -> bool:
    """Return whether a bind host is loopback-only."""
    host = value.strip()
    if host == "localhost":
        return True
    try:
        return ipaddress.ip_address(host).is_loopback
    except ValueError:
        return False


def validate_api_port(value: int) -> int:
    """Validate API port range and reserved-port policy."""
    if value < 1024 or value > 65535:
        raise ValueError("api_port must be between 1024 and 65535")
    return value


def validate_max_loaded_models(value: int) -> int:
    """Validate the model slot limit."""
    if value < 1 or value > MAX_LOADED_MODELS_LIMIT:
        raise ValueError(f"max_loaded_models must be between 1 and {MAX_LOADED_MODELS_LIMIT}")
    return value


def require_lan_policy() -> None:
    """Require an explicit trusted-network opt-in before LAN binding."""
    if os.environ.get(LAN_ALLOW_ENV) != "1":
        raise ValueError(f"LAN access requires {LAN_ALLOW_ENV}=1")


def _configured_model_roots() -> Iterable[Path]:
    raw_roots = os.environ.get(MODEL_ROOTS_ENV, "")
    for raw_root in raw_roots.split(os.pathsep):
        if not raw_root.strip():
            continue
        yield Path(raw_root).expanduser().resolve(strict=False)


def _is_relative_to(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
        return True
    except ValueError:
        return False
