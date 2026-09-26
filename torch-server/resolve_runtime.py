"""Resolve a selected official Torch wheel and an optional image adapter.

Run this with the interpreter that will own the environment. The pip report and
hash-locked requirements are retained so installation need not resolve again.
"""

import argparse
from concurrent.futures import ThreadPoolExecutor, wait
from html.parser import HTMLParser
import json
import platform
import re
import subprocess
import sys
import time
from pathlib import Path
from pip._vendor.packaging import tags as packaging_tags
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
RELEASE_ROOT_URL = "https://download.pytorch.org/whl/"
MAX_RELEASE_ROOT_BYTES = 200_000
MAX_RELEASE_CHANNELS = 64
MAX_RELEASE_INTERPRETERS = 4
MAX_RELEASE_CANDIDATES = 16
MAX_RELEASE_WORKERS = 8
MAX_RELEASE_SCAN_SECONDS = 30
RELEASE_CHANNEL = re.compile(r"cpu|cu\d+|rocm\d+(?:\.\d+)+")
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
METADATA_MARKERS = (
    "metadata-generation-failed",
    "invalid metadata",
    "failed to prepare metadata",
    "metadata preparation failed",
    "error getting requirements to build wheel",
)
MISSING_REQUIREMENT = re.compile(
    r"(?:no matching distribution found for|"
    r"could not find a version that satisfies the requirement)\s+([a-z0-9][^\s;()]*)"
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
        build = request.full_url.split("/whl/", 1)[1].split("/", 1)[0]
        if not official_torch_index_url(newurl, build):
            raise ValueError("Wheel index redirected to an untrusted origin")
        return super().redirect_request(request, fp, code, msg, headers, newurl)


class OfficialDirectoryRedirects(HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        if not official_directory_url(newurl):
            raise ValueError("Wheel directory redirected to an untrusted origin")
        return super().redirect_request(request, fp, code, msg, headers, newurl)


def official_directory_url(url: str) -> bool:
    parsed = urlparse(url)
    return (
        parsed.scheme == "https"
        and parsed.hostname in {"download.pytorch.org", "download-r2.pytorch.org"}
        and parsed.port in (None, 443)
        and parsed.username is None
        and parsed.password is None
        and parsed.path == "/whl/"
        and not parsed.query
        and not parsed.fragment
    )


def official_torch_index_url(url: str, build: str) -> bool:
    parsed = urlparse(url)
    return (
        official_torch_url(url, build)
        and parsed.path == f"/whl/{build}/torch/"
        and not parsed.query
        and not parsed.fragment
    )


def parse_release_channels(hrefs: list[str]) -> list[str]:
    """Keep exact canonical channel directories from the official wheel root."""
    channels = set()
    for href in hrefs:
        try:
            url = urljoin(RELEASE_ROOT_URL, href)
            parsed = urlparse(url)
            match = re.fullmatch(r"/whl/([^/]+)/", parsed.path)
            if (
                parsed.scheme == "https"
                and parsed.hostname in {"download.pytorch.org", "download-r2.pytorch.org"}
                and parsed.port in (None, 443)
                and parsed.username is None
                and parsed.password is None
                and not parsed.query
                and not parsed.fragment
                and match
                and RELEASE_CHANNEL.fullmatch(match.group(1))
            ):
                channels.add(match.group(1))
        except ValueError:
            continue

    def order(channel: str) -> tuple:
        if channel == "cpu":
            return (0, ())
        if channel.startswith("cu"):
            return (1, (int(channel[2:]),))
        return (2, tuple(int(part) for part in channel[4:].split(".")))

    return sorted(channels, key=order)


def torch_root_channels() -> list[str]:
    """Read the bounded official wheel directory before selecting channels."""
    opener = build_opener(OfficialDirectoryRedirects())
    with opener.open(
        Request(RELEASE_ROOT_URL, headers={"Accept": "text/html"}),
        timeout=INDEX_TIMEOUT_SECONDS,
    ) as response:
        if not official_directory_url(response.geturl()):
            raise ValueError("Wheel directory response came from an untrusted origin")
        body = response.read(MAX_RELEASE_ROOT_BYTES + 1)
    if len(body) > MAX_RELEASE_ROOT_BYTES:
        raise ValueError(f"Official wheel directory exceeded {MAX_RELEASE_ROOT_BYTES} bytes")
    parser = WheelLinks()
    parser.feed(body.decode("utf-8", errors="replace"))
    return parse_release_channels(parser.links)


def discovery_builds(selected_build: str) -> list[str]:
    """Search selected build first, then nearby same-family builds and CPU."""
    if selected_build.startswith("cu"):
        family = [build for build in BUILDS if build.startswith("cu") and build != selected_build]
        family.sort(key=lambda build: abs(int(build[2:]) - int(selected_build[2:])))
    elif selected_build.startswith("rocm"):
        rocm_builds = [build for build in BUILDS if build.startswith("rocm")]
        if selected_build not in rocm_builds:
            rocm_builds.append(selected_build)
            rocm_builds.sort(key=lambda build: tuple(int(part) for part in build[4:].split(".")))
        selected_index = rocm_builds.index(selected_build)
        family = [build for build in rocm_builds if build != selected_build]
        family.sort(key=lambda build: abs(rocm_builds.index(build) - selected_index))
    else:
        family = [build for build in reversed(BUILDS) if build.startswith("cu")]
    order = [selected_build, *family[:3], "cpu"]
    return list(dict.fromkeys(order))[:MAX_INDEX_REQUESTS]


def interpreter_tags(interpreter: str) -> tuple[str, set[str]]:
    """Ask an installed native interpreter for its own binary wheel tags."""
    code = (
        "import json,platform,sys;"
        "from pip._vendor.packaging.tags import sys_tags;"
        "print(json.dumps({'python':f'{sys.version_info.major}.{sys.version_info.minor}',"
        "'platform':sys.platform,'machine':platform.machine(),"
        "'implementation':sys.implementation.name,'tags':[str(tag) for tag in sys_tags()]}))"
    )
    completed = subprocess.run(
        [interpreter, "-I", "-c", code], capture_output=True, text=True, timeout=4, check=False
    )
    if completed.returncode:
        raise ValueError(f"Installed interpreter {interpreter} cannot report wheel tags")
    details = json.loads(completed.stdout)
    target = native_target(
        details["platform"], details["machine"], details["python"], details["implementation"]
    )
    suffix = native_platform_pattern(target)
    tags = {tag for tag in details["tags"] if suffix.fullmatch(tag.rsplit("-", 1)[-1])}
    return details["python"], tags


def native_platform_pattern(target: str) -> re.Pattern:
    return {
        "linux": re.compile(r"(?:manylinux[^-]*|musllinux[^-]*|linux)_x86_64"),
        "windows": re.compile(r"win_amd64"),
        "macos": re.compile(r"macosx_\d+_\d+_(?:arm64|universal2)"),
    }[target]


def bootstrap_platform_tags(interpreter: str) -> tuple[str, list[str]]:
    """Read native host platforms from one installed bootstrap interpreter."""
    code = (
        "import json,platform,sys;"
        "from pip._vendor.packaging.tags import platform_tags;"
        "print(json.dumps({'python':f'{sys.version_info.major}.{sys.version_info.minor}',"
        "'platform':sys.platform,'machine':platform.machine(),"
        "'implementation':sys.implementation.name,'platforms':list(platform_tags())}))"
    )
    completed = subprocess.run(
        [interpreter, "-I", "-c", code], capture_output=True, text=True, timeout=4, check=False
    )
    if completed.returncode:
        raise ValueError("Bootstrap interpreter cannot report native platform tags")
    details = json.loads(completed.stdout)
    target = native_target(
        details["platform"], details["machine"], details["python"], details["implementation"]
    )
    pattern = native_platform_pattern(target)
    platforms = list(dict.fromkeys(tag for tag in details["platforms"] if pattern.fullmatch(tag)))
    if not platforms:
        raise ValueError("Bootstrap interpreter reported no native platform tags")
    return target, platforms


def candidate_cpython_tags(python: str, target: str, platforms: list[str]) -> set[str]:
    """Synthesize standard CPython tags on the observed native host platforms."""
    version = candidate_python_version(python)
    if target not in {"linux", "windows", "macos"}:
        raise ValueError("Python candidate target must be native Linux, Windows, or macOS")
    if not platforms or any(not native_platform_pattern(target).fullmatch(p) for p in platforms):
        raise ValueError("Python candidate platforms must match the native host")
    generated = (
        *packaging_tags.cpython_tags(python_version=version, platforms=platforms),
        *packaging_tags.compatible_tags(
            python_version=version, interpreter=f"cp{version[0]}{version[1]}", platforms=platforms
        ),
    )
    return {str(tag) for tag in generated if tag.platform in platforms}


def candidate_python_version(python: str) -> tuple[int, int]:
    """Accept stable CPython 3.10+ minors without guessing an upper bound."""
    match = re.fullmatch(r"3\.([1-9]\d*)", python)
    if not match or int(match.group(1)) < 10:
        raise ValueError("Python candidates must be stable CPython 3.10 or newer minors")
    return 3, int(match.group(1))


def native_target(system: str, machine: str, python: str, implementation: str) -> str:
    """Name a supported native target; reject translated or foreign architectures."""
    if implementation != "cpython":
        raise ValueError("A native CPython interpreter is required")
    if not re.fullmatch(r"3\.\d+", python):
        raise ValueError("A stable CPython 3 interpreter is required")
    if system == "linux" and machine == "x86_64":
        return "linux"
    if system == "win32" and machine.lower() in {"amd64", "x86_64"}:
        return "windows"
    if system == "darwin" and machine == "arm64":
        return "macos"
    raise ValueError("Unsupported or translated native Torch target")


def torch_index_links(build: str) -> list[str]:
    """Read one bounded official binary index page."""
    index_url = f"https://download.pytorch.org/whl/{build}/torch/"
    opener = build_opener(OfficialRedirects())
    with opener.open(
        Request(index_url, headers={"Accept": "text/html"}), timeout=INDEX_TIMEOUT_SECONDS
    ) as response:
        if not official_torch_index_url(response.geturl(), build):
            raise ValueError("Wheel index response came from an untrusted origin")
        body = response.read(MAX_INDEX_BYTES + 1)
    if len(body) > MAX_INDEX_BYTES:
        raise ValueError(f"Official {build} wheel index exceeded {MAX_INDEX_BYTES} bytes")
    parser = WheelLinks()
    parser.feed(body.decode("utf-8", errors="replace"))
    return parser.links


def wheel_match(href: str, version: str, build: str, tags: set[str]) -> dict | None:
    """Match only the exact official Torch binary and an interpreter-compatible tag."""
    try:
        url = urljoin(f"https://download.pytorch.org/whl/{build}/torch/", href)
        parsed = urlparse(url)
        if not official_torch_url(url, build) or parsed.query:
            return None
    except ValueError:
        return None
    filename = unquote(parsed.path.rsplit("/", 1)[-1])
    match = re.fullmatch(r"torch-([^-]+)-([^-]+)-([^-]+)-([^-]+)\.whl", filename)
    if not match:
        return None
    distribution_version = match.group(1)
    mac_cpu = build == "cpu" and any(
        re.fullmatch(r"macosx_\d+_\d+_(?:arm64|universal2)", platform_tag)
        for platform_tag in match.group(4).split(".")
    )
    if distribution_version != f"{version}+{build}" and not (
        mac_cpu and distribution_version == version
    ):
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
    return {"wheelUrl": clean_url, "sha256": digest, "torch": distribution_version}


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
    if not re.fullmatch(r"\d+\.\d+\.\d+", version) or not RELEASE_CHANNEL.fullmatch(selected_build):
        raise ValueError("Select a stable Torch tag and supported build")
    if not re.fullmatch(r"python3\.\d+", selected_python):
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
        issues.append("No installed native CPython interpreter could report compatible wheel tags")
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


def discover_release_options(
    version: str,
    interpreters: list[str],
    root_loader=torch_root_channels,
    index_loader=torch_index_links,
    tag_loader=interpreter_tags,
    python_candidates: list[str] | None = None,
    platform_loader=bootstrap_platform_tags,
) -> dict:
    """Find every exact wheel in a bounded scan of official stable channels."""
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError("Select a stable upstream Torch version")
    if not interpreters:
        raise ValueError("Release options require at least one installed interpreter")
    if python_candidates is not None:
        if len(interpreters) != 1 or not python_candidates:
            raise ValueError("Python candidates require one native bootstrap interpreter")
        for python in python_candidates:
            candidate_python_version(python)
    deadline = time.monotonic() + MAX_RELEASE_SCAN_SECONDS
    issues = []
    installed = {}
    if python_candidates is not None:
        candidates = list(dict.fromkeys(python_candidates))
        if len(candidates) > MAX_RELEASE_CANDIDATES:
            issues.append(f"Only the first {MAX_RELEASE_CANDIDATES} Python candidates were checked")
        try:
            target, platforms = platform_loader(interpreters[0])
            for python in candidates[:MAX_RELEASE_CANDIDATES]:
                installed[f"python{python}"] = candidate_cpython_tags(python, target, platforms)
        except (OSError, ValueError, subprocess.TimeoutExpired, json.JSONDecodeError) as error:
            issues.append(
                f"Bootstrap interpreter could not report native tags: {type(error).__name__}"
            )
    else:
        if len(interpreters) > MAX_RELEASE_INTERPRETERS:
            issues.append(f"Only the first {MAX_RELEASE_INTERPRETERS} interpreters were checked")
        for number, interpreter in enumerate(interpreters[:MAX_RELEASE_INTERPRETERS], start=1):
            try:
                python, tags = tag_loader(interpreter)
                if not re.fullmatch(r"\d+\.\d+", python):
                    raise ValueError("Invalid interpreter version")
                installed.setdefault(f"python{python}", set()).update(tags)
            except (OSError, ValueError, subprocess.TimeoutExpired, json.JSONDecodeError) as error:
                issues.append(
                    f"Interpreter {number} could not report compatible tags: {type(error).__name__}"
                )

    checked_channels = []
    combinations = []

    def result() -> dict:
        complete = not issues
        return {
            "tag": f"v{version}",
            "status": "inconclusive" if not complete else "matches" if combinations else "none",
            "completeScan": complete,
            "checkedChannels": checked_channels,
            "combinations": combinations,
            "issues": issues,
        }

    if not installed:
        issues.append("No interpreter could report compatible native wheel tags")
        return result()
    try:
        raw_channels = root_loader()
    except (OSError, ValueError, TimeoutError) as error:
        issues.append(f"Official wheel directory could not be verified: {type(error).__name__}")
        return result()
    channels = list(dict.fromkeys(raw_channels))
    if any(not RELEASE_CHANNEL.fullmatch(channel) for channel in channels):
        issues.append("Official wheel directory included an invalid channel name")
        channels = [channel for channel in channels if RELEASE_CHANNEL.fullmatch(channel)]
    if not channels:
        issues.append("Official wheel directory contained no canonical CPU, CUDA, or ROCm channels")
        return result()
    if len(channels) > MAX_RELEASE_CHANNELS:
        issues.append(
            f"Official wheel directory exceeded the {MAX_RELEASE_CHANNELS}-channel scan limit"
        )
        channels = channels[:MAX_RELEASE_CHANNELS]
    if time.monotonic() >= deadline:
        issues.append("Release scan deadline elapsed before channel indexes could be checked")
        return result()

    checked_channels.extend(channels)
    executor = ThreadPoolExecutor(max_workers=MAX_RELEASE_WORKERS)
    try:
        futures = {channel: executor.submit(index_loader, channel) for channel in channels}
        done, _ = wait(futures.values(), timeout=max(0, deadline - time.monotonic()))
        seen = set()
        for channel in channels:
            if time.monotonic() >= deadline:
                issues.append(
                    f"{channel} Torch index was not fully checked before the scan deadline"
                )
                continue
            future = futures[channel]
            if future not in done:
                issues.append(f"{channel} Torch index was not verified before the scan deadline")
                continue
            try:
                links = future.result()
            except (OSError, ValueError, TimeoutError) as error:
                issues.append(
                    f"{channel} Torch index could not be verified: {type(error).__name__}"
                )
                continue
            for python, tags in installed.items():
                for href in links:
                    if time.monotonic() >= deadline:
                        issues.append(
                            f"{channel} Torch index was not fully checked before the scan deadline"
                        )
                        break
                    wheel = wheel_match(href, version, channel, tags)
                    if wheel is None:
                        continue
                    identity = (channel, python, wheel["wheelUrl"])
                    if identity in seen:
                        continue
                    seen.add(identity)
                    combination = {
                        "build": channel,
                        "python": python,
                        "wheelUrl": wheel["wheelUrl"],
                    }
                    if wheel["sha256"] is not None:
                        combination["sha256"] = wheel["sha256"]
                    combinations.append(combination)
                if time.monotonic() >= deadline:
                    break
    finally:
        executor.shutdown(wait=False, cancel_futures=True)
    python_order = (
        {f"python{python}": index for index, python in enumerate(candidates)}
        if python_candidates is not None
        else {}
    )
    combinations.sort(
        key=lambda item: (
            checked_channels.index(item["build"]),
            python_order.get(item["python"], len(python_order)),
            item["python"],
            item["wheelUrl"],
        )
    )
    return result()


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
    target = native_target(
        sys.platform,
        platform.machine(),
        f"{sys.version_info.major}.{sys.version_info.minor}",
        sys.implementation.name,
    )
    expected_versions = {f"{version}+{build}"}
    if target == "macos" and build == "cpu":
        expected_versions.add(version)
    if len(torch_entries) != 1 or torch_entries[0]["metadata"]["version"] not in expected_versions:
        raise ValueError(f"Official {build} artifact for Torch {version} was not resolved")
    resolved_torch = torch_entries[0]["metadata"]["version"]
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
        if name == "torchvision":
            vision_version = item["metadata"]["version"]
            local_build = vision_version.endswith(f"+{build}")
            plain_mac_cpu = (
                target == "macos"
                and build == "cpu"
                and re.fullmatch(r"\d+\.\d+\.\d+", vision_version)
            )
            if not (local_build or plain_mac_cpu):
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
        "release": version,
        "torch": resolved_torch,
        "build": build,
        "adapter": adapter,
        "python": f"{sys.version_info.major}.{sys.version_info.minor}",
        "interpreter": sys.executable,
        "implementation": sys.implementation.name,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "artifacts": artifacts,
    }


def resolution_failure(
    stderr: str,
    version: str,
    build: str,
    adapter: str = "none",
    torch_requirement: str | None = None,
) -> tuple[int, str]:
    lowered = stderr.lower()
    if any(marker in lowered for marker in NETWORK_MARKERS):
        return (
            75,
            "Wheel index access failed; availability is inconclusive. Retry with network access.",
        )
    if any(marker in lowered for marker in METADATA_MARKERS):
        return (
            1,
            "Package metadata prevented a conclusive resolution. Review pip output or try another Python or build.",
        )
    missing = MISSING_REQUIREMENT.findall(lowered)
    if f"torch=={torch_requirement or f'{version}+{build}'}" in missing:
        return (
            4,
            f"No official Torch {version}+{build} wheel matches this interpreter and platform. Choose another build or installed Python.",
        )
    if missing:
        requested = set(CORE)
        if adapter != "none":
            requested.update(requirement.partition("==")[0] for requirement in IMAGE)
        if adapter == "nunchaku":
            requested.add("nunchaku")
        names = [re.match(r"[a-z0-9][a-z0-9._-]*", requirement).group() for requirement in missing]
        named = next((name for name in names if name in requested), None)
        return (
            2,
            f"No compatible binary wheel was found for requested package {named}. Choose another Python, build, or adapter."
            if named
            else "A required dependency has no compatible binary wheel. Choose another Python, build, or adapter.",
        )
    if "resolutionimpossible" in lowered or "conflicting dependencies" in lowered:
        return (
            2,
            "The selected package requirements conflict. Choose another Python, build, or adapter.",
        )
    return (
        1,
        "Dependency resolution failed; wheel availability is inconclusive. Review pip output.",
    )


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--version", required=True)
    parser.add_argument("--build")
    parser.add_argument("--adapter", choices=ADAPTERS, default="none")
    parser.add_argument("--output", type=Path)
    parser.add_argument("--discover", action="store_true")
    parser.add_argument("--release-options", action="store_true")
    parser.add_argument("--selected-python")
    parser.add_argument("--interpreter", action="append", default=[])
    parser.add_argument("--python-candidate", action="append")
    args = parser.parse_args()
    if not re.fullmatch(r"\d+\.\d+\.\d+", args.version):
        parser.error("Only stable upstream Torch versions are supported")
    if args.release_options:
        if (
            args.discover
            or args.build
            or args.output
            or args.selected_python
            or args.adapter != "none"
        ):
            parser.error("Release options accepts only --version and one or more --interpreter")
        if not args.interpreter:
            parser.error("Release options requires at least one --interpreter")
        if args.python_candidate is not None and len(args.interpreter) != 1:
            parser.error("Python candidates require exactly one bootstrap --interpreter")
        try:
            if args.python_candidate is None:
                result = discover_release_options(args.version, args.interpreter)
            else:
                result = discover_release_options(
                    args.version, args.interpreter, python_candidates=args.python_candidate
                )
        except ValueError as error:
            parser.exit(2, f"{error}\n")
        print(json.dumps(result), flush=True)
        return
    if args.build is None:
        parser.error("Resolution and discovery require --build")
    if args.python_candidate is not None:
        parser.error("--python-candidate is only valid with --release-options")
    if not RELEASE_CHANNEL.fullmatch(args.build):
        parser.error("Select a canonical CPU, CUDA, or ROCm build channel")
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
        target = native_target(
            sys.platform,
            platform.machine(),
            f"{sys.version_info.major}.{sys.version_info.minor}",
            sys.implementation.name,
        )
        extras = adapter_requirements(args.adapter, args.version, args.build)
    except ValueError as error:
        parser.exit(2, f"{error}\n")
    args.output.mkdir(parents=True, exist_ok=True)
    report_path = args.output / "pip-resolution.json"
    torch_requirement = (
        args.version
        if target == "macos" and args.build == "cpu"
        else f"{args.version}+{args.build}"
    )
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
        f"torch=={torch_requirement}",
        *CORE,
        *extras,
    ]
    completed = subprocess.run(command, check=False, capture_output=True, text=True)
    print(completed.stdout, end="", flush=True)
    print(completed.stderr, end="", file=sys.stderr, flush=True)
    if completed.returncode:
        code, message = resolution_failure(
            completed.stderr, args.version, args.build, args.adapter, torch_requirement
        )
        parser.exit(code, f"{message}\n")
    try:
        report = json.loads(report_path.read_text(encoding="utf-8"))
        requirements, resolution = requirements_from_report(
            report, args.version, args.build, args.adapter
        )
    except (KeyError, TypeError, ValueError, OSError) as error:
        parser.exit(3, f"Invalid wheel resolution: {error}\n")
    (args.output / "requirements.txt").write_text(
        "\n".join(requirements) + "\n", encoding="utf-8", newline="\n"
    )
    (args.output / "resolution.json").write_text(
        json.dumps(resolution, indent=2) + "\n", encoding="utf-8", newline="\n"
    )


if __name__ == "__main__":
    main()
