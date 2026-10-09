"""Linux packaged inference acceptance: pinned bytes and isolated child environment."""

import json
import os
from pathlib import Path

import headless_inference as package
from onnx_runtime_stage import PINS


def verify_runtime(directory):
    directory = Path(directory).resolve()
    selected = json.loads(PINS.read_text())["targets"]["linux-x86_64"]
    for name, item in selected["files"].items():
        path = directory / name
        package.require(
            path.is_file() and not path.is_symlink(), "packaged runtime must be regular"
        )
        package.require(
            path.stat().st_size == item["bytes"] and package.sha256(path) == item["sha256"],
            "packaged runtime differs from official CPU pin",
        )
    return selected


def child_environment(root):
    root = Path(root).resolve()
    # An allowlist prevents all ambient ORT/Pumas/Python/loader/proxy overrides,
    # including ones introduced after this verifier was written.
    return {
        "PATH": "/usr/bin:/bin",
        "LANG": "C.UTF-8",
        "HOME": str(root / "home"),
        "TMPDIR": str(root / "tmp"),
        "XDG_CONFIG_HOME": str(root / "config"),
        "XDG_DATA_HOME": str(root / "data"),
        "XDG_CACHE_HOME": str(root / "cache"),
        "XDG_STATE_HOME": str(root / "state"),
        "XDG_RUNTIME_DIR": str(root / "run"),
        "PUMAS_REGISTRY_DB_PATH": str(root / "registry.db"),
        "ORT_SKIP_DOWNLOAD": "1",
    }


def loaded_runtime(pid, directory):
    """Observe Linux mappings while the model is loaded; not a full dependency audit."""
    directory = Path(directory).resolve()
    selected = verify_runtime(directory)
    expected = directory / "libonnxruntime.so"
    stat = expected.stat()
    matched = False
    for line in Path(f"/proc/{pid}/maps").read_text().splitlines():
        fields = line.split(maxsplit=5)
        if len(fields) < 6 or "libonnxruntime" not in fields[5]:
            continue
        path = Path(fields[5])
        package.require(
            path.parent == directory and path.name in selected["files"],
            "ambient ONNX runtime mapping detected",
        )
        actual = path.stat()
        major, minor = (int(value, 16) for value in fields[3].split(":"))
        package.require(
            int(fields[4]) == actual.st_ino
            and (major, minor) == (os.major(actual.st_dev), os.minor(actual.st_dev)),
            "mapped runtime file identity changed",
        )
        if path == expected:
            package.require(
                actual.st_ino == stat.st_ino and actual.st_dev == stat.st_dev,
                "runtime loader identity changed",
            )
            matched = True
    package.require(matched, "packaged ONNX runtime was not observed loaded")
    return {
        "loader_sha256": package.sha256(expected),
        "mapping": "packaged-file-identity-observed",
        "system_dependency_closure": "unqualified",
    }
