"""Single installed RECORD proof owner; pure data inspection, no pip import."""

import base64
import csv
from email.parser import Parser
import hashlib
import os
from pathlib import Path


def staged_file_digest(path: Path) -> tuple[bytes, int]:
    digest = hashlib.sha256()
    size = 0
    with path.open("rb") as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b""):
            digest.update(chunk)
            size += len(chunk)
    return digest.digest(), size


def installed_file_manifest(
    target: Path, artifacts: list[dict] | None = None, *, canonicalize_name, Version, InvalidVersion
) -> dict:
    """Validate pip's staged wheel files against every installed RECORD."""
    if target.is_symlink() or not target.is_dir():
        raise ValueError("Staged package target is missing or linked")
    records = sorted(target.glob("*.dist-info/RECORD"))
    if not records:
        raise ValueError("Staged wheels have no RECORD files")
    if artifacts is not None:
        expected = {
            canonicalize_name(artifact["name"]): artifact["version"] for artifact in artifacts
        }
        if len(expected) != len(artifacts) or len(records) != len(expected):
            raise ValueError("Staged wheel RECORD set differs from the pip report")
        seen_distributions = set()
        for record in records:
            directory = record.parent.name.removesuffix(".dist-info")
            if "-" not in directory:
                raise ValueError("Staged distribution identity is malformed")
            directory_name, directory_version = directory.rsplit("-", 1)
            name = canonicalize_name(directory_name)
            if name not in expected or name in seen_distributions:
                raise ValueError("Staged wheel RECORD set differs from the pip report")
            seen_distributions.add(name)
            metadata_path = record.parent / "METADATA"
            if metadata_path.is_symlink() or not metadata_path.is_file():
                raise ValueError("Staged distribution METADATA is missing or linked")
            metadata = Parser().parsestr(metadata_path.read_text(encoding="utf-8"))
            if len(metadata.get_all("Name", [])) != 1 or len(metadata.get_all("Version", [])) != 1:
                raise ValueError("Staged distribution METADATA identity is malformed")
            try:
                expected_version = Version(expected[name])
                matches = (
                    Version(directory_version) == expected_version
                    and Version(metadata["Version"]) == expected_version
                )
            except InvalidVersion:
                matches = False
            if canonicalize_name(metadata["Name"]) != name or not matches:
                raise ValueError("Staged distribution identity differs from the pip report")
    claimed = set()
    for record in records:
        if record.is_symlink() or record.parent.is_symlink():
            raise ValueError("Staged wheel RECORD is linked")
        with record.open(newline="", encoding="utf-8") as source:
            for row in csv.reader(source):
                if len(row) != 3:
                    raise ValueError("Malformed staged wheel RECORD")
                name, recorded_hash, recorded_size = row
                if name.startswith("../../"):
                    relocated = name.removeprefix("../../")
                    root, separator, nested = relocated.partition("/")
                    if not separator or root not in {"bin", "share", "Scripts", "Include"}:
                        raise ValueError(
                            f"Staged wheel RECORD escapes its target: "
                            f"{record.parent.name} {name[:256]!r}"
                        )
                    relative = root + "/" + nested
                else:
                    relative = name
                path = Path(relative)
                if (
                    not relative
                    or path.is_absolute()
                    or "\\" in relative
                    or any(part in ("", ".", "..") for part in relative.split("/"))
                ):
                    raise ValueError(
                        f"Staged wheel RECORD escapes its target: "
                        f"{record.parent.name} {name[:256]!r}"
                    )
                if relative in claimed:
                    raise ValueError("Duplicate staged wheel file")
                file = target / path
                if relative.endswith(".pyc") and not recorded_hash and not recorded_size:
                    # pip compiles these during installation; the wheel provides no
                    # digest. The interpreter will regenerate them from checked source.
                    if file.is_symlink():
                        raise ValueError("Staged packages contain a symlink")
                    if file.exists():
                        if not file.is_file():
                            raise ValueError("Staged wheel RECORD names a special file")
                        file.unlink()
                    continue
                if file.is_symlink() or not file.is_file():
                    raise ValueError("Staged wheel RECORD names a missing or linked file")
                claimed.add(relative)
                if len(claimed) > 200_000:
                    raise ValueError("Staged package manifest is too large")
                digest, size = staged_file_digest(file)
                if file != record:
                    expected = "sha256=" + base64.urlsafe_b64encode(digest).rstrip(b"=").decode(
                        "ascii"
                    )
                    if recorded_hash != expected or recorded_size != str(size):
                        raise ValueError("Staged wheel RECORD hash or size differs")
                elif recorded_hash or recorded_size:
                    raise ValueError("Staged wheel RECORD must not self-hash")
    files = []
    for root, dirs, names in os.walk(target, followlinks=False):
        for name in (*dirs, *names):
            file = Path(root) / name
            if file.is_symlink():
                raise ValueError("Staged packages contain a symlink")
        for name in names:
            file = Path(root) / name
            relative = file.relative_to(target).as_posix()
            if relative not in claimed:
                raise ValueError("Staged packages contain an unreported file")
            digest, size = staged_file_digest(file)
            files.append({"path": relative, "sha256": digest.hex(), "size": size})
            if len(files) > 200_000:
                raise ValueError("Staged package manifest is too large")
    if len(files) != len(claimed):
        raise ValueError("Staged wheel RECORD contains files outside the target")
    return {"files": sorted(files, key=lambda item: item["path"])}
