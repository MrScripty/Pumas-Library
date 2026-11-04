"""Supplemental managed-CPython legal texts for declared files an upstream archive omits.

A python-build-standalone ``PYTHON.json`` can declare a license file that the
published full archive does not contain. Such a path is satisfied only by a
checked-in text whose provenance record names exactly that path, the reviewed
build release, the target, and the pinned source archive the text came from,
and whose bytes match the recorded hash. Supplemental texts are never archive
members and are reported separately from them.
"""

import hashlib
import json
from pathlib import Path
import re

DIRECTORY = "scripts/release/licenses/managed-python-supplemental"
RECORD_SUFFIX = ".provenance.json"
REASON = "declared_by_python_json_but_absent_from_upstream_full_archive"
RECORD_KEYS = {
    "schema_version",
    "declared_path",
    "supplemental_reason",
    "note",
    "python_build_standalone_release",
    "pin_authority",
    "targets",
    "component",
    "version",
    "license",
    "source_archive",
    "path_in_source_archive",
    "file",
    "size",
    "sha256",
}
DECLARED_PATH = re.compile(r"licenses/[A-Za-z0-9._-]+")
FILE_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]*")
SHA256 = re.compile(r"[0-9a-f]{64}")


def declared_license_paths(metadata) -> set[str]:
    """Return every ``license_path``/``license_paths`` reference in PYTHON.json."""
    paths = set()

    def visit(value) -> None:
        if isinstance(value, dict):
            for key, item in value.items():
                if key in ("license_path", "license_paths") and item is not None:
                    values = [item] if isinstance(item, str) else item
                    if not isinstance(values, list) or not all(
                        isinstance(path, str) and path for path in values
                    ):
                        raise ValueError(f"Invalid PYTHON.json license reference: {item!r}")
                    paths.update(values)
                else:
                    visit(item)
        elif isinstance(value, list):
            for item in value:
                visit(item)

    visit(metadata)
    return paths


def _positive_int(value) -> bool:
    return isinstance(value, int) and not isinstance(value, bool) and value > 0


def _text(value) -> bool:
    return isinstance(value, str) and bool(value.strip())


def _validate(record, name: str, release: str, targets: set[str] | None) -> None:
    source = record.get("source_archive") if isinstance(record, dict) else None
    listed = record.get("targets") if isinstance(record, dict) else None
    if (
        not isinstance(record, dict)
        or set(record) != RECORD_KEYS
        or record["schema_version"] != 1
        or record["supplemental_reason"] != REASON
        or record["python_build_standalone_release"] != release
        or record["pin_authority"]
        != f"https://github.com/astral-sh/python-build-standalone/blob/{release}/pythonbuild/downloads.py"
        or not isinstance(record["declared_path"], str)
        or not DECLARED_PATH.fullmatch(record["declared_path"])
        or not isinstance(listed, list)
        or not listed
        or not all(_text(target) for target in listed)
        or listed != sorted(set(listed))
        or (targets is not None and not set(listed) <= targets)
        or not all(
            _text(record[key])
            for key in ("note", "component", "version", "license", "path_in_source_archive")
        )
        or not isinstance(source, dict)
        or set(source) != {"url", "size", "sha256"}
        or not isinstance(source["url"], str)
        or not source["url"].startswith("https://")
        or not _positive_int(source["size"])
        or not isinstance(source["sha256"], str)
        or not SHA256.fullmatch(source["sha256"])
        or record["file"] != name.removesuffix(RECORD_SUFFIX)
        or not FILE_NAME.fullmatch(record["file"])
        or not _positive_int(record["size"])
        or not isinstance(record["sha256"], str)
        or not SHA256.fullmatch(record["sha256"])
    ):
        raise ValueError(f"Invalid managed CPython supplemental license record: {name}")


def load_records(directory: Path, release: str, targets: set[str] | None = None) -> list[dict]:
    """Load and byte-verify every supplemental record; refuse orphans and overlaps."""
    if not directory.exists():
        return []
    entries = sorted(directory.iterdir(), key=lambda path: path.name)
    if any(not path.is_file() or path.is_symlink() for path in entries):
        raise ValueError(f"Supplemental license directory must hold regular files: {directory}")
    records = []
    for path in entries:
        if not path.name.endswith(RECORD_SUFFIX):
            continue
        record = json.loads(path.read_bytes())
        _validate(record, path.name, release, targets)
        text = directory / record["file"]
        if not text.is_file() or text.is_symlink():
            raise ValueError(f"Missing managed CPython supplemental license: {record['file']}")
        data = text.read_bytes()
        if hashlib.sha256(data).hexdigest() != record["sha256"] or len(data) != record["size"]:
            raise ValueError(f"Changed managed CPython supplemental license: {record['file']}")
        records.append({**record, "record_path": path, "text_path": text})
    referenced = {record["file"] for record in records}
    orphans = sorted(
        path.name
        for path in entries
        if not path.name.endswith(RECORD_SUFFIX) and path.name not in referenced
    )
    if orphans:
        raise ValueError(f"Supplemental license texts without provenance: {orphans}")
    claimed = set()
    for record in records:
        for target in record["targets"]:
            key = (target, record["declared_path"])
            if key in claimed:
                raise ValueError(f"Duplicate supplemental license for {target}: {key[1]}")
            claimed.add(key)
    return records


def select(
    records: list[dict],
    target: str,
    declared: set[str],
    archive_paths: set[str],
    *,
    strict: bool,
) -> list[dict]:
    """Return the records that satisfy declared paths missing from the archive.

    Every declared path absent from ``archive_paths`` needs exactly one record
    for ``target``. With ``strict``, a record for ``target`` whose path is not
    a declared, missing path is refused as stale.
    """
    missing = declared - archive_paths
    applicable = [record for record in records if target in record["targets"]]
    selected = [record for record in applicable if record["declared_path"] in missing]
    unsatisfied = sorted(missing - {record["declared_path"] for record in selected})
    if unsatisfied:
        raise ValueError(
            "Declared managed CPython licenses absent from the full archive and without "
            f"a verified supplemental text for {target}: {unsatisfied}"
        )
    if strict:
        stale = sorted(record["declared_path"] for record in applicable if record not in selected)
        if stale:
            raise ValueError(
                f"Supplemental managed CPython licenses not needed for {target}: {stale}"
            )
    return sorted(selected, key=lambda record: record["declared_path"])
