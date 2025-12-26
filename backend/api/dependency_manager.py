#!/usr/bin/env python3
"""
Dependency Manager for ComfyUI Setup
Handles system dependency checking and installation
"""

import shutil
import subprocess
import os
from pathlib import Path
from typing import List


class DependencyManager:
    """Manages system dependencies for ComfyUI"""

    def __init__(self, script_dir: Path):
        """
        Initialize dependency manager

        Args:
            script_dir: Path to launcher directory
        """
        self.script_dir = Path(script_dir)

    def check_setproctitle(self) -> bool:
        """Check if setproctitle module is installed"""
        try:
            import setproctitle
            return True
        except ImportError:
            return False

    def check_git(self) -> bool:
        """Check if git is installed"""
        return shutil.which('git') is not None

    def check_brave(self) -> bool:
        """Check if Brave browser is installed"""
        return shutil.which('brave-browser') is not None

    def get_missing_dependencies(self) -> List[str]:
        """Get list of missing dependencies"""
        missing = []
        if not self.check_setproctitle():
            missing.append("setproctitle")
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
        if "setproctitle" in missing:
            try:
                pip_cache_dir = self.script_dir / "launcher-data" / "cache" / "pip"
                pip_cache_dir.mkdir(parents=True, exist_ok=True)
                pip_env = os.environ.copy()
                pip_env["PIP_CACHE_DIR"] = str(pip_cache_dir)
                subprocess.run(
                    ['pip3', 'install', '--user', 'setproctitle'],
                    check=True,
                    stdout=subprocess.DEVNULL,
                    env=pip_env
                )
            except Exception:
                success = False

        # Install system packages (requires sudo)
        system_pkgs = [p for p in missing if p in ("git", "brave-browser")]
        if system_pkgs:
            try:
                subprocess.run(['sudo', 'apt', 'update'], check=True)
                subprocess.run(['sudo', 'apt', 'install', '-y'] + system_pkgs, check=True)
            except Exception:
                success = False

        return success
