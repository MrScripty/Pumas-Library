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
parser.add_argument("--variant", choices=("full", "no-inference"), default="full")
args = parser.parse_args()
root = Path(__file__).resolve().parents[2]
version = json.loads((root / "package.json").read_text())["version"]
packages = args.directory.resolve()
inference_disabled = args.variant == "no-inference"
check_token = "linux-no-inference" if inference_disabled else "linux"
if inference_disabled:
    appimage_name = f"Pumas.Library-no-inference-{version}.AppImage"
    deb_name = f"pumas-library-electron-no-inference_{version}_amd64.deb"
else:
    appimage_name = f"Pumas.Library-{version}.AppImage"
    deb_name = f"pumas-library-electron_{version}_amd64.deb"
subprocess.run(
    ["node", str(root / "scripts/release/check-artifacts.mjs"), str(packages), check_token],
    check=True,
)


def digest(path: Path) -> bytes:
    with path.open("rb") as handle:
        return hashlib.file_digest(handle, "sha256").digest()


with tempfile.TemporaryDirectory(prefix="pumas-installer-smoke-") as temporary:
    extracted = Path(temporary)
    appimage = packages / appimage_name
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
            str(packages / deb_name),
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
        command = [sys.executable, str(root / "scripts/release/smoke-rpc.py"), str(resources)]
        if inference_disabled:
            command.append("--inference-disabled")
        subprocess.run(command, check=True)
        print(f"Verified extracted installer resources and RPC: {resources}")
