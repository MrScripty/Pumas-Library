#!/usr/bin/env python3
"""
ComfyUI Setup Launcher - Main Entry Point
Desktop application using PyWebView with React frontend
"""

import subprocess
import sys
from pathlib import Path

import webview

from backend.api import ComfyUISetupAPI
from backend.config import UI
from backend.logging_config import get_logger, setup_logging

# Initialize logging as early as possible
setup_logging(log_level="INFO", console_level="WARNING")
logger = get_logger(__name__)


class JavaScriptAPI:
    """
    JavaScript API Bridge
    All methods in this class are exposed to the JavaScript frontend via window.pywebview.api
    """

    def __init__(self):
        self.api = ComfyUISetupAPI()

    # ==================== Status Methods ====================

    def get_status(self):
        """Get complete system status - called from JavaScript"""
        return self.api.get_status()

    def get_disk_space(self):
        """Get disk space information - called from JavaScript"""
        return self.api.get_disk_space()

    # ==================== Action Methods ====================

    def install_deps(self):
        """Install missing dependencies"""
        try:
            success = self.api.install_missing_dependencies()
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def toggle_patch(self):
        """Toggle main.py patch"""
        try:
            success = self.api.toggle_patch()
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def toggle_menu(self, tag=None):
        """Toggle menu shortcut (active version when available)"""
        try:
            success = self.api.toggle_menu(tag)
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def toggle_desktop(self, tag=None):
        """Toggle desktop shortcut (active version when available)"""
        try:
            success = self.api.toggle_desktop(tag)
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def get_version_shortcuts(self, tag):
        """Get shortcut state for a specific version"""
        try:
            state = self.api.get_version_shortcut_state(tag)
            return {"success": True, "state": state}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "state": {}}

    def get_all_shortcut_states(self):
        """Get shortcut states for all versions"""
        try:
            states = self.api.get_all_shortcut_states()
            return {"success": True, "states": states}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "states": {}}

    def set_version_shortcuts(self, tag, enabled):
        """Enable/disable menu and desktop shortcuts for a version"""
        try:
            result = self.api.set_version_shortcuts(tag, bool(enabled))
            return {
                "success": result.get("success", False),
                "state": result.get("state"),
                "tag": tag,
                "error": result.get("error"),
            }
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "state": {}}

    def toggle_version_menu(self, tag):
        """Toggle only the menu shortcut for a version"""
        try:
            result = self.api.toggle_version_menu_shortcut(tag)
            return {
                "success": result.get("success", False),
                "state": result.get("state"),
                "tag": tag,
                "error": result.get("error"),
            }
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "state": {}}

    def toggle_version_desktop(self, tag):
        """Toggle only the desktop shortcut for a version"""
        try:
            result = self.api.toggle_version_desktop_shortcut(tag)
            return {
                "success": result.get("success", False),
                "state": result.get("state"),
                "tag": tag,
                "error": result.get("error"),
            }
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def close_window(self):
        """Close the application window and terminate the process"""
        try:
            # Cancel any ongoing installation before closing
            logger.info("Cleaning up before exit")
            if self.api.version_manager:
                # Check if there's an active installation
                progress = self.api.get_installation_progress()
                if progress and not progress.get("completed_at"):
                    logger.info("Active installation detected - cancelling")
                    self.api.cancel_installation()
                    # Give it a moment to clean up
                    import time

                    time.sleep(1)

            # Destroy all windows
            for window in webview.windows:
                window.destroy()
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            logger.error(f"Error during cleanup: {e}", exc_info=True)
        # Exit the application
        sys.exit(0)

    def launch_comfyui(self):
        """Launch ComfyUI using run.sh"""
        try:
            result = self.api.launch_comfyui()
            return {
                "success": result.get("success", False),
                "log_path": result.get("log_path"),
                "error": result.get("error"),
                "ready": result.get("ready"),
            }
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def stop_comfyui(self):
        """Stop running ComfyUI instance"""
        try:
            success = self.api.stop_comfyui()
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    # ==================== Version Management Methods (Phase 5) ====================

    def get_available_versions(self, force_refresh=False):
        """Get list of available ComfyUI versions from GitHub"""
        try:
            versions = self.api.get_available_versions(force_refresh)
            return {"success": True, "versions": versions}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "versions": []}

    def get_installed_versions(self):
        """Get list of installed ComfyUI version tags"""
        try:
            versions = self.api.get_installed_versions()
            return {"success": True, "versions": versions}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "versions": []}

    def validate_installations(self):
        """Validate all installations and clean up incomplete ones"""
        try:
            result = self.api.validate_installations()
            return {"success": True, "result": result}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {
                "success": False,
                "error": str(e),
                "result": {"had_invalid": False, "removed": [], "valid": []},
            }

    def get_installation_progress(self):
        """Get current installation progress (Phase 6.2.5b)"""
        try:
            progress = self.api.get_installation_progress()
            return progress  # Can be None if no installation in progress
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError):
            return None

    def install_version(self, tag):
        """Install a ComfyUI version"""
        try:
            # Note: progress_callback not supported via PyWebView API
            # Frontend should poll get_installation_progress() for progress
            success = self.api.install_version(tag)
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def cancel_installation(self):
        """Cancel the currently running installation"""
        try:
            success = self.api.cancel_installation()
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def calculate_release_size(self, tag, force_refresh=False):
        """Calculate total download size for a release (Phase 6.2.5c)"""
        try:
            result = self.api.calculate_release_size(tag, force_refresh)
            return result if result else None
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            logger.error(f"Error calculating release size: {e}", exc_info=True)
            return None

    def calculate_all_release_sizes(self):
        """Calculate sizes for all available releases (Phase 6.2.5c)"""
        try:
            results = self.api.calculate_all_release_sizes()
            return results
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            logger.error(f"Error calculating all release sizes: {e}", exc_info=True)
            return {}

    def remove_version(self, tag):
        """Remove an installed ComfyUI version"""
        try:
            success = self.api.remove_version(tag)
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def switch_version(self, tag):
        """Switch to a different ComfyUI version"""
        try:
            success = self.api.switch_version(tag)
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def get_active_version(self):
        """Get currently active ComfyUI version"""
        try:
            version = self.api.get_active_version()
            return {"success": True, "version": version}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "version": ""}

    def get_default_version(self):
        """Get configured default ComfyUI version"""
        try:
            version = self.api.get_default_version()
            return {"success": True, "version": version}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "version": ""}

    def set_default_version(self, tag=None):
        """Set the default ComfyUI version (pass None to clear)"""
        try:
            success = self.api.set_default_version(tag)
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def check_version_dependencies(self, tag):
        """Check dependency installation status for a version"""
        try:
            status = self.api.check_version_dependencies(tag)
            return {"success": True, "dependencies": status}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {
                "success": False,
                "error": str(e),
                "dependencies": {"installed": [], "missing": []},
            }

    def install_version_dependencies(self, tag):
        """Install dependencies for a ComfyUI version"""
        try:
            success = self.api.install_version_dependencies(tag)
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def get_version_status(self):
        """Get comprehensive status of all versions"""
        try:
            status = self.api.get_version_status()
            return {"success": True, "status": status}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "status": {}}

    def get_version_info(self, tag):
        """Get detailed information about a specific version"""
        try:
            info = self.api.get_version_info(tag)
            return {"success": True, "info": info}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "info": {}}

    def open_path(self, path):
        """Open an arbitrary path in the system file manager"""
        try:
            return self.api.open_path(path)
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def open_active_install(self):
        """Open the active ComfyUI installation directory"""
        try:
            return self.api.open_active_install()
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def open_url(self, url):
        """Open a URL in the system browser"""
        try:
            return self.api.open_url(url)
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def launch_version(self, tag, extra_args=None):
        """Launch a specific ComfyUI version"""
        try:
            result = self.api.launch_version(tag, extra_args)
            return {
                "success": result.get("success", False),
                "log_path": result.get("log_path"),
                "error": result.get("error"),
                "ready": result.get("ready"),
            }
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    # ==================== Resource Management Methods (Phase 5) ====================

    def get_models(self):
        """Get list of models in shared storage"""
        try:
            models = self.api.get_models()
            return {"success": True, "models": models}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "models": {}}

    def get_custom_nodes(self, version_tag):
        """Get list of custom nodes for a specific version"""
        try:
            nodes = self.api.get_custom_nodes(version_tag)
            return {"success": True, "nodes": nodes}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "nodes": []}

    def install_custom_node(self, git_url, version_tag, node_name=None):
        """Install a custom node for a specific version"""
        try:
            success = self.api.install_custom_node(git_url, version_tag, node_name)
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def update_custom_node(self, node_name, version_tag):
        """Update a custom node to latest version"""
        try:
            success = self.api.update_custom_node(node_name, version_tag)
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def remove_custom_node(self, node_name, version_tag):
        """Remove a custom node from a specific version"""
        try:
            success = self.api.remove_custom_node(node_name, version_tag)
            return {"success": success}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def scan_shared_storage(self):
        """Scan shared storage and get statistics"""
        try:
            result = self.api.scan_shared_storage()
            return {"success": True, "result": result}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "result": {}}

    # ==================== Size Calculation Methods (Phase 6.2.5a/c) ====================

    def get_release_size_info(self, tag, archive_size):
        """Get size information for a release"""
        try:
            info = self.api.get_release_size_info(tag, archive_size)
            return {"success": True, "info": info}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "info": None}

    def get_release_size_breakdown(self, tag):
        """Get size breakdown for display"""
        try:
            breakdown = self.api.get_release_size_breakdown(tag)
            return {"success": True, "breakdown": breakdown}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "breakdown": None}

    def get_release_dependencies(self, tag, top_n=None):
        """Get dependencies for a release sorted by size"""
        try:
            dependencies = self.api.get_release_dependencies(tag, top_n)
            return {"success": True, "dependencies": dependencies}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "dependencies": []}

    # ==================== Cache Status Methods ====================

    def get_github_cache_status(self):
        """Get GitHub releases cache status"""
        try:
            status = self.api.get_github_cache_status()
            return {"success": True, "status": status}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def has_background_fetch_completed(self):
        """Check if background fetch completed"""
        try:
            completed = self.api.has_background_fetch_completed()
            return {"success": True, "completed": completed}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "completed": False}

    def reset_background_fetch_flag(self):
        """Reset background fetch completion flag"""
        try:
            self.api.reset_background_fetch_flag()
            return {"success": True}
        except (AttributeError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    # ==================== Launcher Update Methods ====================

    def get_launcher_version(self):
        """Get current launcher version (git commit)"""
        try:
            from backend.__version__ import __branch__, __version__, is_git_repo

            return {
                "success": True,
                "version": __version__,
                "branch": __branch__,
                "isGitRepo": is_git_repo(),
            }
        except (AttributeError, ImportError, OSError) as e:
            return {"success": False, "error": str(e), "version": "unknown"}

    def check_launcher_updates(self, force_refresh=False):
        """Check if launcher updates are available"""
        try:
            # Initialize updater if not exists
            if not hasattr(self.api, "launcher_updater"):
                from backend.launcher_updater import LauncherUpdater

                self.api.launcher_updater = LauncherUpdater(self.api.metadata_manager)

            result = self.api.launcher_updater.check_for_updates(force_refresh)
            return {"success": True, **result}
        except (AttributeError, ImportError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e), "hasUpdate": False}

    def apply_launcher_update(self):
        """Apply launcher update (pull + rebuild)"""
        try:
            if not hasattr(self.api, "launcher_updater"):
                from backend.launcher_updater import LauncherUpdater

                self.api.launcher_updater = LauncherUpdater(self.api.metadata_manager)

            result = self.api.launcher_updater.apply_update()
            return {"success": result.get("success", False), **result}
        except (AttributeError, ImportError, OSError, RuntimeError, TypeError, ValueError) as e:
            return {"success": False, "error": str(e)}

    def restart_launcher(self):
        """Restart the launcher application"""
        import os

        try:
            # Get the launcher script path
            launcher_root = Path(__file__).parent.parent
            launcher_script = launcher_root / "launcher"

            if launcher_script.exists():
                # Restart via launcher script
                subprocess.Popen([str(launcher_script)], start_new_session=True)
            else:
                # Restart Python directly
                python = sys.executable
                subprocess.Popen(
                    [python, str(launcher_root / "backend" / "main.py")], start_new_session=True
                )

            # Exit current process after a brief delay
            import threading

            def delayed_exit():
                import time

                time.sleep(1)
                os._exit(0)

            threading.Thread(target=delayed_exit, daemon=True).start()

            return {"success": True, "message": "Restarting..."}
        except (OSError, subprocess.SubprocessError, FileNotFoundError) as e:
            return {"success": False, "error": str(e)}


def get_entrypoint():
    """
    Get the entry point for the web content
    Returns either the built frontend or development server URL
    """
    # Determine base directory
    if getattr(sys, "frozen", False):
        # Running as PyInstaller bundle - use extracted temp directory
        base_dir = Path(sys._MEIPASS)
    else:
        # Running in development mode
        base_dir = Path(__file__).parent.parent

    dist_dir = base_dir / "frontend" / "dist"
    index_html = dist_dir / "index.html"

    if index_html.exists():
        # Production mode: serve from built files
        return str(index_html.resolve())
    else:
        # Development mode: connect to Vite dev server
        # User should run `npm run dev` in frontend/ directory first
        return "http://127.0.0.1:3000"


def main():
    """Main application entry point"""
    try:
        import setproctitle

        setproctitle.setproctitle("Linux AI Launcher")
    except (AttributeError, ImportError, OSError):
        pass

    # Parse command-line arguments for debug mode
    debug_mode = "--debug" in sys.argv or "--dev" in sys.argv

    # Create JavaScript API instance
    js_api = JavaScriptAPI()

    # Get entry point (production build or dev server)
    entry = get_entrypoint()

    # Determine if we're in development mode
    is_dev = entry.startswith("http://")

    if is_dev:
        print("=" * 60)  # noqa: print
        print("DEVELOPMENT MODE")  # noqa: print
        print("=" * 60)  # noqa: print
        print(f"Connecting to development server at: {entry}")  # noqa: print
        print("Make sure you have run 'npm run dev' in the frontend/ directory")  # noqa: print
        print("=" * 60)  # noqa: print
        logger.info(f"Development mode: connecting to {entry}")

    if debug_mode:
        print("Developer console enabled (--debug flag)")  # noqa: print
        logger.info("Debug mode enabled")

    # Create and configure the webview window
    window = webview.create_window(
        title="ComfyUI Setup",
        url=entry,
        js_api=js_api,
        width=UI.WINDOW_WIDTH,
        height=UI.WINDOW_HEIGHT,
        resizable=False,
        frameless=True,
        easy_drag=True,
        background_color="#000000",
    )

    # Start the webview application
    # Use 'gtk' backend on Linux for best compatibility with Debian/Mint
    # Enable debug console only if --debug or --dev flag is passed
    webview.start(debug=debug_mode, gui="gtk")


if __name__ == "__main__":
    main()
