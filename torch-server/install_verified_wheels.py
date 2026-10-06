"""Consume an accepted, complete wheel set locally using public pip commands.

Original resolution URLs remain provenance. Only generated local file inputs
reach pip; acquisition and input custody stay with the Rust owner.
"""

import argparse
from email.parser import Parser
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import re
import subprocess
import sys
from urllib.parse import unquote, urlparse
import zipfile

# Embedded standalone tooling is available before target packages are installed.
_tooling = Path(__file__).with_name("packaging-tooling.zip")
if not _tooling.is_file():
    _tooling = Path(__file__).parent / "tooling" / "packaging.zip"
if _tooling.is_file():
    sys.path.insert(0, str(_tooling))

Metadata = importlib.import_module("packaging.metadata").Metadata
default_environment = importlib.import_module("packaging.markers").default_environment
Requirement = importlib.import_module("packaging.requirements").Requirement
SpecifierSet = importlib.import_module("packaging.specifiers").SpecifierSet
sys_tags = importlib.import_module("packaging.tags").sys_tags
parse_tag = importlib.import_module("packaging.tags").parse_tag
canonicalize_name = importlib.import_module("packaging.utils").canonicalize_name
parse_wheel_filename = importlib.import_module("packaging.utils").parse_wheel_filename
Version = importlib.import_module("packaging.version").Version
InvalidVersion = importlib.import_module("packaging.version").InvalidVersion


def wheel_filename(url: str) -> str:
    name = unquote(urlparse(url).path.rsplit("/", 1)[-1])
    if not name or any(c in name for c in "/\\\x00\r\n") or not name.endswith(".whl"):
        raise ValueError("Invalid accepted wheel filename")
    parse_wheel_filename(name)
    return name


class UnsupportedDependencyReference(ValueError):
    pass


def validate_closure(metadata: dict, versions: dict, *, environment=None) -> None:
    """Evaluate required markers and propagate requested extras to a fixed point."""
    extras = {name: {""} for name in metadata}
    environment = default_environment() if environment is None else environment
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
                    raise UnsupportedDependencyReference(
                        "Dependency direct URLs are unsupported for local consumption"
                    )
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


def target_owner():
    spec = importlib.util.spec_from_file_location("pumas_wheel_target", Path(__file__).with_name("wheel_target.py"))
    owner = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(owner)
    return owner


def explicit_target(document):
    return target_owner().WheelTarget(document)


def local_requirements(artifacts: list[dict], wheels: Path, *, wheel_target=None) -> tuple[list[str], dict]:
    if not artifacts or wheels.is_symlink() or not wheels.is_dir():
        raise ValueError("Verified wheel set is missing")
    names, filenames, versions, metadata, lines = set(), set(), {}, {}, []
    selected_target = explicit_target(wheel_target) if wheel_target is not None else None
    compatible = set(selected_target.tags if selected_target is not None else sys_tags())
    environment = dict(selected_target.markers) if selected_target is not None else default_environment()
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
            wheel_members = [
                m for m in archive.infolist() if m.filename.endswith(".dist-info/WHEEL")
            ]
            if (
                len(wheel_members) != 1
                or wheel_members[0].file_size > 64 * 1024
                or wheel_members[0].filename.rsplit("/", 1)[0]
                != members[0].filename.rsplit("/", 1)[0]
            ):
                raise ValueError("Wheel tag metadata is missing, duplicated or oversized")
            wheel_document = Parser().parsestr(archive.read(wheel_members[0]).decode("utf-8"))
            declared_tags = set()
            for value in wheel_document.get_all("Tag", []):
                declared_tags.update(parse_tag(value))
            versions_declared = wheel_document.get_all("Wheel-Version", [])
            if (
                declared_tags != tags
                or len(versions_declared) != 1
                or not re.fullmatch(r"1\.\d+", versions_declared[0])
            ):
                raise ValueError("Wheel tag metadata differs from its compatible filename")
            body = archive.read(members[0])
            try:
                Metadata.from_email(body, validate=True)
            except ExceptionGroup:
                raise ValueError("Wheel METADATA is invalid") from None
            document = Parser().parsestr(body.decode("utf-8"))
        if len(document.get_all("Name", [])) != 1 or len(document.get_all("Version", [])) != 1:
            raise ValueError("Wheel METADATA distribution identity is ambiguous")
        if canonicalize_name(document["Name"]) != name or Version(document["Version"]) != version:
            raise ValueError("Wheel METADATA differs from accepted distribution")
        python = document.get_all("Requires-Python", [])
        if len(python) > 1 or (
            python
            and not SpecifierSet(python[0]).contains(
                environment["python_full_version"], prereleases=True
            )
        ):
            raise ValueError("Wheel Requires-Python differs from the selected interpreter")
        names.add(name)
        filenames.add(filename)
        versions[name], metadata[name] = version, document
        lines.append(f"{name} @ {path.resolve().as_uri()} --hash=sha256:{digest.lower()}")
    if {p.name for p in wheels.iterdir()} != filenames:
        raise ValueError("Verified local wheel directory differs from the exact accepted set")
    validate_closure(metadata, versions, environment=environment)
    return lines, metadata


def install(artifacts: list[dict], wheels: Path, target: Path, output: Path, *, wheel_target=None) -> dict:
    if wheel_target is not None:
        explicit_target(wheel_target).require_native_consumer()
    lines, _ = local_requirements(artifacts, wheels, wheel_target=wheel_target)
    if target.is_symlink() or (target.exists() and (not target.is_dir() or any(target.iterdir()))):
        raise ValueError("Local package target must be an empty owned directory")
    output.mkdir(parents=True, exist_ok=True)
    target.mkdir(parents=True, exist_ok=True)
    requirements = output / "local-requirements.txt"
    requirements.write_text("\n".join(lines) + "\n", encoding="utf-8", newline="\n")
    report = output / "local-pip-report.json"
    # --isolated excludes user settings, but still loads global/site config.
    # Disable every config file in this child only; discard inherited pip options.
    pip_environment = {
        key: value for key, value in os.environ.items() if not key.upper().startswith("PIP_")
    }
    pip_environment["PIP_CONFIG_FILE"] = os.devnull
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
        env=pip_environment,
    )
    if completed.returncode:
        raise ValueError("Exact local wheel installation failed")
    # The existing package owner validates RECORD files without importing any
    # installed package or allowing stage code to execute during verification.
    spec = importlib.util.spec_from_file_location(
        "pumas_resolution_owner", Path(__file__).with_name("wheel_records.py")
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
    manifest = owner.installed_file_manifest(
        target,
        artifacts,
        canonicalize_name=canonicalize_name,
        Version=Version,
        InvalidVersion=InvalidVersion,
    )
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
    parser.add_argument("--recipe-lock", type=Path)
    parser.add_argument("--preview", type=Path)
    parser.add_argument("--target-observation", type=Path)
    args = parser.parse_args()
    if (args.recipe_lock is None) != (args.preview is None):
        parser.error("Qualified wheel consumption requires both lock and preview")
    try:
        resolution = json.loads(args.resolution.read_text(encoding="utf-8"))
        observation = None
        if args.target_observation is not None:
            if args.target_observation.is_symlink() or not args.target_observation.is_file():
                raise ValueError("Approved target observation is missing or linked")
            with args.target_observation.open("rb") as source:
                approved = source.read(64 * 1024 + 1)
            if len(approved) > 64 * 1024:
                raise ValueError("Approved target observation is oversized")
            observation = json.loads(approved)
        owner = target_owner()
        selected = owner.resolution_target(resolution, observation)
        if selected is not None:
            selected.require_native_consumer()
            if owner.interpreter_digest(sys.executable) != observation["interpreter_sha256"]:
                raise ValueError("Native consumer executable differs from target approval")
        if args.recipe_lock is not None:
            spec = importlib.util.spec_from_file_location(
                "pumas_qualified_catalog", Path(__file__).with_name("qualified_wheel_catalog.py")
            )
            catalog = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(catalog)
            lock = args.recipe_lock.read_text(encoding="utf-8")
            preview = json.loads(args.preview.read_text(encoding="utf-8"))
            if (
                preview["requirementsLock"] != lock
                or resolution.get("format") != "pumas-qualified-wheel-catalog-1"
                or resolution.get("recipe_lock_sha256") != hashlib.sha256(lock.encode()).hexdigest()
            ):
                raise ValueError("Qualified catalog provenance differs from the original recipe")
            catalog.validate_recipe_artifacts(
                lock, preview["directArtifacts"], resolution["artifacts"]
            )
        install(resolution["artifacts"], args.wheels, args.target, args.output,
                wheel_target=selected.to_dict() if selected is not None else None)
    except UnsupportedDependencyReference:
        parser.exit(
            3, "Dependency direct URLs are unsupported for local consumption; no source fallback\n"
        )
    except (KeyError, TypeError, ValueError, OSError, zipfile.BadZipFile):
        # Package/URL metadata is untrusted. Keep stdout/stderr diagnostics
        # bounded and do not print locators or arbitrary metadata text.
        parser.exit(3, "Invalid or incomplete verified local wheel set\n")


if __name__ == "__main__":
    main()
