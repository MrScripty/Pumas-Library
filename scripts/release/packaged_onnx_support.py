"""Native packaged inference: pinned bytes, OS observations and child disposal."""

import ctypes
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import sys

import headless_inference as package
from onnx_runtime_stage import PINS


def native_target():
    system, machine = platform.system(), platform.machine().lower()
    if system == "Linux" and machine in {"x86_64", "amd64"}:
        return "linux-x86_64"
    if system == "Windows" and machine in {"x86_64", "amd64"}:
        return "windows-x86_64"
    if system == "Darwin" and machine in {"arm64", "aarch64"}:
        return "macos-arm64"
    raise ValueError("packaged inference requires a supported native host")


def verify_runtime(directory, target=None):
    directory = Path(directory).resolve()
    selected = json.loads(PINS.read_text())["targets"][target or native_target()]
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
    environment = {
        "PATH": "/usr/bin:/bin:/usr/sbin:/sbin",
        "LANG": "C.UTF-8",
        "HOME": str(root / "home"),
        "TMPDIR": str(root / "tmp"),
        "TMP": str(root / "tmp"),
        "TEMP": str(root / "tmp"),
        "USERPROFILE": str(root / "home"),
        "APPDATA": str(root / "config"),
        "LOCALAPPDATA": str(root / "data"),
        "XDG_CONFIG_HOME": str(root / "config"),
        "XDG_DATA_HOME": str(root / "data"),
        "XDG_CACHE_HOME": str(root / "cache"),
        "XDG_STATE_HOME": str(root / "state"),
        "XDG_RUNTIME_DIR": str(root / "run"),
        "PUMAS_REGISTRY_DB_PATH": str(root / "registry.db"),
        "ORT_SKIP_DOWNLOAD": "1",
    }
    if sys.platform == "win32":
        # Read the OS directory from the OS, never from an ambient override.
        buffer = ctypes.create_unicode_buffer(32768)
        package.require(
            ctypes.windll.kernel32.GetWindowsDirectoryW(buffer, len(buffer)) > 0,
            "Windows directory unavailable",
        )
        environment.update(
            SystemRoot=buffer.value,
            WINDIR=buffer.value,
            PATH=str(Path(buffer.value) / "System32"),
        )
    return environment


def windows_modules(pid):
    """Enumerate the owned process's mapped modules through the native PSAPI ABI."""
    from ctypes import wintypes

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenProcess.restype = wintypes.HANDLE
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.K32EnumProcessModulesEx.argtypes = [
        wintypes.HANDLE,
        ctypes.POINTER(wintypes.HMODULE),
        wintypes.DWORD,
        ctypes.POINTER(wintypes.DWORD),
        wintypes.DWORD,
    ]
    kernel.K32GetModuleFileNameExW.argtypes = [
        wintypes.HANDLE,
        wintypes.HMODULE,
        wintypes.LPWSTR,
        wintypes.DWORD,
    ]
    kernel.K32GetModuleFileNameExW.restype = wintypes.DWORD
    handle = kernel.OpenProcess(0x410, False, pid)
    package.require(bool(handle), "cannot observe owned Windows process")
    try:
        modules = (wintypes.HMODULE * 4096)()
        required = wintypes.DWORD()
        package.require(
            kernel.K32EnumProcessModulesEx(
                handle, modules, ctypes.sizeof(modules), ctypes.byref(required), 3
            )
            and 0 < required.value <= ctypes.sizeof(modules),
            "bounded Windows module enumeration failed",
        )
        paths = []
        for module in modules[: required.value // ctypes.sizeof(wintypes.HMODULE)]:
            name = ctypes.create_unicode_buffer(32768)
            length = kernel.K32GetModuleFileNameExW(handle, module, name, len(name))
            package.require(0 < length < len(name) - 1, "mapped module path unavailable")
            paths.append(Path(name.value).resolve())
        return paths
    finally:
        kernel.CloseHandle(handle)


def native_modules(pid, target):
    package.require(target == native_target(), "module observation requires matching host")
    if target == "windows-x86_64":
        return windows_modules(pid)
    package.require(target == "macos-arm64", "native module observer unavailable")
    observed = subprocess.run(
        ["/usr/sbin/lsof", "-a", "-p", str(pid), "-d", "txt", "-F0n"],
        capture_output=True,
        check=True,
        timeout=15,
    ).stdout
    package.require(len(observed) <= 1024 * 1024, "module observation exceeds bound")
    return [
        Path(field[1:].decode()).resolve()
        for item in observed.split(b"\0")
        if (field := item.lstrip(b"\n")).startswith(b"n/")
    ]


def stop_process(process):
    if process.poll() is None:
        try:
            process.send_signal(signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGINT)
            process.wait(timeout=20)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait(timeout=10)
            raise RuntimeError("Packaged backend required forced shutdown")
        except BaseException:
            # A failed signal (including unavailable Windows console control)
            # must fail acceptance and still dispose the process we own.
            if process.poll() is None:
                process.kill()
            process.wait(timeout=10)
            raise
    package.require(process.returncode == 0, "packaged backend did not shut down cleanly")


def loaded_runtime(pid, directory):
    """Observe the loaded packaged ORT; never claim a full dependency audit."""
    directory = Path(directory).resolve()
    target = native_target()
    selected = verify_runtime(directory, target)
    expected = directory / package.TARGETS[target]["runtime_entry"]
    if target != "linux-x86_64":
        observed = [
            path
            for path in native_modules(pid, target)
            if path.name.lower().startswith(("onnxruntime", "libonnxruntime"))
        ]
        package.require(expected in observed, "packaged ONNX runtime was not observed loaded")
        for path in observed:
            package.require(
                path.parent == directory
                and path.name.casefold() in {name.casefold() for name in selected["files"]},
                "ambient ONNX runtime mapping detected",
            )
        verify_runtime(directory, target)
        return {
            "loader_sha256": package.sha256(expected),
            "mapping": "packaged-module-path-observed",
            "native_module_paths": [str(path) for path in observed],
            "identity_observation": "native-module-path-and-current-pinned-bytes",
            "mapped_file_object_identity": "unavailable on this platform",
            "system_dependency_closure": "unqualified",
        }
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
        "identity_observation": "mapped-device-inode-and-current-pinned-bytes",
        "system_dependency_closure": "unqualified",
    }
