"""Resolve a selected official Torch wheel and an optional image adapter.

Run this with the interpreter that will own the environment. The pip report and
hash-locked requirements are retained so installation need not resolve again.
"""

import argparse
from html.parser import HTMLParser
import json
import platform
import re
import subprocess
import sys
from pathlib import Path
from urllib.parse import unquote, urljoin, urlparse, urlunparse
from urllib.request import HTTPRedirectHandler, Request, build_opener

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
    "cu75",
    "cu80",
    "cu90",
    "cu91",
    "cu92",
    "cu100",
    "cu101",
    "cu102",
    "cu110",
    "cu111",
    "cu113",
    "cu115",
    "cu116",
    "cu117",
    "cu118",
    "cu121",
    "cu124",
    "cu126",
    "cu128",
    "cu129",
    "cu130",
    "cu132",
    "cu134",
    "rocm3.7",
    "rocm3.8",
    "rocm3.10",
    "rocm4.0.1",
    "rocm4.1",
    "rocm4.2",
    "rocm4.3.1",
    "rocm4.5.2",
    "rocm5.0",
    "rocm5.1.1",
    "rocm5.2",
    "rocm5.3",
    "rocm5.4.2",
    "rocm5.5",
    "rocm5.6",
    "rocm5.7",
    "rocm6.0",
    "rocm6.1",
    "rocm6.2",
    "rocm6.2.4",
    "rocm6.3",
    "rocm6.4",
    "rocm7.0",
    "rocm7.1",
    "rocm7.2",
    "rocm7.14",
)
ADAPTERS = ("none", "flux2", "nunchaku")
MAX_INDEX_REQUESTS = 5
MAX_INDEX_BYTES = 1_000_000
INDEX_TIMEOUT_SECONDS = 4
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


class WheelLinks(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links = []

    def handle_starttag(self, tag, attrs):
        if tag == "a":
            href = dict(attrs).get("href")
            if href:
                self.links.append(href)


class OfficialRedirects(HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        if not official_torch_url(newurl, request.full_url.split("/whl/", 1)[1].split("/", 1)[0]):
            raise ValueError("Wheel index redirected to an untrusted origin")
        return super().redirect_request(request, fp, code, msg, headers, newurl)


def discovery_builds(selected_build: str) -> list[str]:
    """Search selected build first, then nearby same-family builds and CPU."""
    if selected_build.startswith("cu"):
        family = [build for build in BUILDS if build.startswith("cu") and build != selected_build]
        family.sort(key=lambda build: abs(int(build[2:]) - int(selected_build[2:])))
    elif selected_build.startswith("rocm"):
        rocm_builds = [build for build in BUILDS if build.startswith("rocm")]
        selected_index = rocm_builds.index(selected_build)
        family = [build for build in rocm_builds if build != selected_build]
        family.sort(key=lambda build: abs(rocm_builds.index(build) - selected_index))
    else:
        family = [build for build in reversed(BUILDS) if build.startswith("cu")]
    order = [selected_build, *family[:3], "cpu"]
    return list(dict.fromkeys(order))[:MAX_INDEX_REQUESTS]


def interpreter_tags(interpreter: str) -> tuple[str, set[str]]:
    """Ask an installed interpreter for its own Linux x86_64 wheel tags."""
    code = (
        "import json,platform,sys;"
        "from pip._vendor.packaging.tags import sys_tags;"
        "print(json.dumps({'python':f'{sys.version_info.major}.{sys.version_info.minor}',"
        "'ok':sys.platform=='linux' and platform.machine()=='x86_64' "
        "and sys.implementation.name=='cpython','tags':[str(tag) for tag in sys_tags()]}))"
    )
    completed = subprocess.run(
        [interpreter, "-I", "-c", code], capture_output=True, text=True, timeout=4, check=False
    )
    if completed.returncode:
        raise ValueError(f"Installed interpreter {interpreter} cannot report wheel tags")
    details = json.loads(completed.stdout)
    if not details["ok"]:
        raise ValueError(f"Installed interpreter {interpreter} is not CPython on Linux x86_64")
    return details["python"], set(details["tags"])


def torch_index_links(build: str) -> list[str]:
    """Read one bounded official binary index page."""
    index_url = f"https://download.pytorch.org/whl/{build}/torch/"
    opener = build_opener(OfficialRedirects())
    with opener.open(
        Request(index_url, headers={"Accept": "text/html"}), timeout=INDEX_TIMEOUT_SECONDS
    ) as response:
        if not official_torch_url(response.geturl(), build):
            raise ValueError("Wheel index response came from an untrusted origin")
        body = response.read(MAX_INDEX_BYTES + 1)
    if len(body) > MAX_INDEX_BYTES:
        raise ValueError(f"Official {build} wheel index exceeded {MAX_INDEX_BYTES} bytes")
    parser = WheelLinks()
    parser.feed(body.decode("utf-8", errors="replace"))
    return parser.links


def wheel_match(href: str, version: str, build: str, tags: set[str]) -> dict | None:
    """Match only the exact official Torch binary and an interpreter-compatible tag."""
    url = urljoin(f"https://download.pytorch.org/whl/{build}/torch/", href)
    parsed = urlparse(url)
    if not official_torch_url(url, build) or parsed.query:
        return None
    filename = unquote(parsed.path.rsplit("/", 1)[-1])
    match = re.fullmatch(r"torch-([^-]+)-([^-]+)-([^-]+)-([^-]+)\.whl", filename)
    if not match or match.group(1) != f"{version}+{build}":
        return None
    if not any(
        f"{python}-{abi}-{system}" in tags
        for python in match.group(2).split(".")
        for abi in match.group(3).split(".")
        for system in match.group(4).split(".")
    ):
        return None
    digest = (
        parsed.fragment.removeprefix("sha256=") if parsed.fragment.startswith("sha256=") else None
    )
    if digest is not None and not re.fullmatch(r"[0-9a-fA-F]{64}", digest):
        return None
    clean_url = urlunparse(parsed._replace(fragment=""))
    return {"wheelUrl": clean_url, "sha256": digest}


def discover_alternatives(
    selected_tag: str,
    selected_build: str,
    selected_python: str,
    interpreters: list[str],
    index_loader=torch_index_links,
    tag_loader=interpreter_tags,
) -> dict:
    """Find at most three Torch wheels; dependencies and adapters remain unchecked."""
    version = selected_tag.removeprefix("v")
    if not re.fullmatch(r"\d+\.\d+\.\d+", version) or selected_build not in BUILDS:
        raise ValueError("Select a stable Torch tag and supported build")
    if not re.fullmatch(r"python3\.(10|11|12|13)", selected_python):
        raise ValueError("Select a supported Python version")
    issues = []
    installed = {}
    for interpreter in interpreters[:4]:
        try:
            python, tags = tag_loader(interpreter)
            installed[f"python{python}"] = tags
        except (OSError, ValueError, subprocess.TimeoutExpired, json.JSONDecodeError) as error:
            issues.append(str(error))
    checked_builds = []
    matches = []
    network_inconclusive = False
    for build in discovery_builds(selected_build):
        checked_builds.append(build)
        try:
            links = index_loader(build)
        except (OSError, ValueError, TimeoutError) as error:
            issues.append(f"{build} index unavailable: {error}")
            network_inconclusive = True
            continue
        python_order = sorted(
            installed,
            key=lambda python: (
                (python == selected_python)
                if build == selected_build
                else (python != selected_python)
            ),
        )
        for python in python_order:
            if build == selected_build and python == selected_python:
                continue
            for href in links:
                wheel = wheel_match(href, version, build, installed[python])
                if wheel is not None:
                    matches.append({"tag": selected_tag, "build": build, "python": python, **wheel})
                    break
            if len(matches) == 3:
                break
        if len(matches) == 3:
            break
    if not installed:
        issues.append("No installed CPython interpreter could report Linux x86_64 wheel tags")
        network_inconclusive = True
    issues.append(
        "Search is bounded to at most five official indexes; dependencies and adapters were not resolved"
    )
    return {
        "selectedTag": selected_tag,
        "selectedBuild": selected_build,
        "selectedPython": selected_python,
        "status": "matches" if matches else "inconclusive" if network_inconclusive else "none",
        "incomplete": True,
        "dependenciesNotChecked": True,
        "checkedBuilds": checked_builds,
        "matches": matches,
        "issues": issues,
    }


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
        1,
        "Dependency resolution failed; wheel availability is inconclusive. Review pip output.",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--build", choices=BUILDS, required=True)
    parser.add_argument("--adapter", choices=ADAPTERS, default="none")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--discover", action="store_true")
    parser.add_argument("--selected-python")
    parser.add_argument("--interpreter", action="append", default=[])
    args = parser.parse_args()
    if not re.fullmatch(r"\d+\.\d+\.\d+", args.version):
        parser.error("Only stable upstream Torch versions are supported")
    if args.discover:
        if not args.selected_python or not args.interpreter:
            parser.error("Discovery requires --selected-python and at least one --interpreter")
        try:
            result = discover_alternatives(
                f"v{args.version}", args.build, args.selected_python, args.interpreter
            )
        except ValueError as error:
            parser.exit(2, f"{error}\n")
        print(json.dumps(result), flush=True)
        return
    if args.output is None:
        parser.error("Resolution requires --output")
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
        parser.exit(3, f"Invalid wheel resolution: {error}\n")
    (args.output / "requirements.txt").write_text("\n".join(requirements) + "\n")
    (args.output / "resolution.json").write_text(json.dumps(resolution, indent=2) + "\n")


if __name__ == "__main__":
    main()
