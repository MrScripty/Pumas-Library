"""
Centralized configuration for the ComfyUI Launcher.

This module provides configuration constants for installation, network operations,
UI dimensions, and other system parameters.
"""

from dataclasses import dataclass


@dataclass(frozen=True)
class AppConfig:
    """Application-level configuration."""

    APP_NAME: str = "ComfyUI Setup"
    GITHUB_REPO: str = "comfyanonymous/ComfyUI"
    LOG_FILE_MAX_BYTES: int = 10_485_760  # 10MB
    LOG_FILE_BACKUP_COUNT: int = 5


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
    DOWNLOAD_REQUEST_TIMEOUT_SEC: int = 30
    DOWNLOAD_CHUNK_SIZE_BYTES: int = 8192
    DOWNLOAD_PROGRESS_INTERVAL_SEC: float = 0.5
    DOWNLOAD_TEMP_SUFFIX: str = ".part"
    GITHUB_API_BASE: str = "https://api.github.com"
    GITHUB_RELEASES_PER_PAGE: int = 100
    GITHUB_RELEASES_MAX_PAGES: int = 10
    GITHUB_RELEASES_TTL_SEC: int = 3600


@dataclass(frozen=True)
class PathsConfig:
    """Shared directory and path configurations."""

    CACHE_DIR_NAME: str = "cache"
    PIP_CACHE_DIR_NAME: str = "pip"
    SHARED_RESOURCES_DIR_NAME: str = "shared-resources"
    VERSIONS_DIR_NAME: str = "versions"
    ICONS_DIR_NAME: str = "icons"
    CONSTRAINTS_DIR_NAME: str = "constraints"
    CONSTRAINTS_CACHE_FILENAME: str = "constraints-cache.json"


# Global configuration instances (frozen/immutable)
APP = AppConfig()
INSTALLATION = InstallationConfig()
UI = UIConfig()
NETWORK = NetworkConfig()
PATHS = PathsConfig()
