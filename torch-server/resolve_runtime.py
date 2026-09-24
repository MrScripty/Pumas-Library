"""Resolve a selected official Torch wheel and an optional image adapter.

Run this with the interpreter that will own the environment. The pip report and
hash-locked requirements are retained so installation need not resolve again.
"""

import argparse
import json
import platform
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import urlparse

CORE = ("fastapi", "uvicorn", "psutil", "pillow", "safetensors")
IMAGE = (
    "torchvision",
    "diffusers==0.37.0",
    "transformers==4.57.6",
    "accelerate==1.12.0",
    "peft==0.18.1",
    "sentencepiece==0.2.1",
    "protobuf==6.33.4",
)
NUNCHAKU_URL = (
    "https://github.com/nunchux-ai/nunchaku/releases/download/v1.2.0/"
    "nunchaku-1.2.0%2Btorch2.9-cp312-cp312-linux_x86_64.whl"
)
NUNCHAKU_SHA256 = "6196fbea888d6719fd7aff7ec58f1618ff37c35d4542f2c595aadd2a242e500f"
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
ADAPTERS = ("none", "flux2", "nunchaku")
NETWORK_MARKERS = (
    "name or service not known",
    "temporary failure in name resolution",
    "connection refused",
    "network is unreachable",
    "connection timed out",
    "read timed out",
    "connecttimeout",
    "readtimeout",
    "failed to establish a new connection",
    "max retries exceeded",
    "ssl certificate",
    "certificate verify failed",
    "proxyerror",
    "remote disconnected",
    "connection reset",
    "http 429",
    "http 500",
    "http 502",
    "http 503",
    "http 504",
    "too many requests",
)


def official_torch_url(url: str, build: str) -> bool:
    parsed = urlparse(url)
    return (
        parsed.scheme == "https"
        and parsed.hostname in {"download.pytorch.org", "download-r2.pytorch.org"}
        and parsed.port in (None, 443)
        and parsed.username is None
        and parsed.password is None
        and parsed.path.startswith(f"/whl/{build}/")
    )


def trusted_wheel_url(url: str, build: str, name: str, adapter: str) -> bool:
    parsed = urlparse(url)
    if (
        parsed.scheme != "https"
        or any(char.isspace() for char in url)
        or parsed.username is not None
        or parsed.password is not None
        or parsed.port not in (None, 443)
        or parsed.query
        or parsed.fragment
        or not parsed.path.lower().endswith(".whl")
    ):
        return False
    if name in {"torch", "torchvision"}:
        return official_torch_url(url, build)
    if parsed.hostname == "files.pythonhosted.org" and parsed.path.startswith("/packages/"):
        return True
    if parsed.hostname in {"download.pytorch.org", "download-r2.pytorch.org"}:
        return parsed.path.startswith("/whl/")
    return adapter == "nunchaku" and name == "nunchaku" and url == NUNCHAKU_URL


def adapter_requirements(adapter: str, version: str, build: str) -> tuple[str, ...]:
    if adapter == "none":
        return ()
    if adapter == "flux2":
        return IMAGE
    if adapter == "nunchaku":
        if not (
            version == "2.9.1"
            and build == "cu130"
            and sys.implementation.name == "cpython"
            and sys.version_info[:2] == (3, 12)
            and sys.platform == "linux"
            and platform.machine() == "x86_64"
        ):
            raise ValueError(
                "The qualified Nunchaku wheel requires Torch 2.9.1+cu130, "
                "CPython 3.12, and Linux x86_64"
            )
        return (*IMAGE, f"nunchaku @ {NUNCHAKU_URL}#sha256={NUNCHAKU_SHA256}")
    raise ValueError(f"Unknown adapter: {adapter}")


def requirements_from_report(
    report: dict, version: str, build: str, adapter: str = "none"
) -> tuple[list[str], dict]:
    entries = report.get("install", [])
    if not isinstance(entries, list) or not entries:
        raise ValueError("The pip report contains no resolved artifacts")
    torch_entries = [
        item for item in entries if item["metadata"]["name"].lower().replace("_", "-") == "torch"
    ]
    expected = f"{version}+{build}"
    if len(torch_entries) != 1 or torch_entries[0]["metadata"]["version"] != expected:
        raise ValueError(f"Official {build} artifact for Torch {version} was not resolved")
    lines = []
    artifacts = []
    seen = set()
    for item in entries:
        name = item["metadata"]["name"].lower().replace("_", "-")
        if not re.fullmatch(r"[a-z0-9][a-z0-9.-]*", name) or name in seen:
            raise ValueError(f"Duplicate or invalid distribution name: {name}")
        seen.add(name)
        info = item["download_info"]
        url = info["url"]
        digest = info.get("archive_info", {}).get("hashes", {}).get("sha256")
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-fA-F]{64}", digest):
            raise ValueError(f"No SHA-256 provenance for {name}")
        if not trusted_wheel_url(url, build, name, adapter):
            raise ValueError(f"Untrusted or non-binary artifact for {name}: {url}")
        if name == "torchvision" and not item["metadata"]["version"].endswith(f"+{build}"):
            raise ValueError("Torchvision build differs from the selected Torch build")
        if name == "nunchaku" and (adapter != "nunchaku" or digest.lower() != NUNCHAKU_SHA256):
            raise ValueError("Nunchaku artifact differs from the qualified wheel")
        lines.append(f"{name} @ {url} --hash=sha256:{digest}")
        artifacts.append(
            {"name": name, "version": item["metadata"]["version"], "url": url, "sha256": digest}
        )
    required = {"torch", *CORE}
    if adapter != "none":
        required.update(
            {
                "torchvision",
                "diffusers",
                "transformers",
                "accelerate",
                "peft",
                "sentencepiece",
                "protobuf",
            }
        )
    if adapter == "nunchaku":
        required.add("nunchaku")
    missing = required - seen
    if missing:
        raise ValueError(f"Resolution omitted requested packages: {', '.join(sorted(missing))}")
    return lines, {
        "torch": expected,
        "build": build,
        "adapter": adapter,
        "python": f"{sys.version_info.major}.{sys.version_info.minor}",
        "interpreter": sys.executable,
        "implementation": sys.implementation.name,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "artifacts": artifacts,
    }


def resolution_failure(stderr: str, version: str, build: str) -> tuple[int, str]:
    lowered = stderr.lower()
    if any(marker in lowered for marker in NETWORK_MARKERS):
        return (
            75,
            "Wheel index access failed; availability is inconclusive. Retry with network access.",
        )
    if (
        f"no matching distribution found for torch=={version}+{build}" in lowered
        or f"could not find a version that satisfies the requirement torch=={version}+{build}"
        in lowered
    ):
        return (
            2,
            f"No official {build} Torch {version} wheel matches this interpreter and platform.",
        )
    if (
        "no matching distribution found for " in lowered
        or "could not find a version that satisfies the requirement " in lowered
    ):
        return (
            2,
            "A requested package has no compatible wheel for this interpreter, platform, and build.",
        )
    if "resolutionimpossible" in lowered or "conflicting dependencies" in lowered:
        return 2, "The selected package requirements are incompatible."
    return (
        75,
        "Dependency resolution failed; wheel availability is inconclusive. Review pip output.",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--build", choices=BUILDS, required=True)
    parser.add_argument("--adapter", choices=ADAPTERS, default="none")
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not re.fullmatch(r"\d+\.\d+\.\d+", args.version):
        parser.error("Only stable upstream Torch versions are supported")
    try:
        extras = adapter_requirements(args.adapter, args.version, args.build)
    except ValueError as error:
        parser.exit(2, f"{error}\n")
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
        "--ignore-installed",
        "--only-binary=:all:",
        "--index-url",
        f"https://download.pytorch.org/whl/{args.build}",
        "--extra-index-url",
        "https://pypi.org/simple",
        f"torch=={args.version}+{args.build}",
        *CORE,
        *extras,
    ]
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    print(completed.stdout, end="", flush=True)
    print(completed.stderr, end="", file=sys.stderr, flush=True)
    if completed.returncode:
        code, message = resolution_failure(completed.stderr, args.version, args.build)
        parser.exit(code, f"{message}\n")
    try:
        report = json.loads(report_path.read_text())
        requirements, resolution = requirements_from_report(
            report, args.version, args.build, args.adapter
        )
    except (KeyError, TypeError, ValueError, OSError) as error:
        parser.exit(2, f"Invalid wheel resolution: {error}\n")
    (args.output / "requirements.txt").write_text("\n".join(requirements) + "\n")
    (args.output / "resolution.json").write_text(json.dumps(resolution, indent=2) + "\n")


if __name__ == "__main__":
    main()
