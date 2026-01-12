# -*- mode: python ; coding: utf-8 -*-
#
# PyInstaller Spec File for ComfyUI Setup Launcher - Python Sidecar
# Bundles Python RPC server for use with Electron
#
# NOTE: The desktop GUI is now handled by Electron.
# This spec file builds the Python sidecar (rpc_server.py) that Electron spawns.
#

import os
from pathlib import Path

# Paths
spec_root = Path(SPECPATH)
backend_dir = spec_root / 'backend'

# No frontend files needed - Electron handles the UI
all_datas = []

a = Analysis(
    [str(backend_dir / 'rpc_server.py')],
    pathex=[str(backend_dir)],
    binaries=[],
    datas=all_datas,
    hiddenimports=[],
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
    name='pumas-sidecar',
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
