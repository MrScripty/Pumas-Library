#!/usr/bin/env python3
"""Materialize explicitly pinned CPU ORT members, never execute archive content."""

import argparse
import hashlib
import json
from pathlib import Path
import shutil
import tarfile
import tempfile
import urllib.request
import zipfile

import headless_inference as package

PINS = Path(__file__).with_name("onnx-runtime-pins.json")


def stage(target_id, archive, destination, pins=None):
    pins = json.loads(PINS.read_text()) if pins is None else pins
    selected = pins["targets"][target_id]
    destination = Path(destination)
    package.require(not destination.exists(), "runtime destination must be fresh")
    # Hash and parse the same private snapshot, not a reopened mutable source.
    with tempfile.TemporaryFile() as snapshot:
        with Path(archive).open("rb") as source:
            copied = 0
            while chunk := source.read(65536):
                copied += len(chunk)
                package.require(copied <= selected["archive_bytes"], "runtime archive too large")
                snapshot.write(chunk)
        snapshot.seek(0)
        package.require(
            copied == selected["archive_bytes"]
            and hashlib.file_digest(snapshot, "sha256").hexdigest() == selected["archive_sha256"],
            "official runtime archive identity mismatch",
        )
        snapshot.seek(0)
        is_zip = selected["url"].endswith(".zip")
        with zipfile.ZipFile(snapshot) if is_zip else tarfile.open(fileobj=snapshot) as source:
            entries = source.infolist() if is_zip else source.getmembers()
            names = [entry.filename if is_zip else entry.name for entry in entries]
            package.require(len(names) == len(set(names)), "duplicate runtime archive members")
            payloads = {}
            for name, item in {**selected["files"], **selected["notices"]}.items():
                package.flat_name(name)
                entry = (
                    source.getinfo(item["member"]) if is_zip else source.getmember(item["member"])
                )
                size = entry.file_size if is_zip else entry.size
                regular = (
                    (not entry.is_dir() and (entry.external_attr >> 16) & 0o170000 != 0o120000)
                    if is_zip
                    else entry.isfile()
                )
                package.require(regular and size == item["bytes"], "invalid pinned runtime member")
                with source.open(entry) if is_zip else source.extractfile(entry) as stream:
                    data = stream.read(item["bytes"] + 1)
                package.require(
                    len(data) == item["bytes"]
                    and hashlib.sha256(data).hexdigest() == item["sha256"],
                    "runtime member identity mismatch",
                )
                payloads[name] = data
    destination.mkdir(parents=True)
    try:
        for name, data in payloads.items():
            (destination / name).write_bytes(data)
        record = {
            "target": selected["target"],
            "version": pins["version"],
            "loader_entry": package.TARGETS[target_id]["runtime_entry"],
            "source_archive_sha256": selected["archive_sha256"],
            "files": {
                name: {key: item[key] for key in ("sha256", "bytes")}
                for name, item in selected["files"].items()
            },
        }
        package.validate_runtime_record(record, package.TARGETS[target_id])
        (destination / "runtime-record.json").write_bytes(package.canonical(record))
        (destination / "runtime-inputs.json").write_bytes(
            package.canonical(
                {name: str((destination / name).resolve()) for name in record["files"]}
            )
        )
        return record
    except Exception:
        shutil.rmtree(destination)
        raise


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=package.TARGETS, required=True)
    inputs = parser.add_mutually_exclusive_group(required=True)
    inputs.add_argument("--archive", type=Path)
    inputs.add_argument(
        "--download", action="store_true", help="explicitly fetch the pinned official CPU archive"
    )
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    if args.archive:
        stage(args.target, args.archive, args.output_dir)
    else:
        selected = json.loads(PINS.read_text())["targets"][args.target]
        with tempfile.TemporaryDirectory() as temporary:
            archive = Path(temporary) / "runtime.archive"
            with (
                urllib.request.urlopen(selected["url"], timeout=60) as response,
                archive.open("xb") as stream,
            ):
                count = 0
                while chunk := response.read(65536):
                    count += len(chunk)
                    package.require(
                        count <= selected["archive_bytes"], "runtime download exceeds pin"
                    )
                    stream.write(chunk)
            stage(args.target, archive, args.output_dir)


if __name__ == "__main__":
    main()
