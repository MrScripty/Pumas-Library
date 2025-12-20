#!/usr/bin/env python3
"""
ComfyUI Setup Launcher - Main Entry Point
Desktop application using PyWebView with React frontend
"""

import sys
import webview
from pathlib import Path
from api import ComfyUISetupAPI


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

    # ==================== Action Methods ====================

    def install_deps(self):
        """Install missing dependencies"""
        try:
            success = self.api.install_missing_dependencies()
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def toggle_patch(self):
        """Toggle main.py patch"""
        try:
            success = self.api.toggle_patch()
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def toggle_menu(self):
        """Toggle menu shortcut"""
        try:
            success = self.api.toggle_menu()
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def toggle_desktop(self):
        """Toggle desktop shortcut"""
        try:
            success = self.api.toggle_desktop()
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def close_window(self):
        """Close the application window and terminate the process"""
        try:
            # Destroy all windows
            for window in webview.windows:
                window.destroy()
        except Exception:
            pass
        # Exit the application
        sys.exit(0)

    def launch_comfyui(self):
        """Launch ComfyUI using run.sh"""
        try:
            success = self.api.launch_comfyui()
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def stop_comfyui(self):
        """Stop running ComfyUI instance"""
        try:
            success = self.api.stop_comfyui()
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    # ==================== Version Management Methods (Phase 5) ====================

    def get_available_versions(self, force_refresh=False):
        """Get list of available ComfyUI versions from GitHub"""
        try:
            versions = self.api.get_available_versions(force_refresh)
            return {"success": True, "versions": versions}
        except Exception as e:
            return {"success": False, "error": str(e), "versions": []}

    def get_installed_versions(self):
        """Get list of installed ComfyUI version tags"""
        try:
            versions = self.api.get_installed_versions()
            return {"success": True, "versions": versions}
        except Exception as e:
            return {"success": False, "error": str(e), "versions": []}

    def install_version(self, tag):
        """Install a ComfyUI version"""
        try:
            # Note: progress_callback not supported via PyWebView API
            # Frontend should poll get_version_status() for progress
            success = self.api.install_version(tag)
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def remove_version(self, tag):
        """Remove an installed ComfyUI version"""
        try:
            success = self.api.remove_version(tag)
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def switch_version(self, tag):
        """Switch to a different ComfyUI version"""
        try:
            success = self.api.switch_version(tag)
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def get_active_version(self):
        """Get currently active ComfyUI version"""
        try:
            version = self.api.get_active_version()
            return {"success": True, "version": version}
        except Exception as e:
            return {"success": False, "error": str(e), "version": ""}

    def check_version_dependencies(self, tag):
        """Check dependency installation status for a version"""
        try:
            status = self.api.check_version_dependencies(tag)
            return {"success": True, "dependencies": status}
        except Exception as e:
            return {"success": False, "error": str(e), "dependencies": {"installed": [], "missing": []}}

    def install_version_dependencies(self, tag):
        """Install dependencies for a ComfyUI version"""
        try:
            success = self.api.install_version_dependencies(tag)
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def get_version_status(self):
        """Get comprehensive status of all versions"""
        try:
            status = self.api.get_version_status()
            return {"success": True, "status": status}
        except Exception as e:
            return {"success": False, "error": str(e), "status": {}}

    def get_version_info(self, tag):
        """Get detailed information about a specific version"""
        try:
            info = self.api.get_version_info(tag)
            return {"success": True, "info": info}
        except Exception as e:
            return {"success": False, "error": str(e), "info": {}}

    def launch_version(self, tag, extra_args=None):
        """Launch a specific ComfyUI version"""
        try:
            success = self.api.launch_version(tag, extra_args)
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    # ==================== Resource Management Methods (Phase 5) ====================

    def get_models(self):
        """Get list of models in shared storage"""
        try:
            models = self.api.get_models()
            return {"success": True, "models": models}
        except Exception as e:
            return {"success": False, "error": str(e), "models": {}}

    def get_custom_nodes(self, version_tag):
        """Get list of custom nodes for a specific version"""
        try:
            nodes = self.api.get_custom_nodes(version_tag)
            return {"success": True, "nodes": nodes}
        except Exception as e:
            return {"success": False, "error": str(e), "nodes": []}

    def install_custom_node(self, git_url, version_tag, node_name=None):
        """Install a custom node for a specific version"""
        try:
            success = self.api.install_custom_node(git_url, version_tag, node_name)
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def update_custom_node(self, node_name, version_tag):
        """Update a custom node to latest version"""
        try:
            success = self.api.update_custom_node(node_name, version_tag)
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def remove_custom_node(self, node_name, version_tag):
        """Remove a custom node from a specific version"""
        try:
            success = self.api.remove_custom_node(node_name, version_tag)
            return {"success": success}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def scan_shared_storage(self):
        """Scan shared storage and get statistics"""
        try:
            result = self.api.scan_shared_storage()
            return {"success": True, "result": result}
        except Exception as e:
            return {"success": False, "error": str(e), "result": {}}


def get_entrypoint():
    """
    Get the entry point for the web content
    Returns either the built frontend or development server URL
    """
    # Determine base directory
    if getattr(sys, 'frozen', False):
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
    # Create JavaScript API instance
    js_api = JavaScriptAPI()

    # Get entry point (production build or dev server)
    entry = get_entrypoint()

    # Determine if we're in development mode
    is_dev = entry.startswith("http://")

    if is_dev:
        print("=" * 60)
        print("DEVELOPMENT MODE")
        print("=" * 60)
        print(f"Connecting to development server at: {entry}")
        print("Make sure you have run 'npm run dev' in the frontend/ directory")
        print("=" * 60)

    # Create and configure the webview window
    window = webview.create_window(
        title="ComfyUI Setup",
        url=entry,
        js_api=js_api,
        width=400,
        height=520,
        resizable=False,
        frameless=True,
        easy_drag=True,
        background_color='#000000'
    )

    # Start the webview application
    # Use 'gtk' backend on Linux for best compatibility with Debian/Mint
    webview.start(debug=is_dev, gui='gtk')


if __name__ == "__main__":
    main()
