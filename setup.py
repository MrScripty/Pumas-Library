#!/usr/bin/env python3

import os
import sys
import subprocess
import shutil
import urllib.request
import json
from pathlib import Path
import tkinter as tk
from tkinter import ttk, messagebox

# ----------------------------------------------------------------------
# Determine directories
# ----------------------------------------------------------------------
SCRIPT_DIR = Path(__file__).parent.resolve()
COMFYUI_DIR = SCRIPT_DIR.parent
MAIN_PY = COMFYUI_DIR / "main.py"
ICON_WEBP = SCRIPT_DIR / "comfyui-icon.webp"
RUN_SH = SCRIPT_DIR / "run.sh"

APPS_DIR = Path.home() / ".local" / "share" / "applications"
APPS_FILE = APPS_DIR / "ComfyUI.desktop"
DESKTOP_FILE = Path.home() / "Desktop" / "ComfyUI.desktop"

# ----------------------------------------------------------------------
# ComfyUI version detection
# ----------------------------------------------------------------------
def get_comfyui_version():
    try:
        version = subprocess.check_output(
            ['git', '-C', str(COMFYUI_DIR), 'describe', '--tags', '--always'],
            text=True, stderr=subprocess.DEVNULL
        ).strip()
        if version:
            return version
    except: pass

    try:
        with urllib.request.urlopen("https://api.github.com/repos/comfyanonymous/ComfyUI/releases/latest") as resp:
            data = json.loads(resp.read())
            return data['tag_name'] + " (latest release)"
    except:
        return "Unknown"

# ----------------------------------------------------------------------
# Patch & Shortcut functions
# ----------------------------------------------------------------------
def is_patched():
    if not MAIN_PY.exists(): return False
    return "setproctitle.setproctitle(\"ComfyUI Server\")" in MAIN_PY.read_text()

def revert_main_py():
    backup = MAIN_PY.with_suffix(".py.bak")
    if backup.exists():
        MAIN_PY.write_bytes(backup.read_bytes())
        backup.unlink(missing_ok=True)
        return True, "Reverted from backup"

    try:
        result = subprocess.run(['git', '-C', str(COMFYUI_DIR), 'checkout', '--', 'main.py'],
                                capture_output=True, text=True)
        if result.returncode == 0:
            return True, "Reverted via git"
    except: pass

    try:
        url = "https://raw.githubusercontent.com/comfyanonymous/ComfyUI/master/main.py"
        with urllib.request.urlopen(url) as resp:
            MAIN_PY.write_bytes(resp.read())
        return True, "Reverted from GitHub master"
    except Exception as e:
        return False, str(e)

def patch_main_py():
    if is_patched(): return False, "Already patched"
    backup = MAIN_PY.with_suffix(".py.bak")
    if not backup.exists():
        backup.write_bytes(MAIN_PY.read_bytes())

    content = MAIN_PY.read_text()
    insert_code = "\ntry:\n    import setproctitle\n    setproctitle.setproctitle(\"ComfyUI Server\")\nexcept ImportError:\n    pass\n"

    if 'if __name__ == "__main__":' in content:
        content = content.replace('if __name__ == "__main__":', insert_code + 'if __name__ == "__main__":')
    else:
        content += insert_code

    MAIN_PY.write_text(content)
    return True, "Patched"

def menu_exists(): return APPS_FILE.exists()
def desktop_exists(): return DESKTOP_FILE.exists()

def create_menu_desktop():
    APPS_DIR.mkdir(parents=True, exist_ok=True)
    icon_line = f"Icon={ICON_WEBP.resolve()}" if ICON_WEBP.exists() else ""
    content = f"""[Desktop Entry]
Name=ComfyUI
Comment=Launch ComfyUI with isolated Brave window
Exec=bash "{RUN_SH.resolve()}"
{icon_line}
Terminal=false
Type=Application
Categories=Graphics;ArtificialIntelligence;
"""
    APPS_FILE.write_text(content)
    APPS_FILE.chmod(0o644)
    return True, "Created"

def create_desktop_shortcut():
    if not menu_exists(): create_menu_desktop()
    if desktop_exists(): return False, "Exists"
    DESKTOP_FILE.write_text(APPS_FILE.read_text())
    DESKTOP_FILE.chmod(0o755)
    return True, "Created"

def remove_menu_desktop():
    if APPS_FILE.exists(): APPS_FILE.unlink(); return True
    return False

def remove_desktop_shortcut():
    if DESKTOP_FILE.exists(): DESKTOP_FILE.unlink(); return True
    return False

def check_setproctitle():
    try: import setproctitle; return True
    except: return False

def check_git(): return shutil.which('git') is not None
def check_brave(): return shutil.which('brave-browser') is not None

def install_missing_deps():
    missing = []
    if not check_setproctitle(): missing.append("setproctitle")
    if not check_git(): missing.append("git")
    if not check_brave(): missing.append("brave-browser")

    if not missing: return True, "All good"

    if "setproctitle" in missing:
        subprocess.run(['pip3', 'install', '--user', 'setproctitle'], stdout=subprocess.DEVNULL)

    pkgs = [p for p in missing if p in ("git", "brave-browser")]
    if pkgs:
        subprocess.run(['sudo', 'apt', 'update'])
        subprocess.run(['sudo', 'apt', 'install', '-y'] + pkgs)

    return True, "Installed"

# ----------------------------------------------------------------------
# Checkmark Indicator
# ----------------------------------------------------------------------
class CheckMark(tk.Canvas):
    def __init__(self, master, **kwargs):
        super().__init__(master, width=20, height=20, highlightthickness=0, bg='#1e1e1e', **kwargs)

    def show_success(self):
        self.delete("all")
        self.create_line(4, 10, 8, 14, fill="#55ff55", width=4, capstyle="round")
        self.create_line(8, 14, 16, 6, fill="#55ff55", width=4, capstyle="round")

    def show_idle(self):
        self.delete("all")

# ----------------------------------------------------------------------
# GUI
# ----------------------------------------------------------------------
class ComfyUISetupGUI(tk.Tk):
    def __init__(self):
        super().__init__()
        self.title("ComfyUI Setup")
        self.configure(bg='#1e1e1e')
        self.overrideredirect(True)
        self.resizable(False, False)
        self.attributes('-topmost', True)
        self.after(2000, lambda: self.attributes('-topmost', False))

        # Narrower and taller window to fit all content comfortably
        self.geometry("420x580")

        # Dragging
        self._offsetx = self._offsety = 0
        def start_drag(e): self._offsetx, self._offsety = e.x, e.y
        def drag(e):
            x = self.winfo_x() + e.x - self._offsetx
            y = self.winfo_y() + e.y - self._offsety
            self.geometry(f"+{x}+{y}")

        # Title bar
        title_bar = tk.Frame(self, bg='#252525', height=90)
        title_bar.pack(fill='x')
        title_bar.pack_propagate(False)
        title_bar.bind('<ButtonPress-1>', start_drag)
        title_bar.bind('<B1-Motion>', drag)

        title_frame = tk.Frame(title_bar, bg='#252525')
        title_frame.pack(side='left', expand=True)

        tk.Label(title_frame, text="ComfyUI Launcher Setup", bg='#252525', fg='#ffffff',
                 font=('Segoe UI', 16, 'bold')).pack(pady=(18, 2))
        self.version_label = tk.Label(title_frame, text="", bg='#252525', fg='#aaaaaa',
                                      font=('Segoe UI', 11))
        self.version_label.pack(pady=(0, 18))

        # Clean close button
        close_btn = tk.Label(title_bar, text="✕", bg='#252525', fg='#cccccc',
                             font=('Segoe UI', 20, 'bold'), cursor='hand2')
        close_btn.pack(side='right', padx=18, pady=15)
        close_btn.bind('<Enter>', lambda e: close_btn.config(fg='#ff4444'))
        close_btn.bind('<Leave>', lambda e: close_btn.config(fg='#cccccc'))
        close_btn.bind('<ButtonRelease-1>', lambda e: self.destroy())

        # Main content - reduced horizontal padding for narrower window
        main = tk.Frame(self, bg='#1e1e1e', padx=40, pady=35)
        main.pack(fill='both', expand=True)
        main.bind('<ButtonPress-1>', start_drag)
        main.bind('<B1-Motion>', drag)

        # Button style with fixed minimum width
        style = ttk.Style()
        style.theme_use('clam')
        style.configure('Flat.TButton',
                        background='#2d2d2d',
                        foreground='#ffffff',
                        borderwidth=0,
                        focuscolor='none',
                        font=('Segoe UI', 10),
                        padding=(20, 12))
        style.map('Flat.TButton',
                  background=[('active', '#3d3d3d')])

        # Fixed button width - wide enough for "Unpatch ComfyUI" and "Remove Desktop"
        style.configure('Flat.TButton', minwidth=160)

        row = 0

        # ComfyUI section (was Process Title Patch)
        tk.Label(main, text="ComfyUI", bg='#1e1e1e', fg='#ffffff',
                 font=('Segoe UI', 12, 'bold')).grid(row=row, column=0, columnspan=2, sticky='w', pady=(0, 20))
        row += 1
        self.ind1 = CheckMark(main)
        self.ind1.grid(row=row, column=0, padx=(0, 25), pady=4)
        self.btn_patch = ttk.Button(main, text="Patch ComfyUI", style='Flat.TButton', command=self.handle_patch)
        self.btn_patch.grid(row=row, column=1, sticky='ew', pady=4)
        row += 1

        # Shortcuts section
        tk.Label(main, text="Shortcuts", bg='#1e1e1e', fg='#ffffff',
                 font=('Segoe UI', 12, 'bold')).grid(row=row, column=0, columnspan=2, sticky='w', pady=(30, 15))
        row += 1

        self.ind_menu = CheckMark(main)
        self.ind_menu.grid(row=row, column=0, padx=(0, 25), pady=4)
        self.btn_menu = ttk.Button(main, text="Menu", style='Flat.TButton', command=self.handle_menu)
        self.btn_menu.grid(row=row, column=1, sticky='ew', pady=4)
        row += 1

        self.ind_desk = CheckMark(main)
        self.ind_desk.grid(row=row, column=0, padx=(0, 25), pady=4)
        self.btn_desk = ttk.Button(main, text="Desktop", style='Flat.TButton', command=self.handle_desktop)
        self.btn_desk.grid(row=row, column=1, sticky='ew', pady=4)
        row += 1

        # Dependencies
        tk.Label(main, text="Dependencies", bg='#1e1e1e', fg='#ffffff',
                 font=('Segoe UI', 12, 'bold')).grid(row=row, column=0, columnspan=2, sticky='w', pady=(35, 15))
        row += 1

        self.ind_deps = CheckMark(main)
        self.ind_deps.grid(row=row, column=0, padx=(0, 25), pady=4)
        self.btn_deps = ttk.Button(main, text="Install Missing", style='Flat.TButton', command=self.install_deps)
        self.btn_deps.grid(row=row, column=1, sticky='ew', pady=4)

        # Footer with more breathing room
        self.footer = tk.Label(main, text="", bg='#1e1e1e', fg='#55ff55', font=('Segoe UI', 10, 'italic'))
        self.footer.grid(row=row + 2, column=0, columnspan=2, pady=(50, 0))

        main.grid_columnconfigure(1, weight=1)

        # Keyboard
        self.bind('<Escape>', lambda e: self.destroy())

        # Initial update
        self.version_label.config(text=f"Version: {get_comfyui_version()}")
        self.update_status()

    def update_status(self):
        # Patch
        patched = is_patched()
        self.ind1.show_success() if patched else self.ind1.show_idle()
        self.btn_patch.config(text="Unpatch ComfyUI" if patched else "Patch ComfyUI")

        # Menu
        menu_ok = menu_exists()
        self.ind_menu.show_success() if menu_ok else self.ind_menu.show_idle()
        self.btn_menu.config(text="Remove Menu" if menu_ok else "Menu")

        # Desktop
        desk_ok = desktop_exists()
        self.ind_desk.show_success() if desk_ok else self.ind_desk.show_idle()
        self.btn_desk.config(text="Remove Desktop" if desk_ok else "Desktop")

        # Dependencies
        missing = [n for n, c in [("setproctitle", check_setproctitle()),
                                  ("git", check_git()),
                                  ("brave", check_brave())] if not c]
        all_good = len(missing) == 0
        self.ind_deps.show_success() if all_good else self.ind_deps.show_idle()
        self.btn_deps.config(state='disabled' if all_good else 'normal')
        self.btn_deps.config(text="All Installed" if all_good else "Install Missing")

        # Footer
        if all_good and patched and menu_ok and desktop_exists():
            self.footer.config(text="Setup complete – everything is ready", fg="#55ff55")
        else:
            self.footer.config(text="Complete the items above to finish setup", fg="#ffcc00")

    def handle_patch(self):
        if is_patched(): revert_main_py()
        else: patch_main_py()
        self.update_status()

    def handle_menu(self):
        if menu_exists(): remove_menu_desktop()
        else: create_menu_desktop()
        self.update_status()

    def handle_desktop(self):
        if desktop_exists(): remove_desktop_shortcut()
        else: create_desktop_shortcut()
        self.update_status()

    def install_deps(self):
        install_missing_deps()
        self.update_status()

if __name__ == "__main__":
    if not RUN_SH.exists():
        print(f"[ERROR] run.sh not found in {SCRIPT_DIR}")
        sys.exit(1)

    app = ComfyUISetupGUI()
    app.mainloop()
