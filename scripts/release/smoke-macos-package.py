#!/usr/bin/env python3
"""Mount a DMG, copy its application and verify the installed ARM64 payload.

Run on macOS with a trusted candidate. This checks loading and startup, not
notarization, Gatekeeper acceptance, or interactive desktop workflows.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import plistlib
import signal
import subprocess
import sys
import tempfile


ROOT = Path(__file__).resolve().parents[2]


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify(package, variant="full"):
    if sys.platform != "darwin":
        raise RuntimeError("DMG verification requires native macOS")
    inference_disabled = variant == "no-inference"
    version = json.loads((ROOT / "electron/package.json").read_text())["version"]
    with tempfile.TemporaryDirectory(prefix="pumas-dmg-smoke-") as temporary:
        root = Path(temporary)
        mount = root / "mount"
        mount.mkdir()
        subprocess.run(
            [
                "hdiutil",
                "attach",
                "-readonly",
                "-nobrowse",
                "-mountpoint",
                str(mount),
                str(package),
            ],
            check=True,
            timeout=90,
        )
        try:
            apps = list(mount.glob("*.app"))
            if len(apps) != 1:
                raise RuntimeError(f"Expected one application, found {len(apps)}")
            app = root / apps[0].name
            subprocess.run(["ditto", str(apps[0]), str(app)], check=True, timeout=90)
        finally:
            subprocess.run(["hdiutil", "detach", str(mount)], check=True, timeout=30)

        contents = app / "Contents"
        info = plistlib.loads((contents / "Info.plist").read_bytes())
        if info["CFBundleShortVersionString"] != version:
            raise RuntimeError("Application version does not match the release")
        resources = contents / "Resources"
        for bundled, source in [
            ("pumas-rpc", ROOT / "electron/resources/bin/pumas-rpc"),
            ("LICENSE.txt", ROOT / "LICENSE"),
            (
                "THIRD-PARTY-NOTICES.txt",
                ROOT / "docs/release-attribution/0.7.0/THIRD-PARTY-NOTICES.txt",
            ),
            ("frontend/index.html", ROOT / "frontend/dist/index.html"),
        ]:
            if digest(resources / bundled) != digest(source):
                raise RuntimeError(f"Packaged resource differs from build input: {bundled}")
        executable = contents / "MacOS" / info["CFBundleExecutable"]
        for binary in [executable, resources / "pumas-rpc"]:
            subprocess.run(["lipo", str(binary), "-verify_arch", "arm64"], check=True, timeout=10)
        for library in (ROOT / "electron/resources/bin").glob("libonnxruntime*.dylib"):
            if digest(resources / library.name) != digest(library):
                raise RuntimeError(f"Packaged ONNX library differs: {library.name}")
        command = [
            sys.executable,
            str(ROOT / "scripts/release/smoke-rpc.py"),
            str(resources / "pumas-rpc"),
        ]
        if inference_disabled:
            command.append("--inference-disabled")
        subprocess.run(command, check=True, timeout=75)
        library_root = root / "library"
        (library_root / "shared-resources/models").mkdir(parents=True)
        process = subprocess.Popen(
            [str(executable), "--user-data-dir=" + str(root / "user-data")],
            env={
                **os.environ,
                "XDG_CONFIG_HOME": str(root / "config"),
                "PUMAS_REGISTRY_DB_PATH": str(root / "registry.db"),
                "PUMAS_LAUNCHER_ROOT": str(library_root),
                "PUMAS_RELEASE_SMOKE": "1",
            },
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            start_new_session=True,
        )
        try:
            output, _ = process.communicate(timeout=75)
            print(output)
            if process.returncode != 0:
                raise RuntimeError(f"Desktop exited with {process.returncode}")
        finally:
            # Own the smoke process group, including a backend left by failed startup.
            try:
                os.killpg(process.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            process.wait(timeout=10)
        if "Release smoke startup succeeded" not in output:
            raise RuntimeError("Desktop did not report successful backend initialization")
        print("DMG copy, resource identity, ARM64 loading and desktop startup passed")


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("package", type=Path)
    parser.add_argument("--variant", choices=("full", "no-inference"), default="full")
    args = parser.parse_args()
    verify(args.package.resolve(), args.variant)
