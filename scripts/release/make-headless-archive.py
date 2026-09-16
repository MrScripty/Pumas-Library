#!/usr/bin/env python3
"""Assemble a headless no-inference pumas-rpc archive per the artifact plan.

Packages a `--no-default-features` backend binary with the project license and
third-party notices into the exact archive filename declared in
scripts/release/artifact-plan.json. Linux/macOS produce tar.gz; Windows
produces zip. Startup/inference-absence verification stays in smoke-rpc.py.
"""

import argparse
import json
import shutil
import tarfile
import tempfile
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLAN = json.loads((ROOT / "scripts/release/artifact-plan.json").read_text())
OS_ARCH = {
    "linux": "linux-x86_64",
    "macos": "macos-arm64",
    "windows": "windows-x86_64",
}


def plan_filename(artifact_id: str, version: str) -> str:
    for artifact in PLAN["artifacts"]:
        if artifact["id"] == artifact_id:
            return artifact["filename"].replace("{version}", version)
    raise SystemExit(f"Unknown artifact in plan: {artifact_id}")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--os", choices=sorted(OS_ARCH), required=True)
    parser.add_argument(
        "--version", default=json.loads((ROOT / "package.json").read_text())["version"]
    )
    args = parser.parse_args()

    binary = args.binary
    if binary.is_dir():
        binary = binary / ("pumas-rpc.exe" if args.os == "windows" else "pumas-rpc")
    if not binary.is_file() or binary.stat().st_size == 0:
        raise SystemExit(f"Missing or empty headless backend: {binary}")

    artifact_id = {
        "linux": "headless-linux-archive",
        "macos": "headless-macos-archive",
        "windows": "headless-windows-archive",
    }[args.os]
    filename = plan_filename(artifact_id, args.version)
    notices = ROOT / "docs/release-attribution/0.7.0/THIRD-PARTY-NOTICES.txt"
    license_file = ROOT / "LICENSE"
    for required in (notices, license_file):
        if not required.is_file() or required.stat().st_size == 0:
            raise SystemExit(f"Missing release metadata: {required}")

    args.output_dir.mkdir(parents=True, exist_ok=True)
    # Refuse to mix cohorts: the exact plan filename must not already exist.
    output = args.output_dir / filename
    if output.exists():
        raise SystemExit(f"Refusing to overwrite existing archive: {output}")

    with tempfile.TemporaryDirectory(prefix="pumas-headless-") as staging:
        stage = Path(staging)
        shutil.copy2(binary, stage / binary.name)
        shutil.copy2(license_file, stage / "LICENSE.txt")
        shutil.copy2(notices, stage / "THIRD-PARTY-NOTICES.txt")
        if filename.endswith(".tar.gz"):
            with tarfile.open(output, "w:gz", format=tarfile.PAX_FORMAT) as archive:
                for member in sorted(stage.iterdir()):
                    archive.add(member, arcname=member.name)
        elif filename.endswith(".zip"):
            with zipfile.ZipFile(output, "w", zipfile.ZIP_DEFLATED) as archive:
                for member in sorted(stage.iterdir()):
                    archive.write(member, arcname=member.name)
        else:
            raise SystemExit(f"Unsupported archive format: {filename}")
    print(f"Assembled {output}")


if __name__ == "__main__":
    main()
