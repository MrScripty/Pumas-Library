#!/usr/bin/env python3
"""
Shortcut Manager for ComfyUI
Handles desktop shortcuts, menu entries, and icon generation
"""

import shutil
import subprocess
from pathlib import Path
from types import ModuleType
from typing import Any, Dict, Optional

from backend.config import INSTALLATION
from backend.logging_config import get_logger

logger = get_logger(__name__)

# Optional Pillow import for icon editing (used for version-specific shortcut icons)
Image: Optional[ModuleType]
ImageDraw: Optional[ModuleType]
ImageFont: Optional[ModuleType]

try:
    from PIL import Image, ImageDraw, ImageFont
except ImportError:
    Image = None
    ImageDraw = None
    ImageFont = None


class ShortcutManager:
    """Manages desktop shortcuts and icons for ComfyUI versions"""

    def __init__(
        self,
        script_dir: Path,
        icon_webp: Path,
        shortcut_scripts_dir: Path,
        generated_icons_dir: Path,
        version_manager=None,
        metadata_manager=None,
    ):
        """
        Initialize shortcut manager

        Args:
            script_dir: Path to launcher directory
            icon_webp: Path to base icon file
            shortcut_scripts_dir: Path to store launch scripts
            generated_icons_dir: Path to store generated icons
            version_manager: Optional VersionManager instance
            metadata_manager: Optional MetadataManager instance
        """
        self.script_dir = Path(script_dir)
        self.icon_webp = Path(icon_webp)
        self.shortcut_scripts_dir = Path(shortcut_scripts_dir)
        self.generated_icons_dir = Path(generated_icons_dir)
        self.version_manager = version_manager
        self.metadata_manager = metadata_manager

        # System directories
        self.apps_dir = Path.home() / ".local" / "share" / "applications"
        self.apps_file = self.apps_dir / "ComfyUI.desktop"
        self.desktop_file = Path.home() / "Desktop" / "ComfyUI.desktop"

    def _slugify_tag(self, tag: str) -> str:
        """Convert a version tag into a filesystem-safe slug"""
        if not tag:
            return "unknown"
        safe = "".join(c if c.isalnum() or c in ("-", "_") else "-" for c in tag.strip().lower())
        safe = safe.strip("-_") or "unknown"
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

    def _get_version_shortcut_paths(self, tag: str) -> Dict[str, Any]:
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
            except OSError:
                pass

        scalable_dir = icon_base_dir / "scalable" / "apps"
        for ext in ("png", "webp"):
            try:
                icon_path = scalable_dir / f"{icon_name}.{ext}"
                if icon_path.exists():
                    icon_path.unlink()
            except OSError:
                pass

        generated_icon = self.generated_icons_dir / f"{icon_name}.png"
        try:
            if generated_icon.exists():
                generated_icon.unlink()
        except OSError:
            pass

    def _validate_icon_prerequisites(self) -> bool:
        """Validate that icon generation prerequisites are met."""
        if not self.icon_webp.exists():
            logger.warning("Base icon not found; cannot generate version-specific icon")
            return False

        if not (Image and ImageDraw and ImageFont):
            logger.warning("Pillow not available; skipping version label overlay")
            return False
        assert Image is not None
        assert ImageDraw is not None
        assert ImageFont is not None

        return True

    def _create_icon_canvas(self):
        """Create a square canvas with the base icon centered."""
        assert Image is not None
        base = Image.open(self.icon_webp).convert("RGBA")
        size = max(base.size)

        canvas = Image.new("RGBA", (size, size), (0, 0, 0, 0))
        offset = ((size - base.width) // 2, (size - base.height) // 2)
        canvas.paste(base, offset)

        return canvas

    def _load_icon_font(self, canvas_size: int):
        """Load the font for icon text overlay."""
        assert ImageFont is not None
        font_size = max(28, canvas_size // 5 + 2)
        try:
            return ImageFont.truetype("DejaVuSans-Bold.ttf", font_size)
        except OSError:
            return ImageFont.load_default()

    def _prepare_version_label(self, tag: str) -> str:
        """Prepare version label text from tag."""
        label = tag.lstrip("v")
        if len(label) > 12:
            label = label[:12]
        return label

    def _draw_version_banner(self, canvas, label: str, font):
        """Draw the version banner with text on the canvas."""
        size = canvas.size[0]
        assert ImageDraw is not None
        draw = ImageDraw.Draw(canvas)

        # Calculate text dimensions
        if hasattr(draw, "textbbox"):
            bbox = draw.textbbox((0, 0), label, font=font)
            text_w = bbox[2] - bbox[0]
            text_h = bbox[3] - bbox[1]
        else:
            textsize = getattr(draw, "textsize", None)
            if callable(textsize):
                text_w, text_h = textsize(label, font=font)
            else:
                return None

        # Calculate banner dimensions
        padding = max(6, size // 30)
        banner_height = max(text_h + padding * 2, int(size * 0.28))
        banner_y = (size - banner_height) // 2
        background = (
            padding,
            banner_y,
            size - padding,
            banner_y + banner_height,
        )

        # Draw background banner
        try:
            draw.rounded_rectangle(background, radius=padding, fill=(0, 0, 0, 190))
        except (AttributeError, TypeError, ValueError):
            draw.rectangle(background, fill=(0, 0, 0, 190))

        # Draw text
        draw.text(
            ((size - text_w) / 2, banner_y + (banner_height - text_h) / 2),
            label,
            font=font,
            fill=(255, 255, 255, 230),
        )

    def _save_generated_icon(self, tag: str, canvas) -> Optional[Path]:
        """Save the generated icon to disk."""
        slug = self._slugify_tag(tag)
        dest_path = self.generated_icons_dir / f"comfyui-{slug}.png"
        canvas.save(dest_path, format="PNG")
        return dest_path

    def _generate_version_icon(self, tag: str) -> Optional[Path]:
        """
        Create a PNG icon with the version number overlaid.

        Process:
        1. Validates prerequisites (base icon exists, Pillow available)
        2. Creates square canvas with base icon
        3. Loads font for version label
        4. Draws semi-transparent banner with version text
        5. Saves generated icon to disk

        Args:
            tag: Version tag to display on icon

        Returns:
            Path to generated icon, or None if generation failed
        """
        if not self._validate_icon_prerequisites():
            return None

        try:
            canvas = self._create_icon_canvas()
            font = self._load_icon_font(canvas.size[0])
            label = self._prepare_version_label(tag)
            self._draw_version_banner(canvas, label, font)
            return self._save_generated_icon(tag, canvas)
        except (OSError, TypeError, ValueError) as e:
            logger.error(f"Error generating icon for {tag}: {e}", exc_info=True)
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
                    assert Image is not None
                    with Image.open(icon_source) as img:
                        img = img.convert("RGBA")
                        resampling_enum = getattr(Image, "Resampling", None)
                        if resampling_enum is not None:
                            resample_filter = getattr(
                                resampling_enum, "LANCZOS", resampling_enum.BICUBIC
                            )
                        else:
                            resample_filter = getattr(Image, "LANCZOS", Image.BICUBIC)
                        img.thumbnail((size, size), resample=resample_filter)
                        img.save(dest_icon, format="PNG")
                        conversion_success = True
                else:
                    result = subprocess.run(
                        [
                            "convert",
                            str(icon_source),
                            "-resize",
                            f"{size}x{size}",
                            str(dest_icon),
                        ],
                        capture_output=True,
                        timeout=10,
                    )
                    if result.returncode == 0:
                        conversion_success = True
            except (
                FileNotFoundError,
                OSError,
                TypeError,
                ValueError,
                subprocess.SubprocessError,
            ) as e:
                logger.error(f"Error installing icon size {size} for {tag}: {e}", exc_info=True)

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
                except OSError:
                    pass
            except OSError as e:
                logger.error(f"Error installing fallback icon for {tag}: {e}", exc_info=True)

        # Update icon cache if available
        try:
            subprocess.run(
                ["gtk-update-icon-cache", "-f", "-t", str(icon_base_dir)],
                capture_output=True,
                timeout=5,
            )
        except (FileNotFoundError, OSError, subprocess.SubprocessError):
            pass

        try:
            subprocess.run(
                [
                    "xdg-icon-resource",
                    "install",
                    "--novendor",
                    "--size",
                    "256",
                    str(icon_source),
                    icon_name,
                ],
                capture_output=True,
                timeout=5,
            )
        except (FileNotFoundError, OSError, subprocess.SubprocessError):
            pass

        return icon_name

    def _write_version_launch_script(
        self, tag: str, version_dir: Path, slug: str
    ) -> Optional[Path]:
        """Create a launch script for a specific version"""
        script_path = self.shortcut_scripts_dir / f"launch-{slug}.sh"

        # Use metadata_manager if available, otherwise use default path
        if self.metadata_manager:
            profile_dir = self.metadata_manager.launcher_data_dir / "profiles" / slug
        else:
            profile_dir = self.script_dir / "launcher-data" / "profiles" / slug

        profile_dir.mkdir(parents=True, exist_ok=True)
        server_start_delay = INSTALLATION.SERVER_START_DELAY_SEC

        content = f"""#!/bin/bash
set -euo pipefail

VERSION_DIR="{version_dir}"
VENV_PATH="$VERSION_DIR/venv"
MAIN_PY="$VERSION_DIR/main.py"
PID_FILE="$VERSION_DIR/comfyui.pid"
URL="http://127.0.0.1:8188"
WINDOW_CLASS="ComfyUI-{slug}"
PROFILE_DIR="{profile_dir}"
SERVER_START_DELAY={server_start_delay}
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
        except OSError as e:
            logger.error(f"Error writing launch script for {tag}: {e}", exc_info=True)
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
            for tag in self.version_manager.get_installed_versions():
                states[tag] = self.get_version_shortcut_state(tag)

        active_version = None
        if self.version_manager:
            active_version = self.version_manager.get_active_version()

        return {
            "active": active_version,
            "states": states,
        }

    def create_version_shortcuts(
        self, tag: str, create_menu: bool = True, create_desktop: bool = True
    ) -> Dict[str, Any]:
        """Create menu/desktop shortcuts for a specific version"""
        paths = self._get_version_paths(tag)
        shortcut_paths = self._get_version_shortcut_paths(tag)

        if not paths:
            return {
                "success": False,
                "error": f"Version {tag} is not installed or incomplete.",
            }

        # Ensure base icon exists for menu entries (no version banner)
        base_icon_name = "comfyui"
        self.install_icon()

        # Use version banner icon only for desktop shortcut
        desktop_icon_name = self._install_version_icon(tag)
        if not desktop_icon_name:
            desktop_icon_name = base_icon_name

        launcher_script = self._write_version_launch_script(
            tag, paths["version_dir"], shortcut_paths["slug"]
        )

        if not launcher_script:
            return {"success": False, "error": "Failed to write launch script"}

        results: Dict[str, Any] = {"menu": False, "desktop": False}

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
            except OSError as e:
                logger.error(f"Error creating menu shortcut for {tag}: {e}", exc_info=True)

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
            except OSError as e:
                logger.error(f"Error creating desktop shortcut for {tag}: {e}", exc_info=True)

        results["success"] = (not create_menu or results["menu"]) and (
            not create_desktop or results["desktop"]
        )
        results["state"] = self.get_version_shortcut_state(tag)
        return results

    def remove_version_shortcuts(
        self, tag: str, remove_menu: bool = True, remove_desktop: bool = True
    ) -> Dict[str, Any]:
        """Remove version-specific shortcuts and icons"""
        paths = self._get_version_shortcut_paths(tag)
        if remove_menu:
            try:
                paths["menu"].unlink(missing_ok=True)
            except OSError:
                pass

        if remove_desktop:
            try:
                paths["desktop"].unlink(missing_ok=True)
            except OSError:
                pass

        # Remove launcher script if no shortcuts remain
        state_after = self.get_version_shortcut_state(tag)
        if not state_after["menu"] and not state_after["desktop"]:
            try:
                paths["launcher"].unlink(missing_ok=True)
            except OSError:
                pass
            self._remove_installed_icon(paths["icon_name"])

        return {"success": True, "state": state_after}

    def set_version_shortcuts(
        self, tag: str, enabled: bool, menu: bool = True, desktop: bool = True
    ) -> Dict[str, Any]:
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
                        [
                            "convert",
                            str(self.icon_webp),
                            "-resize",
                            f"{size}x{size}",
                            str(dest_icon),
                        ],
                        capture_output=True,
                        timeout=10,
                    )
                    if result.returncode == 0:
                        conversion_success = True
                except (FileNotFoundError, OSError, subprocess.SubprocessError):
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
                except OSError:
                    pass

            # Update icon cache if available
            try:
                subprocess.run(
                    ["gtk-update-icon-cache", "-f", "-t", str(icon_base_dir)],
                    capture_output=True,
                    timeout=5,
                )
            except (FileNotFoundError, OSError, subprocess.SubprocessError):
                pass

            # Also try xdg-icon-resource as alternative installation method
            try:
                subprocess.run(
                    [
                        "xdg-icon-resource",
                        "install",
                        "--novendor",
                        "--size",
                        "256",
                        str(self.icon_webp),
                        "comfyui",
                    ],
                    capture_output=True,
                    timeout=5,
                )
            except (FileNotFoundError, OSError, subprocess.SubprocessError):
                pass

            return True
        except OSError as e:
            logger.error(f"Error installing icon: {e}", exc_info=True)
            return False

    def create_menu_shortcut(self) -> bool:
        """Create application menu shortcut"""
        logger.info("Legacy shortcuts are disabled; use version-specific shortcuts instead.")
        return False

    def create_desktop_shortcut(self) -> bool:
        """Create desktop shortcut"""
        logger.info("Legacy shortcuts are disabled; use version-specific shortcuts instead.")
        return False

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
