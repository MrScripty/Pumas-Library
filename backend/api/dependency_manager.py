#!/usr/bin/env python3
"""
Dependency Manager for ComfyUI Setup
Handles system dependency checking and installation
"""

import os
import shutil
import subprocess
from pathlib import Path
from typing import List

from backend.logging_config import get_logger

logger = get_logger(__name__)


class DependencyManager:
    """Manages system dependencies for ComfyUI"""

    _python_packages = [
        "setproctitle",
        "huggingface_hub",
        "pydantic",
        "tenacity",
        "blake3",
    ]

    def __init__(self, script_dir: Path):
        """
        Initialize dependency manager

        Args:
            script_dir: Path to launcher directory
        """
        self.script_dir = Path(script_dir)

    def check_setproctitle(self) -> bool:
        """Check if setproctitle module is installed"""
        return self.check_python_package("setproctitle")

    def check_python_package(self, module_name: str) -> bool:
        """Check if a Python module can be imported."""
        try:
            __import__(module_name)
            return True
        except ImportError:
            logger.debug("Python package missing: %s", module_name)
            return False

    def check_git(self) -> bool:
        """Check if git is installed"""
        return shutil.which("git") is not None

    def check_brave(self) -> bool:
        """Check if Brave browser is installed"""
        return shutil.which("brave-browser") is not None

    def get_missing_dependencies(self) -> List[str]:
        """Get list of missing dependencies"""
        missing = []
        for module_name in self._python_packages:
            if not self.check_python_package(module_name):
                missing.append(module_name)
        if not self.check_git():
            missing.append("git")
        if not self.check_brave():
            missing.append("brave-browser")
        return missing

    def install_missing_dependencies(self) -> bool:
        """Install missing dependencies (requires user interaction for sudo)"""
        missing = self.get_missing_dependencies()
        if not missing:
            return True

        success = True

        # Install Python packages
        python_packages = [pkg for pkg in missing if pkg in self._python_packages]
        if python_packages:
            try:
                pip_cache_dir = self.script_dir / "launcher-data" / "cache" / "pip"
                pip_cache_dir.mkdir(parents=True, exist_ok=True)
                pip_env = os.environ.copy()
                pip_env["PIP_CACHE_DIR"] = str(pip_cache_dir)
                subprocess.run(
                    ["pip3", "install", "--user"] + python_packages,
                    check=True,
                    stdout=subprocess.DEVNULL,
                    env=pip_env,
                )
            except subprocess.CalledProcessError as exc:
                logger.error("pip install failed: %s", exc, exc_info=True)
                success = False
            except OSError as exc:
                logger.error("pip install failed to start: %s", exc, exc_info=True)
                success = False

        # Install system packages (requires sudo)
        system_pkgs = [p for p in missing if p in ("git", "brave-browser")]
        if system_pkgs:
            try:
                subprocess.run(["sudo", "apt", "update"], check=True)
                subprocess.run(["sudo", "apt", "install", "-y"] + system_pkgs, check=True)
            except subprocess.CalledProcessError as exc:
                logger.error("System package install failed: %s", exc, exc_info=True)
                success = False
            except OSError as exc:
                logger.error("System package install failed to start: %s", exc, exc_info=True)
                success = False

        return success
