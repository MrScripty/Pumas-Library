#!/usr/bin/env python3
"""Native ORT loader/API probe in a bounded isolated child; never model inference."""

import argparse
import ctypes
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import sys
import tempfile


TARGETS = {
    "linux-x86_64": ("Linux", {"x86_64", "amd64"}, "libonnxruntime.so"),
    "windows-x86_64": ("Windows", {"x86_64", "amd64"}, "onnxruntime.dll"),
    "macos-arm64": ("Darwin", {"arm64", "aarch64"}, "libonnxruntime.dylib"),
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def verify_files(directory, target):
    pins = json.loads(Path(__file__).with_name("onnx-runtime-pins.json").read_text())
    for name, item in pins["targets"][target]["files"].items():
        path = directory / name
        require(path.is_file() and not path.is_symlink(), "native member must be regular")
        require(
            path.stat().st_size == item["bytes"] and digest(path) == item["sha256"],
            "native member differs from checked pin",
        )


def probe_api(path):
    """OrtApiBase's two-function stable C ABI; library is verified by the caller."""
    convention = ctypes.WINFUNCTYPE if sys.platform == "win32" else ctypes.CFUNCTYPE
    get_api_type = convention(ctypes.c_void_p, ctypes.c_uint32)
    get_version_type = convention(ctypes.c_char_p)

    class ApiBase(ctypes.Structure):
        _fields_ = [("get_api", get_api_type), ("get_version", get_version_type)]

    library = ctypes.WinDLL(str(path)) if sys.platform == "win32" else ctypes.CDLL(str(path))
    factory = library.OrtGetApiBase
    factory.argtypes = []
    factory.restype = ctypes.POINTER(ApiBase)
    base = factory()
    require(bool(base), "OrtGetApiBase returned null")
    require(bool(base.contents.get_api(24)), "ORT C API 24 is unavailable")
    require(base.contents.get_version() == b"1.24.2", "ORT version differs from 1.24.2")
    return library


def linux_mappings():
    """All observed file mappings, including host dependencies, with current identity."""
    files = {}
    for line in Path("/proc/self/maps").read_text().splitlines():
        fields = line.split(maxsplit=5)
        if len(fields) != 6 or not fields[5].startswith("/"):
            continue
        path = Path(fields[5])
        stat = path.stat()
        major, minor = (int(part, 16) for part in fields[3].split(":"))
        require(
            (stat.st_ino, os.major(stat.st_dev), os.minor(stat.st_dev))
            == (int(fields[4]), major, minor),
            "mapped native file identity changed",
        )
        if str(path) not in files:
            files[str(path)] = {"sha256": digest(path), "bytes": stat.st_size}
    return files


def worker(directory, target):
    system, machines, entry = TARGETS[target]
    require(
        platform.system() == system and platform.machine().lower() in machines,
        "native probe requires matching host",
    )
    verify_files(directory, target)
    before = linux_mappings() if system == "Linux" else None
    library = probe_api(directory / entry)
    after = linux_mappings() if system == "Linux" else None
    if after is not None:
        require(str(directory / entry) in after, "pinned native loader not observed mapped")
        allowed = json.loads(Path(__file__).with_name("onnx-runtime-pins.json").read_text())[
            "targets"
        ][target]["files"]
        for name in after:
            path = Path(name)
            if "libonnxruntime" in path.name:
                require(
                    path.parent == directory and path.name in allowed,
                    "ambient ORT mapping detected",
                )
    verify_files(directory, target)
    # Retain the native handle through all observations; process exit ends custody.
    require(library is not None, "native handle missing")
    return {
        "target": target,
        "runtime_version": "1.24.2",
        "c_api": 24,
        "loader_sha256": digest(directory / entry),
        "loader_api": "passed",
        "before_mappings": before,
        "after_mappings": after,
        "mapping_scope": "all current process file mappings"
        if after is not None
        else "unavailable on this platform",
        "real_inference": "not_run",
        "session_dependent_closure": "unqualified",
        "signing": "unqualified",
    }


def probe(directory, target, output):
    directory = Path(directory).resolve()
    output = Path(output)
    require(not output.exists(), "probe evidence must be fresh")
    with tempfile.TemporaryDirectory(prefix="pumas-native-probe-") as tmp:
        root = Path(tmp)
        environment = {
            "PATH": os.defpath,
            "HOME": tmp,
            "TMPDIR": tmp,
            "TMP": tmp,
            "TEMP": tmp,
            "LANG": "C.UTF-8",
        }
        # Windows system DLL resolution needs the host's actual OS directory.
        if sys.platform == "win32":
            buffer = ctypes.create_unicode_buffer(32768)
            require(
                ctypes.windll.kernel32.GetWindowsDirectoryW(buffer, len(buffer)) > 0,
                "Windows directory unavailable",
            )
            environment["SystemRoot"] = buffer.value
            environment["WINDIR"] = buffer.value
            environment["PATH"] = str(Path(buffer.value) / "System32")
        completed = subprocess.run(
            [
                sys.executable,
                "-I",
                str(Path(__file__).resolve()),
                "--worker",
                "--target",
                target,
                "--runtime-dir",
                str(directory),
            ],
            cwd=root,
            env=environment,
            check=True,
            capture_output=True,
            text=True,
            timeout=30,
        )
        evidence = json.loads(completed.stdout)
    with output.open("x") as stream:
        stream.write(json.dumps(evidence, indent=2) + "\n")
    return evidence


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime-dir", type=Path, required=True)
    parser.add_argument("--target", choices=TARGETS, required=True)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--worker", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.worker:
        print(json.dumps(worker(args.runtime_dir.resolve(), args.target)))
    elif args.output is None:
        parser.error("--output is required")
    else:
        probe(args.runtime_dir, args.target, args.output)


if __name__ == "__main__":
    main()
