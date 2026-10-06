"""Materialize a finite existing recipe; never ask pip to resolve dependencies.

Only index metadata is read here. Payload acquisition remains with the Rust
owner. Actual acquired wheel metadata is validated by the local installer.
"""

import argparse
import hashlib
import importlib
import importlib.util
from html.parser import HTMLParser
import json
import platform
from pathlib import Path
import re
import sys
import time
from urllib.parse import unquote, urljoin, urlparse, urlunparse
from urllib.error import HTTPError
from urllib.request import HTTPRedirectHandler, Request, build_opener

# Owned embedded standalone tooling, not pip's private vendored namespace.
_tooling = Path(__file__).with_name("packaging-tooling.zip")
if not _tooling.is_file():
    _tooling = Path(__file__).parent / "tooling" / "packaging.zip"
if _tooling.is_file():
    sys.path.insert(0, str(_tooling))
Requirement = importlib.import_module("packaging.requirements").Requirement
SpecifierSet = importlib.import_module("packaging.specifiers").SpecifierSet
sys_tags = importlib.import_module("packaging.tags").sys_tags
canonicalize_name = importlib.import_module("packaging.utils").canonicalize_name
parse_wheel_filename = importlib.import_module("packaging.utils").parse_wheel_filename
Version = importlib.import_module("packaging.version").Version

MAX_LOCK_BYTES = 128 * 1024
MAX_PACKAGES = 128
MAX_INDEX_BYTES = 1024 * 1024
MAX_INDEX_REQUESTS = MAX_PACKAGES * 2
MAX_CATALOG_SECONDS = 180
INDEX_TIMEOUT_SECONDS = 5
INDEXES = ("https://pypi.org/simple/", "https://download.pytorch.org/whl/cu130/")


def wheel_source(url, name, direct=None):
    parsed = urlparse(url)
    if (
        len(url.encode()) > 2048
        or parsed.scheme != "https"
        or parsed.username is not None
        or parsed.password is not None
        or parsed.port not in (None, 443)
        or parsed.query
        or parsed.fragment
        or any(c.isspace() or ord(c) < 32 or ord(c) == 127 for c in url)
        or not parsed.path.endswith(".whl")
    ):
        return False
    if direct is not None:
        return url == direct
    if name in {"torch", "torchvision", "nunchaku"}:
        return False  # These recipe roots are supplied explicitly, never substituted.
    return (
        parsed.hostname == "files.pythonhosted.org" and parsed.path.startswith("/packages/")
    ) or (
        parsed.hostname in {"download.pytorch.org", "download-r2.pytorch.org"}
        and parsed.path.startswith("/whl/")
    )


def parse_lock(lock):
    if len(lock.encode()) > MAX_LOCK_BYTES:
        raise ValueError("Qualified recipe lock is oversized")
    entries = []
    logical = re.sub(r"\\\n[ \t]*", " ", lock)
    for line in logical.splitlines():
        line = line.strip()
        if not line or line.startswith("#"):
            continue
        if line.startswith("--"):
            if line not in (
                "--index-url https://pypi.org/simple",
                "--extra-index-url https://download.pytorch.org/whl/cu130",
            ):
                raise ValueError("Qualified recipe contains unsupported pip options")
            continue
        parts = line.split(" --hash=")
        requirement = Requirement(parts[0].strip())
        if requirement.extras or requirement.marker or not parts[1:]:
            raise ValueError("Qualified recipe entries must be finite and hash-pinned")
        hashes = []
        for value in parts[1:]:
            if not re.fullmatch(r"sha256:[0-9a-fA-F]{64}", value.strip()):
                raise ValueError("Qualified recipe contains an unsupported hash")
            hashes.append(value.strip().split(":", 1)[1].lower())
        name = canonicalize_name(requirement.name)
        source = None
        if requirement.url:
            parsed = urlparse(requirement.url)
            if parsed.fragment and (
                not re.fullmatch(r"sha256=[0-9a-fA-F]{64}", parsed.fragment)
                or parsed.fragment[7:].lower() not in hashes
            ):
                raise ValueError("Qualified direct wheel fragment differs from its hash")
            source = urlunparse(parsed._replace(fragment=""))
            wheel_name, version, _, _ = parse_wheel_filename(
                unquote(parsed.path.rsplit("/", 1)[-1])
            )
            if wheel_name != name or name not in {"torch", "torchvision", "nunchaku"}:
                raise ValueError("Qualified recipe direct root is unsupported")
            if not (
                parsed.hostname in {"download.pytorch.org", "download-r2.pytorch.org"}
                and parsed.path.startswith("/whl/cu130/")
                and name in {"torch", "torchvision"}
            ) and not (
                parsed.hostname == "github.com"
                and name == "nunchaku"
                and parsed.path.startswith("/nunchux-ai/nunchaku/releases/download/v1.2.0/")
            ):
                raise ValueError("Qualified recipe direct root is untrusted")
            if not wheel_source(source, name, source) or len(set(hashes)) != 1:
                raise ValueError("Qualified direct wheel must have one exact trusted digest")
        else:
            pins = list(requirement.specifier)
            if len(pins) != 1 or pins[0].operator != "==" or "*" in pins[0].version:
                raise ValueError("Qualified recipe requires exact named versions")
            version = Version(pins[0].version)
        if name in {entry["name"] for entry in entries}:
            raise ValueError("Qualified recipe repeats a distribution")
        entries.append({"name": name, "version": str(version), "hashes": hashes, "url": source})
    if not entries or len(entries) > MAX_PACKAGES:
        raise ValueError("Qualified recipe package count is invalid")
    return entries


def validate_roots(entries, roots):
    expected = {e["name"]: e for e in entries if e["url"] is not None}
    if len(roots) != len(expected):
        raise ValueError("Selected direct wheels differ from the qualified recipe")
    seen = set()
    for root in roots:
        name = canonicalize_name(root["name"])
        entry = expected.get(name)
        parsed = urlparse(root["url"])
        if (
            entry is None
            or name in seen
            or root["version"] != entry["version"]
            or root["sha256"].lower() != entry["hashes"][0]
            or (parsed.fragment and parsed.fragment != "sha256=" + entry["hashes"][0])
            or urlunparse(parsed._replace(fragment="")) != entry["url"]
        ):
            raise ValueError("Selected direct wheel identity differs from the qualified recipe")
        seen.add(name)


class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.links = []

    def handle_starttag(self, tag, attrs):
        values = dict(attrs)
        if tag == "a" and "href" in values:
            self.links.append(
                {
                    "url": values["href"],
                    "requires-python": values.get("data-requires-python"),
                    "yanked": "data-yanked" in values,
                }
            )


def index_url(url):
    parsed = urlparse(url)
    return (
        parsed.scheme == "https"
        and parsed.hostname in {"pypi.org", "download.pytorch.org"}
        and parsed.port in (None, 443)
        and parsed.username is None
        and parsed.password is None
        and not parsed.query
        and not parsed.fragment
        and any(url.startswith(base) for base in INDEXES)
    )


class IndexRedirects(HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        if not index_url(newurl):
            raise ValueError("Qualified index redirect is unsupported")
        return super().redirect_request(request, fp, code, msg, headers, newurl)


def fetch_index(url):
    if not index_url(url):
        raise ValueError("Qualified index URL is unsupported")
    opener = build_opener(IndexRedirects())
    request = Request(
        url,
        headers={
            "Accept": "application/vnd.pypi.simple.v1+json, text/html",
            "User-Agent": "pumas-library",
        },
    )
    try:
        response = opener.open(request, timeout=INDEX_TIMEOUT_SECONDS)
    except HTTPError as error:
        if error.code == 404:
            return []
        raise
    with response:
        body = response.read(MAX_INDEX_BYTES + 1)
        if len(body) > MAX_INDEX_BYTES or not index_url(response.url):
            raise ValueError("Qualified index response is oversized or unsupported")
        content_type = response.headers.get_content_type()
    if content_type == "application/vnd.pypi.simple.v1+json":
        document = json.loads(body)
        if not str(document["meta"]["api-version"]).startswith("1."):
            raise ValueError("Unsupported qualified index API version")
        return document["files"]
    parser = Links()
    parser.feed(body.decode("utf-8"))
    return parser.links


def target_owner():
    spec = importlib.util.spec_from_file_location(
        "pumas_wheel_target", Path(__file__).with_name("wheel_target.py")
    )
    owner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(owner)
    return owner


def catalog(lock, roots, fetch=fetch_index, *, target_observation=None):
    entries = parse_lock(lock)
    validate_roots(entries, roots)
    target = None
    if target_observation is not None:
        owner = target_owner()
        target = owner.checked_observation(target_observation)
        target_observation = {**target_observation, "target": target.to_dict()}
        if (target_observation["interpreter"] != sys.executable
                or target_observation["interpreter_sha256"] != owner.interpreter_digest(sys.executable)):
            raise ValueError("Qualified target interpreter differs from approved observation")
        target.require_native_consumer()
    # Keep native packaging priority; explicit context only restricts compatibility.
    tags = {tag: rank for rank, tag in enumerate(sys_tags())
            if target is None or tag in target.tags}
    python = Version(platform.python_version() if target is None else target.python)
    start = time.monotonic()
    requests = 0
    artifacts = []
    for entry in entries:
        if entry["url"] is not None:
            candidates = [{"url": entry["url"], "hashes": {"sha256": entry["hashes"][0]}}]
        else:
            candidates = []
            for base in INDEXES:
                requests += 1
                if requests > MAX_INDEX_REQUESTS or time.monotonic() - start > MAX_CATALOG_SECONDS:
                    raise ValueError(
                        "Qualified wheel catalog budget exhausted; availability is inconclusive"
                    )
                address = base + entry["name"] + "/"
                rows = fetch(address)
                if time.monotonic() - start > MAX_CATALOG_SECONDS:
                    raise ValueError(
                        "Qualified wheel catalog budget exhausted; availability is inconclusive"
                    )
                if len(rows) > 10000:
                    raise ValueError("Qualified index has too many candidates")
                candidates.extend({**row, "url": urljoin(address, row["url"])} for row in rows)
        choices = {}
        for candidate in candidates:
            parsed = urlparse(candidate["url"])
            digest = candidate.get("hashes", {}).get("sha256")
            if parsed.fragment:
                if not re.fullmatch(r"sha256=[0-9a-fA-F]{64}", parsed.fragment):
                    continue
                if digest is not None and digest.lower() != parsed.fragment[7:].lower():
                    raise ValueError("Qualified index hash evidence disagrees")
                digest = parsed.fragment[7:]
            source = urlunparse(parsed._replace(fragment=""))
            if digest is None or digest.lower() not in entry["hashes"]:
                continue
            if not wheel_source(source, entry["name"], entry["url"]):
                continue
            try:
                name, version, build, wheel_tags = parse_wheel_filename(
                    unquote(parsed.path.rsplit("/", 1)[-1])
                )
            except ValueError:
                continue
            ranks = [tags[t] for t in wheel_tags if t in tags]
            if name != entry["name"] or version != Version(entry["version"]) or not ranks:
                continue
            requires_python = candidate.get("requires-python")
            if requires_python and not SpecifierSet(requires_python).contains(
                python, prereleases=True
            ):
                continue
            choices[(source, digest.lower())] = (min(ranks), build)
        if not choices:
            raise ValueError(
                "Qualified recipe has no approved compatible wheel; availability is inconclusive"
            )
        rank = min(value[0] for value in choices.values())
        build = max(value[1] for value in choices.values() if value[0] == rank)
        best = [choice for choice, score in choices.items() if score == (rank, build)]
        if len(best) != 1:
            raise ValueError(
                "Qualified recipe wheel choice is ambiguous; qualification is required"
            )
        source, digest = best[0]
        artifacts.append(
            {"name": entry["name"], "version": entry["version"], "url": source, "sha256": digest}
        )
    result = {
        "format": "pumas-qualified-wheel-catalog-1",
        "recipe_lock_sha256": hashlib.sha256(lock.encode()).hexdigest(),
        "release": "2.9.1",
        "torch": "2.9.1+cu130",
        "build": "cu130",
        "python": "3.12" if target is None else target.markers["python_version"],
        "interpreter": sys.executable,
        "implementation": sys.implementation.name,
        "platform": platform.platform(),
        "machine": platform.machine(),
        "adapter": "bundled",
        "artifacts": artifacts,
    }
    return result if target is None else owner.bind_resolution(result, target_observation)


def validate_recipe_artifacts(lock, roots, artifacts):
    entries = parse_lock(lock)
    validate_roots(entries, roots)
    expected = {e["name"]: e for e in entries}
    if len(artifacts) != len(expected):
        raise ValueError("Acquired wheel set differs from the complete qualified recipe")
    seen = set()
    for artifact in artifacts:
        name = canonicalize_name(artifact["name"])
        entry = expected.get(name)
        if (
            entry is None
            or name in seen
            or artifact["version"] != entry["version"]
            or artifact["sha256"].lower() not in entry["hashes"]
            or not wheel_source(artifact["url"], name, entry["url"])
        ):
            raise ValueError("Acquired wheel identity differs from the qualified recipe")
        seen.add(name)


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--lock", type=Path, required=True)
    parser.add_argument("--preview", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-observation", type=Path)
    args = parser.parse_args()
    if (
        sys.platform != "linux"
        or platform.machine() != "x86_64"
        or sys.version_info[:2] != (3, 12)
        or sys.implementation.name != "cpython"
    ):
        parser.exit(3, "Qualified recipe requires its existing CPython3.12/Linux x86_64 target\n")
    try:
        lock = args.lock.read_text(encoding="utf-8")
        preview = json.loads(args.preview.read_text(encoding="utf-8"))
        if preview["requirementsLock"] != lock:
            raise ValueError("Qualified preview differs from the embedded lock")
        observation = None
        if args.target_observation is not None:
            if args.target_observation.is_symlink() or not args.target_observation.is_file():
                raise ValueError("Approved target observation is missing or linked")
            with args.target_observation.open("rb") as source:
                raw = source.read(64 * 1024 + 1)
            if len(raw) > 64 * 1024:
                raise ValueError("Approved target observation is oversized")
            observation = json.loads(raw)
            if not isinstance(observation, dict):
                raise ValueError("Supplied target observation must be an object")
        result = catalog(lock, preview["directArtifacts"], target_observation=observation)
        args.output.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    except (KeyError, TypeError, ValueError, OSError, AttributeError):
        parser.exit(
            3, "Qualified wheel catalog refused; no resolver or source fallback is permitted\n"
        )


if __name__ == "__main__":
    main()
