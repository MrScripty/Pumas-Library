"""Consume an accepted, complete wheel set locally using public pip commands.

Original resolution URLs remain provenance. Only generated local file inputs
reach pip; acquisition and input custody stay with the Rust owner.
"""

import argparse
from email.parser import Parser
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlparse
import zipfile

from pip._vendor.packaging.markers import default_environment
from pip._vendor.packaging.requirements import Requirement
from pip._vendor.packaging.specifiers import SpecifierSet
from pip._vendor.packaging.tags import sys_tags
from pip._vendor.packaging.utils import canonicalize_name, parse_wheel_filename
from pip._vendor.packaging.version import Version


def wheel_filename(url: str) -> str:
    name = unquote(urlparse(url).path.rsplit("/", 1)[-1])
    if not name or any(c in name for c in "/\\\x00\r\n") or not name.endswith(".whl"):
        raise ValueError("Invalid accepted wheel filename")
    parse_wheel_filename(name)
    return name


def validate_closure(metadata: dict, versions: dict) -> None:
    """Evaluate required markers and propagate requested extras to a fixed point."""
    extras = {name: {""} for name in metadata}
    environment = default_environment()
    changed = True
    while changed:
        changed = False
        for name, document in metadata.items():
            available = {canonicalize_name(e) for e in document.get_all("Provides-Extra", [])}
            if not (extras[name] - {""}) <= available:
                raise ValueError("Accepted wheel does not provide a required extra")
            for value in document.get_all("Requires-Dist", []):
                requirement = Requirement(value)
                # Even an inactive URL is refused: local consumption never
                # accepts a hidden locator or delegates retrieval to pip.
                if requirement.url is not None:
                    raise ValueError("Dependency direct URLs are unsupported for local consumption")
                if requirement.marker and not any(
                    requirement.marker.evaluate({**environment, "extra": extra})
                    for extra in extras[name]
                ):
                    continue
                dependency = canonicalize_name(requirement.name)
                if dependency not in versions or not requirement.specifier.contains(
                    versions[dependency], prereleases=True
                ):
                    raise ValueError(
                        "Accepted wheel set omits or contradicts a required dependency"
                    )
                requested = {canonicalize_name(e) for e in requirement.extras}
                if not requested <= extras[dependency]:
                    extras[dependency].update(requested)
                    changed = True


def local_requirements(artifacts: list[dict], wheels: Path) -> tuple[list[str], dict]:
    if not artifacts or wheels.is_symlink() or not wheels.is_dir():
        raise ValueError("Verified wheel set is missing")
    names, filenames, versions, metadata, lines = set(), set(), {}, {}, []
    compatible = set(sys_tags())
    for artifact in artifacts:
        name = canonicalize_name(artifact["name"])
        if name in names:
            raise ValueError("Accepted distribution names are not canonically unique")
        version = Version(artifact["version"])
        digest = artifact["sha256"]
        if not isinstance(digest, str) or not re.fullmatch(r"[0-9a-fA-F]{64}", digest):
            raise ValueError("Accepted wheel lacks SHA-256")
        filename = wheel_filename(artifact["url"])
        wheel_name, wheel_version, _, tags = parse_wheel_filename(filename)
        if (
            filename in filenames
            or wheel_name != name
            or wheel_version != version
            or not tags & compatible
        ):
            raise ValueError("Wheel filename differs from accepted distribution or interpreter")
        path = wheels / filename
        if path.is_symlink() or not path.is_file():
            raise ValueError("Verified wheel member is missing or linked")
        hashed = hashlib.sha256()
        with path.open("rb") as source:
            for chunk in iter(lambda: source.read(1024 * 1024), b""):
                hashed.update(chunk)
        if hashed.hexdigest() != digest.lower():
            raise ValueError("Local wheel differs from accepted SHA-256")
        with zipfile.ZipFile(path) as archive:
            members = [m for m in archive.infolist() if m.filename.endswith(".dist-info/METADATA")]
            if len(members) != 1 or members[0].file_size > 4 * 1024 * 1024:
                raise ValueError("Wheel METADATA is missing, duplicated or oversized")
            document = Parser().parsestr(archive.read(members[0]).decode("utf-8"))
        if len(document.get_all("Name", [])) != 1 or len(document.get_all("Version", [])) != 1:
            raise ValueError("Wheel METADATA distribution identity is ambiguous")
        if canonicalize_name(document["Name"]) != name or Version(document["Version"]) != version:
            raise ValueError("Wheel METADATA differs from accepted distribution")
        python = document.get_all("Requires-Python", [])
        if len(python) > 1 or (
            python
            and not SpecifierSet(python[0]).contains(
                default_environment()["python_full_version"], prereleases=True
            )
        ):
            raise ValueError("Wheel Requires-Python differs from the selected interpreter")
        names.add(name)
        filenames.add(filename)
        versions[name], metadata[name] = version, document
        lines.append(f"{name} @ {path.resolve().as_uri()} --hash=sha256:{digest.lower()}")
    if {p.name for p in wheels.iterdir()} != filenames:
        raise ValueError("Verified local wheel directory differs from the exact accepted set")
    validate_closure(metadata, versions)
    return lines, metadata


def install(artifacts: list[dict], wheels: Path, target: Path, output: Path) -> dict:
    lines, _ = local_requirements(artifacts, wheels)
    if target.is_symlink() or (target.exists() and (not target.is_dir() or any(target.iterdir()))):
        raise ValueError("Local package target must be an empty owned directory")
    output.mkdir(parents=True, exist_ok=True)
    target.mkdir(parents=True, exist_ok=True)
    requirements = output / "local-requirements.txt"
    requirements.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    report = output / "local-pip-report.json"
    completed = subprocess.run(
        [
            sys.executable,
            "-I",
            "-m",
            "pip",
            "--isolated",
            "install",
            "--no-index",
            "--no-deps",
            "--require-hashes",
            "--only-binary=:all:",
            "--no-cache-dir",
            "--disable-pip-version-check",
            "--ignore-installed",
            "--no-compile",
            "--target",
            str(target),
            "--report",
            str(report),
            "-r",
            str(requirements),
        ],
        check=False,
    )
    if completed.returncode:
        raise ValueError("Exact local wheel installation failed")
    # The existing package owner validates RECORD files without importing any
    # installed package or allowing stage code to execute during verification.
    spec = importlib.util.spec_from_file_location(
        "pumas_resolution_owner", Path(__file__).with_name("resolve_runtime.py")
    )
    owner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(owner)
    local_report = json.loads(report.read_text(encoding="utf-8"))
    if local_report.get("version") != "1":
        raise ValueError("Unsupported local pip installation report version")
    installed = local_report["install"]
    expected = {
        canonicalize_name(artifact["name"]): (
            artifact["version"],
            (wheels / wheel_filename(artifact["url"])).resolve().as_uri(),
            artifact["sha256"].lower(),
        )
        for artifact in artifacts
    }
    observed = {}
    for item in installed:
        name = canonicalize_name(item["metadata"]["name"])
        if name in observed:
            raise ValueError("Local installation report repeats a distribution")
        observed[name] = (
            item["metadata"]["version"],
            item["download_info"]["url"],
            item["download_info"]["archive_info"]["hashes"]["sha256"].lower(),
        )
    if observed != expected:
        raise ValueError("Local installation report differs from exact accepted inputs")
    manifest = owner.installed_file_manifest(target, artifacts)
    (output / "installed-files.json").write_text(
        json.dumps(manifest, separators=(",", ":")) + "\n", encoding="utf-8", newline="\n"
    )
    return manifest


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--resolution", type=Path, required=True)
    parser.add_argument("--wheels", type=Path, required=True)
    parser.add_argument("--target", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        resolution = json.loads(args.resolution.read_text(encoding="utf-8"))
        install(resolution["artifacts"], args.wheels, args.target, args.output)
    except (KeyError, TypeError, ValueError, OSError, zipfile.BadZipFile):
        # Package/URL metadata is untrusted. Keep stdout/stderr diagnostics
        # bounded and do not print locators or arbitrary metadata text.
        parser.exit(3, "Invalid or incomplete verified local wheel set\n")


if __name__ == "__main__":
    main()
