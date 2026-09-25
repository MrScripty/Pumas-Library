#!/usr/bin/env python3
"""Exercise a native CPU Torch install through a real, isolated pumas-rpc binary.

Usage: torch-managed-python-acceptance.py /path/to/pumas-rpc
This downloads a managed Python and Torch wheels into a disposable launcher root.
"""

import argparse
import hashlib
import json
import ntpath
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import subprocess
import sys
import tempfile
import time
import urllib.request
import unittest


TAG = "v2.14.0"
PROFILE_ID = "torch-managed-python-acceptance"
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def backend_environment(root: Path) -> dict[str, str]:
    """Remove ambient interpreter selectors before launching the backend."""
    blocked = ("PYTHON", "PIP_", "UV_", "VIRTUAL_ENV", "CONDA", "PYENV", "PDM_")
    env = {key: value for key, value in os.environ.items() if not key.upper().startswith(blocked)}
    env["PATH"] = (
        str(Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32")
        if os.name == "nt"
        else ""
    )
    env["HOME"] = str(root)
    env["USERPROFILE"] = str(root)
    env["APPDATA"] = str(root / "config")
    env["LOCALAPPDATA"] = str(root / "local-config")
    env["XDG_CONFIG_HOME"] = str(root / "config")
    env["XDG_CACHE_HOME"] = str(root / "cache")
    env["PUMAS_REGISTRY_DB_PATH"] = str(root / "registry.db")
    env["TMPDIR"] = str(root / "tmp")
    env["TMP"] = str(root / "tmp")
    env["TEMP"] = str(root / "tmp")
    return env


def wait_for_backend(process: subprocess.Popen[bytes], log_path: Path) -> str:
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        log = log_path.read_text(errors="replace")
        match = re.search(r"RPC_PORT=(\d+)", log)
        if process.poll() is not None:
            raise RuntimeError(f"Backend exited with {process.returncode}: {log[-3000:]}")
        if match:
            return f"http://127.0.0.1:{match[1]}"
        time.sleep(0.1)
    raise RuntimeError(
        f"Backend did not announce a port: {log_path.read_text(errors='replace')[-3000:]}"
    )


def rpc(base: str, method: str, params: dict | None = None, timeout: int = 120):
    request = urllib.request.Request(
        base + "/rpc",
        data=json.dumps(
            {"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}
        ).encode(),
        headers={"Content-Type": "application/json"},
    )
    with HTTP.open(request, timeout=timeout) as response:
        payload = json.load(response)
    if "error" in payload:
        raise RuntimeError(f"{method}: {payload['error']}")
    return payload["result"]


def require(condition: bool, step: str, result) -> None:
    if not condition:
        raise RuntimeError(f"{step} failed: {json.dumps(result, default=str)[:1200]}")


def canonical_path_key(path: str, *, windows: bool) -> str:
    """Compare canonical Windows paths with or without the verbatim prefix."""
    if not windows:
        return path
    if path.startswith("\\\\?\\UNC\\"):
        path = "\\\\" + path[8:]
    elif path.startswith("\\\\?\\"):
        path = path[4:]
    return ntpath.normcase(ntpath.normpath(path))


def path_is_within(path: Path | str, root: Path | str, *, windows: bool) -> bool:
    if not windows:
        return Path(path).is_relative_to(Path(root))
    child_key = canonical_path_key(str(path), windows=True)
    root_key = canonical_path_key(str(root), windows=True)
    try:
        return ntpath.commonpath((child_key, root_key)) == root_key
    except ValueError:
        return False


class WindowsCanonicalPathFixture(unittest.TestCase):
    def test_verbatim_drive_path_matches_plain_drive_path(self) -> None:
        verbatim = r"\\?\C:\managed\python\python.exe"
        plain = r"C:\managed\python\python.exe"
        self.assertEqual(
            canonical_path_key(verbatim, windows=True),
            canonical_path_key(plain, windows=True),
        )

    def test_verbatim_child_is_within_plain_depot(self) -> None:
        depot = r"C:\managed\python"
        child = r"\\?\C:\managed\python\cpython-3.14\python.exe"
        sibling = r"\\?\C:\managed\python-other\python.exe"
        self.assertTrue(path_is_within(child, depot, windows=True))
        self.assertFalse(path_is_within(sibling, depot, windows=True))
        self.assertFalse(path_is_within(r"\\?\D:\managed\python\python.exe", depot, windows=True))


class EvidenceCollectionFixture(unittest.TestCase):
    def test_copies_only_selected_install_reports_and_log(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "launcher"
            output = Path(temporary) / "evidence"
            runtime = root / "torch-versions" / TAG
            runtime.mkdir(parents=True)
            (root / "rpc.log").write_text("backend log", encoding="utf-8")
            for name in INSTALL_REPORTS:
                (runtime / name).write_text(name, encoding="utf-8")
            (runtime / "wheel.whl").write_text("excluded", encoding="utf-8")
            (runtime / "venv").mkdir()
            result = {"success": True, "cpython_version": "3.14.0"}
            collect_evidence(root, output, result, install_succeeded=True)
            self.assertEqual(
                {item.name for item in output.iterdir()},
                {"rpc.log", "acceptance.json", *INSTALL_REPORTS},
            )
            self.assertEqual(json.loads((output / "acceptance.json").read_text()), result)

    def test_failure_retains_log_without_install_reports(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "launcher"
            output = Path(temporary) / "evidence"
            runtime = root / "torch-versions" / TAG
            runtime.mkdir(parents=True)
            (root / "rpc.log").write_text("failure log", encoding="utf-8")
            (runtime / "runtime.json").write_text("partial", encoding="utf-8")
            collect_evidence(root, output, {"success": False}, install_succeeded=False)
            self.assertEqual(
                {item.name for item in output.iterdir()}, {"rpc.log", "acceptance.json"}
            )

    def test_existing_destination_is_not_modified(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "launcher"
            output = Path(temporary) / "evidence"
            root.mkdir()
            output.mkdir()
            sentinel = output / "acceptance.json"
            sentinel.write_text("previous run", encoding="utf-8")
            with self.assertRaises(FileExistsError):
                collect_evidence(root, output, {"success": False}, install_succeeded=False)
            self.assertEqual(sentinel.read_text(encoding="utf-8"), "previous run")


class LauncherRootFinalizationFixture(unittest.TestCase):
    def test_removes_root_only_after_safe_completion(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            parent = Path(temporary)
            for label, cleanup_safe in (("success", True), ("failure", False), ("forced", False)):
                root = parent / label
                root.mkdir()
                (root / "rpc.log").write_text(label, encoding="utf-8")
                finalize_launcher_root(root, cleanup_safe=cleanup_safe)
                self.assertEqual(root.exists(), not cleanup_safe, label)


INSTALL_REPORTS = (
    "runtime.json",
    "resolution.json",
    "pip-resolution.json",
    "probe-results.json",
)


def collect_evidence(root: Path, output: Path, result: dict, *, install_succeeded: bool) -> None:
    output.mkdir(parents=True, exist_ok=False)
    log = root / "rpc.log"
    if log.is_file():
        shutil.copyfile(log, output / log.name)
    if install_succeeded:
        runtime = root / "torch-versions" / TAG
        for name in INSTALL_REPORTS:
            report = runtime / name
            if report.is_file():
                shutil.copyfile(report, output / name)
    (output / "acceptance.json").write_text(
        json.dumps(result, indent=2, sort_keys=True) + "\n", encoding="utf-8"
    )


def finalize_launcher_root(root: Path, *, cleanup_safe: bool) -> None:
    if cleanup_safe:
        shutil.rmtree(root)


def wait_for_install(base: str, deadline_seconds: int = 2700) -> dict:
    deadline = time.monotonic() + deadline_seconds
    last = None
    while time.monotonic() < deadline:
        progress = rpc(base, "get_installation_progress", {"appId": "torch"})
        if isinstance(progress, dict):
            last = progress
            require(progress.get("tag") == TAG, "installation tag", progress)
            if progress.get("success") is False or progress.get("error"):
                raise RuntimeError(f"Torch installation failed: {json.dumps(progress)[:1200]}")
            if progress.get("success") is True and progress.get("completedAt"):
                return progress
        time.sleep(2)
    raise RuntimeError(f"Torch installation timed out: {json.dumps(last)[:1200]}")


def managed_python_evidence(root: Path, preview: dict) -> dict:
    recipe_path = root / "torch-versions" / TAG / "runtime.json"
    with recipe_path.open(encoding="utf-8") as recipe_file:
        recipe = json.load(recipe_file)
    managed = recipe.get("managed_python", {})
    provider = managed.get("provider", {})
    distribution = managed.get("distribution", {})
    executable = managed.get("executable", {})
    version = provider.get("version")
    archive_hash = provider.get("archiveSha256")
    require(
        provider.get("name") == "uv"
        and isinstance(version, str)
        and re.fullmatch(r"\d+\.\d+\.\d+", version) is not None
        and isinstance(archive_hash, str)
        and re.fullmatch(r"[0-9a-f]{64}", archive_hash) is not None,
        "managed Python provider",
        provider,
    )
    python_version = distribution.get("version")
    target = distribution.get("targetTriple")
    require(
        distribution.get("implementation") == "CPython"
        and isinstance(python_version, str)
        and re.fullmatch(r"\d+\.\d+\.\d+", python_version) is not None
        and "python" + ".".join(python_version.split(".")[:2]) == preview["python"]
        and isinstance(target, str)
        and bool(target),
        "managed CPython distribution",
        distribution,
    )
    path_value = executable.get("path")
    recorded_hash = executable.get("sha256")
    require(
        isinstance(path_value, str)
        and isinstance(recorded_hash, str)
        and re.fullmatch(r"[0-9a-f]{64}", recorded_hash) is not None,
        "managed Python executable record",
        executable,
    )
    path = Path(path_value)
    managed_root = (root / "launcher-data" / "managed-python" / "python").resolve(strict=True)
    canonical = path.resolve(strict=True)
    require(
        path.is_absolute()
        and canonical_path_key(str(path), windows=os.name == "nt")
        == canonical_path_key(str(canonical), windows=os.name == "nt")
        and canonical.is_file()
        and path_is_within(canonical, managed_root, windows=os.name == "nt"),
        "managed Python executable location",
        executable,
    )
    digest = hashlib.sha256()
    with canonical.open("rb") as python_file:
        for chunk in iter(lambda: python_file.read(1024 * 1024), b""):
            digest.update(chunk)
    require(digest.hexdigest() == recorded_hash, "managed Python executable hash", executable)
    runtime = root / "torch-versions" / TAG
    venv_python = (
        runtime / "venv" / "Scripts" / "python.exe"
        if os.name == "nt"
        else runtime / "venv" / "bin" / "python"
    )
    require(venv_python.is_file(), "installed venv Python", str(venv_python))
    identity = subprocess.run(
        [
            str(venv_python),
            "-I",
            "-c",
            "import json,platform,sys; print(json.dumps({"
            "'implementation':platform.python_implementation(),"
            "'version':platform.python_version(),"
            "'base_executable':sys._base_executable}))",
        ],
        cwd=root,
        env=backend_environment(root),
        capture_output=True,
        text=True,
        timeout=30,
        check=True,
    )
    observed = json.loads(identity.stdout)
    base_path = Path(observed["base_executable"])
    require(
        observed.get("implementation") == "CPython"
        and observed.get("version") == python_version
        and base_path.is_absolute()
        and canonical_path_key(str(base_path.resolve(strict=True)), windows=os.name == "nt")
        == canonical_path_key(str(canonical), windows=os.name == "nt"),
        "installed venv interpreter identity",
        observed,
    )
    return {
        "cpython_version": python_version,
        "provider": "uv",
        "provider_version": version,
        "provider_archive_sha256": archive_hash,
        "target": target,
    }


def exercise(base: str, root: Path, state: dict) -> dict:
    options = rpc(base, "get_torch_release_options", {"tag": TAG}, timeout=180)
    require(
        options.get("tag") == TAG and options.get("completeScan") is True,
        "release options",
        options,
    )
    require(
        options.get("status") == "matches"
        and any(item.get("build") == "cpu" for item in options.get("combinations", [])),
        "CPU release combination",
        options,
    )

    preview_result = rpc(
        base,
        "preview_torch_runtime",
        {"tag": TAG, "build": "cpu", "python": "auto", "adapter": "none"},
        timeout=900,
    )
    require(preview_result.get("status") == "resolved", "Torch preview", preview_result)
    preview = preview_result["preview"]
    require(
        preview.get("tag") == TAG
        and preview.get("build") == "cpu"
        and preview.get("adapter") == "none"
        and preview.get("python", "").startswith("python3.")
        and preview.get("qualification") in {"qualified", "unverified"}
        and isinstance(preview.get("previewId"), str)
        and bool(preview["previewId"]),
        "qualified managed Python preview",
        preview,
    )
    started = rpc(
        base,
        "install_version",
        {"appId": "torch", "tag": TAG, "previewId": preview["previewId"]},
    )
    require(started.get("success") is True, "installation start", started)
    wait_for_install(base)
    state["install_succeeded"] = True
    python_evidence = managed_python_evidence(root, preview)

    probe = rpc(base, "get_torch_runtime_probe", {"tag": TAG}, timeout=60)
    capabilities = probe.get("capabilities", {})
    require(
        probe.get("status") == "passed"
        and probe.get("core_status") == "passed"
        and probe.get("stale") is False
        and capabilities.get("torch_import", {}).get("status") == "passed"
        and capabilities.get("torch_import", {}).get("version", "").split("+")[0] == "2.14.0"
        and capabilities.get("cpu_tensor", {}).get("status") == "passed",
        "installed Torch identity and CPU probe",
        probe,
    )
    require(
        capabilities.get("sidecar_app", {}).get("status") == "passed",
        "installed sidecar dependencies",
        probe,
    )

    selected = rpc(base, "switch_version", {"appId": "torch", "tag": TAG})
    require(selected.get("success") is True, "Torch selection", selected)
    profile = {
        "profile_id": PROFILE_ID,
        "provider": "torch",
        "provider_mode": "torch_serve",
        "management_mode": "managed",
        "name": "Native CPU acceptance",
        "enabled": True,
        "device": {"mode": "cpu"},
    }
    upserted = rpc(base, "upsert_runtime_profile", {"profile": profile})
    require(upserted.get("success") is True, "managed CPU profile", upserted)
    trial = rpc(base, "trial_torch_runtime", {"tag": TAG, "profileId": PROFILE_ID}, timeout=100)
    require(
        trial.get("success") is True
        and trial.get("startupStatus") == "passed"
        and trial.get("healthStatus") == "passed"
        and trial.get("protocol") == 3
        and trial.get("startedByTrial") is True
        and isinstance(trial.get("generation"), str)
        and trial["generation"].isdigit(),
        "Torch sidecar trial",
        trial,
    )
    stopped = rpc(
        base,
        "stop_runtime_profile_if_generation",
        {"profileId": PROFILE_ID, "generation": trial["generation"]},
    )
    require(
        stopped.get("success") is True and stopped.get("stopped") is True,
        "generation stop",
        stopped,
    )
    return {
        "release": TAG,
        "build": "cpu",
        "python": preview["python"],
        **python_evidence,
        "artifact_count": len(preview["artifacts"]),
        "platform": sys.platform,
        "architecture": platform.machine(),
        "probe": "passed",
        "trial": "passed",
        "sidecar_protocol": trial["protocol"],
        "sidecar_generation": trial["generation"],
        "stop_result": stopped["stopped"],
        "stopped": True,
    }


def stop_backend(process: subprocess.Popen[bytes]) -> None:
    if process.poll() is not None:
        require(process.returncode == 0, "backend exit", {"returncode": process.returncode})
        return
    if os.name == "nt":
        process.send_signal(signal.CTRL_BREAK_EVENT)
    else:
        process.send_signal(signal.SIGINT)
    try:
        process.wait(timeout=30)
    except subprocess.TimeoutExpired as error:
        process.kill()
        process.wait(timeout=10)
        raise RuntimeError("Backend required forced shutdown") from error
    require(process.returncode == 0, "backend shutdown", {"returncode": process.returncode})


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", nargs="?", type=Path)
    parser.add_argument("--evidence-dir", type=Path, help="retain concise acceptance reports")
    parser.add_argument("--self-test", action="store_true", help="run script fixtures")
    args = parser.parse_args()
    if args.self_test:
        suite = unittest.TestSuite(
            unittest.defaultTestLoader.loadTestsFromTestCase(fixture)
            for fixture in (
                WindowsCanonicalPathFixture,
                EvidenceCollectionFixture,
                LauncherRootFinalizationFixture,
            )
        )
        result = unittest.TestResult()
        suite.run(result)
        require(result.wasSuccessful(), "script fixtures", result.errors + result.failures)
        print(f"{result.testsRun} script fixtures passed")
        return
    if args.binary is None:
        parser.error("a pumas-rpc binary is required")
    binary = args.binary.resolve()
    if binary.is_dir():
        binary /= "pumas-rpc.exe" if os.name == "nt" else "pumas-rpc"
    if not binary.is_file():
        parser.error(f"Backend binary does not exist: {binary}")

    evidence_dir = args.evidence_dir.absolute() if args.evidence_dir is not None else None
    if evidence_dir is not None and os.path.lexists(evidence_dir):
        parser.error(f"Evidence directory already exists: {evidence_dir}")
    root = Path(tempfile.mkdtemp(prefix="pumas-torch-native-cpu-"))
    state = {"install_succeeded": False}
    acceptance = {"success": False}
    success_ready = False
    try:
        (root / "tmp").mkdir()
        log_path = root / "rpc.log"
        creationflags = subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0
        with log_path.open("wb") as log:
            process = subprocess.Popen(
                [str(binary), "--launcher-root", str(root), "--port", "0"],
                cwd=root,
                env=backend_environment(root),
                stdout=log,
                stderr=subprocess.STDOUT,
                creationflags=creationflags,
            )
            base = None
            try:
                base = wait_for_backend(process, log_path)
                result = exercise(base, root, state)
            finally:
                try:
                    if base is not None:
                        require(
                            process.poll() is None,
                            "backend remained live before shutdown",
                            {"returncode": process.returncode},
                        )
                        shutdown = rpc(base, "shutdown", timeout=60)
                        require(
                            shutdown.get("status") == "shutting_down"
                            and not shutdown.get("errors"),
                            "managed profile shutdown",
                            shutdown,
                        )
                finally:
                    stop_backend(process)
        acceptance = {"success": True, "cleanup_safe": True, **result}
        success_ready = True
    finally:
        if not success_ready:
            acceptance = {
                "success": False,
                "cleanup_safe": False,
                "retained_root": str(root),
            }
        try:
            if evidence_dir is not None:
                collect_evidence(
                    root,
                    evidence_dir,
                    acceptance,
                    install_succeeded=state["install_succeeded"],
                )
            if success_ready:
                print(json.dumps(acceptance), flush=True)
        except BaseException:
            success_ready = False
            if evidence_dir is not None and evidence_dir.is_dir():
                try:
                    (evidence_dir / "acceptance.json").write_text(
                        json.dumps(
                            {"success": False, "cleanup_safe": False, "retained_root": str(root)},
                            indent=2,
                            sort_keys=True,
                        )
                        + "\n",
                        encoding="utf-8",
                    )
                except OSError:
                    pass
            raise
        finally:
            if not success_ready:
                print(f"Retained launcher root: {root}", file=sys.stderr, flush=True)
            finalize_launcher_root(root, cleanup_safe=success_ready)


if __name__ == "__main__":
    main()
