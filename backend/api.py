#!/usr/bin/env python3
"""
ComfyUI Setup API - Backend business logic
Handles all ComfyUI setup operations without UI dependencies
"""

import os
import sys
import subprocess
import shutil
import urllib.request
import json
import tomllib
from pathlib import Path
from typing import Dict, List, Any, Optional


class ComfyUISetupAPI:
    """Main API class for ComfyUI setup operations"""

    def __init__(self):
        # Determine directories based on launcher location
        # Handle both development mode and PyInstaller bundled mode
        if getattr(sys, 'frozen', False):
            # Running as PyInstaller bundle
            # Search upward from executable location to find ComfyUI root
            self.comfyui_dir = self._find_comfyui_root(Path(sys.executable).parent)
            # Launcher directory is where run.sh and icon should be
            # Try common locations
            launcher_candidates = [
                self.comfyui_dir / "Linux-ComfyUI-Launcher",
                Path(sys.executable).parent.parent,  # dist/ parent
                Path(sys.executable).parent,  # same dir as executable
            ]
            self.script_dir = None
            for candidate in launcher_candidates:
                if candidate.exists():
                    self.script_dir = candidate
                    break
            if not self.script_dir:
                # Fallback to executable directory
                self.script_dir = Path(sys.executable).parent
        else:
            # Running in development mode
            self.script_dir = Path(__file__).parent.parent.resolve()
            self.comfyui_dir = self.script_dir.parent

        self.main_py = self.comfyui_dir / "main.py"
        self.icon_webp = self.script_dir / "comfyui-icon.webp"
        self.run_sh = self.script_dir / "run.sh"

        # System directories
        self.apps_dir = Path.home() / ".local" / "share" / "applications"
        self.apps_file = self.apps_dir / "ComfyUI.desktop"
        self.desktop_file = Path.home() / "Desktop" / "ComfyUI.desktop"

        # Initialize version management components (Phase 2-4)
        self._init_version_management()

    def _init_version_management(self):
        """Initialize version management components"""
        try:
            from backend.metadata_manager import MetadataManager
            from backend.github_integration import GitHubReleasesFetcher
            from backend.resource_manager import ResourceManager
            from backend.version_manager import VersionManager
            from backend.release_data_fetcher import ReleaseDataFetcher
            from backend.package_size_resolver import PackageSizeResolver
            from backend.release_size_calculator import ReleaseSizeCalculator

            launcher_data_dir = self.script_dir / "launcher-data"
            cache_dir = launcher_data_dir / "cache"

            self.metadata_manager = MetadataManager(launcher_data_dir)
            self.github_fetcher = GitHubReleasesFetcher(self.metadata_manager)
            self.resource_manager = ResourceManager(self.script_dir, self.metadata_manager)
            self.version_manager = VersionManager(
                self.script_dir,
                self.metadata_manager,
                self.github_fetcher,
                self.resource_manager
            )

            # Initialize size calculation components (Phase 6.2.5a)
            self.release_data_fetcher = ReleaseDataFetcher(cache_dir)
            self.package_size_resolver = PackageSizeResolver(cache_dir)
            self.release_size_calculator = ReleaseSizeCalculator(
                cache_dir,
                self.release_data_fetcher,
                self.package_size_resolver
            )
        except Exception as e:
            print(f"Warning: Version management initialization failed: {e}")
            self.metadata_manager = None
            self.github_fetcher = None
            self.resource_manager = None
            self.version_manager = None

    def _find_comfyui_root(self, start_path: Path) -> Path:
        """
        Search upward from start_path to find ComfyUI root directory.
        ComfyUI root is identified by the presence of main.py and pyproject.toml.
        """
        current = start_path.resolve()

        # Search up to 5 levels
        for _ in range(5):
            main_py = current / "main.py"
            pyproject = current / "pyproject.toml"

            # Check if both files exist
            if main_py.exists() and pyproject.exists():
                # Verify it's ComfyUI by checking pyproject.toml
                try:
                    with open(pyproject, 'rb') as f:
                        data = tomllib.load(f)
                        if data.get('project', {}).get('name') == 'ComfyUI':
                            return current
                except Exception:
                    pass

            # Move up one directory
            parent = current.parent
            if parent == current:
                # Reached filesystem root
                break
            current = parent

        # Fallback: return the parent of start_path
        return start_path.parent

    # ==================== Version Detection ====================

    def get_comfyui_version(self) -> str:
        """Get ComfyUI version from pyproject.toml, git, or GitHub API"""
        # Try reading from pyproject.toml first
        pyproject_path = self.comfyui_dir / "pyproject.toml"
        if pyproject_path.exists():
            try:
                with open(pyproject_path, 'rb') as f:
                    data = tomllib.load(f)
                    version = data.get('project', {}).get('version')
                    if version:
                        return version
            except Exception:
                pass

        # Try git describe
        try:
            version = subprocess.check_output(
                ['git', '-C', str(self.comfyui_dir), 'describe', '--tags', '--always'],
                text=True,
                stderr=subprocess.DEVNULL
            ).strip()
            if version:
                return version
        except Exception:
            pass

        # Fallback to GitHub API
        try:
            with urllib.request.urlopen(
                "https://api.github.com/repos/comfyanonymous/ComfyUI/releases/latest",
                timeout=5
            ) as resp:
                data = json.loads(resp.read())
                return data['tag_name'] + " (latest)"
        except Exception:
            pass

        return "Unknown"

    def check_for_new_release(self) -> Dict[str, Any]:
        """Check if a new release is available on GitHub"""
        try:
            # Get current local version
            current_version = None
            current_tag = None

            try:
                # Try to get the exact tag first
                current_tag = subprocess.check_output(
                    ['git', '-C', str(self.comfyui_dir), 'describe', '--tags', '--exact-match'],
                    text=True,
                    stderr=subprocess.DEVNULL
                ).strip()
                current_version = current_tag
            except Exception:
                # If not on an exact tag, get the description
                try:
                    current_version = subprocess.check_output(
                        ['git', '-C', str(self.comfyui_dir), 'describe', '--tags', '--always'],
                        text=True,
                        stderr=subprocess.DEVNULL
                    ).strip()
                    # Extract just the tag part (before any -N-hash suffix)
                    if '-' in current_version:
                        current_tag = current_version.split('-')[0]
                    else:
                        current_tag = current_version
                except Exception:
                    pass

            # Get latest release from GitHub
            with urllib.request.urlopen(
                "https://api.github.com/repos/comfyanonymous/ComfyUI/releases/latest",
                timeout=3
            ) as resp:
                data = json.loads(resp.read())
                latest_tag = data.get('tag_name', '')

                # Compare versions - check if current tag differs from latest
                if current_tag and latest_tag:
                    has_update = current_tag != latest_tag
                    return {
                        "has_update": has_update,
                        "latest_version": latest_tag,
                        "current_version": current_version or current_tag
                    }
                else:
                    return {
                        "has_update": False,
                        "latest_version": latest_tag,
                        "current_version": current_version
                    }
        except Exception as e:
            print(f"Error checking for new release: {e}")
            return {
                "has_update": False,
                "latest_version": None,
                "current_version": None
            }

    # ==================== Dependency Checking ====================

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

    # ==================== Patch Management ====================

    def is_patched(self) -> bool:
        """Check if main.py is patched with setproctitle"""
        if not self.main_py.exists():
            return False
        content = self.main_py.read_text()
        return 'setproctitle.setproctitle("ComfyUI Server")' in content

    def patch_main_py(self) -> bool:
        """Patch main.py to set process title"""
        if self.is_patched():
            return False

        # Create backup
        backup = self.main_py.with_suffix(".py.bak")
        if not backup.exists():
            backup.write_bytes(self.main_py.read_bytes())

        # Insert patch code
        content = self.main_py.read_text()
        insert_code = (
            '\ntry:\n'
            '    import setproctitle\n'
            '    setproctitle.setproctitle("ComfyUI Server")\n'
            'except ImportError:\n'
            '    pass\n'
        )

        if 'if __name__ == "__main__":' in content:
            content = content.replace(
                'if __name__ == "__main__":',
                insert_code + 'if __name__ == "__main__":'
            )
        else:
            content += insert_code

        self.main_py.write_text(content)
        return True

    def revert_main_py(self) -> bool:
        """Revert main.py to original state"""
        backup = self.main_py.with_suffix(".py.bak")

        # Try backup first
        if backup.exists():
            self.main_py.write_bytes(backup.read_bytes())
            backup.unlink(missing_ok=True)
            return True

        # Try git checkout
        try:
            subprocess.run(
                ['git', '-C', str(self.comfyui_dir), 'checkout', '--', 'main.py'],
                capture_output=True,
                check=True
            )
            return True
        except Exception:
            pass

        # Try downloading from GitHub
        try:
            url = "https://raw.githubusercontent.com/comfyanonymous/ComfyUI/master/main.py"
            with urllib.request.urlopen(url, timeout=10) as resp:
                self.main_py.write_bytes(resp.read())
            return True
        except Exception:
            return False

    # ==================== Shortcut Management ====================

    def menu_exists(self) -> bool:
        """Check if menu shortcut exists"""
        return self.apps_file.exists()

    def desktop_exists(self) -> bool:
        """Check if desktop shortcut exists"""
        return self.desktop_file.exists()

    def install_icon(self) -> bool:
        """Install icon to system icon directory"""
        try:
            if not self.icon_webp.exists():
                return False

            # Try to convert webp to png using ImageMagick or just copy the webp
            icon_base_dir = Path.home() / ".local" / "share" / "icons" / "hicolor"

            # Try converting webp to PNG for better compatibility
            png_sizes = [256, 128, 64, 48]
            conversion_success = False

            for size in png_sizes:
                try:
                    icon_dir = icon_base_dir / f"{size}x{size}" / "apps"
                    icon_dir.mkdir(parents=True, exist_ok=True)
                    dest_icon = icon_dir / "comfyui.png"

                    # Try ImageMagick convert
                    result = subprocess.run(
                        ['convert', str(self.icon_webp), '-resize', f'{size}x{size}', str(dest_icon)],
                        capture_output=True,
                        timeout=10
                    )
                    if result.returncode == 0:
                        conversion_success = True
                except Exception:
                    pass

            # If conversion failed, try copying webp as fallback
            if not conversion_success:
                # Copy to scalable directory as webp
                icon_dir = icon_base_dir / "scalable" / "apps"
                icon_dir.mkdir(parents=True, exist_ok=True)
                dest_icon = icon_dir / "comfyui.webp"
                shutil.copy2(self.icon_webp, dest_icon)

                # Also try to create a symlink with .png extension for compatibility
                png_link = icon_dir / "comfyui.png"
                try:
                    if png_link.exists():
                        png_link.unlink()
                    png_link.symlink_to(dest_icon)
                except Exception:
                    pass

            # Update icon cache if available
            try:
                subprocess.run(['gtk-update-icon-cache', '-f', '-t', str(icon_base_dir)],
                              capture_output=True, timeout=5)
            except Exception:
                pass

            # Also try xdg-icon-resource as alternative installation method
            try:
                subprocess.run(['xdg-icon-resource', 'install', '--novendor', '--size', '256',
                               str(self.icon_webp), 'comfyui'],
                              capture_output=True, timeout=5)
            except Exception:
                pass

            return True
        except Exception as e:
            print(f"Error installing icon: {e}")
            return False

    def create_menu_shortcut(self) -> bool:
        """Create application menu shortcut"""
        # Check if run.sh exists
        if not self.run_sh.exists():
            print(f"Warning: run.sh not found at {self.run_sh}")
            print("Shortcut will be created but may not work until run.sh is present.")

        # Install icon to system directory
        self.install_icon()

        self.apps_dir.mkdir(parents=True, exist_ok=True)

        # Use the installed system icon instead of direct path
        icon_line = "Icon=comfyui"  # Use icon name instead of full path

        content = f"""[Desktop Entry]
Name=ComfyUI
Comment=Launch ComfyUI with isolated Brave window
Exec=bash "{self.run_sh.resolve()}"
{icon_line}
Terminal=false
Type=Application
Categories=Graphics;ArtificialIntelligence;
"""

        self.apps_file.write_text(content)
        self.apps_file.chmod(0o644)
        return True

    def create_desktop_shortcut(self) -> bool:
        """Create desktop shortcut"""
        if self.desktop_exists():
            return False

        # Check if run.sh exists
        if not self.run_sh.exists():
            print(f"Warning: run.sh not found at {self.run_sh}")
            print("Shortcut will be created but may not work until run.sh is present.")

        # Install icon to system directory
        self.install_icon()

        # Use the installed system icon instead of direct path
        icon_line = "Icon=comfyui"  # Use icon name instead of full path

        content = f"""[Desktop Entry]
Name=ComfyUI
Comment=Launch ComfyUI with isolated Brave window
Exec=bash "{self.run_sh.resolve()}"
{icon_line}
Terminal=false
Type=Application
Categories=Graphics;ArtificialIntelligence;
"""

        self.desktop_file.write_text(content)
        self.desktop_file.chmod(0o755)
        return True

    def remove_menu_shortcut(self) -> bool:
        """Remove application menu shortcut"""
        if self.apps_file.exists():
            self.apps_file.unlink()
            return True
        return False

    def remove_desktop_shortcut(self) -> bool:
        """Remove desktop shortcut"""
        if self.desktop_file.exists():
            self.desktop_file.unlink()
            return True
        return False

    # ==================== Dependency Installation ====================

    def install_missing_dependencies(self) -> bool:
        """Install missing dependencies (requires user interaction for sudo)"""
        missing = self.get_missing_dependencies()
        if not missing:
            return True

        success = True

        # Install Python packages
        if "setproctitle" in missing:
            try:
                subprocess.run(
                    ['pip3', 'install', '--user', 'setproctitle'],
                    check=True,
                    stdout=subprocess.DEVNULL
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

    # ==================== Status API ====================

    def get_status(self) -> Dict[str, Any]:
        """Get complete system status"""
        missing_deps = self.get_missing_dependencies()
        deps_ready = len(missing_deps) == 0
        patched = self.is_patched()
        menu = self.menu_exists()
        desktop = self.desktop_exists()
        running = self.is_comfyui_running()

        # Check for new releases
        release_info = self.check_for_new_release()

        # Determine status message
        if running:
            message = "ComfyUI is running"
        elif not deps_ready:
            message = "Missing dependencies detected."
        elif deps_ready and patched and menu and desktop:
            message = "Setup complete – everything is ready"
        else:
            message = "System ready. Configure options below."

        return {
            "version": self.get_comfyui_version(),
            "deps_ready": deps_ready,
            "missing_deps": missing_deps,
            "patched": patched,
            "menu_shortcut": menu,
            "desktop_shortcut": desktop,
            "comfyui_running": running,
            "message": message,
            "release_info": release_info
        }

    # ==================== Action Handlers ====================

    def toggle_patch(self) -> bool:
        """Toggle main.py patch"""
        if self.is_patched():
            return self.revert_main_py()
        else:
            return self.patch_main_py()

    def toggle_menu(self) -> bool:
        """Toggle menu shortcut"""
        if self.menu_exists():
            return self.remove_menu_shortcut()
        else:
            return self.create_menu_shortcut()

    def toggle_desktop(self) -> bool:
        """Toggle desktop shortcut"""
        if self.desktop_exists():
            return self.remove_desktop_shortcut()
        else:
            return self.create_desktop_shortcut()

    def is_comfyui_running(self) -> bool:
        """Check if ComfyUI is currently running"""
        try:
            # Method 1: Check for PID file (created by run.sh)
            pid_file = self.comfyui_dir / "comfyui.pid"
            if pid_file.exists():
                try:
                    pid = int(pid_file.read_text().strip())
                    # Check if process with this PID exists
                    os.kill(pid, 0)  # Signal 0 just checks if process exists
                    return True
                except (ValueError, ProcessLookupError, OSError):
                    # PID file is stale
                    pass

            # Method 2: Search for process by name (if patched)
            if self.is_patched():
                try:
                    result = subprocess.run(
                        ['pgrep', '-f', 'ComfyUI Server'],
                        capture_output=True,
                        text=True
                    )
                    return result.returncode == 0 and result.stdout.strip()
                except Exception:
                    pass

            return False
        except Exception:
            return False

    def stop_comfyui(self) -> bool:
        """Stop running ComfyUI instance"""
        try:
            import time

            # First, kill the Brave browser process running ComfyUI
            try:
                # Find and kill Brave processes with ComfyUI in the command line
                result = subprocess.run(
                    ['pgrep', '-f', 'brave.*--app=http://127.0.0.1'],
                    capture_output=True,
                    text=True,
                    timeout=5
                )

                if result.returncode == 0 and result.stdout.strip():
                    # Kill each Brave process found
                    pids = result.stdout.strip().split('\n')
                    for pid in pids:
                        try:
                            os.kill(int(pid), 9)  # SIGKILL - force kill immediately
                        except (ValueError, ProcessLookupError):
                            pass
            except Exception:
                pass  # Continue even if this fails

            # Stop the ComfyUI server
            pid_file = self.comfyui_dir / "comfyui.pid"
            if pid_file.exists():
                try:
                    pid = int(pid_file.read_text().strip())
                    os.kill(pid, 15)  # SIGTERM for graceful shutdown
                    # Wait a moment
                    time.sleep(1)
                    # Force kill if still running
                    try:
                        os.kill(pid, 9)  # SIGKILL
                    except ProcessLookupError:
                        pass  # Already dead
                    pid_file.unlink(missing_ok=True)
                    return True
                except (ValueError, ProcessLookupError):
                    pid_file.unlink(missing_ok=True)

            # Try by process name if patched
            if self.is_patched():
                try:
                    subprocess.run(['pkill', '-9', '-f', 'ComfyUI Server'], check=False)
                    return True
                except Exception:
                    pass

            return False
        except Exception as e:
            print(f"Error stopping ComfyUI: {e}")
            return False

    def launch_comfyui(self) -> bool:
        """Launch ComfyUI using run.sh script"""
        try:
            if not self.run_sh.exists():
                print(f"Error: run.sh not found at {self.run_sh}")
                return False

            # Check if already running
            if self.is_comfyui_running():
                print("ComfyUI is already running")
                return False

            # Launch run.sh in the background
            subprocess.Popen(
                ['bash', str(self.run_sh)],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
                start_new_session=True
            )
            return True
        except Exception as e:
            print(f"Error launching ComfyUI: {e}")
            return False

    # ==================== Version Management API (Phase 5) ====================

    def get_available_versions(self, force_refresh: bool = False) -> List[Dict[str, Any]]:
        """
        Get list of available ComfyUI versions from GitHub

        Args:
            force_refresh: Force refresh from GitHub API (bypass cache)

        Returns:
            List of release dictionaries
        """
        if not self.version_manager:
            return []
        return self.version_manager.get_available_releases(force_refresh)

    def get_installed_versions(self) -> List[str]:
        """
        Get list of installed ComfyUI version tags

        Returns:
            List of version tags (e.g., ['v0.2.0', 'v0.1.5'])
        """
        if not self.version_manager:
            return []
        return self.version_manager.get_installed_versions()

    def get_installation_progress(self) -> Optional[Dict[str, Any]]:
        """
        Get current installation progress (Phase 6.2.5b)

        Returns:
            Progress state dict or None if no installation in progress
        """
        if not self.version_manager:
            return None
        return self.version_manager.get_installation_progress()

    def install_version(self, tag: str, progress_callback=None) -> bool:
        """
        Install a ComfyUI version

        Args:
            tag: Version tag to install (e.g., 'v0.2.0')
            progress_callback: Optional callback for progress updates

        Returns:
            True if installation successful
        """
        if not self.version_manager:
            return False
        return self.version_manager.install_version(tag, progress_callback)

    def remove_version(self, tag: str) -> bool:
        """
        Remove an installed ComfyUI version

        Args:
            tag: Version tag to remove

        Returns:
            True if removal successful
        """
        if not self.version_manager:
            return False
        return self.version_manager.remove_version(tag)

    def switch_version(self, tag: str) -> bool:
        """
        Switch to a different ComfyUI version

        Args:
            tag: Version tag to switch to

        Returns:
            True if switch successful
        """
        if not self.version_manager:
            return False
        return self.version_manager.set_active_version(tag)

    def get_active_version(self) -> str:
        """
        Get currently active ComfyUI version

        Returns:
            Active version tag or empty string if none
        """
        if not self.version_manager:
            return ""
        return self.version_manager.get_active_version() or ""

    def check_version_dependencies(self, tag: str) -> Dict[str, Any]:
        """
        Check dependency installation status for a version

        Args:
            tag: Version tag to check

        Returns:
            Dict with 'installed' and 'missing' lists
        """
        if not self.version_manager:
            return {"installed": [], "missing": []}
        return self.version_manager.check_dependencies(tag)

    def install_version_dependencies(self, tag: str, progress_callback=None) -> bool:
        """
        Install dependencies for a ComfyUI version

        Args:
            tag: Version tag to install dependencies for
            progress_callback: Optional callback for progress updates

        Returns:
            True if installation successful
        """
        if not self.version_manager:
            return False
        return self.version_manager.install_dependencies(tag, progress_callback)

    def get_version_status(self) -> Dict[str, Any]:
        """
        Get comprehensive status of all versions

        Returns:
            Dict with version status information
        """
        if not self.version_manager:
            return {
                "installedCount": 0,
                "activeVersion": None,
                "versions": {}
            }
        return self.version_manager.get_version_status()

    def get_version_info(self, tag: str) -> Dict[str, Any]:
        """
        Get detailed information about a specific version

        Args:
            tag: Version tag

        Returns:
            Dict with version information
        """
        if not self.version_manager:
            return {}
        return self.version_manager.get_version_info(tag)

    def launch_version(self, tag: str, extra_args: List[str] = None) -> bool:
        """
        Launch a specific ComfyUI version

        Args:
            tag: Version tag to launch
            extra_args: Optional additional command line arguments

        Returns:
            True if launch successful
        """
        if not self.version_manager:
            return False
        success, process = self.version_manager.launch_version(tag, extra_args)
        return success

    # ==================== Resource Management API (Phase 5) ====================

    def get_models(self) -> Dict[str, Any]:
        """
        Get list of models in shared storage

        Returns:
            Dict mapping model paths to model info
        """
        if not self.resource_manager:
            return {}
        return self.resource_manager.get_models()

    def get_custom_nodes(self, version_tag: str) -> List[str]:
        """
        Get list of custom nodes for a specific version

        Args:
            version_tag: Version tag to get custom nodes for

        Returns:
            List of custom node names
        """
        if not self.resource_manager:
            return []

        return self.resource_manager.list_version_custom_nodes(version_tag)

    def install_custom_node(self, git_url: str, version_tag: str, node_name: str = None) -> bool:
        """
        Install a custom node for a specific version

        Args:
            git_url: Git repository URL
            version_tag: ComfyUI version tag
            node_name: Optional custom node name

        Returns:
            True if installation successful
        """
        if not self.resource_manager:
            return False
        return self.resource_manager.install_custom_node(git_url, version_tag, node_name)

    def update_custom_node(self, node_name: str, version_tag: str) -> bool:
        """
        Update a custom node to latest version

        Args:
            node_name: Custom node directory name
            version_tag: ComfyUI version tag

        Returns:
            True if update successful
        """
        if not self.resource_manager:
            return False
        return self.resource_manager.update_custom_node(node_name, version_tag)

    def remove_custom_node(self, node_name: str, version_tag: str) -> bool:
        """
        Remove a custom node from a specific version

        Args:
            node_name: Custom node directory name
            version_tag: ComfyUI version tag

        Returns:
            True if removal successful
        """
        if not self.resource_manager:
            return False
        return self.resource_manager.remove_custom_node(node_name, version_tag)

    def scan_shared_storage(self) -> Dict[str, Any]:
        """
        Scan shared storage and get statistics

        Returns:
            Dict with scan results
        """
        if not self.resource_manager:
            return {
                "modelCount": 0,
                "totalSize": 0,
                "categoryCounts": {}
            }
        return self.resource_manager.scan_shared_storage()

    def get_release_size_info(self, tag: str, archive_size: int) -> Optional[Dict[str, Any]]:
        """
        Get size information for a release (Phase 6.2.5a/c)

        Args:
            tag: Release tag
            archive_size: Size of the archive in bytes

        Returns:
            Dict with size breakdown or None if not available
        """
        if not hasattr(self, 'release_size_calculator'):
            return None

        try:
            # Calculate release size (uses cache if available)
            result = self.release_size_calculator.calculate_release_size(tag, archive_size)
            return result
        except Exception as e:
            print(f"Error calculating release size: {e}")
            return None

    def get_release_size_breakdown(self, tag: str) -> Optional[Dict[str, Any]]:
        """
        Get size breakdown for display (Phase 6.2.5c)

        Args:
            tag: Release tag

        Returns:
            Dict with formatted size breakdown or None if not available
        """
        if not hasattr(self, 'release_size_calculator'):
            return None

        try:
            return self.release_size_calculator.get_size_breakdown(tag)
        except Exception as e:
            print(f"Error getting size breakdown: {e}")
            return None

    def get_release_dependencies(self, tag: str, top_n: Optional[int] = None) -> List[Dict[str, Any]]:
        """
        Get dependencies for a release sorted by size (Phase 6.2.5c)

        Args:
            tag: Release tag
            top_n: Optional limit to top N packages

        Returns:
            List of dependency dicts sorted by size (largest first)
        """
        if not hasattr(self, 'release_size_calculator'):
            return []

        try:
            return self.release_size_calculator.get_sorted_dependencies(tag, top_n)
        except Exception as e:
            print(f"Error getting dependencies: {e}")
            return []
