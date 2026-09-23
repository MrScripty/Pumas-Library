"""Resolve official binary Torch artifacts for an explicitly chosen build.

The interpreter running this script is the interpreter that will own the venv.
The pip report is kept as provenance; installation uses its exact URLs and hashes.
"""

import argparse
import json
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import urlparse

CORE = ("fastapi", "uvicorn", "psutil", "pillow", "safetensors")
BUILDS = (
    "cpu",
    "cu118",
    "cu121",
    "cu124",
    "cu126",
    "cu128",
    "cu130",
    "rocm6.1",
    "rocm6.2",
    "rocm6.3",
    "rocm6.4",
    "rocm7.0",
    "rocm7.1",
)


def official_torch_url(url: str, build: str) -> bool:
    parsed = urlparse(url)
    return (
        parsed.scheme == "https"
        and parsed.hostname in {"download.pytorch.org", "download-r2.pytorch.org"}
        and parsed.path.startswith(f"/whl/{build}/")
    )


def requirements_from_report(report: dict, version: str, build: str) -> tuple[list[str], dict]:
    entries = report.get("install", [])
    torch = next((item for item in entries if item["metadata"]["name"].lower() == "torch"), None)
    expected = f"{version}+{build}"
    if torch is None or torch["metadata"]["version"] != expected:
        raise ValueError(f"Official {build} artifact for Torch {version} was not resolved")
    if not official_torch_url(torch["download_info"]["url"], build):
        raise ValueError("Resolved Torch artifact is not from the selected official build index")
    lines = []
    artifacts = []
    for item in entries:
        name = item["metadata"]["name"]
        info = item["download_info"]
        url = info["url"]
        digest = info.get("archive_info", {}).get("hashes", {}).get("sha256")
        if not digest or not re.fullmatch(r"[0-9a-fA-F]{64}", digest):
            raise ValueError(f"No SHA-256 provenance for {name}")
        if not urlparse(url).scheme == "https" or not url.lower().split("?", 1)[0].endswith(".whl"):
            raise ValueError(f"No binary HTTPS artifact for {name}")
        lines.append(f"{name} @ {url} --hash=sha256:{digest}")
        artifacts.append(
            {"name": name, "version": item["metadata"]["version"], "url": url, "sha256": digest}
        )
    return lines, {
        "torch": expected,
        "build": build,
        "python": f"{sys.version_info.major}.{sys.version_info.minor}",
        "artifacts": artifacts,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--build", choices=BUILDS, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"\d+\.\d+\.\d+", args.version):
        parser.error("Only stable upstream Torch versions are supported")
    args.output.mkdir(parents=True, exist_ok=True)
    report_path = args.output / "pip-resolution.json"
    command = [
        sys.executable,
        "-I",
        "-m",
        "pip",
        "--isolated",
        "install",
        "--dry-run",
        "--report",
        str(report_path),
        "--only-binary=:all:",
        "--index-url",
        f"https://download.pytorch.org/whl/{args.build}",
        "--extra-index-url",
        "https://pypi.org/simple",
        f"torch=={args.version}+{args.build}",
        *CORE,
    ]
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    print(completed.stdout, end="", flush=True)
    print(completed.stderr, end="", file=sys.stderr, flush=True)
    if completed.returncode:
        if any(
            marker in completed.stderr.lower()
            for marker in (
                "name or service not known",
                "temporary failure in name resolution",
                "connection refused",
                "network is unreachable",
            )
        ):
            print(
                "Cannot reach official wheel indexes; upstream artifact availability is inconclusive. Retry when network access is available.",
                file=sys.stderr,
            )
            raise SystemExit(75)
        raise SystemExit(
            f"No compatible official {args.build} wheel for Torch {args.version} and Python {sys.version_info.major}.{sys.version_info.minor}; choose another installed interpreter or build. Interpreter provisioning is not supported."
        )
    report = json.loads(report_path.read_text())
    requirements, resolution = requirements_from_report(report, args.version, args.build)
    (args.output / "requirements.txt").write_text("\n".join(requirements) + "\n")
    (args.output / "resolution.json").write_text(json.dumps(resolution, indent=2) + "\n")


if __name__ == "__main__":
    main()
