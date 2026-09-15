#!/usr/bin/env python3
"""Install the native NSIS candidate and start it and the portable candidate."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def start_desktop(executable, root):
    library = root / "library"
    (library / "shared-resources/models").mkdir(parents=True)
    user_data = root / "user-data"
    environment = {
        **os.environ,
        "APPDATA": str(root / "config"),
        "XDG_CONFIG_HOME": str(root / "config"),
        "PUMAS_REGISTRY_DB_PATH": str(root / "registry.db"),
        "PUMAS_LAUNCHER_ROOT": str(library),
        "PUMAS_RELEASE_SMOKE": "1",
    }
    log_path = root / "desktop.log"
    with log_path.open("wb") as log:
        process = subprocess.Popen(
            [str(executable), "--user-data-dir=" + str(user_data)],
            env=environment,
            stdout=log,
            stderr=subprocess.STDOUT,
        )
        try:
            process.wait(timeout=90)
            if process.returncode != 0:
                raise RuntimeError(f"Desktop exited with {process.returncode}")
        finally:
            if process.poll() is None:
                subprocess.run(
                    ["taskkill", "/PID", str(process.pid), "/T", "/F"],
                    check=False,
                    timeout=15,
                )
                process.wait(timeout=10)
            print(log_path.read_text(errors="replace"))
    # Windows GUI executables may log only to electron-log's file transport.
    logs = [log_path, *user_data.glob("logs/*.log")]
    if not any(
        "Release smoke startup succeeded" in path.read_text(errors="replace") for path in logs
    ):
        raise RuntimeError("Desktop did not report successful backend initialization")


def verify(directory):
    if sys.platform != "win32":
        raise RuntimeError("Windows installer verification requires native Windows")
    version = json.loads((ROOT / "electron/package.json").read_text())["version"]
    installer = directory / f"Pumas.Library.Setup.{version}.exe"
    portable = directory / f"Pumas.Library.{version}.exe"
    with tempfile.TemporaryDirectory(prefix="pumas-windows-smoke-") as temporary:
        root = Path(temporary)
        installed = root / "installed"
        try:
            # NSIS requires /D last, with its value unquoted even when it has spaces.
            # Popen receives a native command line directly; no shell is involved.
            subprocess.run(f'"{installer}" /S /D={installed}', check=True, timeout=120)
            resources = installed / "resources"
            for bundled, source in [
                ("pumas-rpc.exe", ROOT / "electron/resources/bin/pumas-rpc.exe"),
                ("app.asar", directory / "win-unpacked/resources/app.asar"),
                ("LICENSE.txt", ROOT / "LICENSE"),
                (
                    "THIRD-PARTY-NOTICES.txt",
                    ROOT / "docs/release-attribution/0.7.0/THIRD-PARTY-NOTICES.txt",
                ),
                ("frontend/index.html", ROOT / "frontend/dist/index.html"),
            ]:
                if digest(resources / bundled) != digest(source):
                    raise RuntimeError(f"Installed resource differs: {bundled}")
            for library in (ROOT / "electron/resources/bin").glob("*.dll"):
                if digest(resources / library.name) != digest(library):
                    raise RuntimeError(f"Installed DLL differs: {library.name}")
            subprocess.run(
                [sys.executable, str(ROOT / "scripts/release/smoke-rpc.py"), str(resources)],
                check=True,
                timeout=75,
            )
            start_desktop(installed / "Pumas Library.exe", root / "nsis-smoke")
            start_desktop(portable, root / "portable-smoke")
        finally:
            uninstallers = list(installed.glob("Uninstall*.exe"))
            if len(uninstallers) == 1:
                # Run in place and wait instead of NSIS's detached temporary copy.
                subprocess.run(
                    f'"{uninstallers[0]}" /S _?={installed}',
                    check=True,
                    timeout=90,
                )
        print("NSIS installation, resource identity, RPC and both desktop startups passed")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("directory", type=Path)
    verify(parser.parse_args().directory.resolve())
