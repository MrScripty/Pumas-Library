#!/usr/bin/env python3
"""
Patch Manager for ComfyUI Setup
Handles patching main.py with setproctitle for process naming
"""

import re
import subprocess
import urllib.request
from pathlib import Path
from typing import Optional, Tuple


class PatchManager:
    """Manages main.py patching for ComfyUI versions"""

    def __init__(self, comfyui_dir: Path, main_py: Path, version_manager=None):
        """
        Initialize patch manager

        Args:
            comfyui_dir: Path to ComfyUI root directory
            main_py: Path to main.py (legacy single installation)
            version_manager: Optional VersionManager instance for multi-version support
        """
        self.comfyui_dir = Path(comfyui_dir)
        self.main_py = Path(main_py)
        self.version_manager = version_manager

    def _build_server_title(self, tag: Optional[str] = None) -> str:
        """
        Build the process title for ComfyUI, including version when available.

        Args:
            tag: Optional version tag (e.g., v0.2.0)

        Returns:
            Process title string for setproctitle
        """
        base = "ComfyUI Server"
        if tag:
            return f"{base} - {tag}"
        return base

    def _get_target_main_py(
        self, tag: Optional[str] = None
    ) -> tuple[Optional[Path], Optional[str]]:
        """
        Resolve which main.py should be patched.

        Prefers the active managed version if one is selected, otherwise falls
        back to the legacy single-install location.
        """
        active_tag = None

        # Explicit tag override (used during installation)
        if tag and self.version_manager:
            try:
                version_path = self.version_manager.get_version_path(tag)
                if version_path:
                    main_py = version_path / "main.py"
                    if main_py.exists():
                        return main_py, tag
                    print(f"main.py not found for version {tag} at {main_py}")
                    return None, tag
            except Exception as e:
                print(f"Error resolving main.py for version {tag}: {e}")
                return None, tag

        if self.version_manager:
            try:
                active_tag = self.version_manager.get_active_version()
                if active_tag:
                    version_path = self.version_manager.get_active_version_path()
                    if version_path:
                        main_py = version_path / "main.py"
                        if main_py.exists():
                            return main_py, active_tag
                        print(f"main.py not found for active version {active_tag} at {main_py}")
                        return None, active_tag
            except Exception as e:
                print(f"Error determining active version for patching: {e}")
                return None, active_tag

        if self.main_py.exists():
            return self.main_py, None

        # Only print error if we have installed versions but can't find main.py
        # For fresh installs with no versions, silently return None
        if self.version_manager:
            try:
                installed_versions = self.version_manager.list_installed_versions()
                if installed_versions:
                    # We have versions but no main.py found - this is an error
                    print(f"No main.py found to patch at {self.main_py}")
            except Exception:
                pass  # Silently handle errors checking for installed versions

        return None, active_tag

    def _is_main_py_patched(self, main_py: Path, expected_title: Optional[str] = None) -> bool:
        """
        Check if the provided main.py is patched with setproctitle

        Args:
            main_py: Path to the target main.py
            expected_title: Optional exact title to look for
        """
        try:
            content = main_py.read_text()
            if expected_title:
                return (
                    f'setproctitle.setproctitle("{expected_title}")' in content
                    or f"setproctitle.setproctitle('{expected_title}')" in content
                )

            # Fallback: any ComfyUI Server setproctitle call (with or without version suffix)
            return bool(
                re.search(r'setproctitle\.setproctitle\(["\']ComfyUI Server[^"\']*["\']\)', content)
            )
        except Exception as e:
            print(f"Error reading {main_py} to check patch state: {e}")
            return False

    def is_patched(self, tag: Optional[str] = None) -> bool:
        """Check if selected main.py is patched with setproctitle"""
        main_py, _active_tag = self._get_target_main_py(tag)
        if not main_py:
            return False
        return self._is_main_py_patched(main_py)

    def patch_main_py(self, tag: Optional[str] = None) -> bool:
        """Patch selected main.py to set process title"""
        main_py, active_tag = self._get_target_main_py(tag)
        if not main_py:
            print("No active version found to patch. Select a version first.")
            return False

        server_title = self._build_server_title(active_tag)
        expected_line = f'setproctitle.setproctitle("{server_title}")'
        expected_line_single = f"setproctitle.setproctitle('{server_title}')"

        # Read existing content first to determine patch state
        try:
            content = main_py.read_text()
        except Exception as e:
            print(f"Error reading {main_py} for patching: {e}")
            return False

        # Already patched with the correct title - nothing to do
        if expected_line in content or expected_line_single in content:
            return False

        # Create backup
        backup = main_py.with_suffix(".py.bak")
        if not backup.exists():
            backup.write_bytes(main_py.read_bytes())

        # If an older patch exists, upgrade it to include the version-specific title
        pattern = r'setproctitle\.setproctitle\(["\']ComfyUI Server[^"\']*["\']\)'
        upgraded_content, replaced = re.subn(pattern, expected_line, content, count=1)
        if replaced:
            main_py.write_text(upgraded_content)
            return True

        # Insert patch code
        insert_code = (
            "\ntry:\n"
            "    import setproctitle\n"
            f'    setproctitle.setproctitle("{server_title}")\n'
            "except ImportError:\n"
            "    pass\n"
        )

        if 'if __name__ == "__main__":' in content:
            content = content.replace(
                'if __name__ == "__main__":', insert_code + 'if __name__ == "__main__":', 1
            )
        else:
            content += insert_code

        main_py.write_text(content)
        return True

    def revert_main_py(self, tag: Optional[str] = None) -> bool:
        """Revert selected main.py to original state"""
        main_py, active_tag = self._get_target_main_py(tag)
        if not main_py:
            print("No active version found to unpatch. Select a version first.")
            return False

        backup = main_py.with_suffix(".py.bak")

        # Try backup first
        if backup.exists():
            main_py.write_bytes(backup.read_bytes())
            backup.unlink(missing_ok=True)
            return True

        # Try git checkout (only if repo data is present)
        repo_dir = main_py.parent
        if (repo_dir / ".git").exists():
            try:
                subprocess.run(
                    ["git", "-C", str(repo_dir), "checkout", "--", main_py.name],
                    capture_output=True,
                    check=True,
                )
                return True
            except Exception:
                pass

        # Try downloading from GitHub
        try:
            ref = active_tag or "master"
            url = f"https://raw.githubusercontent.com/comfyanonymous/ComfyUI/{ref}/main.py"
            with urllib.request.urlopen(url, timeout=10) as resp:
                main_py.write_bytes(resp.read())
            return True
        except Exception:
            return False
