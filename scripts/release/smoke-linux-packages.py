#!/usr/bin/env python3
"""Extract exact Linux installers and verify their resources and backend startup."""

import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile

parser = argparse.ArgumentParser()
parser.add_argument("directory", type=Path)
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
version = json.loads((root / "package.json").read_text())["version"]
packages = args.directory.resolve()
subprocess.run(
    ["node", str(root / "scripts/release/check-artifacts.mjs"), str(packages), "linux"],
    check=True,
)


def digest(path: Path) -> bytes:
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").digest()


with tempfile.TemporaryDirectory(prefix="pumas-installer-smoke-") as temporary:
    extracted = Path(temporary)
    appimage = packages / f"Pumas.Library-{version}.AppImage"
    appimage.chmod(appimage.stat().st_mode | 0o100)
    subprocess.run(
        [str(appimage), "--appimage-extract"],
        cwd=extracted,
        stdout=subprocess.DEVNULL,
        check=True,
    )
    subprocess.run(
        [
            "dpkg-deb",
            "-x",
            str(packages / f"pumas-library-electron_{version}_amd64.deb"),
            str(extracted / "deb"),
        ],
        check=True,
    )
    for resources in (
        extracted / "squashfs-root/resources",
        extracted / "deb/opt/Pumas Library/resources",
    ):
        for bundled, source in (
            ("pumas-rpc", root / "electron/resources/bin/pumas-rpc"),
            ("LICENSE.txt", root / "LICENSE"),
            (
                "THIRD-PARTY-NOTICES.txt",
                root / "docs/release-attribution/0.7.0/THIRD-PARTY-NOTICES.txt",
            ),
            ("frontend/index.html", root / "frontend/dist/index.html"),
        ):
            if digest(resources / bundled) != digest(source):
                raise RuntimeError(f"Packaged resource differs from build input: {bundled}")
        for library in (root / "electron/resources/bin").iterdir():
            if digest(resources / library.name) != digest(library):
                raise RuntimeError(f"Packaged native dependency differs: {library.name}")
        if not (resources / "app.asar").is_file():
            raise RuntimeError("Packaged Electron application is missing")
        subprocess.run(
            [sys.executable, str(root / "scripts/release/smoke-rpc.py"), str(resources)],
            check=True,
        )
        print(f"Verified extracted installer resources and RPC: {resources}")
