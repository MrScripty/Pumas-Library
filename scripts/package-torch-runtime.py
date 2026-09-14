#!/usr/bin/env python3
"""Produce the two assets consumed by the shared VersionManager Torch strategy."""

import argparse
import hashlib
import io
import json
import tarfile
from pathlib import Path


def package(source: Path, output: Path) -> Path:
    recipe = json.loads((source / "runtime/runtime.json").read_text())
    lock = source / "runtime/requirements.lock"
    if not lock.is_file() or "--hash=sha256:" not in lock.read_text():
        raise ValueError("Resolve the hash-locked runtime requirements before packaging")
    output.mkdir(parents=True, exist_ok=True)
    archive = output / "pumas-torch-runtime-linux-x86_64.tar.gz"
    files = {p.relative_to(source).as_posix(): p for p in source.glob("*.py")}
    files.update({p.relative_to(source).as_posix(): p for p in (source / "loaders").glob("*.py")})
    files["LICENSE"] = source.parent / "LICENSE"
    files["runtime.json"] = source / "runtime/runtime.json"
    files["requirements.txt"] = lock
    with tarfile.open(archive, "w:gz") as bundle:
        for name, path in sorted(files.items()):
            data = path.read_bytes()
            entry = tarfile.TarInfo(name)
            entry.size = len(data)
            entry.mode = 0o644
            bundle.addfile(entry, io.BytesIO(data))
    digest = hashlib.sha256(archive.read_bytes()).hexdigest()
    archive.with_suffix(archive.suffix + ".sha256").write_text(f"{digest}  {archive.name}\n")
    print(f"Release tag: {recipe['recipe_id']}\n{archive}\nSHA-256: {digest}")
    return archive


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    package(Path(__file__).resolve().parents[1] / "torch-server", args.output)
