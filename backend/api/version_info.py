#!/usr/bin/env python3
"""
Version Information Manager for ComfyUI
Handles version detection and release checking
"""

import json
import subprocess
import tomllib
import urllib.error
import urllib.request
from pathlib import Path
from typing import Any, Dict, Optional

from backend.logging_config import get_logger

logger = get_logger(__name__)


class VersionInfoManager:
    """Manages ComfyUI version information and release checking"""

    def __init__(self, comfyui_dir: Path, github_fetcher=None):
        """
        Initialize version info manager

        Args:
            comfyui_dir: Path to ComfyUI installation directory
            github_fetcher: Optional GitHubReleasesFetcher instance
        """
        self.comfyui_dir = Path(comfyui_dir)
        self.github_fetcher = github_fetcher
        self._release_info_cache: Optional[Dict[str, Any]] = None

    def get_comfyui_version(self) -> str:
        """Get ComfyUI version from pyproject.toml, git, or GitHub API"""
        # Try reading from pyproject.toml first
        pyproject_path = self.comfyui_dir / "pyproject.toml"
        if pyproject_path.exists():
            try:
                with open(pyproject_path, "rb") as f:
                    data = tomllib.load(f)
                    version = data.get("project", {}).get("version")
                    if isinstance(version, str) and version:
                        return version
            except OSError as exc:
                logger.debug("Failed to read %s: %s", pyproject_path, exc)
            except tomllib.TOMLDecodeError as exc:
                logger.debug("Failed to parse %s: %s", pyproject_path, exc)

        # Try git describe
        try:
            version = subprocess.check_output(
                ["git", "-C", str(self.comfyui_dir), "describe", "--tags", "--always"],
                text=True,
                stderr=subprocess.DEVNULL,
            ).strip()
            if version:
                return version
        except subprocess.CalledProcessError as exc:
            logger.debug("Git describe failed: %s", exc)
        except OSError as exc:
            logger.debug("Git describe failed: %s", exc)

        # Fallback to GitHub API
        try:
            with urllib.request.urlopen(
                "https://api.github.com/repos/comfyanonymous/ComfyUI/releases/latest",
                timeout=5,
            ) as resp:
                data = json.loads(resp.read())
                if isinstance(data, dict):
                    tag_name = data.get("tag_name")
                    if isinstance(tag_name, str):
                        return f"{tag_name} (latest)"
        except urllib.error.URLError as exc:
            logger.debug("Failed to fetch GitHub release info: %s", exc)
        except OSError as exc:
            logger.debug("Failed to fetch GitHub release info: %s", exc)
        except json.JSONDecodeError as exc:
            logger.debug("Failed to parse GitHub release info: %s", exc)
        except KeyError as exc:
            logger.debug("Failed to parse GitHub release info: %s", exc)
        except ValueError as exc:
            logger.debug("Failed to parse GitHub release info: %s", exc)

        return "Unknown"

    def check_for_new_release(self, force_refresh: bool = False) -> Dict[str, Any]:
        """Check if a new release is available on GitHub (cached)"""
        if self._release_info_cache and not force_refresh:
            return self._release_info_cache

        try:
            # Get current local version
            current_version = None
            current_tag = None

            try:
                # Try to get the exact tag first
                current_tag = subprocess.check_output(
                    [
                        "git",
                        "-C",
                        str(self.comfyui_dir),
                        "describe",
                        "--tags",
                        "--exact-match",
                    ],
                    text=True,
                    stderr=subprocess.DEVNULL,
                ).strip()
                current_version = current_tag
            except subprocess.CalledProcessError as exc:
                logger.debug("Git exact tag check failed: %s", exc)
            except OSError as exc:
                logger.debug("Git exact tag check failed: %s", exc)
                # If not on an exact tag, get the description
                try:
                    current_version = subprocess.check_output(
                        [
                            "git",
                            "-C",
                            str(self.comfyui_dir),
                            "describe",
                            "--tags",
                            "--always",
                        ],
                        text=True,
                        stderr=subprocess.DEVNULL,
                    ).strip()
                    # Extract just the tag part (before any -N-hash suffix)
                    if "-" in current_version:
                        current_tag = current_version.split("-")[0]
                    else:
                        current_tag = current_version
                except subprocess.CalledProcessError as exc:
                    logger.debug("Git tag description failed: %s", exc)
                except OSError as exc:
                    logger.debug("Git tag description failed: %s", exc)

            # Use cached GitHub releases (TTL handled by GitHubReleasesFetcher)
            latest_tag = None
            if self.github_fetcher:
                try:
                    releases = self.github_fetcher.get_releases(force_refresh=False)
                    if releases:
                        latest_tag = releases[0].get("tag_name") or None
                except OSError as e:
                    logger.warning(f"Using cached/stale releases after error: {e}")
                except RuntimeError as e:
                    logger.warning(f"Using cached/stale releases after error: {e}")
                except TypeError as e:
                    logger.warning(f"Using cached/stale releases after error: {e}")
                except ValueError as e:
                    logger.warning(f"Using cached/stale releases after error: {e}")

            if current_tag and latest_tag:
                has_update = current_tag != latest_tag
                self._release_info_cache = {
                    "has_update": has_update,
                    "latest_version": latest_tag,
                    "current_version": current_version or current_tag,
                }
            else:
                self._release_info_cache = {
                    "has_update": False,
                    "latest_version": latest_tag,
                    "current_version": current_version,
                }
        except OSError as e:
            logger.error(f"Error checking for new release: {e}", exc_info=True)
            self._release_info_cache = {
                "has_update": False,
                "latest_version": None,
                "current_version": None,
            }
        except RuntimeError as e:
            logger.error(f"Error checking for new release: {e}", exc_info=True)
            self._release_info_cache = {
                "has_update": False,
                "latest_version": None,
                "current_version": None,
            }
        except TypeError as e:
            logger.error(f"Error checking for new release: {e}", exc_info=True)
            self._release_info_cache = {
                "has_update": False,
                "latest_version": None,
                "current_version": None,
            }
        except ValueError as e:
            logger.error(f"Error checking for new release: {e}", exc_info=True)
            self._release_info_cache = {
                "has_update": False,
                "latest_version": None,
                "current_version": None,
            }
        except subprocess.SubprocessError as e:
            logger.error(f"Error checking for new release: {e}", exc_info=True)
            self._release_info_cache = {
                "has_update": False,
                "latest_version": None,
                "current_version": None,
            }

        return self._release_info_cache
