#!/usr/bin/env python3
"""Assemble a headless no-inference pumas-rpc archive per the artifact plan.

Packages a `--no-default-features` backend binary with the project license and
third-party notices into the exact archive filename declared in
scripts/release/artifact-plan.json. Linux/macOS produce tar.gz; Windows
produces zip. Startup/inference-absence verification stays in smoke-rpc.py.
"""

import argparse
import gzip
import hashlib
import json
import re
import shutil
import stat
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


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        while chunk := stream.read(65536):
            digest.update(chunk)
    return digest.hexdigest()


def write_archive(stage: Path, output: Path) -> None:
    """Stable assembly bytes within the same Python/zlib environment.

    This does not prove independently compiled executables are reproducible.
    """
    # Keep exclusive creation outside cleanup ownership: an existing path must
    # never be removed. Include the final buffered flush/close in that ownership.
    raw = output.open("xb")
    try:
        with raw:
            if output.name.endswith(".tar.gz"):
                with (
                    gzip.GzipFile(
                        filename="", fileobj=raw, mode="wb", mtime=0, compresslevel=9
                    ) as gz,
                    tarfile.open(fileobj=gz, mode="w|", format=tarfile.USTAR_FORMAT) as archive,
                ):
                    for path in sorted(stage.iterdir()):
                        member = tarfile.TarInfo(path.name)
                        member.size = path.stat().st_size
                        member.mode = 0o755 if path.name == "pumas-rpc" else 0o644
                        with path.open("rb") as source:
                            archive.addfile(member, source)
            elif output.name.endswith(".zip"):
                with zipfile.ZipFile(raw, "w", zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
                    for path in sorted(stage.iterdir()):
                        member = zipfile.ZipInfo(path.name, date_time=(1980, 1, 1, 0, 0, 0))
                        member.create_system = 3
                        member.compress_type = zipfile.ZIP_DEFLATED
                        member.file_size = path.stat().st_size
                        member.external_attr = (stat.S_IFREG | 0o644) << 16
                        with path.open("rb") as source, archive.open(member, "w") as target:
                            shutil.copyfileobj(source, target, length=65536)
            else:
                raise ValueError("Unsupported headless archive format")
    except BaseException:
        # The raw context has attempted close, including after an inner failure.
        # Cleanup still runs when that final close itself raises.
        output.unlink()
        raise


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
        "--binary-sha256",
        help="Require these exact executable bytes before assembly; lowercase SHA256",
    )
    parser.add_argument(
        "--version", default=json.loads((ROOT / "package.json").read_text())["version"]
    )
    args = parser.parse_args()

    binary = args.binary
    if binary.is_dir():
        binary = binary / ("pumas-rpc.exe" if args.os == "windows" else "pumas-rpc")
    binary_name = "pumas-rpc.exe" if args.os == "windows" else "pumas-rpc"
    if (
        binary.name != binary_name
        or binary.is_symlink()
        or not binary.is_file()
        or binary.stat().st_size == 0
    ):
        raise SystemExit(f"Missing or empty headless backend: {binary}")
    if args.binary_sha256 is not None and not re.fullmatch(r"[a-f0-9]{64}", args.binary_sha256):
        parser.error("--binary-sha256 requires a lowercase SHA256")

    artifact_id = {
        "linux": "headless-linux-archive",
        "macos": "headless-macos-archive",
        "windows": "headless-windows-archive",
    }[args.os]
    filename = plan_filename(artifact_id, args.version)
    notices = ROOT / "docs/release-attribution/0.7.0/THIRD-PARTY-NOTICES.txt"
    license_file = ROOT / "LICENSE"
    for required in (notices, license_file):
        if required.is_symlink() or not required.is_file() or required.stat().st_size == 0:
            raise SystemExit(f"Missing release metadata: {required}")

    with tempfile.TemporaryDirectory(prefix="pumas-headless-") as staging:
        stage = Path(staging)
        shutil.copyfile(binary, stage / binary_name)
        shutil.copyfile(license_file, stage / "LICENSE.txt")
        shutil.copyfile(notices, stage / "THIRD-PARTY-NOTICES.txt")
        observed_binary_sha256 = sha256(stage / binary_name)
        if args.binary_sha256 is not None and observed_binary_sha256 != args.binary_sha256:
            raise SystemExit("Headless executable SHA256 mismatch; no archive assembled")
        args.output_dir.mkdir(parents=True, exist_ok=True)
        output = args.output_dir / filename
        # Exclusive creation also refuses a concurrently created archive.
        write_archive(stage, output)
    print(f"Assembled {output}; SHA256 {sha256(output)}; binary SHA256 {observed_binary_sha256}")


if __name__ == "__main__":
    main()
