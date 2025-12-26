"""
Centralized configuration for the ComfyUI Launcher.

This module provides configuration constants for installation, network operations,
UI dimensions, and other system parameters.
"""

from dataclasses import dataclass


@dataclass(frozen=True)
class InstallationConfig:
    """Configuration for installation process."""

    # Package manager timeouts
    UV_INSTALL_TIMEOUT_SEC: int = 600
    PIP_FALLBACK_TIMEOUT_SEC: int = 900
    VENV_CREATION_TIMEOUT_SEC: int = 120

    # Subprocess timeouts
    SUBPROCESS_QUICK_TIMEOUT_SEC: int = 5
    SUBPROCESS_STANDARD_TIMEOUT_SEC: int = 30
    SUBPROCESS_LONG_TIMEOUT_SEC: int = 60
    SUBPROCESS_STOP_TIMEOUT_SEC: int = 2
    SUBPROCESS_KILL_TIMEOUT_SEC: int = 1

    # Download and network
    DOWNLOAD_RETRY_ATTEMPTS: int = 3
    URL_FETCH_TIMEOUT_SEC: int = 15
    URL_QUICK_CHECK_TIMEOUT_SEC: int = 3

    # Server startup
    SERVER_START_DELAY_SEC: int = 8


@dataclass(frozen=True)
class UIConfig:
    """UI dimensions and timing."""

    WINDOW_WIDTH: int = 400
    WINDOW_HEIGHT: int = 520
    LOADING_MIN_DURATION_MS: int = 800
    STATUS_POLL_INTERVAL_MS: int = 4000
    PROGRESS_POLL_INTERVAL_MS: int = 1000


@dataclass(frozen=True)
class NetworkConfig:
    """Network-related configuration."""

    REQUEST_TIMEOUT_SEC: int = 15
    QUICK_REQUEST_TIMEOUT_SEC: int = 3
    MAX_RETRIES: int = 3


@dataclass(frozen=True)
class PathsConfig:
    """Shared directory and path configurations."""

    CACHE_DIR_NAME: str = "cache"
    SHARED_RESOURCES_DIR_NAME: str = "shared-resources"
    VERSIONS_DIR_NAME: str = "versions"
    ICONS_DIR_NAME: str = "icons"


# Global configuration instances (frozen/immutable)
INSTALLATION = InstallationConfig()
UI = UIConfig()
NETWORK = NetworkConfig()
PATHS = PathsConfig()
