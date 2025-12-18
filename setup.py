#!/usr/bin/env python3

"""
setup.py - Revised Setup script for ComfyUI launcher

This script performs two tasks:
1. Inserts the setproctitle code block into ComfyUI's main.py
   (right after the initial import block, before the if __name__ == "__main__": section)
2. Creates a desktop shortcut (.desktop file) that directly runs the run.sh script
   with the original webp icon.

No tray script is created.
No icon conversion is performed.
"""

import os
import sys
from pathlib import Path

# ----------------------------------------------------------------------
# Determine directories (same as run.sh)
# ----------------------------------------------------------------------
SCRIPT_DIR = Path(__file__).parent.resolve()
COMFYUI_DIR = SCRIPT_DIR.parent
MAIN_PY = COMFYUI_DIR / "main.py"
ICON_WEBP = SCRIPT_DIR / "comfyui-icon.webp"
RUN_SH = SCRIPT_DIR / "run.sh"
DESKTOP_FILE = Path.home() / "Desktop" / "ComfyUI.desktop"

# ----------------------------------------------------------------------
# 1. Patch main.py - insert setproctitle block
# ----------------------------------------------------------------------
def patch_main_py():
    if not MAIN_PY.exists():
        print(f"[ERROR] main.py not found at {MAIN_PY}")
        sys.exit(1)

    backup = MAIN_PY.with_suffix(".py.bak")
    if not backup.exists():
        backup.write_bytes(MAIN_PY.read_bytes())
        print(f"Backed up original main.py to {backup}")

    content = MAIN_PY.read_text()

    # Marker to prevent double-patching
    marker = "try:\n    import setproctitle\n    setproctitle.setproctitle(\"ComfyUI Server\")"
    if marker in content:
        print("main.py already patched with setproctitle block")
        return

    # The code to insert
    insert_code = """\ntry:
    import setproctitle
    setproctitle.setproctitle("ComfyUI Server")
except ImportError:
    pass  # ignore if not installed
"""

    # Find the right insertion point:
    # After the last import line, before the if __name__ == "__main__": block
    lines = content.splitlines()
    new_lines = []
    inserted = False
    import_section_ended = False
    last_import_index = -1

    for i, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("import ") or stripped.startswith("from "):
            last_import_index = i
        new_lines.append(line)

        # After we pass the imports and hit a non-blank, non-comment line that is not another import
        if (last_import_index >= 0 and i > last_import_index and
                stripped and not stripped.startswith("#") and
                not stripped.startswith("import ") and not stripped.startswith("from ")):
            if not import_section_ended:
                new_lines.append(insert_code)
                inserted = True
                import_section_ended = True

    # Fallback: if we couldn't find a clean spot, insert right before if __name__ == "__main__":
    if not inserted:
        for i, line in enumerate(lines):
            new_lines.append(line)
            if line.strip() == 'if __name__ == "__main__":':
                new_lines.insert(i, insert_code)
                inserted = True
                break

    if not inserted:
        print("[ERROR] Could not find suitable place to insert the setproctitle block.")
        print("    You may need to add it manually.")
        return

    MAIN_PY.write_text("\n".join(new_lines) + "\n")
    print("Patched main.py: inserted setproctitle block after imports")

# ----------------------------------------------------------------------
# 2. Create .desktop shortcut that runs run.sh directly
# ----------------------------------------------------------------------
def create_desktop_shortcut():
    if not RUN_SH.exists():
        print(f"[ERROR] run.sh not found at {RUN_SH}")
        sys.exit(1)

    if not ICON_WEBP.exists():
        print(f"[WARNING] Icon not found at {ICON_WEBP}, creating shortcut without icon")
        icon_line = ""
    else:
        icon_line = f"Icon={ICON_WEBP}"

    desktop_content = f"""[Desktop Entry]
Name=ComfyUI
Comment=Launch ComfyUI with isolated Brave window
Exec=bash "{RUN_SH}"
{icon_line}
Terminal=false
Type=Application
Categories=Graphics;ArtificialIntelligence;
StartupNotify=true
"""

    DESKTOP_FILE.write_text(desktop_content)
    DESKTOP_FILE.chmod(0o755)
    print(f"Created desktop shortcut: {DESKTOP_FILE}")
    print("    Double-click it to launch ComfyUI using run.sh")

# ----------------------------------------------------------------------
# Main
# ----------------------------------------------------------------------
if __name__ == "__main__":
    print("=== ComfyUI Setup ===")

    if not RUN_SH.exists():
        print(f"[ERROR] run.sh not found in {SCRIPT_DIR}")
        print("    Place setup.py in the same folder as run.sh")
        sys.exit(1)

    patch_main_py()
    create_desktop_shortcut()

    print("\nSetup complete!")
    print("- A desktop shortcut 'ComfyUI.desktop' has been created.")
    print("- Double-click it to start ComfyUI (it runs your run.sh script).")
    print("- main.py has been patched to set process title to 'ComfyUI Server'.")
    print("- Original main.py backed up as main.py.bak")
