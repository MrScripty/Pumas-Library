"""Private live-owner selected-packet consumer; serialized data is not authority.

Rust retains the accepted packet/catalog/stage. Recheck the original catalog and
public lock, install only exact local files using public pip, and inspect RECORD.
No resolver report is manufactured and no installed package is imported.
"""
import argparse
import importlib.util
import json
from pathlib import Path
import sys
import tomllib
import zipfile


def helper(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


selection = helper("offline_wheel_selection")
consumer = helper("install_verified_wheels")


def checked_packet(request, observation, evidence, wheels, directory, packet):
    projection = selection.prepare(request, observation, evidence, wheels)
    check = selection.check
    check(json.loads(selection.read(directory / "projection.json", 2 * 1024 * 1024)) == projection,
          "Retained projection differs")
    for name, key in (("roots.in", "roots"), ("constraints.in", "constraints")):
        check(selection.read(directory / name, 2 * 1024 * 1024)
              == ("\n".join(projection[key]) + "\n").encode(), "Retained solver inputs differ")
    body = selection.read(directory / "pylock.toml", 2 * 1024 * 1024)
    actual = selection.selected_packet(request, observation, evidence, projection,
                                       tomllib.loads(body.decode("utf-8")), directory)
    actual["lock_sha256"] = selection.catalog.hashlib.sha256(body).hexdigest()
    original = {k: v for k, v in packet.items() if k != "qualified_solver"}
    check(actual == original, "Selected packet differs from actual original closure")
    target = selection.catalog.checked_target(request, observation)
    target.require_native_consumer()
    check(consumer.target_owner().interpreter_digest(sys.executable)
          == observation["interpreter_sha256"], "Selected executable differs")
    return target


def consume(request, observation, evidence, wheels, directory, packet, target, output, *, verify_only=False):
    wheel_target = checked_packet(request, observation, evidence, wheels, directory, packet)
    artifacts = packet["selected"]
    expanded = 0
    for item in artifacts:
        with zipfile.ZipFile(item["local"]) as archive:
            expanded += sum(member.file_size for member in archive.infolist())
    selection.check(expanded <= 64 * 1024 * 1024, "Selected installed byte budget exceeded")
    paths = [item["local"] for item in artifacts]
    if verify_only:
        lines, _ = consumer.local_requirements(
            artifacts, wheels, wheel_target=wheel_target.to_dict(),
            local_paths=paths, requested_extras=packet["extras"],
        )
        selection.check(selection.read(output / "local-requirements.txt", 2 * 1024 * 1024)
                        == ("\n".join(lines) + "\n").encode(), "Local requirements changed")
        consumer.validate_installation_report(artifacts, wheels, output / "local-pip-report.json", local_paths=paths)
        records = helper("wheel_records")
        manifest = records.installed_file_manifest(
            target, artifacts, canonicalize_name=consumer.canonicalize_name,
            Version=consumer.Version, InvalidVersion=consumer.InvalidVersion,
        )
        selection.check(json.loads(selection.read(output / "installed-files.json", 32 * 1024 * 1024))
                        == manifest, "Installed manifest changed")
    else:
        manifest = consumer.install(
            artifacts, wheels, target, output, wheel_target=wheel_target.to_dict(),
            local_paths=paths, requested_extras=packet["extras"],
        )
    # A genuine public-pip report describes this actual native child, separately
    # from original source identity and the qualified solver's selected lock.
    report = json.loads(selection.read(output / "local-pip-report.json", 2 * 1024 * 1024))
    selection.check(report["environment"] == dict(wheel_target.markers)
                    and isinstance(report["pip_version"], str) and report["pip_version"],
                    "Installation report target differs")
    checked_packet(request, observation, evidence, wheels, directory, packet)
    return manifest


def main():
    parser = argparse.ArgumentParser()
    for name in ("request", "observation", "catalog", "wheels", "directory", "packet", "target", "output"):
        parser.add_argument("--" + name, type=Path, required=True)
    parser.add_argument("--verify-only", action="store_true")
    args = parser.parse_args()
    try:
        decode = selection.catalog.decode
        consume(decode(selection.read(args.request, 10 * 1024 * 1024)),
                decode(selection.read(args.observation, 64 * 1024)),
                decode(selection.read(args.catalog, 2 * 1024 * 1024)),
                args.wheels, args.directory,
                decode(selection.read(args.packet, 2 * 1024 * 1024)), args.target, args.output, verify_only=args.verify_only)
    except (ValueError, TypeError, KeyError, OSError, zipfile.BadZipFile, ExceptionGroup):
        parser.exit(21, "Selected local wheel consumption refused\n")


if __name__ == "__main__":
    main()
