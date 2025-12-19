# -*- mode: python ; coding: utf-8 -*-
#
# PyInstaller Spec File for ComfyUI Setup Launcher
# Bundles Python backend + React frontend into single executable
#

from PyInstaller.utils.hooks import collect_data_files
import os
from pathlib import Path

# Paths
spec_root = Path(SPECPATH)
frontend_dist = spec_root / 'frontend' / 'dist'
backend_dir = spec_root / 'backend'

# Collect all frontend build files
frontend_datas = []
if frontend_dist.exists():
    for root, dirs, files in os.walk(frontend_dist):
        for file in files:
            src_path = os.path.join(root, file)
            # Calculate relative path from dist directory
            rel_path = os.path.relpath(src_path, frontend_dist)
            # Map to frontend/dist in bundle
            dest_dir = os.path.join('frontend', 'dist', os.path.dirname(rel_path))
            frontend_datas.append((src_path, dest_dir))

# Collect additional data files that might be needed
additional_datas = []

# Collect PyWebView data files if any
pywebview_datas = collect_data_files('webview')
additional_datas.extend(pywebview_datas)

# Combine all data files
all_datas = frontend_datas + additional_datas

a = Analysis(
    [str(backend_dir / 'main.py')],
    pathex=[str(backend_dir)],
    binaries=[],
    datas=all_datas,
    hiddenimports=[
        'webview',
        'webview.platforms.gtk',
    ],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        'matplotlib',
        'numpy',
        'pandas',
        'PIL',
        'scipy',
        'tkinter',
        '_tkinter',
    ],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=None,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=None)

exe = EXE(
    pyz,
    a.scripts,
    a.binaries,
    a.zipfiles,
    a.datas,
    [],
    name='comfyui-setup',
    debug=False,
    bootloader_ignore_signals=False,
    strip=True,  # Strip symbols for smaller size
    upx=False,  # Disable UPX - MUCH faster builds, minimal size difference
    upx_exclude=[],
    runtime_tmpdir=None,
    console=False,  # No console window
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)
