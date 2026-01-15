#!/usr/bin/env python3
"""
ComfyUI Setup Launcher - JavaScript API Bridge
Provides the API class used by the RPC server for Electron IPC.

NOTE: PyWebView has been removed. This file is now only used by rpc_server.py.
For the Electron main process, see electron/src/main.ts
"""

import subprocess
import sys
from pathlib import Path
from typing import Any, Callable

from backend.api import ComfyUISetupAPI
from backend.logging_config import get_logger, setup_logging

# Initialize logging as early as possible
setup_logging(log_level="INFO", console_level="WARNING")
logger = get_logger(__name__)


class JavaScriptAPI:
    """
    JavaScript API Bridge

    This class provides the API methods that are exposed to the frontend.
    In Electron mode, these are called via JSON-RPC from the Python sidecar (rpc_server.py).
    """

    def __init__(self):
        self.api = ComfyUISetupAPI()

    def _call_api(
        self,
        action: str,
        func: Callable[[], Any],
        on_error: Callable[[Exception], Any],
    ) -> Any:
        try:
            return func()
        except Exception as exc:  # noqa: generic-exception
            # Catch all standard exceptions to prevent cascading failures
            # This ensures the launcher can still report errors gracefully
            logger.error("%s failed: %s", action, exc, exc_info=True)
            return on_error(exc)

    # ==================== Status Methods ====================

    def get_status(self):
        """Get complete system status - called from JavaScript"""
        return self.api.get_status()

    def get_disk_space(self):
        """Get disk space information - called from JavaScript"""
        return self.api.get_disk_space()

    def get_system_resources(self):
        """Get current system resource usage (CPU, GPU, RAM, Disk) - called from JavaScript"""
        return self.api.get_system_resources()

    # ==================== Action Methods ====================

    def install_deps(self):
        """Install missing dependencies"""
        return self._call_api(
            "install_deps",
            lambda: {"success": self.api.install_missing_dependencies()},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def toggle_patch(self):
        """Toggle main.py patch"""
        return self._call_api(
            "toggle_patch",
            lambda: {"success": self.api.toggle_patch()},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def toggle_menu(self, tag=None):
        """Toggle menu shortcut (active version when available)"""
        return self._call_api(
            "toggle_menu",
            lambda: {"success": self.api.toggle_menu(tag)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def toggle_desktop(self, tag=None):
        """Toggle desktop shortcut (active version when available)"""
        return self._call_api(
            "toggle_desktop",
            lambda: {"success": self.api.toggle_desktop(tag)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def get_version_shortcuts(self, tag):
        """Get shortcut state for a specific version"""
        return self._call_api(
            "get_version_shortcuts",
            lambda: {"success": True, "state": self.api.get_version_shortcut_state(tag)},
            lambda exc: {"success": False, "error": str(exc), "state": {}},
        )

    def get_all_shortcut_states(self):
        """Get shortcut states for all versions"""
        return self._call_api(
            "get_all_shortcut_states",
            lambda: {"success": True, "states": self.api.get_all_shortcut_states()},
            lambda exc: {"success": False, "error": str(exc), "states": {}},
        )

    def set_version_shortcuts(self, tag, enabled):
        """Enable/disable menu and desktop shortcuts for a version"""

        def _do():
            result = self.api.set_version_shortcuts(tag, bool(enabled))
            return {
                "success": result.get("success", False),
                "state": result.get("state"),
                "tag": tag,
                "error": result.get("error"),
            }

        return self._call_api(
            "set_version_shortcuts",
            _do,
            lambda exc: {"success": False, "error": str(exc), "state": {}},
        )

    def toggle_version_menu(self, tag):
        """Toggle only the menu shortcut for a version"""

        def _do():
            result = self.api.toggle_version_menu_shortcut(tag)
            return {
                "success": result.get("success", False),
                "state": result.get("state"),
                "tag": tag,
                "error": result.get("error"),
            }

        return self._call_api(
            "toggle_version_menu",
            _do,
            lambda exc: {"success": False, "error": str(exc), "state": {}},
        )

    def toggle_version_desktop(self, tag):
        """Toggle only the desktop shortcut for a version"""

        def _do():
            result = self.api.toggle_version_desktop_shortcut(tag)
            return {
                "success": result.get("success", False),
                "state": result.get("state"),
                "tag": tag,
                "error": result.get("error"),
            }

        return self._call_api(
            "toggle_version_desktop",
            _do,
            lambda exc: {"success": False, "error": str(exc)},
        )

    def close_window(self):
        """Close the application and terminate the process.

        In Electron mode, window closure is handled by Electron's main process.
        This method handles cleanup and signals the RPC server to shut down.
        """

        def _do():
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

            return {"success": True}

        result = self._call_api("close_window", _do, lambda _exc: {"success": False})
        # Signal shutdown - Electron will handle the actual window close
        sys.exit(0)

    def launch_comfyui(self):
        """Launch ComfyUI using run.sh"""

        def _do():
            result = self.api.launch_comfyui()
            return {
                "success": result.get("success", False),
                "log_path": result.get("log_path"),
                "error": result.get("error"),
                "ready": result.get("ready"),
            }

        return self._call_api(
            "launch_comfyui", _do, lambda exc: {"success": False, "error": str(exc)}
        )

    def stop_comfyui(self):
        """Stop running ComfyUI instance"""
        return self._call_api(
            "stop_comfyui",
            lambda: {"success": self.api.stop_comfyui()},
            lambda exc: {"success": False, "error": str(exc)},
        )

    # ==================== Version Management Methods (Phase 5) ====================

    def get_available_versions(self, force_refresh=False, app_id=None):
        """Get list of available versions from GitHub"""
        return self._call_api(
            "get_available_versions",
            lambda: {
                "success": True,
                "versions": self.api.get_available_versions(force_refresh, app_id),
            },
            lambda exc: {"success": False, "error": str(exc), "versions": []},
        )

    def get_installed_versions(self, app_id=None):
        """Get list of installed version tags"""
        return self._call_api(
            "get_installed_versions",
            lambda: {"success": True, "versions": self.api.get_installed_versions(app_id)},
            lambda exc: {"success": False, "error": str(exc), "versions": []},
        )

    def validate_installations(self, app_id=None):
        """Validate all installations and clean up incomplete ones"""
        return self._call_api(
            "validate_installations",
            lambda: {"success": True, "result": self.api.validate_installations(app_id)},
            lambda exc: {
                "success": False,
                "error": str(exc),
                "result": {"had_invalid": False, "removed": [], "valid": []},
            },
        )

    def get_installation_progress(self, app_id=None):
        """Get current installation progress (Phase 6.2.5b)"""
        return self._call_api(
            "get_installation_progress",
            lambda: self.api.get_installation_progress(app_id),
            lambda _exc: None,
        )

    def install_version(self, tag, app_id=None):
        """Install a ComfyUI version"""
        return self._call_api(
            "install_version",
            lambda: {"success": self.api.install_version(tag, app_id=app_id)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def cancel_installation(self, app_id=None):
        """Cancel the currently running installation"""
        return self._call_api(
            "cancel_installation",
            lambda: {"success": self.api.cancel_installation(app_id)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def calculate_release_size(self, tag, force_refresh=False, app_id=None):
        """Calculate total download size for a release (Phase 6.2.5c)"""
        return self._call_api(
            "calculate_release_size",
            lambda: self.api.calculate_release_size(tag, force_refresh, app_id) or None,
            lambda _exc: None,
        )

    def calculate_all_release_sizes(self):
        """Calculate sizes for all available releases (Phase 6.2.5c)"""
        return self._call_api(
            "calculate_all_release_sizes",
            self.api.calculate_all_release_sizes,
            lambda _exc: {},
        )

    def remove_version(self, tag, app_id=None):
        """Remove an installed ComfyUI version"""
        return self._call_api(
            "remove_version",
            lambda: {"success": self.api.remove_version(tag, app_id)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def switch_version(self, tag, app_id=None):
        """Switch to a different ComfyUI version"""
        return self._call_api(
            "switch_version",
            lambda: {"success": self.api.switch_version(tag, app_id)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def get_active_version(self, app_id=None):
        """Get currently active ComfyUI version"""
        return self._call_api(
            "get_active_version",
            lambda: {"success": True, "version": self.api.get_active_version(app_id)},
            lambda exc: {"success": False, "error": str(exc), "version": ""},
        )

    def get_default_version(self, app_id=None):
        """Get configured default ComfyUI version"""
        return self._call_api(
            "get_default_version",
            lambda: {"success": True, "version": self.api.get_default_version(app_id)},
            lambda exc: {"success": False, "error": str(exc), "version": ""},
        )

    def set_default_version(self, tag=None, app_id=None):
        """Set the default ComfyUI version (pass None to clear)"""
        return self._call_api(
            "set_default_version",
            lambda: {"success": self.api.set_default_version(tag, app_id)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def check_version_dependencies(self, tag, app_id=None):
        """Check dependency installation status for a version"""
        return self._call_api(
            "check_version_dependencies",
            lambda: {
                "success": True,
                "dependencies": self.api.check_version_dependencies(tag, app_id),
            },
            lambda exc: {
                "success": False,
                "error": str(exc),
                "dependencies": {"installed": [], "missing": []},
            },
        )

    def install_version_dependencies(self, tag, app_id=None):
        """Install dependencies for a ComfyUI version"""
        return self._call_api(
            "install_version_dependencies",
            lambda: {"success": self.api.install_version_dependencies(tag, app_id=app_id)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def get_version_status(self, app_id=None):
        """Get comprehensive status of all versions"""
        return self._call_api(
            "get_version_status",
            lambda: {"success": True, "status": self.api.get_version_status(app_id)},
            lambda exc: {"success": False, "error": str(exc), "status": {}},
        )

    def get_version_info(self, tag, app_id=None):
        """Get detailed information about a specific version"""
        return self._call_api(
            "get_version_info",
            lambda: {"success": True, "info": self.api.get_version_info(tag, app_id)},
            lambda exc: {"success": False, "error": str(exc), "info": {}},
        )

    def open_path(self, path):
        """Open an arbitrary path in the system file manager"""
        return self._call_api(
            "open_path",
            lambda: self.api.open_path(path),
            lambda exc: {"success": False, "error": str(exc)},
        )

    def open_active_install(self, app_id=None):
        """Open the active ComfyUI installation directory"""
        return self._call_api(
            "open_active_install",
            lambda: self.api.open_active_install(app_id),
            lambda exc: {"success": False, "error": str(exc)},
        )

    def open_url(self, url):
        """Open a URL in the system browser"""
        return self._call_api(
            "open_url",
            lambda: self.api.open_url(url),
            lambda exc: {"success": False, "error": str(exc)},
        )

    def open_model_import_dialog(self):
        """Open native file picker for model import.

        In Electron mode, file dialogs are handled by Electron's main process
        via IPC, not by Python. This method is kept for API compatibility but
        returns an error indicating Electron should handle it.
        """
        # File dialogs are now handled by Electron's dialog.showOpenDialog()
        # See electron/src/preload.ts for the implementation
        return {
            "success": False,
            "error": "File dialogs are handled by Electron",
            "paths": [],
        }

    def launch_version(self, tag, extra_args=None, app_id=None):
        """Launch a specific ComfyUI version"""

        def _do():
            result = self.api.launch_version(tag, extra_args, app_id)
            return {
                "success": result.get("success", False),
                "log_path": result.get("log_path"),
                "error": result.get("error"),
                "ready": result.get("ready"),
            }

        return self._call_api(
            "launch_version", _do, lambda exc: {"success": False, "error": str(exc)}
        )

    # ==================== Resource Management Methods (Phase 5) ====================

    def get_models(self):
        """Get list of models in shared storage"""
        return self._call_api(
            "get_models",
            lambda: {"success": True, "models": self.api.get_models()},
            lambda exc: {"success": False, "error": str(exc), "models": {}},
        )

    def refresh_model_index(self):
        """Rebuild the model library index"""
        return self._call_api(
            "refresh_model_index",
            lambda: {"success": self.api.refresh_model_index()},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def refresh_model_mappings(self, app_id="comfyui"):
        """Refresh model mappings for all installed versions"""
        return self._call_api(
            "refresh_model_mappings",
            lambda: {"success": True, "results": self.api.refresh_model_mappings(app_id)},
            lambda exc: {"success": False, "error": str(exc), "results": {}},
        )

    def import_model(self, local_path, family, official_name, repo_id=None):
        """Import a local model into the library"""
        return self._call_api(
            "import_model",
            lambda: self.api.import_model(local_path, family, official_name, repo_id),
            lambda exc: {"success": False, "error": str(exc)},
        )

    def download_model_from_hf(
        self,
        repo_id,
        family,
        official_name,
        model_type=None,
        subtype="",
        quant=None,
    ):
        """Download a model from Hugging Face into the library"""
        return self._call_api(
            "download_model_from_hf",
            lambda: self.api.download_model_from_hf(
                repo_id, family, official_name, model_type, subtype, quant
            ),
            lambda exc: {"success": False, "error": str(exc)},
        )

    def start_model_download_from_hf(
        self,
        repo_id,
        family,
        official_name,
        model_type=None,
        subtype="",
        quant=None,
    ):
        """Start a Hugging Face download with progress tracking"""
        return self._call_api(
            "start_model_download_from_hf",
            lambda: self.api.start_model_download_from_hf(
                repo_id, family, official_name, model_type, subtype, quant
            ),
            lambda exc: {"success": False, "error": str(exc)},
        )

    def get_model_download_status(self, download_id):
        """Get status for a model download"""
        return self._call_api(
            "get_model_download_status",
            lambda: self.api.get_model_download_status(download_id),
            lambda exc: {"success": False, "error": str(exc)},
        )

    def cancel_model_download(self, download_id):
        """Cancel an active model download"""
        return self._call_api(
            "cancel_model_download",
            lambda: self.api.cancel_model_download(download_id),
            lambda exc: {"success": False, "error": str(exc)},
        )

    def search_hf_models(self, query, kind=None, limit=25):
        """Search Hugging Face models for the download UI"""
        return self._call_api(
            "search_hf_models",
            lambda: self.api.search_hf_models(query, kind, limit),
            lambda exc: {"success": False, "error": str(exc), "models": []},
        )

    def get_related_models(self, model_id, limit=25):
        """Get related Hugging Face models for a library model"""
        return self._call_api(
            "get_related_models",
            lambda: self.api.get_related_models(model_id, limit),
            lambda exc: {"success": False, "error": str(exc), "models": []},
        )

    def get_model_overrides(self, rel_path):
        """Get overrides for a model by relative path"""
        return self._call_api(
            "get_model_overrides",
            lambda: {"success": True, "overrides": self.api.get_model_overrides(rel_path)},
            lambda exc: {"success": False, "error": str(exc), "overrides": {}},
        )

    def update_model_overrides(self, rel_path, overrides):
        """Update overrides for a model by relative path"""
        return self._call_api(
            "update_model_overrides",
            lambda: {"success": self.api.update_model_overrides(rel_path, overrides)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def get_custom_nodes(self, version_tag):
        """Get list of custom nodes for a specific version"""
        return self._call_api(
            "get_custom_nodes",
            lambda: {"success": True, "nodes": self.api.get_custom_nodes(version_tag)},
            lambda exc: {"success": False, "error": str(exc), "nodes": []},
        )

    def install_custom_node(self, git_url, version_tag, node_name=None):
        """Install a custom node for a specific version"""
        return self._call_api(
            "install_custom_node",
            lambda: {"success": self.api.install_custom_node(git_url, version_tag, node_name)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def update_custom_node(self, node_name, version_tag):
        """Update a custom node to latest version"""
        return self._call_api(
            "update_custom_node",
            lambda: {"success": self.api.update_custom_node(node_name, version_tag)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def remove_custom_node(self, node_name, version_tag):
        """Remove a custom node from a specific version"""
        return self._call_api(
            "remove_custom_node",
            lambda: {"success": self.api.remove_custom_node(node_name, version_tag)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    def scan_shared_storage(self):
        """Scan shared storage and get statistics"""
        return self._call_api(
            "scan_shared_storage",
            lambda: {"success": True, "result": self.api.scan_shared_storage()},
            lambda exc: {"success": False, "error": str(exc), "result": {}},
        )

    def get_link_health(self, version_tag=None):
        """Get health status of model symlinks"""
        return self._call_api(
            "get_link_health",
            lambda: self.api.get_link_health(version_tag),
            lambda exc: {"success": False, "error": str(exc)},
        )

    # ==================== Model Import API Methods ====================

    def import_batch(self, specs):
        """Import multiple models in a batch operation"""
        return self._call_api(
            "import_batch",
            lambda: self.api.import_batch(specs),
            lambda exc: {
                "success": False,
                "error": str(exc),
                "imported": 0,
                "failed": len(specs) if specs else 0,
                "results": [],
            },
        )

    def lookup_hf_metadata_for_file(self, filename, file_path=None):
        """Look up HuggingFace metadata for a file"""
        return self._call_api(
            "lookup_hf_metadata_for_file",
            lambda: self.api.lookup_hf_metadata_for_file(filename, file_path),
            lambda exc: {
                "success": False,
                "found": False,
                "error": str(exc),
            },
        )

    def detect_sharded_sets(self, file_paths):
        """Detect and group sharded model files"""
        return self._call_api(
            "detect_sharded_sets",
            lambda: self.api.detect_sharded_sets(file_paths),
            lambda exc: {
                "success": False,
                "error": str(exc),
                "groups": {},
            },
        )

    def validate_file_type(self, file_path):
        """Validate file type using magic bytes"""
        return self._call_api(
            "validate_file_type",
            lambda: self.api.validate_file_type(file_path),
            lambda exc: {
                "success": False,
                "valid": False,
                "detected_type": "error",
                "error": str(exc),
            },
        )

    def get_network_status(self):
        """Get network status including circuit breaker state"""
        return self._call_api(
            "get_network_status",
            lambda: self.api.get_network_status(),
            lambda exc: {"success": False, "error": str(exc)},
        )

    def get_library_status(self):
        """Get current library status including indexing state"""
        return self._call_api(
            "get_library_status",
            lambda: self.api.get_library_status(),
            lambda exc: {
                "success": False,
                "error": str(exc),
                "indexing": False,
                "model_count": 0,
            },
        )

    def get_file_link_count(self, file_path):
        """Get number of hard links for a file"""
        return self._call_api(
            "get_file_link_count",
            lambda: self.api.get_file_link_count(file_path),
            lambda exc: {
                "success": False,
                "link_count": 1,
                "is_hard_linked": False,
                "error": str(exc),
            },
        )

    def check_files_writable(self, file_paths):
        """Check if files can be safely deleted"""
        return self._call_api(
            "check_files_writable",
            lambda: self.api.check_files_writable(file_paths),
            lambda exc: {
                "success": False,
                "all_writable": False,
                "details": [],
                "error": str(exc),
            },
        )

    def mark_metadata_as_manual(self, model_id):
        """Mark model metadata as manually corrected"""
        return self._call_api(
            "mark_metadata_as_manual",
            lambda: self.api.mark_metadata_as_manual(model_id),
            lambda exc: {"success": False, "error": str(exc)},
        )

    def get_embedded_metadata(self, file_path):
        """Extract embedded metadata from a model file (GGUF or safetensors)"""
        return self._call_api(
            "get_embedded_metadata",
            lambda: self.api.get_embedded_metadata(file_path),
            lambda exc: {
                "success": False,
                "file_type": "unknown",
                "metadata": None,
                "error": str(exc),
            },
        )

    def search_models_fts(self, query, limit=100, offset=0, model_type=None, tags=None):
        """Search local model library using FTS5 full-text search"""
        return self._call_api(
            "search_models_fts",
            lambda: self.api.search_models_fts(query, limit, offset, model_type, tags),
            lambda exc: {
                "success": False,
                "error": str(exc),
                "models": [],
                "total_count": 0,
                "query_time_ms": 0,
                "query": "",
            },
        )

    # ==================== Size Calculation Methods (Phase 6.2.5a/c) ====================

    def get_release_size_info(self, tag, archive_size):
        """Get size information for a release"""
        return self._call_api(
            "get_release_size_info",
            lambda: {"success": True, "info": self.api.get_release_size_info(tag, archive_size)},
            lambda exc: {"success": False, "error": str(exc), "info": None},
        )

    def get_release_size_breakdown(self, tag):
        """Get size breakdown for display"""
        return self._call_api(
            "get_release_size_breakdown",
            lambda: {"success": True, "breakdown": self.api.get_release_size_breakdown(tag)},
            lambda exc: {"success": False, "error": str(exc), "breakdown": None},
        )

    def get_release_dependencies(self, tag, top_n=None):
        """Get dependencies for a release sorted by size"""
        return self._call_api(
            "get_release_dependencies",
            lambda: {
                "success": True,
                "dependencies": self.api.get_release_dependencies(tag, top_n),
            },
            lambda exc: {"success": False, "error": str(exc), "dependencies": []},
        )

    # ==================== Cache Status Methods ====================

    def get_github_cache_status(self, app_id=None):
        """Get GitHub releases cache status"""
        return self._call_api(
            "get_github_cache_status",
            lambda: {"success": True, "status": self.api.get_github_cache_status(app_id)},
            lambda exc: {"success": False, "error": str(exc), "status": {}},
        )

    def has_background_fetch_completed(self):
        """Check if background fetch completed"""
        return self._call_api(
            "has_background_fetch_completed",
            lambda: {"success": True, "completed": self.api.has_background_fetch_completed()},
            lambda exc: {"success": False, "error": str(exc), "completed": False},
        )

    def reset_background_fetch_flag(self):
        """Reset background fetch completion flag"""
        return self._call_api(
            "reset_background_fetch_flag",
            lambda: {"success": bool(self.api.reset_background_fetch_flag() is None)},
            lambda exc: {"success": False, "error": str(exc)},
        )

    # ==================== Launcher Update Methods ====================

    def get_launcher_version(self):
        """Get current launcher version (git commit)"""

        def _do():
            from backend.__version__ import __branch__, __version__, is_git_repo

            return {
                "success": True,
                "version": __version__,
                "branch": __branch__,
                "isGitRepo": is_git_repo(),
            }

        return self._call_api(
            "get_launcher_version",
            _do,
            lambda exc: {"success": False, "error": str(exc), "version": "unknown"},
        )

    def check_launcher_updates(self, force_refresh=False):
        """Check if launcher updates are available"""

        def _do():
            # Initialize updater if not exists
            if not hasattr(self.api, "launcher_updater"):
                from backend.launcher_updater import LauncherUpdater

                self.api.launcher_updater = LauncherUpdater(self.api.metadata_manager)

            result = self.api.launcher_updater.check_for_updates(force_refresh)
            return {"success": True, **result}

        return self._call_api(
            "check_launcher_updates",
            _do,
            lambda exc: {"success": False, "error": str(exc), "hasUpdate": False},
        )

    def apply_launcher_update(self):
        """Apply launcher update (pull + rebuild)"""

        def _do():
            if not hasattr(self.api, "launcher_updater"):
                from backend.launcher_updater import LauncherUpdater

                self.api.launcher_updater = LauncherUpdater(self.api.metadata_manager)

            result = self.api.launcher_updater.apply_update()
            return {"success": result.get("success", False), **result}

        return self._call_api(
            "apply_launcher_update",
            _do,
            lambda exc: {"success": False, "error": str(exc)},
        )

    def restart_launcher(self):
        """Restart the launcher application"""
        import os

        def _do():
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
                    [python, str(launcher_root / "backend" / "main.py")],
                    start_new_session=True,
                )

            # Exit current process after a brief delay
            import threading

            def delayed_exit():
                import time

                time.sleep(1)
                os._exit(0)

            threading.Thread(target=delayed_exit, daemon=True).start()

            return {"success": True, "message": "Restarting..."}

        return self._call_api(
            "restart_launcher",
            _do,
            lambda exc: {"success": False, "error": str(exc)},
        )


# NOTE: PyWebView main() entry point has been removed.
# The application now runs via Electron with a Python sidecar.
# See:
#   - electron/src/main.ts - Electron main process
#   - backend/rpc_server.py - Python RPC server (uses JavaScriptAPI)
