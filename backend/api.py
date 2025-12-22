#!/usr/bin/env python3
"""
ComfyUI Setup API - Backend business logic
Handles all ComfyUI setup operations without UI dependencies
"""

import os
import re
import sys
import subprocess
import shutil
import urllib.request
import json
import tomllib
import webbrowser
import threading
from pathlib import Path
from typing import Dict, List, Any, Optional

from backend.file_opener import open_in_file_manager
from backend.models import GitHubRelease

# Optional Pillow import for icon editing (used for version-specific shortcut icons)
try:
    from PIL import Image, ImageDraw, ImageFont
except Exception:
    Image = None
    ImageDraw = None
    ImageFont = None


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
        self.launcher_data_dir = self.script_dir / "launcher-data"
        self.shortcut_scripts_dir = self.launcher_data_dir / "shortcuts"
        self.generated_icons_dir = self.launcher_data_dir / "icons"

        # System directories
        self.apps_dir = Path.home() / ".local" / "share" / "applications"
        self.apps_file = self.apps_dir / "ComfyUI.desktop"
        self.desktop_file = Path.home() / "Desktop" / "ComfyUI.desktop"
        self._release_info_cache: Optional[Dict[str, Any]] = None

        # Ensure directories used by shortcut tooling exist
        self.shortcut_scripts_dir.mkdir(parents=True, exist_ok=True)
        self.generated_icons_dir.mkdir(parents=True, exist_ok=True)

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
                self.package_size_resolver,
                self.resource_manager.shared_dir / "uv"
            )

            self._prefetch_releases_if_needed()
        except Exception as e:
            print(f"Warning: Version management initialization failed: {e}")
            self.metadata_manager = None
            self.github_fetcher = None
            self.resource_manager = None
            self.version_manager = None
            self.release_size_calculator = None

    def _prefetch_releases_if_needed(self):
        """Fetch releases in background on startup when cache is empty."""
        try:
            if not self.github_fetcher or not self.metadata_manager:
                return

            cache = self.metadata_manager.load_github_cache()
            if cache and cache.get("releases"):
                return

            def _background_fetch():
                try:
                    self.github_fetcher.get_releases(force_refresh=False)
                except Exception as exc:
                    print(f"Background release fetch failed: {exc}")

            threading.Thread(target=_background_fetch, daemon=True).start()
        except Exception as e:
            print(f"Prefetch init error: {e}")

    def _refresh_release_sizes_async(
        self,
        releases: List[GitHubRelease],
        installed_tags: set[str],
        force_refresh: bool = False
    ):
        """
        Calculate release sizes in the background, prioritizing non-installed releases.
        """
        if not self.release_size_calculator:
            return

        # Build priority queue: non-installed first
        def sort_key(release: GitHubRelease):
            tag = release.get('tag_name', '')
            return 0 if tag not in installed_tags else 1

        pending = sorted(releases, key=sort_key)

        def _worker():
            for release in pending:
                tag = release.get('tag_name', '')
                if not tag:
                    continue
                # Skip if already cached and not forcing
                if not force_refresh and self.release_size_calculator.get_cached_size(tag):
                    continue
                try:
                    self.calculate_release_size(tag, force_refresh=force_refresh)
                except Exception as exc:
                    print(f"Size refresh failed for {tag}: {exc}")

        threading.Thread(target=_worker, daemon=True).start()

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

            # Use cached GitHub releases (TTL handled by GitHubReleasesFetcher)
            latest_tag = None
            if self.github_fetcher:
                try:
                    releases = self.github_fetcher.get_releases(force_refresh=False)
                    if releases:
                        latest_tag = releases[0].get('tag_name') or None
                except Exception as e:
                    print(f"Warning: using cached/stale releases after error: {e}")

            if current_tag and latest_tag:
                has_update = current_tag != latest_tag
                self._release_info_cache = {
                    "has_update": has_update,
                    "latest_version": latest_tag,
                    "current_version": current_version or current_tag
                }
            else:
                self._release_info_cache = {
                    "has_update": False,
                    "latest_version": latest_tag,
                    "current_version": current_version
                }
        except Exception as e:
            print(f"Error checking for new release: {e}")
            self._release_info_cache = {
                "has_update": False,
                "latest_version": None,
                "current_version": None
            }

        return self._release_info_cache

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

    # ==================== Patch Management ====================

    def _get_target_main_py(self, tag: Optional[str] = None) -> tuple[Optional[Path], Optional[str]]:
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

        print(f"No main.py found to patch at {self.main_py}")
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
            return bool(re.search(r'setproctitle\\.setproctitle\\([\"\\\']ComfyUI Server[^\"\\\']*[\"\\\']\\)', content))
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
            '\ntry:\n'
            '    import setproctitle\n'
            f'    setproctitle.setproctitle("{server_title}")\n'
            'except ImportError:\n'
            '    pass\n'
        )

        if 'if __name__ == "__main__":' in content:
            content = content.replace(
                'if __name__ == "__main__":',
                insert_code + 'if __name__ == "__main__":',
                1
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
                    ['git', '-C', str(repo_dir), 'checkout', '--', main_py.name],
                    capture_output=True,
                    check=True
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

    # ==================== Shortcut Management ====================

    def _slugify_tag(self, tag: str) -> str:
        """Convert a version tag into a filesystem-safe slug"""
        if not tag:
            return "unknown"
        safe = ''.join(c if c.isalnum() or c in ('-', '_') else '-' for c in tag.strip().lower())
        safe = safe.strip('-_') or "unknown"
        return safe

    def _get_version_paths(self, tag: str) -> Optional[Dict[str, Path]]:
        """Resolve key paths for a specific installed version"""
        if not self.version_manager:
            return None

        version_dir = self.version_manager.get_version_path(tag)
        if not version_dir:
            return None

        venv_python = version_dir / "venv" / "bin" / "python"
        main_py = version_dir / "main.py"

        if not venv_python.exists() or not main_py.exists():
            return None

        return {
            "version_dir": version_dir,
            "venv_python": venv_python,
            "main_py": main_py,
            "pid_file": version_dir / "comfyui.pid",
        }

    def _get_version_shortcut_paths(self, tag: str) -> Dict[str, Path]:
        """Return paths for version-specific shortcut artifacts"""
        slug = self._slugify_tag(tag)
        return {
            "slug": slug,
            "menu": self.apps_dir / f"ComfyUI-{slug}.desktop",
            "desktop": Path.home() / "Desktop" / f"ComfyUI-{slug}.desktop",
            "icon_name": f"comfyui-{slug}",
            "launcher": self.shortcut_scripts_dir / f"launch-{slug}.sh",
        }

    def _remove_installed_icon(self, icon_name: str):
        """Remove installed icon variants for a version-specific shortcut"""
        icon_base_dir = Path.home() / ".local" / "share" / "icons" / "hicolor"
        sizes = [256, 128, 64, 48]
        for size in sizes:
            icon_path = icon_base_dir / f"{size}x{size}" / "apps" / f"{icon_name}.png"
            try:
                if icon_path.exists():
                    icon_path.unlink()
            except Exception:
                pass

        scalable_dir = icon_base_dir / "scalable" / "apps"
        for ext in ("png", "webp"):
            try:
                icon_path = scalable_dir / f"{icon_name}.{ext}"
                if icon_path.exists():
                    icon_path.unlink()
            except Exception:
                pass

        generated_icon = self.generated_icons_dir / f"{icon_name}.png"
        try:
            if generated_icon.exists():
                generated_icon.unlink()
        except Exception:
            pass

    def _generate_version_icon(self, tag: str) -> Optional[Path]:
        """Create a PNG icon with the version number overlaid"""
        if not self.icon_webp.exists():
            print("Base icon not found; cannot generate version-specific icon")
            return None

        if not (Image and ImageDraw and ImageFont):
            print("Pillow not available; skipping version label overlay")
            return None

        slug = self._slugify_tag(tag)
        dest_path = self.generated_icons_dir / f"comfyui-{slug}.png"

        try:
            base = Image.open(self.icon_webp).convert("RGBA")
            size = max(base.size)

            # Ensure we have a square canvas for consistent text placement
            canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
            offset = ((size - base.width) // 2, (size - base.height) // 2)
            canvas.paste(base, offset)

            draw = ImageDraw.Draw(canvas)
            label = tag.lstrip('v')
            if len(label) > 12:
                label = label[:12]

            try:
                # Larger overlay text for legibility (+2px bump)
                font_size = max(28, size // 5 + 2)
                font = ImageFont.truetype("DejaVuSans-Bold.ttf", font_size)
            except Exception:
                font = ImageFont.load_default()

            if hasattr(draw, "textbbox"):
                bbox = draw.textbbox((0, 0), label, font=font)
                text_w = bbox[2] - bbox[0]
                text_h = bbox[3] - bbox[1]
            else:
                text_w, text_h = draw.textsize(label, font=font)

            padding = max(6, size // 30)
            banner_height = max(text_h + padding * 2, int(size * 0.28))
            banner_y = (size - banner_height) // 2
            background = (
                padding,
                banner_y,
                size - padding,
                banner_y + banner_height,
            )

            try:
                draw.rounded_rectangle(background, radius=padding, fill=(0, 0, 0, 190))
            except Exception:
                draw.rectangle(background, fill=(0, 0, 0, 190))

            draw.text(
                ((size - text_w) / 2, banner_y + (banner_height - text_h) / 2),
                label,
                font=font,
                fill=(255, 255, 255, 230),
            )

            canvas.save(dest_path, format="PNG")
            return dest_path
        except Exception as e:
            print(f"Error generating icon for {tag}: {e}")
            return None

    def _install_version_icon(self, tag: str) -> str:
        """Install a version-specific icon with the version label"""
        slug = self._slugify_tag(tag)
        icon_name = f"comfyui-{slug}"
        icon_source = self._generate_version_icon(tag)

        # Fallback to base icon if overlay generation fails
        if not icon_source and self.icon_webp.exists():
            icon_source = self.icon_webp

        if not icon_source:
            return "comfyui"

        icon_base_dir = Path.home() / ".local" / "share" / "icons" / "hicolor"
        png_sizes = [256, 128, 64, 48]
        conversion_success = False

        for size in png_sizes:
            try:
                icon_dir = icon_base_dir / f"{size}x{size}" / "apps"
                icon_dir.mkdir(parents=True, exist_ok=True)
                dest_icon = icon_dir / f"{icon_name}.png"

                if Image:
                    with Image.open(icon_source) as img:
                        img = img.convert("RGBA")
                        resampling = getattr(getattr(Image, "Resampling", Image), "LANCZOS", Image.LANCZOS)
                        img.thumbnail((size, size), resample=resampling)
                        img.save(dest_icon, format="PNG")
                        conversion_success = True
                else:
                    result = subprocess.run(
                        ['convert', str(icon_source), '-resize', f'{size}x{size}', str(dest_icon)],
                        capture_output=True,
                        timeout=10
                    )
                    if result.returncode == 0:
                        conversion_success = True
            except Exception as e:
                print(f"Error installing icon size {size} for {tag}: {e}")

        if not conversion_success:
            try:
                icon_dir = icon_base_dir / "scalable" / "apps"
                icon_dir.mkdir(parents=True, exist_ok=True)
                dest_icon = icon_dir / f"{icon_name}{icon_source.suffix}"
                shutil.copy2(icon_source, dest_icon)

                png_link = icon_dir / f"{icon_name}.png"
                try:
                    if png_link.exists():
                        png_link.unlink()
                    png_link.symlink_to(dest_icon)
                except Exception:
                    pass
            except Exception as e:
                print(f"Error installing fallback icon for {tag}: {e}")

        # Update icon cache if available
        try:
            subprocess.run(['gtk-update-icon-cache', '-f', '-t', str(icon_base_dir)],
                           capture_output=True, timeout=5)
        except Exception:
            pass

        try:
            subprocess.run(
                ['xdg-icon-resource', 'install', '--novendor', '--size', '256',
                 str(icon_source), icon_name],
                capture_output=True,
                timeout=5
            )
        except Exception:
            pass

        return icon_name

    def _write_version_launch_script(self, tag: str, version_dir: Path, slug: str) -> Optional[Path]:
        """Create a launch script for a specific version"""
        script_path = self.shortcut_scripts_dir / f"launch-{slug}.sh"
        profile_dir = self.launcher_data_dir / "profiles" / slug
        profile_dir.mkdir(parents=True, exist_ok=True)

        content = f"""#!/bin/bash
set -euo pipefail

VERSION_DIR="{version_dir}"
VENV_PATH="$VERSION_DIR/venv"
MAIN_PY="$VERSION_DIR/main.py"
PID_FILE="$VERSION_DIR/comfyui.pid"
URL="http://127.0.0.1:8188"
WINDOW_CLASS="ComfyUI-{slug}"
PROFILE_DIR="{profile_dir}"
SERVER_START_DELAY=8
SERVER_PID=""

log() {{
    echo "[\\$(date +'%H:%M:%S')] $*"
}}

stop_previous_instance() {{
    if [[ -f "$PID_FILE" ]]; then
        local pid
        pid=$(cat "$PID_FILE" 2>/dev/null || echo "")
        if [[ -n "$pid" ]] && kill -0 "$pid" 2>/dev/null; then
            log "Stopping previous server (PID: $pid)..."
            kill "$pid" 2>/dev/null || true
            sleep 2
            kill -9 "$pid" 2>/dev/null || true
        fi
        rm -f "$PID_FILE"
    fi
}}

close_existing_app_window() {{
    if command -v wmctrl >/dev/null 2>&1; then
        local wins
        wins=$(wmctrl -l -x 2>/dev/null | grep -i "$WINDOW_CLASS" | awk '{{print $1}}' || true)
        if [[ -n "$wins" ]]; then
            for win_id in $wins; do
                wmctrl -i -c "$win_id" || true
            done
            sleep 1
        fi
    fi
}}

start_comfyui() {{
    if [[ ! -x "$VENV_PATH/bin/python" ]]; then
        echo "Missing virtual environment for {tag}"
        exit 1
    fi

    cd "$VERSION_DIR"
    log "Starting ComfyUI {tag}..."
    "$VENV_PATH/bin/python" "$MAIN_PY" --enable-manager &
    SERVER_PID=$!
    echo "$SERVER_PID" > "$PID_FILE"
}}

open_app() {{
    if command -v brave-browser >/dev/null 2>&1; then
        mkdir -p "$PROFILE_DIR"
        log "Opening Brave window for {tag}..."
        brave-browser --app="$URL" --new-window --user-data-dir="$PROFILE_DIR" --class="$WINDOW_CLASS" >/dev/null 2>&1 &
    else
        log "Opening default browser..."
        xdg-open "$URL" >/dev/null 2>&1 &
    fi
}}

cleanup() {{
    if [[ -n "$SERVER_PID" ]] && kill -0 "$SERVER_PID" 2>/dev/null; then
        kill "$SERVER_PID" 2>/dev/null || true
    fi
    rm -f "$PID_FILE"
}}

trap cleanup EXIT

stop_previous_instance
close_existing_app_window
start_comfyui

log "Waiting $SERVER_START_DELAY seconds for server to start..."
sleep "$SERVER_START_DELAY"
open_app

wait $SERVER_PID
"""

        try:
            script_path.write_text(content)
            script_path.chmod(0o755)
            return script_path
        except Exception as e:
            print(f"Error writing launch script for {tag}: {e}")
            return None

    def get_version_shortcut_state(self, tag: str) -> Dict[str, Any]:
        """Return the current shortcut state for a version"""
        paths = self._get_version_shortcut_paths(tag)
        return {
            "tag": tag,
            "menu": paths["menu"].exists(),
            "desktop": paths["desktop"].exists(),
        }

    def get_all_shortcut_states(self) -> Dict[str, Any]:
        """Get shortcut states for all installed versions"""
        states: Dict[str, Any] = {}
        if self.version_manager:
            for tag in self.get_installed_versions():
                states[tag] = self.get_version_shortcut_state(tag)
        return {
            "active": self.get_active_version(),
            "states": states,
        }

    def create_version_shortcuts(self, tag: str, create_menu: bool = True, create_desktop: bool = True) -> Dict[str, Any]:
        """Create menu/desktop shortcuts for a specific version"""
        paths = self._get_version_paths(tag)
        shortcut_paths = self._get_version_shortcut_paths(tag)

        if not paths:
            return {"success": False, "error": f"Version {tag} is not installed or incomplete."}

        # Ensure base icon exists for menu entries (no version banner)
        base_icon_name = "comfyui"
        self.install_icon()

        # Use version banner icon only for desktop shortcut
        desktop_icon_name = self._install_version_icon(tag)
        if not desktop_icon_name:
            desktop_icon_name = base_icon_name

        launcher_script = self._write_version_launch_script(tag, paths["version_dir"], shortcut_paths["slug"])

        if not launcher_script:
            return {"success": False, "error": "Failed to write launch script"}

        results = {"menu": False, "desktop": False}

        if create_menu:
            try:
                self.apps_dir.mkdir(parents=True, exist_ok=True)
                content = f"""[Desktop Entry]
Name=ComfyUI {tag}
Comment=Launch ComfyUI {tag}
Exec=bash "{launcher_script.resolve()}"
Icon={base_icon_name}
Terminal=false
Type=Application
Categories=Graphics;ArtificialIntelligence;
"""
                shortcut_paths["menu"].write_text(content)
                # Mark executable to be trusted by desktop environments (especially on Desktop)
                shortcut_paths["menu"].chmod(0o755)
                results["menu"] = True
            except Exception as e:
                print(f"Error creating menu shortcut for {tag}: {e}")

        if create_desktop:
            try:
                desktop_dir = shortcut_paths["desktop"].parent
                desktop_dir.mkdir(parents=True, exist_ok=True)
                content = f"""[Desktop Entry]
Name=ComfyUI
Comment=Launch ComfyUI {tag}
Exec=bash "{launcher_script.resolve()}"
Icon={desktop_icon_name}
Terminal=false
Type=Application
Categories=Graphics;ArtificialIntelligence;
"""
                shortcut_paths["desktop"].write_text(content)
                shortcut_paths["desktop"].chmod(0o755)
                results["desktop"] = True
            except Exception as e:
                print(f"Error creating desktop shortcut for {tag}: {e}")

        results["success"] = (not create_menu or results["menu"]) and (not create_desktop or results["desktop"])
        results["state"] = self.get_version_shortcut_state(tag)
        return results

    def remove_version_shortcuts(self, tag: str, remove_menu: bool = True, remove_desktop: bool = True) -> Dict[str, Any]:
        """Remove version-specific shortcuts and icons"""
        paths = self._get_version_shortcut_paths(tag)
        if remove_menu:
            try:
                paths["menu"].unlink(missing_ok=True)
            except Exception:
                pass

        if remove_desktop:
            try:
                paths["desktop"].unlink(missing_ok=True)
            except Exception:
                pass

        # Remove launcher script if no shortcuts remain
        state_after = self.get_version_shortcut_state(tag)
        if not state_after["menu"] and not state_after["desktop"]:
            try:
                paths["launcher"].unlink(missing_ok=True)
            except Exception:
                pass
            self._remove_installed_icon(paths["icon_name"])

        return {"success": True, "state": state_after}

    def set_version_shortcuts(self, tag: str, enabled: bool, menu: bool = True, desktop: bool = True) -> Dict[str, Any]:
        """Ensure shortcuts for a version are enabled/disabled"""
        if enabled:
            result = self.create_version_shortcuts(tag, create_menu=menu, create_desktop=desktop)
        else:
            result = self.remove_version_shortcuts(tag, remove_menu=menu, remove_desktop=desktop)
        result["state"] = self.get_version_shortcut_state(tag)
        result["tag"] = tag
        result["success"] = bool(result.get("success", False))
        return result

    def toggle_version_menu_shortcut(self, tag: str) -> Dict[str, Any]:
        """Toggle only the menu shortcut for a version"""
        current = self.get_version_shortcut_state(tag)
        return self.set_version_shortcuts(tag, not current["menu"], menu=True, desktop=False)

    def toggle_version_desktop_shortcut(self, tag: str) -> Dict[str, Any]:
        """Toggle only the desktop shortcut for a version"""
        current = self.get_version_shortcut_state(tag)
        return self.set_version_shortcuts(tag, not current["desktop"], menu=False, desktop=True)

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
        active_version = self.get_active_version() if self.version_manager else None
        if active_version:
            shortcut_state = self.get_version_shortcut_state(active_version)
            menu = shortcut_state["menu"]
            desktop = shortcut_state["desktop"]
        else:
            shortcut_state = {"menu": self.menu_exists(), "desktop": self.desktop_exists()}
            menu = shortcut_state["menu"]
            desktop = shortcut_state["desktop"]
        running_processes = self._detect_comfyui_processes()
        running = bool(running_processes)

        # Check for new releases
        release_info = self.check_for_new_release()

        # Determine status message
        if running:
            message = ""  # Suppress running banner text in GUI
        elif not deps_ready:
            message = "Missing dependencies detected."
        elif deps_ready and patched and menu and desktop:
            message = "Setup complete – everything is ready"
        else:
            message = ""

        return {
            "version": self.get_comfyui_version(),
            "deps_ready": deps_ready,
            "missing_deps": missing_deps,
            "patched": patched,
            "menu_shortcut": menu,
            "desktop_shortcut": desktop,
            "shortcut_version": active_version,
            "comfyui_running": running,
            "running_processes": running_processes,
            "message": message,
            "release_info": release_info
        }

    def get_disk_space(self) -> Dict[str, Any]:
        """
        Get disk space information for the launcher directory

        Returns:
            Dictionary with total, used, free space in bytes and usage percentage
        """
        try:
            import shutil
            stat = shutil.disk_usage(self.script_dir)
            usage_percent = (stat.used / stat.total) * 100 if stat.total > 0 else 0

            return {
                "success": True,
                "total": stat.total,
                "used": stat.used,
                "free": stat.free,
                "percent": round(usage_percent, 1)
            }
        except Exception as e:
            return {
                "success": False,
                "error": str(e),
                "total": 0,
                "used": 0,
                "free": 0,
                "percent": 0
            }

    # ==================== Action Handlers ====================

    def toggle_patch(self) -> bool:
        """Toggle main.py patch"""
        if self.is_patched():
            return self.revert_main_py()
        else:
            return self.patch_main_py()

    def toggle_menu(self, tag: Optional[str] = None) -> bool:
        """Toggle menu shortcut (version-specific when available)"""
        target = tag or (self.get_active_version() if self.version_manager else None)

        if target:
            result = self.toggle_version_menu_shortcut(target)
            return bool(result.get("success", False))

        if self.menu_exists():
            return self.remove_menu_shortcut()
        return self.create_menu_shortcut()

    def toggle_desktop(self, tag: Optional[str] = None) -> bool:
        """Toggle desktop shortcut (version-specific when available)"""
        target = tag or (self.get_active_version() if self.version_manager else None)

        if target:
            result = self.toggle_version_desktop_shortcut(target)
            return bool(result.get("success", False))

        if self.desktop_exists():
            return self.remove_desktop_shortcut()
        return self.create_desktop_shortcut()

    def _get_known_version_paths(self) -> Dict[str, Path]:
        """Return a mapping of installed version tags to their paths"""
        tag_paths: Dict[str, Path] = {}
        if not self.version_manager:
            return tag_paths

        try:
            for tag in self.version_manager.get_installed_versions():
                version_path = self.version_manager.get_version_path(tag)
                if version_path:
                    tag_paths[tag] = version_path
        except Exception as e:
            print(f"Error collecting version paths: {e}")

        return tag_paths

    def _detect_comfyui_processes(self) -> List[Dict[str, Any]]:
        """
        Detect running ComfyUI processes using PID files and process table scan.

        Returns:
            List of process info dictionaries (pid, source, tag, etc.)
        """
        processes: List[Dict[str, Any]] = []
        seen_pids: set[int] = set()

        tag_paths = self._get_known_version_paths()

        # 1) PID file checks (legacy root + per-version)
        pid_candidates: List[tuple[Optional[str], Path]] = [
            (None, self.comfyui_dir / "comfyui.pid")
        ]
        pid_candidates.extend([
            (tag, path / "comfyui.pid") for tag, path in tag_paths.items()
        ])

        for tag, pid_file in pid_candidates:
            if not pid_file.exists():
                continue
            try:
                pid = int(pid_file.read_text().strip())
                os.kill(pid, 0)
                if pid not in seen_pids:
                    processes.append({
                        "pid": pid,
                        "source": "pid_file",
                        "tag": tag,
                        "pid_file": str(pid_file)
                    })
                    seen_pids.add(pid)
            except (ValueError, ProcessLookupError, OSError):
                continue

        # 2) Process table scan (helps when PID files are missing/stale)
        try:
            ps = subprocess.run(
                ['ps', '-eo', 'pid=,args='],
                capture_output=True,
                text=True,
                timeout=3
            )
            ps_output = ps.stdout.splitlines()
        except Exception as e:
            print(f"Error scanning process table: {e}")
            ps_output = []

        for line in ps_output:
            line = line.strip()
            if not line:
                continue

            parts = line.split(None, 1)
            if len(parts) != 2:
                continue

            pid_str, cmdline = parts
            try:
                pid = int(pid_str)
            except ValueError:
                continue

            if pid in seen_pids:
                continue

            lower_cmd = cmdline.lower()
            has_title = "comfyui server" in lower_cmd
            has_main = "main.py" in cmdline and ("comfyui" in lower_cmd)

            if not (has_title or has_main):
                continue

            inferred_tag = None
            for tag, path in tag_paths.items():
                if str(path) in cmdline:
                    inferred_tag = tag
                    break

            processes.append({
                "pid": pid,
                "source": "process_scan",
                "tag": inferred_tag,
                "cmd": cmdline
            })
            seen_pids.add(pid)

        return processes

    def is_comfyui_running(self) -> bool:
        """Check if ComfyUI is currently running"""
        try:
            return bool(self._detect_comfyui_processes())
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

            # Stop the ComfyUI server (all detected processes)
            processes = self._detect_comfyui_processes()
            killed = False

            for proc in processes:
                pid = proc.get("pid")
                if pid is None:
                    continue
                try:
                    os.kill(pid, 15)  # SIGTERM for graceful shutdown
                    time.sleep(0.5)
                    try:
                        os.kill(pid, 9)  # SIGKILL as fallback
                    except ProcessLookupError:
                        pass
                    killed = True
                except (ProcessLookupError, OSError):
                    pass
                except Exception as e:
                    print(f"Error stopping PID {pid}: {e}")

                pid_file = proc.get("pid_file")
                if pid_file:
                    try:
                        Path(pid_file).unlink(missing_ok=True)
                    except Exception:
                        pass

            if killed:
                return True

            # Fallback: try process name kill if nothing was found
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
            # Prefer launching the active managed version if available
            if self.version_manager:
                active_tag = self.version_manager.get_active_version()
                if active_tag:
                    success, _process = self.version_manager.launch_version(active_tag)
                    if success:
                        print(f"Launched active managed version: {active_tag}")
                        return True
                    else:
                        print(f"Failed to launch managed version {active_tag}, falling back to legacy run.sh")

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
        Get list of available ComfyUI versions from GitHub with size information

        Args:
            force_refresh: Force refresh from GitHub API (bypass cache)

        Returns:
            List of release dictionaries with size data
        """
        if not self.version_manager:
            return []

        releases_source = "cache"
        releases = []

        # Try to fetch (optionally forced); on failure, fall back to cached data without clearing it
        try:
            releases = self.version_manager.get_available_releases(force_refresh)
            releases_source = "remote" if force_refresh else "cache/remote"
        except Exception as e:
            print(f"Error fetching releases (force_refresh={force_refresh}): {e}")
            releases = []

        if force_refresh and not releases:
            try:
                cache = self.metadata_manager.load_github_cache() if self.metadata_manager else None
                if cache and cache.get("releases"):
                    releases = cache.get("releases", [])
                    releases_source = "cache-fallback"
                    print("Using cached releases due to fetch error/rate-limit.")
            except Exception as e:
                print(f"Error loading cached releases after fetch failure: {e}")

        # Enrich releases with size information (Phase 6.2.5c) + installing flag
        installing_tag = None
        active_progress = None
        try:
            active_progress = self.version_manager.get_installation_progress()
            if active_progress and not active_progress.get('completed_at'):
                installing_tag = active_progress.get('tag')
        except Exception as e:
            print(f"Error checking installation progress for releases: {e}")

        enriched_releases = []
        for release in releases:
            tag = release.get('tag_name', '')

            # Get cached size data if available
            size_data = self.release_size_calculator.get_cached_size(tag)

            # Add size information to release
            release_with_size = dict(release)
            if not release_with_size.get('html_url') and tag:
                release_with_size['html_url'] = f"https://github.com/comfyanonymous/ComfyUI/releases/tag/{tag}"
            if size_data:
                release_with_size['total_size'] = size_data['total_size']
                release_with_size['archive_size'] = size_data['archive_size']
                release_with_size['dependencies_size'] = size_data['dependencies_size']
            else:
                # Size not yet calculated
                release_with_size['total_size'] = None
                release_with_size['archive_size'] = None
                release_with_size['dependencies_size'] = None

            # Flag releases currently installing
            release_with_size['installing'] = bool(installing_tag and tag == installing_tag)

            enriched_releases.append(release_with_size)

        # Kick off background size refresh prioritizing non-installed releases
        try:
            installed_tags = set(self.get_installed_versions())
            self._refresh_release_sizes_async(enriched_releases, installed_tags, force_refresh)
        except Exception as e:
            print(f"Error scheduling size refresh: {e}")

        return enriched_releases

    def get_installed_versions(self) -> List[str]:
        """
        Get list of installed ComfyUI version tags

        Returns:
            List of version tags (e.g., ['v0.2.0', 'v0.1.5'])
        """
        if not self.version_manager:
            return []
        return self.version_manager.get_installed_versions()

    def validate_installations(self) -> Dict[str, Any]:
        """
        Validate all installations and clean up incomplete ones

        Returns:
            Dict with validation results:
                - had_invalid: bool
                - removed: List[str]
                - valid: List[str]
        """
        if not self.version_manager:
            return {
                'had_invalid': False,
                'removed': [],
                'valid': []
            }
        return self.version_manager.validate_installations()

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
        install_ok = self.version_manager.install_version(tag, progress_callback)
        if not install_ok:
            return False

        # Automatically patch the newly installed version so the UI button isn't needed
        patched = self.patch_main_py(tag)
        if not patched and not self.is_patched(tag):
            print(f"Warning: Installation succeeded but patching {tag} failed.")
            return False

        return True

    def cancel_installation(self) -> bool:
        """
        Cancel the currently running installation

        Returns:
            True if cancellation was requested
        """
        if not self.version_manager:
            return False
        return self.version_manager.cancel_installation()

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
        removed = self.version_manager.remove_version(tag)
        if removed:
            # Clean up any version-specific shortcuts and icons
            self.remove_version_shortcuts(tag, remove_menu=True, remove_desktop=True)
        return removed

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

    def get_default_version(self) -> str:
        """
        Get configured default ComfyUI version

        Returns:
            Default version tag or empty string if none
        """
        if not self.version_manager:
            return ""
        return self.version_manager.get_default_version() or ""

    def set_default_version(self, tag: Optional[str]) -> bool:
        """
        Set the default ComfyUI version (or clear when tag is None)
        """
        if not self.version_manager:
            return False
        return self.version_manager.set_default_version(tag)

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

    def open_path(self, path: str) -> Dict[str, Any]:
        """
        Open a filesystem path in the user's file manager (cross-platform).

        Args:
            path: Path to open (absolute or relative to launcher root)

        Returns:
            Dict with success status and optional error message
        """
        return open_in_file_manager(path, base_dir=self.script_dir)

    def open_url(self, url: str) -> Dict[str, Any]:
        """
        Open a URL in the default system browser.

        Args:
            url: URL to open (must start with http:// or https://)

        Returns:
            Dict with success status and optional error message
        """
        if not url or not str(url).strip():
            return {"success": False, "error": "URL is required"}

        if not (url.startswith("http://") or url.startswith("https://")):
            return {"success": False, "error": "Only http/https URLs are allowed"}

        try:
            opened = webbrowser.open(url, new=2)
            if not opened:
                # Fallback to xdg-open/xdg-utils on Linux
                opener = shutil.which("xdg-open")
                if opener:
                    result = subprocess.run([opener, url], capture_output=True)
                    if result.returncode != 0:
                        return {"success": False, "error": f"xdg-open returned {result.returncode}"}
                    return {"success": True}
                return {"success": False, "error": "Unable to open browser"}
            return {"success": True}
        except Exception as e:
            return {"success": False, "error": str(e)}

    def open_active_install(self) -> Dict[str, Any]:
        """
        Open the active ComfyUI installation directory in the file manager.

        Returns:
            Dict with success status and optional error message
        """
        if not self.version_manager:
            return {"success": False, "error": "Version manager not initialized"}

        active_path = self.version_manager.get_active_version_path()
        if not active_path:
            return {"success": False, "error": "No active version or installation incomplete"}

        return self.open_path(str(active_path))

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

    def calculate_release_size(self, tag: str, force_refresh: bool = False) -> Optional[Dict[str, Any]]:
        """
        Calculate total download size for a release (Phase 6.2.5c)

        Args:
            tag: Release tag to calculate size for
            force_refresh: Force recalculation even if cached

        Returns:
            Dict with size breakdown or None if calculation fails
        """
        try:
            # Get release from GitHub
            release = self.version_manager.github_fetcher.get_release_by_tag(tag)
            if not release:
                print(f"Release {tag} not found")
                return None

            # Get archive size from zipball_url
            download_url = release.get('zipball_url') or release.get('tarball_url')
            archive_size = None

            if download_url:
                archive_size = self._get_content_length(download_url)

            # Fallback estimate if HEAD fails
            if not archive_size:
                archive_size = 125 * 1024 * 1024  # 125 MB estimate

            # Calculate total size including dependencies
            result = self.release_size_calculator.calculate_release_size(
                tag=tag,
                archive_size=archive_size,
                force_refresh=force_refresh
            )

            return result
        except Exception as e:
            print(f"Error calculating release size for {tag}: {e}")
            return None

    def calculate_all_release_sizes(self, progress_callback=None) -> Dict[str, Dict[str, Any]]:
        """
        Calculate sizes for all available releases (Phase 6.2.5c)

        Args:
            progress_callback: Optional callback(current, total, tag)

        Returns:
            Dict mapping tag to size data
        """
        releases = self.version_manager.get_available_releases()
        results = {}
        total = len(releases)

        for i, release in enumerate(releases):
            tag = release.get('tag_name', '')
            if progress_callback:
                progress_callback(i + 1, total, tag)

            result = self.calculate_release_size(tag)
            if result:
                results[tag] = result

        return results

    def _get_content_length(self, url: str) -> Optional[int]:
        """
        Perform a HEAD request to retrieve Content-Length for a URL.
        """
        try:
            req = urllib.request.Request(url, method='HEAD')
            req.add_header('User-Agent', 'ComfyUI-Version-Manager/1.0')
            with urllib.request.urlopen(req, timeout=10) as resp:
                length = resp.headers.get('Content-Length')
                if length:
                    return int(length)
        except Exception as e:
            print(f"Warning: Failed to fetch Content-Length for {url}: {e}")
        return None

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
