"""Packaged RPC inference: Pumas-installed runtime, OS observations and child disposal.

Pumas packages bundle no inference runtime. The verifier installs ONNX Runtime
through the extracted RPC's own version manager, then observes which library
the owned process tree actually mapped.
"""

import ctypes
import math
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import sys
import time

import headless_inference as package

RUNTIME_APP_ID = "onnx-runtime"
# Pumas installs versions under <launcher-root>/<app versions directory>/<tag>.
RUNTIME_VERSIONS_DIRECTORY = "onnx-runtime-versions"
CPU_TAG = re.compile(r"v([0-9]+)\.([0-9]+)\.([0-9]+)\+cpu\Z")
RUNTIME_LIBRARY_PREFIXES = ("onnxruntime", "libonnxruntime")
# Native runtimes Pumas installs through its version manager. None may sit
# beside a packaged RPC; Pumas's own sidecar sources are embedded in the binary.
BUNDLED_RUNTIME_PREFIXES = (
    *RUNTIME_LIBRARY_PREFIXES,
    "llama",
    "libllama",
    "ggml",
    "libggml",
    "torch",
    "libtorch",
    "c10",
    "libc10",
)
PROFILE_ID = "onnx-verification"
MODEL_ALIAS = "verification-nomic"
EMBEDDING_DIMENSIONS = 256


def native_target():
    system, machine = platform.system(), platform.machine().lower()
    if system == "Linux" and machine in {"x86_64", "amd64"}:
        return "linux-x86_64"
    if system == "Windows" and machine in {"x86_64", "amd64"}:
        return "windows-x86_64"
    if system == "Darwin" and machine in {"arm64", "aarch64"}:
        return "macos-arm64"
    raise ValueError("packaged inference requires a supported native host")


def require_no_bundled_runtime(directory):
    """Refuse a runtime library shipped beside the RPC; Pumas must install it."""
    bundled = sorted(
        path.name
        for path in Path(directory).iterdir()
        if path.name.lower().startswith(BUNDLED_RUNTIME_PREFIXES)
    )
    package.require(
        not bundled,
        f"runtime files must not be bundled beside the RPC; Pumas installs runtimes: {bundled}",
    )


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


def select_cpu_release(listing):
    """Newest stable `+cpu` ONNX Runtime tag Pumas offers for this host."""
    package.require(type(listing) is dict, f"unexpected ONNX Runtime listing: {listing!r}")
    package.require(
        listing.get("rate_limited") is not True,
        "GitHub rate-limited the ONNX Runtime release listing; retry after "
        f"{listing.get('retry_after_secs')} seconds",
    )
    versions = listing.get("versions")
    package.require(
        listing.get("success") is True and type(versions) is list,
        f"Pumas did not list ONNX Runtime releases: {listing.get('error') or listing!r}",
    )
    candidates = {}
    for release in versions:
        tag = release.get("tagName") if type(release) is dict else None
        match = CPU_TAG.fullmatch(tag) if isinstance(tag, str) else None
        if match and release.get("prerelease") is not True:
            candidates[tuple(int(part) for part in match.groups())] = tag
    package.require(
        candidates,
        "Pumas offers no stable +cpu ONNX Runtime release for this host; listed: "
        f"{[item.get('tagName') for item in versions[:20] if type(item) is dict]}",
    )
    return candidates[max(candidates)]


def installed_versions(rpc):
    result = rpc("get_installed_versions", {"app_id": RUNTIME_APP_ID})
    package.require(
        type(result) is dict
        and result.get("success") is True
        and type(result.get("versions")) is list,
        f"Pumas did not report installed ONNX Runtime versions: {result!r}",
    )
    return result["versions"]


def install_runtime(rpc, tag, timeout=600, interval=1.0, clock=time.monotonic, sleep=time.sleep):
    """Install `tag` through the RPC's version manager and wait for completion."""
    started = rpc("install_version", {"app_id": RUNTIME_APP_ID, "tag": tag})
    package.require(
        type(started) is dict and started.get("success") is True,
        f"Pumas refused to install ONNX Runtime {tag}: "
        f"{(started or {}).get('error') or 'no reason reported'}",
    )
    deadline = clock() + timeout
    last = None
    while True:
        progress = rpc("get_installation_progress", {"app_id": RUNTIME_APP_ID})
        if progress is not None:
            last = progress
            package.require(
                progress.get("tag") == tag,
                f"another ONNX Runtime installation is in progress: {progress.get('tag')}",
            )
            if progress.get("completedAt"):
                package.require(
                    progress.get("success") is True,
                    f"ONNX Runtime {tag} installation failed: "
                    f"{progress.get('error') or 'no reason reported'} "
                    f"(log: {progress.get('logPath')})",
                )
                break
        elif tag in installed_versions(rpc):
            # A finished installation's progress record may already be cleared.
            break
        package.require(
            clock() < deadline,
            f"ONNX Runtime {tag} installation did not finish within {timeout} seconds; "
            f"last stage {(last or {}).get('stage')}, "
            f"overall {(last or {}).get('overallProgress')}",
        )
        sleep(interval)
    installed = installed_versions(rpc)
    package.require(
        tag in installed,
        f"ONNX Runtime {tag} reported complete but is not installed; installed: {installed}",
    )
    return {"app_id": RUNTIME_APP_ID, "tag": tag, "installed_versions": installed}


def runtime_profile(tag):
    """A managed CPU ONNX profile pinned to the installed version `tag`."""
    return {
        "profile_id": PROFILE_ID,
        "provider": "onnx_runtime",
        "provider_mode": "onnx_serve",
        "management_mode": "managed",
        "name": "Installer verification",
        "device": {"mode": "cpu"},
        "runtime_version": tag,
    }


def exercise_installed_runtime(rpc, post, model_dir, observe, log=print):
    """Install ORT through Pumas, serve the fixture on a pinned profile, embed, unload.

    `rpc(method, params)` returns a JSON-RPC result; `post(path, body)` returns
    the HTTP JSON body; `observe(tag)` returns evidence of the loaded runtime.
    """
    tag = select_cpu_release(
        rpc("get_available_versions", {"app_id": RUNTIME_APP_ID, "force_refresh": True})
    )
    log("runtime", tag, flush=True)
    install = install_runtime(rpc, tag)
    profile = rpc("upsert_runtime_profile", {"profile": runtime_profile(tag)})
    package.require(
        type(profile) is dict and profile.get("success") is True,
        f"Pumas refused the ONNX runtime profile for {tag}: {(profile or {}).get('error')}",
    )
    launched = rpc("launch_runtime_profile", {"profile_id": PROFILE_ID})
    package.require(
        type(launched) is dict and launched.get("success") is True,
        f"ONNX runtime profile for {tag} did not launch: {(launched or {}).get('error')}",
    )
    imported = rpc(
        "import_model_in_place",
        {
            "model_dir": str(model_dir),
            "official_name": "Nomic fixture",
            "family": "nomic_bert",
            "model_type": "embedding",
        },
    )
    log("import", str(imported)[:1000], flush=True)
    package.require(imported.get("success") is True, f"Import failed: {imported}")
    model_id = imported["model_id"]
    loaded = rpc(
        "serve_model",
        {
            "request": {
                "model_id": model_id,
                "config": {
                    "provider": "onnx_runtime",
                    "profile_id": PROFILE_ID,
                    "device_mode": "cpu",
                    "keep_loaded": True,
                    "model_alias": MODEL_ALIAS,
                },
            }
        },
    )
    package.require(loaded.get("loaded") is True, f"Load failed: {loaded}")
    embedding = post(
        "/v1/embeddings",
        {
            "model": MODEL_ALIAS,
            "input": ["search_query: hello world"],
            "dimensions": EMBEDDING_DIMENSIONS,
        },
    )
    vector = embedding["data"][0]["embedding"]
    package.require(
        len(vector) == EMBEDDING_DIMENSIONS
        and all(type(x) in (int, float) and math.isfinite(x) for x in vector),
        f"Embedding must contain {EMBEDDING_DIMENSIONS} finite numeric values",
    )
    runtime_evidence = observe(tag)
    unloaded = rpc(
        "unserve_model",
        {
            "request": {
                "model_id": model_id,
                "provider": "onnx_runtime",
                "profile_id": PROFILE_ID,
                "model_alias": MODEL_ALIAS,
            }
        },
    )
    package.require(unloaded.get("unloaded") is True, f"Unload failed: {unloaded}")
    return {
        "runtime_install": "passed",
        "runtime_version": tag,
        "installed_versions": install["installed_versions"],
        "runtime": runtime_evidence,
        "import": "passed",
        "load": "passed",
        "embedding_dimensions": len(vector),
        "finite": True,
        "unload": "passed",
    }


def without_verbatim_prefix(path):
    """Windows reports modules loaded through a canonical path as `\\\\?\\C:\\...`."""
    if path.startswith("\\\\?\\UNC\\"):
        return "\\\\" + path[len("\\\\?\\UNC\\") :]
    if path.startswith("\\\\?\\"):
        return path[len("\\\\?\\") :]
    return path


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
    package.require(bool(handle), f"cannot observe owned Windows process {pid}")
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
            paths.append(Path(without_verbatim_prefix(name.value)).resolve())
        return paths
    finally:
        kernel.CloseHandle(handle)


def windows_parent_map():
    """PID -> parent PID for every process, through a Toolhelp snapshot."""
    from ctypes import wintypes

    class ProcessEntry(ctypes.Structure):
        _fields_ = [
            ("dwSize", wintypes.DWORD),
            ("cntUsage", wintypes.DWORD),
            ("th32ProcessID", wintypes.DWORD),
            ("th32DefaultHeapID", ctypes.c_size_t),
            ("th32ModuleID", wintypes.DWORD),
            ("cntThreads", wintypes.DWORD),
            ("th32ParentProcessID", wintypes.DWORD),
            ("pcPriClassBase", ctypes.c_long),
            ("dwFlags", wintypes.DWORD),
            ("szExeFile", ctypes.c_wchar * 260),
        ]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel.Process32FirstW.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)]
    kernel.Process32NextW.argtypes = [wintypes.HANDLE, ctypes.POINTER(ProcessEntry)]
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    snapshot = kernel.CreateToolhelp32Snapshot(0x2, 0)  # TH32CS_SNAPPROCESS
    package.require(
        snapshot and snapshot != wintypes.HANDLE(-1).value,
        f"Windows process snapshot unavailable: error {ctypes.get_last_error()}",
    )
    try:
        entry = ProcessEntry()
        entry.dwSize = ctypes.sizeof(ProcessEntry)
        parents = {}
        more = kernel.Process32FirstW(snapshot, ctypes.byref(entry))
        while more:
            parents[entry.th32ProcessID] = entry.th32ParentProcessID
            more = kernel.Process32NextW(snapshot, ctypes.byref(entry))
        return parents
    finally:
        kernel.CloseHandle(snapshot)


def linux_parent_map(proc=Path("/proc")):
    parents = {}
    for entry in proc.iterdir():
        if not entry.name.isdigit():
            continue
        try:
            # `pid (comm) state ppid ...`; comm may contain spaces and parentheses.
            fields = (entry / "stat").read_text().rpartition(")")[2].split()
        except (FileNotFoundError, ProcessLookupError, PermissionError):
            continue
        parents[int(entry.name)] = int(fields[1])
    return parents


def ps_parent_map():
    observed = subprocess.run(
        ["/bin/ps", "-A", "-o", "pid=,ppid="], capture_output=True, check=True, timeout=15
    ).stdout
    package.require(len(observed) <= 1024 * 1024, "process listing exceeds bound")
    return {
        int(pid): int(ppid)
        for pid, ppid in (line.split() for line in observed.decode().splitlines() if line.strip())
    }


def parent_map(target):
    if target == "linux-x86_64":
        return linux_parent_map()
    if target == "windows-x86_64":
        return windows_parent_map()
    return ps_parent_map()


def process_tree(root, parents):
    """`root` and every descendant in a PID -> parent PID map."""
    tree, frontier = {root}, [root]
    while frontier:
        parent = frontier.pop()
        for pid, owner in parents.items():
            if owner == parent and pid not in tree and pid != owner:
                tree.add(pid)
                frontier.append(pid)
    return tree


SHARED_LIBRARY = re.compile(r"(\.so(\.[0-9]+)*|\.dylib|\.dll)\Z")


def is_runtime_library(path):
    # ONNX Runtime also maps its own cache files (for example
    # onnxruntime.db-shm in the user cache); only shared libraries count.
    name = Path(path).name.lower()
    return name.startswith(RUNTIME_LIBRARY_PREFIXES) and bool(SHARED_LIBRARY.search(name))


def linux_mapped_runtimes(pid, proc=Path("/proc")):
    """ORT mappings of one process, each bound to its mapped device and inode."""
    mapped = set()
    for line in (proc / str(pid) / "maps").read_text().splitlines():
        fields = line.split(maxsplit=5)
        if len(fields) < 6 or not is_runtime_library(fields[5]):
            continue
        path = Path(fields[5])
        actual = path.stat()
        major, minor = (int(value, 16) for value in fields[3].split(":"))
        package.require(
            int(fields[4]) == actual.st_ino
            and (major, minor) == (os.major(actual.st_dev), os.minor(actual.st_dev)),
            f"mapped ONNX Runtime file identity changed: {path}",
        )
        mapped.add(path)
    return mapped


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


def mapped_runtimes(pid, target):
    if target == "linux-x86_64":
        return linux_mapped_runtimes(pid)
    return {path for path in native_modules(pid, target) if is_runtime_library(path)}


def installed_runtime(pid, launcher_root, tag, target=None):
    """Observe that the owned process tree runs the ORT Pumas installed as `tag`.

    Every mapped ONNX Runtime library in the RPC process and its descendants (an
    out-of-process ONNX host included) must reside in the version directory Pumas
    installed; an ambient or bundled library fails. This is not a complete audit
    of every loaded system dependency.
    """
    target = target or native_target()
    version_dir = (Path(launcher_root) / RUNTIME_VERSIONS_DIRECTORY / tag).resolve()
    package.require(
        version_dir.is_dir(), f"Pumas-installed ONNX Runtime {tag} is missing at {version_dir}"
    )
    observed = {}
    for process in sorted(process_tree(pid, parent_map(target))):
        try:
            paths = mapped_runtimes(process, target)
        except (FileNotFoundError, ProcessLookupError):
            continue  # A short-lived child exited between listing and observation.
        for path in paths:
            observed.setdefault(Path(path), set()).add(process)
    package.require(
        observed,
        f"no ONNX Runtime library was observed loaded by the owned Pumas process tree of {pid}",
    )
    for path in observed:
        package.require(
            Path(path).resolve().is_relative_to(version_dir),
            f"ONNX Runtime loaded from outside the Pumas-installed {tag} directory: {path}",
        )
    return {
        "mapping": "pumas-installed-runtime-observed",
        "runtime_version": tag,
        "libraries": {
            Path(path).resolve().relative_to(version_dir).as_posix(): package.sha256(path)
            for path in observed
        },
        "observing_process_count": len(set().union(*observed.values())),
        "identity_observation": (
            "mapped-device-inode" if target == "linux-x86_64" else "native-module-path"
        ),
        "system_dependency_closure": "unqualified",
    }


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
