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
