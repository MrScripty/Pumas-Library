#!/usr/bin/env python3
"""Exercise a native CPU Torch install through a real, isolated pumas-rpc binary.

Usage: torch-managed-python-acceptance.py /path/to/pumas-rpc
This downloads a managed Python and Torch wheels into a disposable launcher root.
"""

import argparse
from contextlib import contextmanager
import errno
import hashlib
import io
import json
import ntpath
import os
from pathlib import Path
import platform
import re
import selectors
import shutil
import signal
import subprocess
import sys
import tarfile
import tempfile
import threading
import time
from urllib.parse import unquote, urlsplit
import urllib.request
import unittest
from unittest.mock import patch


TAG = "v2.14.0"
PROFILE_ID = "torch-managed-python-acceptance"
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))
REVIEWED_PBS_RELEASE = "20260901"
MAPPING_AUTHORITY_URL = (
    "https://github.com/astral-sh/python-build-standalone/blob/20260901/src/release.rs"
)
FULL_FLAVORS = {
    "x86_64-unknown-linux-gnu": "pgo+lto",
    "aarch64-apple-darwin": "pgo+lto",
    "x86_64-pc-windows-msvc": "pgo",
}
MAX_RELEASE_JSON_BYTES = 8 * 1024 * 1024
MAX_FULL_ARCHIVE_BYTES = 200 * 1024 * 1024
MAX_LICENSE_BYTES = 16 * 1024 * 1024
MAX_LICENSE_TOTAL_BYTES = 64 * 1024 * 1024
MAX_ARCHIVE_MEMBERS = 200_000
MAX_UNCOMPRESSED_TAR_STREAM_BYTES = 2 * 1024 * 1024 * 1024
DOWNLOAD_DEADLINE_SECONDS = 300
RELEASE_OPTIONS_RPC_TIMEOUT_SECONDS = 900
VERSION_PREFLIGHT_MAX_ATTEMPTS = 3
VERSION_PREFLIGHT_BUDGET_SECONDS = 1800
VERSION_PREFLIGHT_MAX_DELAY_SECONDS = 900


class HTTPSOnlyRedirectHandler(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, fp, code, msg, headers, newurl):
        require(urlsplit(newurl).scheme == "https", "HTTPS-only download redirect", newurl)
        return super().redirect_request(request, fp, code, msg, headers, newurl)


DOWNLOAD_HTTP = urllib.request.build_opener(HTTPSOnlyRedirectHandler())


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


def rpc(base: str, method: str, params: dict | None = None, timeout: float = 120):
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
        if isinstance(result, dict):
            priority = ("error", "reason", "code", "status", "completeScan", "tag", "issues")
            summary = {}
            for key in priority:
                if key not in result:
                    continue
                value = result[key]
                if key == "issues" and isinstance(value, list):
                    value = {
                        "sample": [
                            issue[:300] if isinstance(issue, str) else "<non-string issue>"
                            for issue in value[:3]
                        ],
                        "omitted": max(0, len(value) - 3),
                    }
                elif isinstance(value, str):
                    value = value[:300]
                summary[key] = value
            result = {
                **summary,
                **{key: value for key, value in result.items() if key not in priority},
            }
        raise RuntimeError(f"{step} failed: {json.dumps(result, default=str)[:1200]}")


def preflight_torch_release(
    base: str,
    evidence: dict,
    *,
    rpc_call=None,
    clock=time.monotonic,
    sleep=time.sleep,
) -> None:
    """Wait only for an explicit release-list rate limit before scanning assets."""
    if rpc_call is None:
        rpc_call = rpc
    started = clock()
    attempts = []
    evidence["version_preflight"] = {"attempts": attempts, "elapsed_seconds": 0}

    def finish(status: str) -> None:
        evidence["version_preflight"]["elapsed_seconds"] = round(clock() - started, 3)
        evidence["version_preflight"]["status"] = status

    def fail(reason: str) -> None:
        finish(reason)
        raise RuntimeError(
            "Torch version preflight failed: "
            + json.dumps(evidence["version_preflight"], sort_keys=True)
        )

    for number in range(1, VERSION_PREFLIGHT_MAX_ATTEMPTS + 1):
        elapsed = clock() - started
        if elapsed >= VERSION_PREFLIGHT_BUDGET_SECONDS:
            fail("time_budget_exhausted")
        # The per-call timeout cannot exceed the remaining preflight budget.
        timeout = min(120, VERSION_PREFLIGHT_BUDGET_SECONDS - elapsed)
        attempt = {"number": number}
        attempts.append(attempt)
        try:
            result = rpc_call(
                base,
                "get_available_versions",
                {"app_id": "torch", "force_refresh": False},
                timeout=timeout,
            )
        except Exception:
            attempt["status"] = "rpc_failure"
            fail("rpc_failure")
        if clock() - started > VERSION_PREFLIGHT_BUDGET_SECONDS:
            attempt["status"] = "time_budget_exhausted"
            fail("time_budget_exhausted")
        if (
            isinstance(result, dict)
            and result.get("success") is True
            and "rate_limited" not in result
            and "retry_after_secs" not in result
        ):
            versions = result.get("versions")
            if not isinstance(versions, list) or not all(
                isinstance(item, dict) and isinstance(item.get("tagName"), str) for item in versions
            ):
                attempt["status"] = "malformed_listing"
                fail("malformed_listing")
            if not any(item["tagName"] == TAG for item in versions):
                attempt["status"] = "requested_tag_missing"
                fail("requested_tag_missing")
            attempt["status"] = "requested_tag_found"
            finish("passed")
            return
        if not (
            isinstance(result, dict)
            and result.get("success") is False
            and result.get("rate_limited") is True
        ):
            attempt["status"] = "unexpected_outcome"
            fail("unexpected_outcome")
        delay = result.get("retry_after_secs")
        if type(delay) is not int or delay < 0 or delay > VERSION_PREFLIGHT_MAX_DELAY_SECONDS:
            attempt["status"] = "invalid_retry_delay"
            fail("invalid_retry_delay")
        attempt.update(status="rate_limited", retry_after_secs=delay)
        if number == VERSION_PREFLIGHT_MAX_ATTEMPTS:
            fail("attempts_exhausted")
        if clock() - started + delay >= VERSION_PREFLIGHT_BUDGET_SECONDS:
            fail("time_budget_exhausted")
        try:
            sleep(delay)
        except Exception:
            attempt["status"] = "sleep_failure"
            fail("sleep_failure")


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


class FailureSummaryFixture(unittest.TestCase):
    def test_top_level_error_is_visible_before_bulky_release_options(self) -> None:
        result = {
            "checkedChannels": [{"channel": "cpu", "detail": "x" * 3000}],
            "combinations": [{"build": "cpu", "detail": "y" * 3000}],
            "status": "error",
            "code": "release_discovery_failed",
            "reason": "rate_limited",
            "error": "GitHub release discovery rate limited",
            "completeScan": False,
            "tag": TAG,
            "issues": ["GitHub release asset scan stopped before all channels were checked"],
        }
        with self.assertRaises(RuntimeError) as failure:
            require(False, "release options", result)
        message = str(failure.exception)
        self.assertIn("GitHub release discovery rate limited", message)
        self.assertIn("release_discovery_failed", message)
        self.assertIn("rate_limited", message)
        self.assertIn('"completeScan": false', message)
        self.assertIn(TAG, message)
        self.assertIn("GitHub release asset scan stopped", message)
        self.assertLess(message.index('"error"'), message.index('"checkedChannels"'))
        self.assertLessEqual(len(message), len("release options failed: ") + 1200)


class VersionPreflightFixture(unittest.TestCase):
    @staticmethod
    def listing(tag: str = TAG) -> dict:
        return {"success": True, "versions": [{"tagName": tag}]}

    @staticmethod
    def limited(delay) -> dict:
        return {
            "success": False,
            "rate_limited": True,
            "retry_after_secs": delay,
            "error": "untrusted detail must not appear in diagnostics",
        }

    def run_preflight(self, responses: list) -> tuple[dict, list, list]:
        now = [0]
        sleeps = []
        calls = []
        state = {}
        responses = iter(responses)

        def fake_rpc(base, method, params, timeout):
            calls.append((base, method, params, timeout))
            return next(responses)

        def fake_sleep(seconds):
            sleeps.append(seconds)
            now[0] += seconds

        preflight_torch_release(
            "http://127.0.0.1:1",
            state,
            rpc_call=fake_rpc,
            clock=lambda: now[0],
            sleep=fake_sleep,
        )
        return state["version_preflight"], sleeps, calls

    def test_success_first_attempt(self) -> None:
        evidence, sleeps, calls = self.run_preflight([self.listing()])
        self.assertEqual(evidence["status"], "passed")
        self.assertEqual(evidence["elapsed_seconds"], 0)
        self.assertEqual(evidence["attempts"], [{"number": 1, "status": "requested_tag_found"}])
        self.assertEqual(sleeps, [])
        self.assertEqual(
            [(method, params) for _, method, params, _ in calls],
            [("get_available_versions", {"app_id": "torch", "force_refresh": False})],
        )

    def test_advertised_687_second_delay_is_used_in_full(self) -> None:
        evidence, sleeps, calls = self.run_preflight([self.limited(687), self.listing()])
        self.assertEqual(sleeps, [687])
        self.assertEqual(len(calls), 2)
        self.assertEqual(evidence["elapsed_seconds"], 687)
        self.assertEqual(evidence["status"], "passed")
        self.assertEqual(evidence["attempts"][0]["retry_after_secs"], 687)
        self.assertNotIn("untrusted detail", str(evidence))

    def test_invalid_delays_fail_without_sleep(self) -> None:
        for delay in (None, "687", True, -1, 901):
            with self.subTest(delay=delay):
                with self.assertRaisesRegex(RuntimeError, "invalid_retry_delay"):
                    self.run_preflight([self.limited(delay)])

    def test_delay_cannot_consume_remaining_budget(self) -> None:
        state = {}
        calls = []

        def fake_rpc(_base, method, _params, timeout):
            calls.append(method)
            return self.limited(900)

        with self.assertRaisesRegex(RuntimeError, "time_budget_exhausted"):
            preflight_torch_release(
                "base",
                state,
                rpc_call=fake_rpc,
                clock=lambda: 1000 if calls else 0,
                sleep=lambda _seconds: self.fail("must not sleep past budget"),
            )
        self.assertEqual(calls, ["get_available_versions"])
        self.assertEqual(state["version_preflight"]["elapsed_seconds"], 1000)

    def test_rpc_timeout_does_not_exceed_subsecond_remaining_budget(self) -> None:
        clock_calls = [0]
        timeouts = []

        def fake_clock():
            return clock_calls.pop(0) if clock_calls else 1799.75

        def fake_rpc(_base, _method, _params, timeout):
            timeouts.append(timeout)
            return self.listing()

        evidence = {}
        preflight_torch_release("base", evidence, rpc_call=fake_rpc, clock=fake_clock)
        self.assertEqual(timeouts, [0.25])
        self.assertEqual(evidence["version_preflight"]["status"], "passed")

    def test_malformed_listing_and_conflicting_success_fail(self) -> None:
        for response, expected in (
            ({"success": True, "versions": "not a list"}, "malformed_listing"),
            ({"success": True, "versions": [{"tag_name": TAG}]}, "malformed_listing"),
            ({"success": True, "versions": [], "rate_limited": True}, "unexpected_outcome"),
        ):
            with self.subTest(expected=expected):
                with self.assertRaisesRegex(RuntimeError, expected):
                    self.run_preflight([response])

    def test_attempts_exhausted_without_final_sleep(self) -> None:
        now = [0]
        sleeps = []
        state = {}

        def fake_rpc(_base, _method, _params, timeout):
            return self.limited(1)

        def fake_sleep(seconds):
            sleeps.append(seconds)
            now[0] += seconds

        with self.assertRaisesRegex(RuntimeError, "attempts_exhausted"):
            preflight_torch_release(
                "base", state, rpc_call=fake_rpc, clock=lambda: now[0], sleep=fake_sleep
            )
        self.assertEqual(sleeps, [1, 1])
        self.assertEqual(state["version_preflight"]["elapsed_seconds"], 2)
        self.assertEqual(len(state["version_preflight"]["attempts"]), 3)

    def test_generic_failure_and_missing_tag_stop_before_options_or_install(self) -> None:
        for response, expected in (
            ({"success": False, "error": "private failure"}, "unexpected_outcome"),
            (self.listing("v2.14.0-rc1"), "requested_tag_missing"),
            (self.limited(None), "invalid_retry_delay"),
        ):
            with self.subTest(expected=expected):
                calls = []

                def fake_rpc(_base, method, _params=None, timeout=120):
                    calls.append(method)
                    return response

                with patch.object(sys.modules[__name__], "rpc", side_effect=fake_rpc):
                    with self.assertRaisesRegex(RuntimeError, expected) as failure:
                        exercise("base", Path("unused"), {"install_succeeded": False})
                self.assertEqual(calls, ["get_available_versions"])
                self.assertNotIn("private failure", str(failure.exception))


class EvidenceCollectionFixture(unittest.TestCase):
    def test_copies_only_selected_install_reports_and_log(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "launcher"
            output = Path(temporary) / "evidence"
            runtime = root / "torch-versions" / TAG
            runtime.mkdir(parents=True)
            (runtime / "runtime.json").write_text("fixture runtime", encoding="utf-8")
            (root / "rpc.log").write_text("backend log", encoding="utf-8")
            (root / "rpc-restart.log").write_text("restart log", encoding="utf-8")
            for name in INSTALL_REPORTS:
                if name != "runtime.json":
                    (runtime / name).write_text(name, encoding="utf-8")
            (runtime / "wheel.whl").write_text("excluded", encoding="utf-8")
            (runtime / "venv").mkdir()
            result = {"success": True, "cpython_version": "3.14.0"}

            def fake_full_collector(_root: Path, evidence: Path) -> None:
                destination = evidence / "managed-python-licenses"
                destination.mkdir()
                (destination / "full-archive-manifest.json").write_text(
                    '{"scope":"fixture_full_archive"}', encoding="utf-8"
                )

            with patch.object(
                sys.modules[__name__],
                "export_full_archive_licenses",
                side_effect=fake_full_collector,
            ) as collector:
                collect_evidence(root, output, result, install_succeeded=True)
            collector.assert_called_once_with(root, output)
            self.assertEqual(
                {item.name for item in output.iterdir()},
                {
                    "initial-backend-session.txt",
                    "restart-backend-session.txt",
                    "acceptance.json",
                    "managed-python-licenses",
                    *INSTALL_REPORTS,
                },
            )
            self.assertEqual(json.loads((output / "acceptance.json").read_text()), result)
            self.assertEqual((output / "initial-backend-session.txt").read_text(), "backend log")
            self.assertEqual((output / "restart-backend-session.txt").read_text(), "restart log")
            manifest = json.loads(
                (output / "managed-python-licenses" / "full-archive-manifest.json").read_text()
            )
            self.assertEqual(manifest["scope"], "fixture_full_archive")

    def test_failure_retains_log_without_install_reports(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "launcher"
            output = Path(temporary) / "evidence"
            runtime = root / "torch-versions" / TAG
            runtime.mkdir(parents=True)
            (root / "rpc.log").write_text("failure log", encoding="utf-8")
            (root / "rpc-restart.log").write_text("restart failure", encoding="utf-8")
            (root / "restart-cpu-operation-failure.json").write_text(
                '{"failure_type":"CalledProcessError"}', encoding="utf-8"
            )
            (runtime / "runtime.json").write_text("partial", encoding="utf-8")
            collect_evidence(root, output, {"success": False}, install_succeeded=False)
            self.assertEqual(
                {item.name for item in output.iterdir()},
                {
                    "initial-backend-session.txt",
                    "restart-backend-session.txt",
                    "restart-cpu-operation-failure.json",
                    "acceptance.json",
                },
            )
            self.assertEqual(
                json.loads((output / "restart-cpu-operation-failure.json").read_text())[
                    "failure_type"
                ],
                "CalledProcessError",
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


class RestartAcceptanceFixture(unittest.TestCase):
    @unittest.skipIf(os.name == "nt", "POSIX process group fixture")
    def test_permission_denied_rechecks_group_liveness(self) -> None:
        observations = iter((True, False))
        signals = []

        def denied(group, requested_signal):
            signals.append((group, requested_signal))
            raise PermissionError(errno.EPERM, "denied")

        self.assertFalse(
            signal_posix_group_once(42, observe=lambda _group: next(observations), send=denied)
        )
        self.assertEqual(signals, [(42, signal.SIGKILL)])

        observations = iter((True, True))
        with self.assertRaises(PermissionError):
            signal_posix_group_once(42, observe=lambda _group: next(observations), send=denied)

    @unittest.skipIf(os.name == "nt", "POSIX process group fixture")
    def test_bounded_runner_does_not_signal_drained_group(self) -> None:
        with patch.object(os, "killpg", side_effect=PermissionError(errno.EPERM, "denied")) as kill:
            result = run_bounded_posix_subprocess(
                [sys.executable, "-c", "pass"],
                cwd=Path.cwd(),
                env=os.environ.copy(),
                timeout=5,
            )
        self.assertEqual(result.returncode, 0)
        kill.assert_not_called()

    @unittest.skipIf(os.name == "nt", "POSIX process group fixture")
    def test_cleanup_reaps_leader_and_descendant_after_observer_failure(self) -> None:
        real_observe = posix_group_has_live_members
        observations = 0

        def fail_once(group):
            nonlocal observations
            observations += 1
            if observations == 1:
                raise RuntimeError("group observation failed")
            return real_observe(group)

        with patch.object(
            sys.modules[__name__],
            "posix_group_has_live_members",
            side_effect=fail_once,
        ):
            group = self.assert_failed_posix_cleanup(RuntimeError, "group observation failed")
        self.assertFalse(posix_group_has_live_members(group))

    @unittest.skipIf(os.name == "nt", "POSIX process group fixture")
    def test_cleanup_reaps_leader_and_descendant_after_live_eperm(self) -> None:
        real_killpg = os.killpg
        signals = []

        def deny_once(group, requested_signal):
            signals.append((group, requested_signal))
            if len(signals) == 1:
                raise PermissionError(errno.EPERM, "denied")
            return real_killpg(group, requested_signal)

        with patch.object(os, "killpg", side_effect=deny_once):
            group = self.assert_failed_posix_cleanup(PermissionError, "denied")
        self.assertGreaterEqual(len(signals), 2)
        self.assertLess(len(signals), 100)
        self.assertFalse(posix_group_has_live_members(group))

    @unittest.skipIf(os.name == "nt", "POSIX process group fixture")
    def test_persistent_group_eperm_retains_custody_until_explicit_drain(self) -> None:
        real_killpg = os.killpg
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            leader_path = root / "leader.pid"
            marker = root / "descendant-escaped"
            descendant = (
                "import pathlib,sys,time; time.sleep(2); "
                "pathlib.Path(sys.argv[1]).write_text('escaped')"
            )
            parent = (
                "import os,pathlib,subprocess,sys,time; "
                "pathlib.Path(sys.argv[2]).write_text(str(os.getpid())); "
                "subprocess.Popen([sys.executable,'-c',sys.argv[1],sys.argv[3]],"
                "stdout=sys.stdout,stderr=sys.stderr); time.sleep(10)"
            )
            retained = None
            try:
                with patch.object(os, "killpg", side_effect=PermissionError(errno.EPERM, "denied")):
                    with self.assertRaises(PermissionError):
                        run_bounded_posix_subprocess(
                            [
                                sys.executable,
                                "-c",
                                parent,
                                descendant,
                                str(leader_path),
                                str(marker),
                            ],
                            cwd=root,
                            env=os.environ.copy(),
                            timeout=0.25,
                        )
                group = int(leader_path.read_text())
                retained = next(child for child in UN_DRAINED_POSIX_CHILDREN if child.pid == group)
                self.assertIsNone(retained.returncode, "leader was reaped before group drain")
                self.assertTrue(posix_group_has_live_members(group))
            finally:
                if retained is not None:
                    deadline = time.monotonic() + 5
                    while posix_group_has_live_members(retained.pid):
                        real_killpg(retained.pid, signal.SIGKILL)
                        self.assertLess(time.monotonic(), deadline)
                        time.sleep(0.02)
                    retained.wait()
                    UN_DRAINED_POSIX_CHILDREN.remove(retained)
            self.assertFalse(posix_group_has_live_members(group))
            time.sleep(2.2)
            self.assertFalse(marker.exists())

    def assert_failed_posix_cleanup(self, failure_type: type[Exception], message: str) -> int:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            leader_path = root / "leader.pid"
            descendant_marker = root / "descendant-escaped"
            descendant = (
                "import pathlib,sys,time; time.sleep(1); "
                "pathlib.Path(sys.argv[1]).write_text('escaped')"
            )
            parent = (
                "import os,pathlib,subprocess,sys,time; "
                "pathlib.Path(sys.argv[2]).write_text(str(os.getpid())); "
                "subprocess.Popen([sys.executable,'-c',sys.argv[1],sys.argv[3]],"
                "stdout=sys.stdout,stderr=sys.stderr); "
                "os.write(1,b'partial-out'); time.sleep(10)"
            )
            with self.assertRaises(failure_type) as raised:
                run_bounded_posix_subprocess(
                    [
                        sys.executable,
                        "-c",
                        parent,
                        descendant,
                        str(leader_path),
                        str(descendant_marker),
                    ],
                    cwd=root,
                    env=os.environ.copy(),
                    timeout=0.25,
                )
            self.assertIn(message, str(raised.exception))
            leader = int(leader_path.read_text())
            with self.assertRaises(ProcessLookupError):
                os.kill(leader, 0)
            time.sleep(1.2)
            self.assertFalse(descendant_marker.exists())
            return leader

    def test_session_preserves_original_failure_when_stop_fails(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            log_path = root / "rpc-restart.log"
            with (
                patch.object(subprocess, "Popen"),
                patch.object(
                    sys.modules[__name__],
                    "wait_for_backend",
                    side_effect=ValueError("startup failed"),
                ),
                patch.object(
                    sys.modules[__name__], "stop_backend", side_effect=RuntimeError("stop failed")
                ),
            ):
                with self.assertRaisesRegex(ValueError, "startup failed") as raised:
                    run_backend_session(Path("/binary"), root, log_path, lambda _: None)
            self.assertIn("stop failed", raised.exception.__notes__[0])
            self.assertTrue(log_path.is_file())

    def test_restart_checks_persisted_identity_and_repeats_runtime_trial(self) -> None:
        first = {
            "python": "python3.14",
            "cpython_version": "3.14.7",
            "provider": "uv",
            "provider_version": "0.8.1",
            "provider_archive_sha256": "a" * 64,
            "target": "x86_64-unknown-linux-gnu",
            "distribution_implementation": "CPython",
            "distribution_catalog_key": "cpython-3.14.7-linux-x86_64-gnu",
            "distribution_source_url": self.source_url(),
            "executable_path": "/launcher/launcher-data/managed-python/python/one/python3.14",
            "executable_sha256": "c" * 64,
        }
        calls = []

        def fake_rpc(_base, method, params=None, timeout=120):
            calls.append((method, params))
            if method == "get_active_version":
                return {"success": True, "version": TAG}
            if method == "get_runtime_profiles_snapshot":
                return {
                    "success": True,
                    "snapshot": {
                        "profiles": [
                            {
                                "profile_id": PROFILE_ID,
                                "provider": "torch",
                                "provider_mode": "torch_serve",
                                "management_mode": "managed",
                                "enabled": True,
                                "device": {"mode": "cpu"},
                            }
                        ]
                    },
                }
            if method == "get_torch_runtime_probe":
                return {
                    "status": "passed",
                    "core_status": "passed",
                    "stale": False,
                    "capabilities": {
                        "torch_import": {"status": "passed", "version": "2.14.0+cpu"},
                        "cpu_tensor": {"status": "passed"},
                        "sidecar_app": {"status": "passed"},
                    },
                }
            if method == "trial_torch_runtime":
                return {
                    "success": True,
                    "startupStatus": "passed",
                    "healthStatus": "passed",
                    "protocol": 3,
                    "startedByTrial": True,
                    "generation": "2",
                }
            if method == "stop_runtime_profile_if_generation":
                return {"success": True, "stopped": True}
            self.fail(method)

        with (
            patch.object(sys.modules[__name__], "rpc", side_effect=fake_rpc),
            patch.object(
                sys.modules[__name__],
                "run_restart_cpu_operation",
                side_effect=lambda _root: (
                    calls.append(("cpu_operation", None))
                    or {"version": "2.14.0+cpu", "device": "cpu", "result": 14}
                ),
            ) as cpu_operation,
            patch.object(
                sys.modules[__name__],
                "managed_python_evidence",
                return_value={
                    key: first[key]
                    for key in (
                        "cpython_version",
                        "provider",
                        "provider_version",
                        "provider_archive_sha256",
                        "target",
                        "distribution_implementation",
                        "distribution_catalog_key",
                        "distribution_source_url",
                        "executable_path",
                        "executable_sha256",
                    )
                },
            ) as evidence,
        ):
            restart = exercise_restart("http://second", Path("/launcher"), first)
        evidence.assert_called_once_with(Path("/launcher"), {"python": "python3.14"})
        cpu_operation.assert_called_once_with(Path("/launcher"))
        self.assertEqual(restart["active_version"], TAG)
        self.assertEqual(restart["profile_id"], PROFILE_ID)
        self.assertEqual(restart["sidecar_generation"], "2")
        self.assertEqual(restart["interpreter"], "persisted")
        self.assertEqual(
            restart["cpu_operation"], {"version": "2.14.0+cpu", "device": "cpu", "result": 14}
        )
        self.assertEqual(
            [method for method, _ in calls],
            [
                "get_active_version",
                "get_runtime_profiles_snapshot",
                "cpu_operation",
                "get_torch_runtime_probe",
                "trial_torch_runtime",
                "stop_runtime_profile_if_generation",
            ],
        )

    def test_fresh_cpu_operation_uses_isolated_persisted_venv(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = (
                root / "torch-versions" / TAG / "venv" / "Scripts" / "python.exe"
                if os.name == "nt"
                else root / "torch-versions" / TAG / "venv" / "bin" / "python"
            )
            executable.parent.mkdir(parents=True)
            executable.touch()
            response = subprocess.CompletedProcess(
                args=[],
                returncode=0,
                stdout='{"version":"2.14.0+cpu","device":"cpu","result":14}',
                stderr="",
            )
            with patch.object(
                sys.modules[__name__], "run_bounded_subprocess", return_value=response
            ) as run:
                proof = run_restart_cpu_operation(root)
            self.assertEqual(proof, {"version": "2.14.0+cpu", "device": "cpu", "result": 14})
            args, kwargs = run.call_args
            self.assertEqual(args[0][:2], [str(executable), "-I"])
            self.assertEqual(kwargs["cwd"], root)
            self.assertEqual(kwargs["env"], backend_environment(root))
            self.assertLessEqual(kwargs["timeout"], 60)

    def test_fresh_cpu_operation_rejects_wrong_result(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = (
                root / "torch-versions" / TAG / "venv" / "Scripts" / "python.exe"
                if os.name == "nt"
                else root / "torch-versions" / TAG / "venv" / "bin" / "python"
            )
            executable.parent.mkdir(parents=True)
            executable.touch()
            for output in (
                {"version": "2.13.0", "device": "cpu", "result": 14},
                {"version": "2.14.0+cpu", "device": "cuda:0", "result": 14},
                {"version": "2.14.0+cpu", "device": "cpu", "result": 13},
            ):
                with (
                    self.subTest(output=output),
                    patch.object(
                        sys.modules[__name__],
                        "run_bounded_subprocess",
                        return_value=subprocess.CompletedProcess(
                            args=[], returncode=0, stdout=json.dumps(output), stderr=""
                        ),
                    ),
                ):
                    with self.assertRaisesRegex(RuntimeError, "fresh CPU tensor operation"):
                        run_restart_cpu_operation(root)
                    report = json.loads((root / "restart-cpu-operation-failure.json").read_text())
                    self.assertEqual(report["failure_type"], "RuntimeError")
                    self.assertEqual(json.loads(report["stdout_tail"]), output)

    def test_fresh_cpu_operation_retains_bounded_subprocess_failure(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = (
                root / "torch-versions" / TAG / "venv" / "Scripts" / "python.exe"
                if os.name == "nt"
                else root / "torch-versions" / TAG / "venv" / "bin" / "python"
            )
            executable.parent.mkdir(parents=True)
            executable.touch()
            failure = subprocess.CalledProcessError(
                1,
                [str(executable)],
                output=b"x" * 5000 + b"stdout-end",
                stderr=b"y" * 5000 + b"ImportError: missing DLL",
            )
            with patch.object(sys.modules[__name__], "run_bounded_subprocess", side_effect=failure):
                with self.assertRaises(subprocess.CalledProcessError) as raised:
                    run_restart_cpu_operation(root)
            report = json.loads((root / "restart-cpu-operation-failure.json").read_text())
            self.assertEqual(report["failure_type"], "CalledProcessError")
            self.assertEqual(report["returncode"], 1)
            self.assertLessEqual(len(report["stdout_tail"]), 2048)
            self.assertLessEqual(len(report["stderr_tail"]), 2048)
            self.assertTrue(report["stdout_tail"].endswith("stdout-end"))
            self.assertTrue(report["stderr_tail"].endswith("ImportError: missing DLL"))
            self.assertIn("restart-cpu-operation-failure.json", raised.exception.__notes__[0])

    def test_fresh_cpu_operation_retains_timeout_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            executable = (
                root / "torch-versions" / TAG / "venv" / "Scripts" / "python.exe"
                if os.name == "nt"
                else root / "torch-versions" / TAG / "venv" / "bin" / "python"
            )
            executable.parent.mkdir(parents=True)
            executable.touch()
            failure = subprocess.TimeoutExpired(
                [str(executable)], 60, output=b"partial stdout", stderr=b"partial stderr"
            )
            with patch.object(sys.modules[__name__], "run_bounded_subprocess", side_effect=failure):
                with self.assertRaises(subprocess.TimeoutExpired):
                    run_restart_cpu_operation(root)
            report = json.loads((root / "restart-cpu-operation-failure.json").read_text())
            self.assertEqual(report["failure_type"], "TimeoutExpired")
            self.assertEqual(report["timeout_seconds"], 60)
            self.assertEqual(report["stdout_tail"], "partial stdout")
            self.assertEqual(report["stderr_tail"], "partial stderr")

    def test_bounded_runner_drains_large_output_and_preserves_tails(self) -> None:
        command = [
            sys.executable,
            "-c",
            "import os; os.write(1,b'a'*131072+b'STDOUT_END'); "
            "os.write(2,b'b'*131072+b'STDERR_END')",
        ]
        result = run_bounded_subprocess(command, cwd=Path.cwd(), env=os.environ.copy(), timeout=5)
        self.assertIsInstance(result, subprocess.CompletedProcess)
        self.assertEqual(result.returncode, 0)
        self.assertEqual(len(result.stdout), 2048)
        self.assertEqual(len(result.stderr), 2048)
        self.assertTrue(result.stdout.endswith(b"STDOUT_END"))
        self.assertTrue(result.stderr.endswith(b"STDERR_END"))

        with self.assertRaises(subprocess.CalledProcessError) as raised:
            run_bounded_subprocess(
                [sys.executable, "-c", "import os,sys; os.write(2,b'failure'); sys.exit(7)"],
                cwd=Path.cwd(),
                env=os.environ.copy(),
                timeout=5,
            )
        self.assertEqual(raised.exception.returncode, 7)
        self.assertEqual(raised.exception.stderr, b"failure")

    def test_bounded_runner_kills_and_reaps_on_timeout(self) -> None:
        start = time.monotonic()
        with self.assertRaises(subprocess.TimeoutExpired) as raised:
            run_bounded_subprocess(
                [
                    sys.executable,
                    "-c",
                    "import os,time; os.write(1,b'partial stdout'); "
                    "os.write(2,b'partial stderr'); time.sleep(10)",
                ],
                cwd=Path.cwd(),
                env=os.environ.copy(),
                timeout=0.25,
            )
        self.assertLess(time.monotonic() - start, 3)
        self.assertEqual(raised.exception.stdout, b"partial stdout")
        self.assertEqual(raised.exception.stderr, b"partial stderr")

    def test_bounded_runner_owns_descendant_with_inherited_pipes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            marker = Path(temporary) / "escaped-child.txt"
            descendant = (
                "import pathlib,sys,time; time.sleep(1); "
                "pathlib.Path(sys.argv[1]).write_text('escaped')"
            )
            parent = (
                "import os,subprocess,sys; "
                "subprocess.Popen([sys.executable,'-c',sys.argv[1],sys.argv[2]],"
                "stdout=sys.stdout,stderr=sys.stderr); "
                "os.write(1,b'parent-tail'); os.write(2,b'parent-error')"
            )
            before = {thread.ident for thread in threading.enumerate()}
            result = run_bounded_subprocess(
                [sys.executable, "-c", parent, descendant, str(marker)],
                cwd=Path(temporary),
                env=os.environ.copy(),
                timeout=3,
            )
            self.assertEqual(result.stdout, b"parent-tail")
            self.assertEqual(result.stderr, b"parent-error")
            self.assertEqual({thread.ident for thread in threading.enumerate()} - before, set())
            time.sleep(1.2)
            self.assertFalse(marker.exists(), "descendant outlived bounded subprocess")

    def test_bounded_runner_timeout_kills_descendant_and_retains_partial_output(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            marker = Path(temporary) / "timed-out-child.txt"
            descendant = (
                "import pathlib,sys,time; time.sleep(1); "
                "pathlib.Path(sys.argv[1]).write_text('escaped')"
            )
            parent = (
                "import os,subprocess,sys,time; "
                "subprocess.Popen([sys.executable,'-c',sys.argv[1],sys.argv[2]],"
                "stdout=sys.stdout,stderr=sys.stderr); "
                "os.write(1,b'partial-out'); os.write(2,b'partial-err'); time.sleep(10)"
            )
            before = {thread.ident for thread in threading.enumerate()}
            with self.assertRaises(subprocess.TimeoutExpired) as raised:
                run_bounded_subprocess(
                    [sys.executable, "-c", parent, descendant, str(marker)],
                    cwd=Path(temporary),
                    env=os.environ.copy(),
                    timeout=0.25,
                )
            self.assertEqual(raised.exception.stdout, b"partial-out")
            self.assertEqual(raised.exception.stderr, b"partial-err")
            self.assertEqual({thread.ident for thread in threading.enumerate()} - before, set())
            time.sleep(1.2)
            self.assertFalse(marker.exists(), "timed-out descendant outlived bounded subprocess")

    @unittest.skipIf(os.name == "nt", "POSIX detached process group fixture")
    def test_bounded_runner_returns_promptly_when_descendant_escapes_group(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            pid_path = Path(temporary) / "detached.pid"
            completion_path = Path(temporary) / "detached-completed"
            descendant = (
                "import pathlib,sys,time; time.sleep(2); "
                "pathlib.Path(sys.argv[1]).write_text('completed')"
            )
            parent = (
                "import os,pathlib,subprocess,sys,time; "
                "child=subprocess.Popen([sys.executable,'-c',sys.argv[1],sys.argv[3]],"
                "stdout=sys.stdout,stderr=sys.stderr,start_new_session=True); "
                "pathlib.Path(sys.argv[2]).write_text(str(child.pid)); "
                "os.write(1,b'parent-tail'); os.write(2,b'parent-error'); time.sleep(10)"
            )
            before = {thread.ident for thread in threading.enumerate()}
            start = time.monotonic()
            try:
                with self.assertRaises(subprocess.TimeoutExpired) as raised:
                    run_bounded_subprocess(
                        [
                            sys.executable,
                            "-c",
                            parent,
                            descendant,
                            str(pid_path),
                            str(completion_path),
                        ],
                        cwd=Path(temporary),
                        env=os.environ.copy(),
                        timeout=0.25,
                    )
                self.assertLess(time.monotonic() - start, 1.5)
                self.assertIn(b"parent-tail", raised.exception.stdout)
                self.assertIn(b"parent-error", raised.exception.stderr)
                self.assertEqual({thread.ident for thread in threading.enumerate()} - before, set())
                self.assertTrue(posix_group_has_live_members(int(pid_path.read_text())))
                self.assertFalse(completion_path.exists())
            finally:
                if pid_path.exists():
                    deadline = time.monotonic() + 5
                    while not completion_path.exists() and time.monotonic() < deadline:
                        time.sleep(0.05)
                    self.assertTrue(completion_path.exists(), "detached descendant did not exit")
                    detached_group = int(pid_path.read_text())
                    while (
                        posix_group_has_live_members(detached_group) and time.monotonic() < deadline
                    ):
                        time.sleep(0.05)
                    self.assertFalse(posix_group_has_live_members(detached_group))

    def test_bounded_runner_cleans_up_if_reader_thread_cannot_start(self) -> None:
        spawned = []
        real_popen = subprocess.Popen
        real_start = threading.Thread.start
        starts = 0

        def track_process(*args, **kwargs):
            kwargs["creationflags"] = 0
            kwargs["start_new_session"] = os.name != "nt"
            process = real_popen(*args, **kwargs)
            spawned.append(process)
            return process

        def fail_second_start(reader):
            nonlocal starts
            starts += 1
            if starts == 2:
                raise RuntimeError("reader start failed")
            return real_start(reader)

        class FakeJob:
            def admit_and_resume(self, _process):
                pass

            def close(self):
                if spawned and spawned[0].poll() is None:
                    spawned[0].kill()

        with (
            patch.object(sys.modules[__name__], "native_windows", return_value=True),
            patch.object(sys.modules[__name__], "windows_kill_job", return_value=FakeJob()),
            patch.object(subprocess, "Popen", side_effect=track_process),
            patch.object(threading.Thread, "start", fail_second_start),
        ):
            with self.assertRaisesRegex(RuntimeError, "reader start failed"):
                run_bounded_subprocess(
                    [sys.executable, "-c", "import time; time.sleep(10)"],
                    cwd=Path.cwd(),
                    env=os.environ.copy(),
                    timeout=5,
                )
        self.assertEqual(len(spawned), 1)
        self.assertIsNotNone(spawned[0].poll())
        self.assertTrue(spawned[0].stdout.closed)
        self.assertTrue(spawned[0].stderr.closed)

    def test_windows_admission_precedes_resume_and_reader_start(self) -> None:
        events = []

        class FakeProcess:
            pid = 123
            returncode = 0

            def __init__(self):
                self.stdout = io.BytesIO(b"output")
                self.stderr = io.BytesIO(b"")

            def wait(self, timeout=None):
                events.append("wait")
                return 0

            def poll(self):
                return 0

        class FakeJob:
            def admit_and_resume(self, _process):
                events.append("admit_resume")

            def close(self):
                events.append("job_close")

        def fake_job():
            events.append("job_create")
            return FakeJob()

        def fake_spawn(*_args, **kwargs):
            events.append("spawn")
            self.assertEqual(kwargs["creationflags"], 0x4)
            self.assertFalse(kwargs["start_new_session"])
            return FakeProcess()

        with (
            patch.object(sys.modules[__name__], "native_windows", return_value=True),
            patch.object(sys.modules[__name__], "windows_kill_job", side_effect=fake_job),
            patch.object(subprocess, "Popen", side_effect=fake_spawn),
        ):
            result = run_bounded_subprocess(["fake-python"], cwd=Path.cwd(), env={}, timeout=1)
        self.assertEqual(result.stdout, b"output")
        self.assertEqual(events[:4], ["job_create", "spawn", "admit_resume", "wait"])
        self.assertIn("job_close", events[4:])

    def test_windows_admission_failure_reaps_suspended_child(self) -> None:
        events = []

        class FakeProcess:
            pid = 123

            def __init__(self):
                self.returncode = None
                self.stdout = io.BytesIO()
                self.stderr = io.BytesIO()

            def poll(self):
                return self.returncode

            def kill(self):
                events.append("kill")
                self.returncode = -9

            def wait(self, timeout=None):
                events.append("reap")
                return self.returncode

        class FakeJob:
            def admit_and_resume(self, _process):
                events.append("admit")
                raise RuntimeError("admission failed")

            def close(self):
                events.append("job_close")

        process = FakeProcess()
        with (
            patch.object(sys.modules[__name__], "native_windows", return_value=True),
            patch.object(sys.modules[__name__], "windows_kill_job", return_value=FakeJob()),
            patch.object(subprocess, "Popen", return_value=process),
        ):
            with self.assertRaisesRegex(RuntimeError, "admission failed"):
                run_bounded_subprocess(["fake-python"], cwd=Path.cwd(), env={}, timeout=1)
        self.assertEqual(events, ["admit", "job_close", "kill", "reap"])
        self.assertTrue(process.stdout.closed)
        self.assertTrue(process.stderr.closed)

    def test_failed_fresh_cpu_operation_prevents_probe_and_trial(self) -> None:
        first = {
            "python": "python3.14",
            "cpython_version": "3.14.7",
            "provider": "uv",
            "provider_version": "0.8.1",
            "provider_archive_sha256": "a" * 64,
            "target": "x86_64-unknown-linux-gnu",
            "distribution_implementation": "CPython",
            "distribution_catalog_key": "cpython-3.14.7-linux-x86_64-gnu",
            "distribution_source_url": self.source_url(),
            "executable_path": "/launcher/launcher-data/managed-python/python/one/python3.14",
            "executable_sha256": "c" * 64,
        }
        responses = {
            "get_active_version": {"success": True, "version": TAG},
            "get_runtime_profiles_snapshot": {
                "success": True,
                "snapshot": {
                    "profiles": [
                        {
                            "profile_id": PROFILE_ID,
                            "provider": "torch",
                            "provider_mode": "torch_serve",
                            "management_mode": "managed",
                            "enabled": True,
                            "device": {"mode": "cpu"},
                        }
                    ]
                },
            },
        }
        with (
            patch.object(
                sys.modules[__name__],
                "rpc",
                side_effect=lambda _, method, *args, **kwargs: responses[method],
            ) as rpc_mock,
            patch.object(
                sys.modules[__name__],
                "managed_python_evidence",
                return_value={key: value for key, value in first.items() if key != "python"},
            ),
            patch.object(
                sys.modules[__name__],
                "run_restart_cpu_operation",
                side_effect=RuntimeError("fresh CPU tensor operation failed"),
            ),
        ):
            with self.assertRaisesRegex(RuntimeError, "fresh CPU tensor operation"):
                exercise_restart("http://second", Path("/launcher"), first)
        self.assertEqual(rpc_mock.call_count, 2)

    def test_restart_rejects_interpreter_drift_before_runtime_trial(self) -> None:
        first = {
            "python": "python3.14",
            "cpython_version": "3.14.7",
            "provider": "uv",
            "provider_version": "0.8.1",
            "provider_archive_sha256": "a" * 64,
            "target": "x86_64-unknown-linux-gnu",
            "distribution_implementation": "CPython",
            "distribution_catalog_key": "cpython-3.14.7-linux-x86_64-gnu",
            "distribution_source_url": self.source_url(),
            "executable_path": "/launcher/launcher-data/managed-python/python/one/python3.14",
            "executable_sha256": "c" * 64,
        }
        responses = {
            "get_active_version": {"success": True, "version": TAG},
            "get_runtime_profiles_snapshot": {
                "success": True,
                "snapshot": {
                    "profiles": [
                        {
                            "profile_id": PROFILE_ID,
                            "provider": "torch",
                            "provider_mode": "torch_serve",
                            "management_mode": "managed",
                            "enabled": True,
                            "device": {"mode": "cpu"},
                        }
                    ]
                },
            },
        }
        changes = {
            "provider_archive_sha256": "b" * 64,
            "executable_sha256": "d" * 64,
            "executable_path": "/launcher/launcher-data/managed-python/python/two/python3.14",
            "distribution_source_url": self.source_url().replace("20260901", "20260902"),
            "distribution_catalog_key": "cpython-3.14.7-linux-aarch64-gnu",
        }
        for key, changed in changes.items():
            with (
                self.subTest(key=key),
                patch.object(
                    sys.modules[__name__],
                    "rpc",
                    side_effect=lambda _, method, *args, **kwargs: responses[method],
                ) as rpc_mock,
                patch.object(
                    sys.modules[__name__],
                    "managed_python_evidence",
                    return_value={
                        **{key: value for key, value in first.items() if key != "python"},
                        key: changed,
                    },
                ),
            ):
                with self.assertRaisesRegex(RuntimeError, "persisted managed Python identity"):
                    exercise_restart("http://second", Path("/launcher"), first)
                self.assertEqual(rpc_mock.call_count, 2)

    @staticmethod
    def source_url() -> str:
        return (
            "https://releases.astral.sh/github/python-build-standalone/releases/download/"
            "20260901/cpython-3.14.7%2B20260901-x86_64-unknown-linux-gnu-"
            "install_only_stripped.tar.gz"
        )


class FullArchiveLicenseFixture(unittest.TestCase):
    @staticmethod
    def source_url(
        target: str, *, release: str = REVIEWED_PBS_RELEASE, flavor: str = "install_only_stripped"
    ) -> str:
        return (
            "https://releases.astral.sh/github/python-build-standalone/releases/download/"
            f"{release}/cpython-3.14.7%2B{release}-{target}-{flavor}.tar.gz"
        )

    @staticmethod
    def asset(name: str, content: bytes = b"archive") -> dict:
        encoded_name = name.replace("+", "%2B")
        return {
            "id": 456,
            "name": name,
            "state": "uploaded",
            "size": len(content),
            "digest": "sha256:" + hashlib.sha256(content).hexdigest(),
            "browser_download_url": (
                "https://github.com/astral-sh/python-build-standalone/releases/download/"
                f"{REVIEWED_PBS_RELEASE}/{encoded_name}"
            ),
        }

    @classmethod
    def collector_asset(cls, *, target: str = "x86_64-unknown-linux-gnu") -> dict:
        source_url = cls.source_url(target)
        name = full_archive_name(source_url, "3.14.7", target)
        encoded_name = name.replace("+", "%2B")
        return {
            "name": name,
            "url": (
                "https://github.com/astral-sh/python-build-standalone/releases/download/"
                f"{REVIEWED_PBS_RELEASE}/{encoded_name}"
            ),
            "source_url": source_url,
            "version": "3.14.7",
            "target": target,
            "sha256": "a" * 64,
            "size": 1,
            "release_id": 123,
            "asset_id": 456,
        }

    @staticmethod
    def archive(
        path: Path,
        *,
        missing: bool = False,
        unsafe: bool = False,
        symlink: bool = False,
        duplicate: bool = False,
        oversized_declared: bool = False,
        pax_bytes: int = 0,
        zstd: bool = False,
        metadata_overrides: dict | None = None,
    ) -> None:
        metadata = {
            "version": "8",
            "license_path": "licenses/LICENSE.cpython.txt",
            "python_version": "3.14.7",
            "target_triple": "x86_64-unknown-linux-gnu",
            "build_options": "pgo+lto",
            "python_implementation_name": "cpython",
        }
        metadata.update(metadata_overrides or {})
        entries = [
            ("python/PYTHON.json", json.dumps(metadata).encode()),
            ("python/licenses/LICENSE.cpython.txt", b"CPython license"),
        ]
        if missing:
            metadata["license_path"] = "licenses/MISSING.txt"
            entries[0] = ("python/PYTHON.json", json.dumps(metadata).encode())
        if unsafe:
            entries.append(("python/licenses/../../escape", b"escape"))
        if duplicate:
            entries.append(entries[1])
        if oversized_declared:
            metadata["build_info"] = {"extensions": {"sample": {"license_path": "lib/EXTRA.txt"}}}
            entries[0] = ("python/PYTHON.json", json.dumps(metadata).encode())
            entries.append(("python/lib/EXTRA.txt", b"X" * 1024))
        archive_format = tarfile.PAX_FORMAT if pax_bytes else tarfile.DEFAULT_FORMAT
        mode = "w:zst" if zstd else "w"
        with tarfile.open(path, mode, format=archive_format) as tar:
            for index, (name, content) in enumerate(entries):
                member = tarfile.TarInfo(name)
                member.size = len(content)
                if pax_bytes and index == 0:
                    member.pax_headers = {"comment": "x" * pax_bytes}
                tar.addfile(member, io.BytesIO(content))
            if symlink:
                member = tarfile.TarInfo("python/licenses/LINK.txt")
                member.type = tarfile.SYMTYPE
                member.linkname = "../../escape"
                tar.addfile(member)

    def test_reviewed_target_mappings_and_unknown_variants(self) -> None:
        for target, flavor in FULL_FLAVORS.items():
            with self.subTest(target=target):
                name = full_archive_name(self.source_url(target), "3.14.7", target)
                self.assertEqual(
                    name,
                    f"cpython-3.14.7+{REVIEWED_PBS_RELEASE}-{target}-{flavor}-full.tar.zst",
                )
        target = "x86_64-unknown-linux-gnu"
        rejected = (
            (self.source_url(target, release="20260902"), "3.14.7", target),
            (self.source_url(target), "3.14.7", "aarch64-unknown-linux-gnu"),
            (self.source_url(target, flavor="debug"), "3.14.7", target),
            (self.source_url(target), "3.14.8", target),
        )
        for source, version, requested_target in rejected:
            with self.subTest(source=source, target=requested_target, version=version):
                with self.assertRaises(RuntimeError):
                    full_archive_name(source, version, requested_target)

    def test_official_asset_requires_unique_digest_size_and_url(self) -> None:
        name = full_archive_name(
            self.source_url("x86_64-unknown-linux-gnu"), "3.14.7", "x86_64-unknown-linux-gnu"
        )
        asset = self.asset(name)
        release = {"id": 123, "tag_name": REVIEWED_PBS_RELEASE, "assets": [asset]}
        selected = select_full_asset(release, name)
        self.assertEqual(selected["size"], len(b"archive"))
        self.assertEqual((selected["release_id"], selected["asset_id"]), (123, 456))
        self.assertEqual(selected["url"], asset["browser_download_url"])
        self.assertIn("%2B", selected["url"])
        self.assertNotIn("+", urlsplit(selected["url"]).path)
        for bad in (
            {**release, "tag_name": "20260902"},
            {**release, "id": None},
            {**release, "assets": [asset, asset]},
            {**release, "assets": [{**asset, "id": None}]},
            {**release, "assets": [{**asset, "digest": None}]},
            {**release, "assets": [{**asset, "size": None}]},
            {**release, "assets": [{**asset, "size": MAX_FULL_ARCHIVE_BYTES + 1}]},
            {
                **release,
                "assets": [{**asset, "browser_download_url": "https://example.invalid/asset"}],
            },
        ):
            with self.subTest(bad=bad):
                with self.assertRaises(RuntimeError):
                    select_full_asset(bad, name)

    def test_download_rejects_corrupt_digest_and_size(self) -> None:
        payload = b"verified archive"
        asset = {"size": len(payload), "sha256": hashlib.sha256(payload).hexdigest()}
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "archive.tar.zst"
            with self.assertRaises(RuntimeError):
                write_verified_archive(io.BytesIO(b"tampered archive"), destination, asset)
            destination.unlink()
            with self.assertRaises(RuntimeError):
                write_verified_archive(io.BytesIO(payload[:-1]), destination, asset)
            destination.unlink()
            with self.assertRaises(RuntimeError):
                write_verified_archive(io.BytesIO(payload + b"x"), destination, asset)
            destination.unlink()
            with self.assertRaises(RuntimeError):
                write_verified_archive(
                    io.BytesIO(payload),
                    destination,
                    asset,
                    deadline=time.monotonic() - 1,
                )

    def test_download_file_is_private_and_http_redirect_is_rejected(self) -> None:
        payload = b"verified archive"
        asset = {"size": len(payload), "sha256": hashlib.sha256(payload).hexdigest()}
        with tempfile.TemporaryDirectory() as temporary:
            destination = Path(temporary) / "archive.tar.zst"
            write_verified_archive(io.BytesIO(payload), destination, asset)
            if os.name != "nt":
                self.assertEqual(destination.stat().st_mode & 0o777, 0o600)
            request = urllib.request.Request("https://github.com/asset")
            with self.assertRaises(RuntimeError):
                HTTPSOnlyRedirectHandler().redirect_request(
                    request, None, 302, "Found", {}, "http://example.invalid/asset"
                )

    def test_body_read_uses_remaining_deadline_for_socket_timeout(self) -> None:
        class Socket:
            timeout = None

            def settimeout(self, value: float) -> None:
                self.timeout = value

        class Response:
            def __init__(self) -> None:
                self.fp = type("Buffered", (), {"raw": type("Raw", (), {"_sock": Socket()})()})()

            def read1(self, limit: int) -> bytes:
                return b"x"[:limit]

        response = Response()
        self.assertEqual(socket_read1_before_deadline(response, 1, time.monotonic() + 0.2), b"x")
        self.assertGreater(response.fp.raw._sock.timeout, 0)
        self.assertLessEqual(response.fp.raw._sock.timeout, 0.2)
        with self.assertRaises(RuntimeError):
            socket_read1_before_deadline(response, 1, time.monotonic() - 1)

    def test_header_trickle_cannot_extend_worker_deadline(self) -> None:
        # A synthetic HTTPResponse emits one header byte at a time. Its
        # internal readline can exceed a resettable socket timeout.
        trickle = """
import http.client
import io
import time
class Raw(io.RawIOBase):
    body = iter(b'HTTP/1.1 200 OK\\r\\nContent-Length: 1\\r\\n\\r\\nx')
    def readable(self): return True
    def readinto(self, buffer):
        time.sleep(0.025)
        try: buffer[0] = next(self.body)
        except StopIteration: return 0
        return 1
class Socket:
    def makefile(self, mode): return io.BufferedReader(Raw())
http.client.HTTPResponse(Socket()).begin()
"""
        started = time.monotonic()
        with self.assertRaisesRegex(RuntimeError, "exceeded 0.25s deadline"):
            run_bounded_command([sys.executable, "-c", trickle], b"", timeout=0.25)
        self.assertLess(time.monotonic() - started, 2)

    def test_streaming_archive_collects_declared_license(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            archive = base / "archive.tar"
            self.archive(archive)
            asset = self.collector_asset()
            asset["size"] = archive.stat().st_size
            extract_full_archive_licenses(archive, base / "evidence", asset, mode="r|")
            manifest = json.loads(
                (base / "evidence/managed-python-licenses/full-archive-manifest.json").read_text()
            )
            self.assertEqual(manifest["target"], asset["target"])
            self.assertEqual(manifest["source_url"], asset["source_url"])
            self.assertEqual(manifest["archive_url"], asset["url"])
            self.assertEqual(manifest["mapping_authority"], MAPPING_AUTHORITY_URL)
            self.assertEqual((manifest["release_id"], manifest["asset_id"]), (123, 456))
            metadata_bytes = (
                base / "evidence/managed-python-licenses/full-archive/PYTHON.json"
            ).read_bytes()
            self.assertEqual(json.loads(metadata_bytes)["python_version"], "3.14.7")
            self.assertEqual(manifest["python_json"]["path"], "PYTHON.json")
            self.assertEqual(
                manifest["python_json"]["sha256"], hashlib.sha256(metadata_bytes).hexdigest()
            )
            self.assertEqual(manifest["files"][0]["path"], "licenses/LICENSE.cpython.txt")
            self.assertEqual(
                (
                    base
                    / "evidence/managed-python-licenses/full-archive/licenses/LICENSE.cpython.txt"
                ).read_bytes(),
                b"CPython license",
            )

    def test_archive_rejects_unsafe_missing_duplicate_and_symlink_members(self) -> None:
        for kind in ("missing", "unsafe", "duplicate", "symlink"):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                archive = base / "archive.tar"
                self.archive(archive, **{kind: True})
                asset = self.collector_asset()
                with self.assertRaises(RuntimeError):
                    extract_full_archive_licenses(archive, base / "evidence", asset, mode="r|")

    def test_archive_metadata_must_match_selected_distribution(self) -> None:
        mismatches = (
            {"version": "9"},
            {"python_version": "3.14.8"},
            {"target_triple": "aarch64-apple-darwin"},
            {"build_options": "pgo"},
            {"python_implementation_name": "pypy"},
        )
        for mismatch in mismatches:
            with self.subTest(mismatch=mismatch), tempfile.TemporaryDirectory() as temporary:
                base = Path(temporary)
                archive = base / "archive.tar"
                self.archive(archive, metadata_overrides=mismatch)
                with self.assertRaises(RuntimeError):
                    extract_full_archive_licenses(
                        archive, base / "evidence", self.collector_asset(), mode="r|"
                    )

    def test_member_and_license_byte_bounds(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            archive = base / "archive.tar"
            self.archive(archive)
            with patch.object(sys.modules[__name__], "MAX_ARCHIVE_MEMBERS", 1):
                with self.assertRaises(RuntimeError):
                    extract_full_archive_licenses(
                        archive, base / "members", self.collector_asset(), mode="r|"
                    )
            with patch.object(sys.modules[__name__], "MAX_LICENSE_TOTAL_BYTES", 1):
                with self.assertRaises(RuntimeError):
                    extract_full_archive_licenses(
                        archive, base / "bytes", self.collector_asset(), mode="r|"
                    )
            with patch.object(sys.modules[__name__], "MAX_UNCOMPRESSED_TAR_STREAM_BYTES", 1):
                with self.assertRaises(RuntimeError):
                    extract_full_archive_licenses(
                        archive, base / "uncompressed", self.collector_asset(), mode="r|"
                    )

    def test_uncompressed_bound_counts_pax_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            plain = base / "plain.tar"
            pax = base / "pax.tar"
            self.archive(plain)
            self.archive(pax, pax_bytes=8192)
            with patch.object(sys.modules[__name__], "MAX_UNCOMPRESSED_TAR_STREAM_BYTES", 12_000):
                extract_full_archive_licenses(
                    plain, base / "plain-evidence", self.collector_asset(), mode="r|"
                )
                with self.assertRaisesRegex(RuntimeError, "bounded uncompressed tar stream"):
                    extract_full_archive_licenses(
                        pax, base / "pax-evidence", self.collector_asset(), mode="r|"
                    )

    @unittest.skipUnless(sys.version_info >= (3, 14), "zstd is in the stdlib from CPython 3.14")
    def test_zstd_bound_counts_decompressed_pax_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            plain = base / "plain.tar.zst"
            pax = base / "pax.tar.zst"
            self.archive(plain, zstd=True)
            self.archive(pax, pax_bytes=8192, zstd=True)
            with patch.object(sys.modules[__name__], "MAX_UNCOMPRESSED_TAR_STREAM_BYTES", 12_000):
                extract_full_archive_licenses(
                    plain, base / "plain-evidence", self.collector_asset()
                )
                with self.assertRaisesRegex(RuntimeError, "bounded uncompressed tar stream"):
                    extract_full_archive_licenses(
                        pax, base / "pax-evidence", self.collector_asset()
                    )

    def test_oversized_declared_license_outside_license_directory_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            archive = base / "archive.tar"
            self.archive(archive, oversized_declared=True)
            with patch.object(sys.modules[__name__], "MAX_LICENSE_BYTES", 512):
                with self.assertRaises(RuntimeError):
                    extract_full_archive_licenses(
                        archive, base / "evidence", self.collector_asset(), mode="r|"
                    )
            self.assertFalse((base / "evidence/managed-python-licenses/full-archive").exists())

    def test_temporary_download_is_removed_after_success_or_failure(self) -> None:
        for fail in (False, True):
            with self.subTest(fail=fail), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary) / "launcher"
                (root / "tmp").mkdir(parents=True)
                runtime = root / "torch-versions" / TAG
                runtime.mkdir(parents=True)
                asset = self.collector_asset()
                (runtime / "runtime.json").write_text(
                    json.dumps(
                        {
                            "managed_python": {
                                "distribution": {
                                    "sourceUrl": asset["source_url"],
                                    "targetTriple": asset["target"],
                                    "version": asset["version"],
                                }
                            }
                        }
                    ),
                    encoding="utf-8",
                )

                def fake_download(_asset: dict, destination: Path) -> None:
                    destination.write_bytes(b"verified fixture")
                    if fail:
                        raise RuntimeError("fixture download failure")

                with (
                    patch.object(sys, "version_info", (3, 14)),
                    patch.object(sys.modules[__name__], "fetch_full_asset", return_value=asset),
                    patch.object(
                        sys.modules[__name__], "download_full_archive", side_effect=fake_download
                    ),
                    patch.object(sys.modules[__name__], "extract_full_archive_licenses"),
                ):
                    if fail:
                        with self.assertRaises(RuntimeError):
                            export_full_archive_licenses(root, Path(temporary) / "evidence")
                    else:
                        export_full_archive_licenses(root, Path(temporary) / "evidence")
                self.assertFalse((root / "tmp/selected-managed-python-full.tar.zst").exists())


INSTALL_REPORTS = (
    "runtime.json",
    "resolution.json",
    "pip-resolution.json",
    "probe-results.json",
)


def full_archive_name(source_url: str, version: str, target: str) -> str:
    """Admit only the reviewed install-only to full-archive relationship."""
    source = urlsplit(source_url)
    parts = source.path.split("/")
    require(
        source.scheme == "https"
        and source.netloc == "releases.astral.sh"
        and not source.query
        and not source.fragment
        and len(parts) == 7
        and parts[1:5] == ["github", "python-build-standalone", "releases", "download"]
        and parts[5] == REVIEWED_PBS_RELEASE
        and target in FULL_FLAVORS
        and re.fullmatch(r"\d+\.\d+\.\d+", version) is not None,
        "reviewed managed Python full-archive mapping",
        {"source_url": source_url, "version": version, "target": target},
    )
    install_name = unquote(parts[6])
    allowed = {
        f"cpython-{version}+{REVIEWED_PBS_RELEASE}-{target}-install_only.tar.gz",
        f"cpython-{version}+{REVIEWED_PBS_RELEASE}-{target}-install_only_stripped.tar.gz",
    }
    require(install_name in allowed, "reviewed install-only archive flavor", install_name)
    return f"cpython-{version}+{REVIEWED_PBS_RELEASE}-{target}-{FULL_FLAVORS[target]}-full.tar.zst"


def select_full_asset(release: dict, name: str) -> dict:
    require(
        release.get("tag_name") == REVIEWED_PBS_RELEASE,
        "official release tag",
        release.get("tag_name"),
    )
    release_id = release.get("id")
    require(type(release_id) is int and release_id > 0, "official release ID", release_id)
    assets = release.get("assets")
    require(isinstance(assets, list), "official release assets", type(assets).__name__)
    matches = [asset for asset in assets if isinstance(asset, dict) and asset.get("name") == name]
    require(
        len(matches) == 1, "unique official full archive", {"name": name, "matches": len(matches)}
    )
    asset = matches[0]
    digest = asset.get("digest")
    size = asset.get("size")
    asset_id = asset.get("id")
    url = asset.get("browser_download_url")
    parsed = urlsplit(url) if isinstance(url, str) else None
    require(
        asset.get("state") == "uploaded"
        and type(asset_id) is int
        and asset_id > 0
        and isinstance(digest, str)
        and re.fullmatch(r"sha256:[0-9a-f]{64}", digest) is not None
        and type(size) is int
        and 0 < size <= MAX_FULL_ARCHIVE_BYTES
        and parsed is not None
        and parsed.scheme == "https"
        and parsed.netloc == "github.com"
        and not parsed.query
        and not parsed.fragment
        and unquote(parsed.path)
        == f"/astral-sh/python-build-standalone/releases/download/{REVIEWED_PBS_RELEASE}/{name}",
        "official full-archive digest, size, and URL",
        {"name": name, "digest": digest, "size": size, "url": url},
    )
    return {
        "name": name,
        "url": url,
        "size": size,
        "sha256": digest.removeprefix("sha256:"),
        "release_id": release_id,
        "asset_id": asset_id,
    }


def fetch_full_asset(name: str) -> dict:
    body = run_network_worker("release", {}, timeout=60)
    require(len(body) <= MAX_RELEASE_JSON_BYTES, "bounded official release response", len(body))
    return select_full_asset(json.loads(body), name)


def socket_read1_before_deadline(response, limit: int, deadline: float) -> bytes:
    remaining = deadline - time.monotonic()
    require(remaining > 0, "HTTPS body deadline", limit)
    stream = getattr(response, "fp", None)
    sock = getattr(getattr(stream, "raw", None), "_sock", None)
    if sock is not None:
        sock.settimeout(min(30.0, remaining))
    chunk = response.read1(limit)
    require(time.monotonic() <= deadline, "HTTPS body deadline", limit)
    return chunk


def run_bounded_command(command: list[str], payload: bytes, *, timeout: int | float) -> bytes:
    try:
        # subprocess.run kills and reaps a timed-out child. OS scheduling can add
        # a brief kill/reap delay beyond the requested network deadline.
        completed = subprocess.run(
            command, input=payload, capture_output=True, timeout=timeout, check=False
        )
    except subprocess.TimeoutExpired as error:
        raise RuntimeError(f"HTTPS worker exceeded {timeout}s deadline") from error
    require(
        completed.returncode == 0,
        "HTTPS worker",
        {
            "returncode": completed.returncode,
            "stderr": completed.stderr[-500:].decode(errors="replace"),
        },
    )
    return completed.stdout


def run_network_worker(operation: str, payload: dict, *, timeout: int) -> bytes:
    return run_bounded_command(
        [sys.executable, str(Path(__file__).resolve()), "--network-worker", operation],
        json.dumps(payload).encode(),
        timeout=timeout,
    )


def worker_fetch_release() -> bytes:
    url = (
        "https://api.github.com/repos/astral-sh/python-build-standalone/releases/tags/"
        + REVIEWED_PBS_RELEASE
    )
    request = urllib.request.Request(
        url,
        headers={"Accept": "application/vnd.github+json", "User-Agent": "Pumas-release-acceptance"},
    )
    deadline = time.monotonic() + 60
    with DOWNLOAD_HTTP.open(
        request, timeout=min(30, max(0.001, deadline - time.monotonic()))
    ) as response:
        require(
            urlsplit(response.geturl()).scheme == "https",
            "HTTPS official release response",
            response.geturl(),
        )
        chunks = []
        size = 0
        while True:
            chunk = socket_read1_before_deadline(
                response, min(1024 * 1024, MAX_RELEASE_JSON_BYTES + 1 - size), deadline
            )
            if not chunk:
                break
            size += len(chunk)
            require(size <= MAX_RELEASE_JSON_BYTES, "bounded official release response", size)
            chunks.append(chunk)
        body = b"".join(chunks)
    require(len(body) <= MAX_RELEASE_JSON_BYTES, "bounded official release response", len(body))
    return body


def write_verified_archive(
    source, destination: Path, asset: dict, *, deadline: float | None = None
) -> None:
    if deadline is None:
        deadline = time.monotonic() + DOWNLOAD_DEADLINE_SECONDS
    digest = hashlib.sha256()
    size = 0
    descriptor = os.open(destination, os.O_WRONLY | os.O_CREAT | os.O_EXCL, 0o600)
    with os.fdopen(descriptor, "wb") as output:
        while True:
            require(time.monotonic() <= deadline, "full-archive download deadline", size)
            chunk = socket_read1_before_deadline(source, 1024 * 1024, deadline)
            require(time.monotonic() <= deadline, "full-archive download deadline", size)
            if not chunk:
                break
            size += len(chunk)
            require(
                size <= asset["size"] and size <= MAX_FULL_ARCHIVE_BYTES,
                "bounded full-archive download",
                size,
            )
            digest.update(chunk)
            output.write(chunk)
    require(
        size == asset["size"] and digest.hexdigest() == asset["sha256"],
        "full-archive size and SHA-256",
        {"size": size, "sha256": digest.hexdigest()},
    )


def download_full_archive(asset: dict, destination: Path) -> None:
    run_network_worker(
        "archive",
        {"asset": asset, "destination": str(destination)},
        timeout=DOWNLOAD_DEADLINE_SECONDS,
    )


def worker_download_full_archive(asset: dict, destination: Path) -> None:
    request = urllib.request.Request(
        asset["url"], headers={"User-Agent": "Pumas-release-acceptance"}
    )
    deadline = time.monotonic() + DOWNLOAD_DEADLINE_SECONDS
    with DOWNLOAD_HTTP.open(
        request, timeout=min(30, max(0.001, deadline - time.monotonic()))
    ) as response:
        require(
            urlsplit(response.geturl()).scheme == "https",
            "HTTPS full-archive response",
            response.geturl(),
        )
        write_verified_archive(response, destination, asset, deadline=deadline)


def safe_archive_path(name: str) -> str:
    normalized = name.rstrip("/")
    parts = normalized.split("/")
    require(
        normalized
        and not normalized.startswith("/")
        and "\\" not in normalized
        and all(part not in ("", ".", "..") and ":" not in part for part in parts),
        "safe full-archive member path",
        name,
    )
    return normalized


def declared_license_paths(metadata: dict) -> set[str]:
    paths = set()

    def visit(value) -> None:
        if isinstance(value, dict):
            for key, item in value.items():
                if key == "license_path" and item is not None:
                    values = [item] if isinstance(item, str) else item
                    require(
                        isinstance(values, list) and all(isinstance(path, str) for path in values),
                        "PYTHON.json license references",
                        item,
                    )
                    for path in values:
                        safe_archive_path(path)
                        paths.add("python/" + path)
                else:
                    visit(item)
        elif isinstance(value, list):
            for item in value:
                visit(item)

    require(isinstance(metadata, dict), "PYTHON.json object", type(metadata).__name__)
    require(isinstance(metadata.get("license_path"), str), "distribution license_path", metadata)
    visit(metadata)
    require(bool(paths), "declared full-archive licenses", metadata.get("license_path"))
    return paths


def validate_full_archive_metadata(metadata: dict, asset: dict) -> None:
    expected_name = full_archive_name(asset["source_url"], asset["version"], asset["target"])
    require(asset["name"] == expected_name, "selected full-archive identity", asset["name"])
    require(
        metadata.get("version") == "8"
        and metadata.get("python_version") == asset["version"]
        and metadata.get("target_triple") == asset["target"]
        and metadata.get("build_options") == FULL_FLAVORS[asset["target"]]
        and metadata.get("python_implementation_name") == "cpython",
        "PYTHON.json selected distribution identity",
        {
            key: metadata.get(key)
            for key in (
                "version",
                "python_version",
                "target_triple",
                "build_options",
                "python_implementation_name",
            )
        },
    )


class BoundedReader:
    """Count every decompressed tar byte consumed, including PAX metadata."""

    def __init__(self, source, limit: int) -> None:
        self.source = source
        self.limit = limit
        self.bytes_read = 0

    def read(self, size: int = -1) -> bytes:
        remaining = self.limit - self.bytes_read
        requested = remaining + 1 if size is None or size < 0 else min(size, remaining + 1)
        content = self.source.read(requested)
        self.bytes_read += len(content)
        require(
            self.bytes_read <= self.limit,
            "bounded uncompressed tar stream",
            self.bytes_read,
        )
        return content

    def close(self) -> None:
        self.source.close()


@contextmanager
def open_bounded_tar(archive: Path, mode: str):
    """Open only the tested tar stream modes with a decompressed-byte bound."""
    if mode == "r|zst":
        from compression import zstd

        source = zstd.open(archive, "rb")
    elif mode == "r|":
        source = archive.open("rb")
    else:
        raise RuntimeError(f"Unsupported managed-Python archive mode: {mode}")
    with source:
        reader = BoundedReader(source, MAX_UNCOMPRESSED_TAR_STREAM_BYTES)
        with tarfile.open(fileobj=reader, mode="r|") as tar:
            yield tar


def extract_full_archive_licenses(
    archive: Path, output: Path, asset: dict, *, mode: str = "r|zst"
) -> None:
    members = set()
    regular_sizes = {}
    candidates = set()
    metadata = None
    metadata_bytes = None
    license_bytes = 0
    with open_bounded_tar(archive, mode) as tar:
        for member in tar:
            name = safe_archive_path(member.name)
            require(name not in members, "unique full-archive member", name)
            members.add(name)
            require(
                len(members) <= MAX_ARCHIVE_MEMBERS, "bounded full-archive members", len(members)
            )
            require(member.size >= 0, "nonnegative full-archive member size", name)
            if member.isfile():
                regular_sizes[name] = member.size
            if name == "python/PYTHON.json":
                require(
                    member.isfile() and 0 < member.size <= MAX_LICENSE_BYTES,
                    "PYTHON.json member",
                    name,
                )
                extracted = tar.extractfile(member)
                require(extracted is not None, "PYTHON.json body", name)
                body = extracted.read(MAX_LICENSE_BYTES + 1)
                require(len(body) == member.size, "complete PYTHON.json", name)
                metadata = json.loads(body)
                metadata_bytes = body
            if name.startswith("python/licenses/"):
                require(
                    member.isfile() or member.isdir(), "regular full-archive license member", name
                )
                if member.isfile():
                    require(
                        0 < member.size <= MAX_LICENSE_BYTES,
                        "bounded full-archive license member",
                        name,
                    )
                    license_bytes += member.size
                    require(
                        license_bytes <= MAX_LICENSE_TOTAL_BYTES,
                        "bounded full-archive licenses",
                        license_bytes,
                    )
                    candidates.add(name)
    require(
        metadata is not None and metadata_bytes is not None,
        "full-archive PYTHON.json",
        asset["name"],
    )
    validate_full_archive_metadata(metadata, asset)
    declared = declared_license_paths(metadata)
    wanted = candidates | declared
    require(bool(candidates), "full-archive license directory", asset["name"])
    require(
        declared <= members, "declared full-archive license members", sorted(declared - members)
    )
    require(len(wanted) <= MAX_ARCHIVE_MEMBERS, "bounded selected licenses", len(wanted))
    selected_bytes = 0
    for name in wanted:
        size = regular_sizes.get(name)
        require(
            size is not None and 0 < size <= MAX_LICENSE_BYTES,
            "bounded selected full-archive license member",
            {"path": name, "size": size},
        )
        selected_bytes += size
        require(
            selected_bytes <= MAX_LICENSE_TOTAL_BYTES,
            "bounded selected full-archive licenses",
            selected_bytes,
        )
    files = []
    found = set()
    destination = output / "managed-python-licenses" / "full-archive"
    with open_bounded_tar(archive, mode) as tar:
        for member in tar:
            name = safe_archive_path(member.name)
            if name not in wanted:
                continue
            require(name not in found, "unique selected license member", name)
            require(
                member.isfile() and 0 < member.size <= MAX_LICENSE_BYTES,
                "full-archive license text",
                name,
            )
            extracted = tar.extractfile(member)
            require(extracted is not None, "full-archive license body", name)
            content = extracted.read(MAX_LICENSE_BYTES + 1)
            require(len(content) == member.size, "complete full-archive license text", name)
            relative = Path(name).relative_to("python")
            copied = destination / relative
            copied.parent.mkdir(parents=True, exist_ok=True)
            copied.write_bytes(content)
            files.append(
                {
                    "path": relative.as_posix(),
                    "sha256": hashlib.sha256(content).hexdigest(),
                    "declared": name in declared,
                }
            )
            found.add(name)
    require(found == wanted, "all selected full-archive licenses", sorted(wanted - found))
    (destination / "PYTHON.json").write_bytes(metadata_bytes)
    (output / "managed-python-licenses" / "full-archive-manifest.json").write_text(
        json.dumps(
            {
                "scope": "selected_full_archive_license_directory_and_declared_references",
                "source_url": asset["source_url"],
                "target": asset["target"],
                "archive_name": asset["name"],
                "archive_url": asset["url"],
                "mapping_authority": MAPPING_AUTHORITY_URL,
                "archive_sha256": asset["sha256"],
                "archive_size": asset["size"],
                "release_id": asset["release_id"],
                "asset_id": asset["asset_id"],
                "python_json": {
                    "path": "PYTHON.json",
                    "sha256": hashlib.sha256(metadata_bytes).hexdigest(),
                    "size": len(metadata_bytes),
                },
                "files": sorted(files, key=lambda item: item["path"]),
            },
            indent=2,
            sort_keys=True,
        )
        + "\n",
        encoding="utf-8",
    )


def export_full_archive_licenses(root: Path, output: Path) -> None:
    require(sys.version_info >= (3, 14), "acceptance-tool CPython 3.14 for zstd", sys.version)
    recipe = json.loads(
        (root / "torch-versions" / TAG / "runtime.json").read_text(encoding="utf-8")
    )
    distribution = recipe["managed_python"]["distribution"]
    source_url = distribution["sourceUrl"]
    target = distribution["targetTriple"]
    name = full_archive_name(source_url, distribution["version"], target)
    asset = fetch_full_asset(name)
    asset["source_url"] = source_url
    asset["target"] = target
    asset["version"] = distribution["version"]
    archive = root / "tmp" / "selected-managed-python-full.tar.zst"
    try:
        download_full_archive(asset, archive)
        extract_full_archive_licenses(archive, output, asset)
    finally:
        archive.unlink(missing_ok=True)


def collect_evidence(root: Path, output: Path, result: dict, *, install_succeeded: bool) -> None:
    output.mkdir(parents=True, exist_ok=False)
    for source_name, evidence_name in (
        ("rpc.log", "initial-backend-session.txt"),
        ("rpc-restart.log", "restart-backend-session.txt"),
    ):
        log = root / source_name
        if log.is_file():
            shutil.copyfile(log, output / evidence_name)
    cpu_failure = root / "restart-cpu-operation-failure.json"
    if cpu_failure.is_file():
        shutil.copyfile(cpu_failure, output / cpu_failure.name)
    if install_succeeded:
        runtime = root / "torch-versions" / TAG
        for name in INSTALL_REPORTS:
            report = runtime / name
            if report.is_file():
                shutil.copyfile(report, output / name)
        export_full_archive_licenses(root, output)
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
    source_url = distribution.get("sourceUrl")
    catalog_key = distribution.get("catalogKey")
    require(
        distribution.get("implementation") == "CPython"
        and isinstance(python_version, str)
        and re.fullmatch(r"\d+\.\d+\.\d+", python_version) is not None
        and "python" + ".".join(python_version.split(".")[:2]) == preview["python"]
        and isinstance(target, str)
        and bool(target)
        and isinstance(source_url, str)
        and isinstance(catalog_key, str)
        and re.fullmatch(rf"cpython-{re.escape(python_version)}-[a-z0-9_-]+", catalog_key)
        is not None,
        "managed CPython distribution",
        distribution,
    )
    full_archive_name(source_url, python_version, target)
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
        "distribution_implementation": "CPython",
        "distribution_catalog_key": catalog_key,
        "distribution_source_url": source_url,
        "executable_path": str(canonical),
        "executable_sha256": recorded_hash,
    }


def native_windows() -> bool:
    return os.name == "nt"


def windows_kill_job():
    """Create a kill-on-close Job before starting a suspended Windows child."""
    import ctypes
    from ctypes import wintypes

    class IoCounters(ctypes.Structure):
        _fields_ = [
            ("read_operations", ctypes.c_ulonglong),
            ("write_operations", ctypes.c_ulonglong),
            ("other_operations", ctypes.c_ulonglong),
            ("read_bytes", ctypes.c_ulonglong),
            ("write_bytes", ctypes.c_ulonglong),
            ("other_bytes", ctypes.c_ulonglong),
        ]

    class BasicLimitInformation(ctypes.Structure):
        _fields_ = [
            ("per_process_user_time", ctypes.c_longlong),
            ("per_job_user_time", ctypes.c_longlong),
            ("limit_flags", wintypes.DWORD),
            ("minimum_working_set_size", ctypes.c_size_t),
            ("maximum_working_set_size", ctypes.c_size_t),
            ("active_process_limit", wintypes.DWORD),
            ("affinity", ctypes.c_size_t),
            ("priority_class", wintypes.DWORD),
            ("scheduling_class", wintypes.DWORD),
        ]

    class ExtendedLimitInformation(ctypes.Structure):
        _fields_ = [
            ("basic", BasicLimitInformation),
            ("io", IoCounters),
            ("process_memory_limit", ctypes.c_size_t),
            ("job_memory_limit", ctypes.c_size_t),
            ("peak_process_memory_used", ctypes.c_size_t),
            ("peak_job_memory_used", ctypes.c_size_t),
        ]

    class ThreadEntry(ctypes.Structure):
        _fields_ = [
            ("dw_size", wintypes.DWORD),
            ("usage", wintypes.DWORD),
            ("thread_id", wintypes.DWORD),
            ("owner_process_id", wintypes.DWORD),
            ("base_priority", wintypes.LONG),
            ("delta_priority", wintypes.LONG),
            ("flags", wintypes.DWORD),
        ]

    kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    kernel.CreateJobObjectW.argtypes = [ctypes.c_void_p, wintypes.LPCWSTR]
    kernel.CreateJobObjectW.restype = wintypes.HANDLE
    kernel.SetInformationJobObject.argtypes = [
        wintypes.HANDLE,
        ctypes.c_int,
        ctypes.c_void_p,
        wintypes.DWORD,
    ]
    kernel.SetInformationJobObject.restype = wintypes.BOOL
    kernel.AssignProcessToJobObject.argtypes = [wintypes.HANDLE, wintypes.HANDLE]
    kernel.AssignProcessToJobObject.restype = wintypes.BOOL
    kernel.CreateToolhelp32Snapshot.argtypes = [wintypes.DWORD, wintypes.DWORD]
    kernel.CreateToolhelp32Snapshot.restype = wintypes.HANDLE
    kernel.Thread32First.argtypes = [wintypes.HANDLE, ctypes.POINTER(ThreadEntry)]
    kernel.Thread32First.restype = wintypes.BOOL
    kernel.Thread32Next.argtypes = [wintypes.HANDLE, ctypes.POINTER(ThreadEntry)]
    kernel.Thread32Next.restype = wintypes.BOOL
    kernel.OpenThread.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel.OpenThread.restype = wintypes.HANDLE
    kernel.ResumeThread.argtypes = [wintypes.HANDLE]
    kernel.ResumeThread.restype = wintypes.DWORD
    kernel.CloseHandle.argtypes = [wintypes.HANDLE]
    kernel.CloseHandle.restype = wintypes.BOOL
    handle = kernel.CreateJobObjectW(None, None)
    if not handle:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        limits = ExtendedLimitInformation()
        limits.basic.limit_flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE
        if not kernel.SetInformationJobObject(
            handle, 9, ctypes.byref(limits), ctypes.sizeof(limits)
        ):
            raise ctypes.WinError(ctypes.get_last_error())
    except BaseException:
        kernel.CloseHandle(handle)
        raise

    class Job:
        def __init__(self):
            self.closed = False

        def admit_and_resume(self, process: subprocess.Popen[bytes]) -> None:
            if not kernel.AssignProcessToJobObject(handle, wintypes.HANDLE(int(process._handle))):
                raise ctypes.WinError(ctypes.get_last_error())
            snapshot = kernel.CreateToolhelp32Snapshot(0x4, 0)  # TH32CS_SNAPTHREAD
            if snapshot == ctypes.c_void_p(-1).value:
                raise ctypes.WinError(ctypes.get_last_error())
            try:
                entry = ThreadEntry()
                entry.dw_size = ctypes.sizeof(ThreadEntry)
                found = []
                has_entry = kernel.Thread32First(snapshot, ctypes.byref(entry))
                while has_entry:
                    if entry.owner_process_id == process.pid:
                        found.append(entry.thread_id)
                    has_entry = kernel.Thread32Next(snapshot, ctypes.byref(entry))
            finally:
                kernel.CloseHandle(snapshot)
            if len(found) != 1:
                raise RuntimeError("Suspended Windows child did not have one initial thread")
            thread = kernel.OpenThread(0x2, False, found[0])  # THREAD_SUSPEND_RESUME
            if not thread:
                raise ctypes.WinError(ctypes.get_last_error())
            try:
                previous_count = kernel.ResumeThread(thread)
                if previous_count != 1:
                    raise RuntimeError("Could not resume the suspended Windows child")
            finally:
                kernel.CloseHandle(thread)

        def close(self) -> None:
            if not self.closed:
                self.closed = True
                if not kernel.CloseHandle(handle):
                    raise ctypes.WinError(ctypes.get_last_error())

    return Job()


def posix_group_has_live_members(group: int) -> bool:
    observed = subprocess.run(
        ["/bin/ps", "-A", "-o", "pgid=,stat="],
        capture_output=True,
        check=True,
        timeout=5,
    )
    for line in observed.stdout.decode("utf-8", errors="replace").splitlines():
        fields = line.split()
        if fields and fields[0] == str(group):
            if len(fields) < 2:
                raise RuntimeError("Incomplete POSIX process-group state")
            if not fields[1].startswith("Z"):
                return True
    return False


def signal_posix_group_once(group: int, *, observe=None, send=None) -> bool:
    """Signal only observed live members; retain EPERM if any remain live."""
    if observe is None:
        observe = posix_group_has_live_members
    if send is None:
        send = os.killpg
    if not observe(group):
        return False
    try:
        send(group, signal.SIGKILL)
    except ProcessLookupError:
        pass
    except PermissionError as error:
        if error.errno != errno.EPERM or observe(group):
            raise
        return False
    return observe(group)


def drain_posix_group(group: int) -> None:
    deadline = time.monotonic() + 5
    while signal_posix_group_once(group):
        if time.monotonic() >= deadline:
            raise RuntimeError("Owned POSIX process group did not drain")
        time.sleep(0.02)


def emergency_drain_posix_group(group: int) -> None:
    """Retry only the owned group while its unreaped leader pins the PGID."""
    deadline = time.monotonic() + 0.5
    last_error = None
    while True:
        try:
            os.killpg(group, signal.SIGKILL)
        except OSError as error:
            last_error = error
        if not posix_group_has_live_members(group):
            return
        if time.monotonic() >= deadline:
            raise RuntimeError("Owned POSIX process group did not drain") from last_error
        time.sleep(0.02)


UN_DRAINED_POSIX_CHILDREN: list[subprocess.Popen[bytes]] = []


def run_bounded_posix_subprocess(
    args: list[str], *, cwd: Path, env: dict[str, str], timeout: float
) -> subprocess.CompletedProcess[bytes]:
    """Drain POSIX pipes without reader threads or unbounded buffering."""
    process = subprocess.Popen(
        args,
        cwd=cwd,
        env=env,
        stdin=subprocess.DEVNULL,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        start_new_session=True,
    )
    stdout_tail = bytearray()
    stderr_tail = bytearray()
    streams = ((process.stdout, stdout_tail), (process.stderr, stderr_tail))
    selector = None
    timed_out = False
    open_pipes = False
    group_drained = False

    def drain_ready(wait_seconds: float) -> None:
        for key, _ in selector.select(wait_seconds):
            retained = key.data
            try:
                chunk = os.read(key.fd, 2048)
            except BlockingIOError:
                continue
            if chunk:
                retained[:] = (retained + chunk)[-2048:]
            else:
                selector.unregister(key.fileobj)

    try:
        selector = selectors.DefaultSelector()
        for stream, retained in streams:
            os.set_blocking(stream.fileno(), False)
            selector.register(stream, selectors.EVENT_READ, retained)
        deadline = time.monotonic() + timeout
        while os.waitid(os.P_PID, process.pid, os.WEXITED | os.WNOHANG | os.WNOWAIT) is None:
            remaining = deadline - time.monotonic()
            if remaining <= 0:
                timed_out = True
                break
            drain_ready(min(remaining, 0.05))
        # WNOWAIT keeps the leader present until descendants have drained,
        # preventing its process-group ID from being reused before signaling.
        drain_posix_group(process.pid)
        group_drained = True
        process.wait()
        grace_deadline = time.monotonic() + 0.25
        while selector.get_map() and time.monotonic() < grace_deadline:
            drain_ready(max(0, grace_deadline - time.monotonic()))
        open_pipes = bool(selector.get_map())
    finally:
        primary_error = sys.exc_info()[1]
        cleanup_errors = []
        try:
            if not group_drained:
                # The leader is still unreaped, so its PGID remains ours even
                # if the normal observer or group signal failed.
                try:
                    emergency_drain_posix_group(process.pid)
                    group_drained = True
                except Exception as error:
                    cleanup_errors.append(f"emergency group drain: {error}")
                    try:
                        os.kill(process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                    except OSError as leader_error:
                        cleanup_errors.append(f"direct leader kill: {leader_error}")
                    # Keep the unreaped leader alive as a custody pin if
                    # descendants cannot be proven drained.
                    UN_DRAINED_POSIX_CHILDREN.append(process)
            if group_drained:
                try:
                    process.wait(timeout=5)
                except Exception as error:
                    cleanup_errors.append(f"leader reap: {error}")
        finally:
            if selector is not None:
                selector.close()
            for stream, _ in streams:
                if stream is not None:
                    stream.close()
        if cleanup_errors:
            if primary_error is not None:
                primary_error.add_note("POSIX cleanup also failed: " + "; ".join(cleanup_errors))
            else:
                raise RuntimeError("POSIX cleanup failed: " + "; ".join(cleanup_errors))
    stdout = bytes(stdout_tail)
    stderr = bytes(stderr_tail)
    if timed_out:
        raise subprocess.TimeoutExpired(args, timeout, output=stdout, stderr=stderr)
    if open_pipes:
        error = RuntimeError("Owned subprocess exited but output pipes remained open")
        error.stdout = stdout
        error.stderr = stderr
        raise error
    if process.returncode != 0:
        raise subprocess.CalledProcessError(process.returncode, args, output=stdout, stderr=stderr)
    return subprocess.CompletedProcess(args, process.returncode, stdout=stdout, stderr=stderr)


def run_bounded_subprocess(
    args: list[str], *, cwd: Path, env: dict[str, str], timeout: float
) -> subprocess.CompletedProcess[bytes]:
    """Drain both child pipes, retaining only their last 2048 bytes."""
    windows = native_windows()
    if not windows:
        return run_bounded_posix_subprocess(args, cwd=cwd, env=env, timeout=timeout)
    job = windows_kill_job() if windows else None
    try:
        process = subprocess.Popen(
            args,
            cwd=cwd,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=not windows,
            creationflags=0x4 if windows else 0,  # CREATE_SUSPENDED before Job admission
        )
    except BaseException:
        if job is not None:
            job.close()
        raise
    stdout_tail = bytearray()
    stderr_tail = bytearray()

    def drain(stream, retained: bytearray) -> None:
        with stream:
            read_partial = getattr(stream, "read1", stream.read)
            while chunk := read_partial(2048):
                retained[:] = (retained + chunk)[-2048:]

    readers = [
        threading.Thread(target=drain, args=(process.stdout, stdout_tail), daemon=True),
        threading.Thread(target=drain, args=(process.stderr, stderr_tail), daemon=True),
    ]
    started_readers = []
    timed_out = False
    try:
        if job is not None:
            job.admit_and_resume(process)
        for reader in readers:
            reader.start()
            started_readers.append(reader)
        try:
            process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = True
    finally:
        try:
            if job is not None:
                job.close()
            else:
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
        finally:
            try:
                if process.poll() is None:
                    process.kill()
            except ProcessLookupError:
                pass
            process.wait()
            for reader in started_readers:
                reader.join(timeout=2)
            for stream in (process.stdout, process.stderr):
                if stream is not None and not stream.closed:
                    stream.close()
    stdout = bytes(stdout_tail)
    stderr = bytes(stderr_tail)
    if timed_out:
        raise subprocess.TimeoutExpired(args, timeout, output=stdout, stderr=stderr)
    if any(reader.is_alive() for reader in started_readers):
        raise RuntimeError("Bounded subprocess output readers did not finish")
    if process.returncode != 0:
        raise subprocess.CalledProcessError(process.returncode, args, output=stdout, stderr=stderr)
    return subprocess.CompletedProcess(args, process.returncode, stdout=stdout, stderr=stderr)


def run_restart_cpu_operation(root: Path) -> dict:
    runtime = root / "torch-versions" / TAG
    venv_python = (
        runtime / "venv" / "Scripts" / "python.exe"
        if os.name == "nt"
        else runtime / "venv" / "bin" / "python"
    )
    require(venv_python.is_file(), "persisted Torch venv Python", str(venv_python))
    operation = None
    try:
        operation = run_bounded_subprocess(
            [
                str(venv_python),
                "-I",
                "-c",
                "import json,torch; "
                "tensor=torch.arange(1,4,dtype=torch.int64,device='cpu'); "
                "print(json.dumps({'version':torch.__version__,'device':str(tensor.device),"
                "'result':int((tensor*tensor).sum().item())}))",
            ],
            cwd=root,
            env=backend_environment(root),
            timeout=60,
        )
        proof = json.loads(operation.stdout)
        require(
            isinstance(proof, dict)
            and isinstance(proof.get("version"), str)
            and proof["version"].split("+")[0] == "2.14.0"
            and proof.get("device") == "cpu"
            and type(proof.get("result")) is int
            and proof["result"] == 14,
            "fresh CPU tensor operation after restart",
            proof,
        )
        return {"version": proof["version"], "device": proof["device"], "result": proof["result"]}
    except Exception as error:

        def tail(value) -> str:
            if value is None:
                return ""
            if isinstance(value, bytes):
                value = value.decode("utf-8", errors="replace")
            return str(value)[-2048:]

        stdout = getattr(error, "stdout", None)
        stderr = getattr(error, "stderr", None)
        if operation is not None:
            stdout = operation.stdout
            stderr = operation.stderr
        report = {
            "failure_type": type(error).__name__,
            "message": tail(error)[-512:],
            "returncode": getattr(error, "returncode", None),
            "timeout_seconds": error.timeout
            if isinstance(error, subprocess.TimeoutExpired)
            else None,
            "stdout_tail": tail(stdout),
            "stderr_tail": tail(stderr),
        }
        report_path = root / "restart-cpu-operation-failure.json"
        try:
            report_path.write_text(
                json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8"
            )
            error.add_note(
                f"Fresh CPU operation diagnostics: {report_path}; "
                f"stderr tail: {report['stderr_tail'][-512:]}"
            )
        except OSError as report_error:
            error.add_note(f"Could not retain CPU operation diagnostics: {report_error}")
        raise


def exercise(base: str, root: Path, state: dict) -> dict:
    preflight_torch_release(base, state)
    options = rpc(
        base,
        "get_torch_release_options",
        {"tag": TAG},
        timeout=RELEASE_OPTIONS_RPC_TIMEOUT_SECONDS,
    )
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
        "version_preflight": state["version_preflight"],
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


def exercise_restart(base: str, root: Path, first: dict) -> dict:
    active = rpc(base, "get_active_version", {"appId": "torch"})
    require(
        active.get("success") is True and active.get("version") == TAG,
        "persisted active Torch version after restart",
        active,
    )
    snapshot = rpc(base, "get_runtime_profiles_snapshot")
    profiles = snapshot.get("snapshot", {}).get("profiles", [])
    matching = [profile for profile in profiles if profile.get("profile_id") == PROFILE_ID]
    require(
        snapshot.get("success") is True
        and len(matching) == 1
        and matching[0].get("provider") == "torch"
        and matching[0].get("provider_mode") == "torch_serve"
        and matching[0].get("management_mode") == "managed"
        and matching[0].get("enabled") is True
        and matching[0].get("device", {}).get("mode") == "cpu",
        "persisted managed CPU profile after restart",
        snapshot,
    )
    python_evidence = managed_python_evidence(root, {"python": first["python"]})
    identity_keys = (
        "cpython_version",
        "provider",
        "provider_version",
        "provider_archive_sha256",
        "target",
        "distribution_implementation",
        "distribution_catalog_key",
        "distribution_source_url",
        "executable_path",
        "executable_sha256",
    )
    require(
        all(python_evidence[key] == first[key] for key in identity_keys),
        "persisted managed Python identity after restart",
        python_evidence,
    )
    cpu_operation = run_restart_cpu_operation(root)
    probe = rpc(base, "get_torch_runtime_probe", {"tag": TAG}, timeout=60)
    capabilities = probe.get("capabilities", {})
    require(
        probe.get("status") == "passed"
        and probe.get("core_status") == "passed"
        and probe.get("stale") is False
        and capabilities.get("torch_import", {}).get("status") == "passed"
        and capabilities.get("torch_import", {}).get("version", "").split("+")[0] == "2.14.0"
        and capabilities.get("cpu_tensor", {}).get("status") == "passed"
        and capabilities.get("sidecar_app", {}).get("status") == "passed",
        "Torch identity and CPU probe after restart",
        probe,
    )
    trial = rpc(base, "trial_torch_runtime", {"tag": TAG, "profileId": PROFILE_ID}, timeout=100)
    require(
        trial.get("success") is True
        and trial.get("startupStatus") == "passed"
        and trial.get("healthStatus") == "passed"
        and trial.get("protocol") == 3
        and trial.get("startedByTrial") is True
        and isinstance(trial.get("generation"), str)
        and trial["generation"].isdigit(),
        "Torch sidecar trial after restart",
        trial,
    )
    stopped = rpc(
        base,
        "stop_runtime_profile_if_generation",
        {"profileId": PROFILE_ID, "generation": trial["generation"]},
    )
    require(
        stopped.get("success") is True and stopped.get("stopped") is True,
        "generation stop after restart",
        stopped,
    )
    return {
        "active_version": active["version"],
        "profile_id": matching[0]["profile_id"],
        "interpreter": "persisted",
        "managed_python": python_evidence,
        "cpu_operation": cpu_operation,
        "probe": "passed",
        "trial": "passed",
        "sidecar_protocol": trial["protocol"],
        "sidecar_generation": trial["generation"],
        "stop_result": stopped["stopped"],
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


def run_backend_session(binary: Path, root: Path, log_path: Path, action):
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
            return action(base)
        finally:
            primary_error = sys.exc_info()[1]
            shutdown_error = None
            try:
                if base is not None:
                    require(
                        process.poll() is None,
                        "backend remained live before shutdown",
                        {"returncode": process.returncode},
                    )
                    shutdown = rpc(base, "shutdown", timeout=60)
                    require(
                        shutdown.get("status") == "shutting_down" and not shutdown.get("errors"),
                        "managed profile shutdown",
                        shutdown,
                    )
            except BaseException as error:
                shutdown_error = error
            try:
                stop_backend(process)
            except BaseException as error:
                if primary_error is not None:
                    primary_error.add_note(f"Backend stop also failed: {error}")
                elif shutdown_error is not None:
                    shutdown_error.add_note(f"Backend stop also failed: {error}")
                else:
                    raise
            if shutdown_error is not None:
                if primary_error is not None:
                    primary_error.add_note(f"Backend shutdown also failed: {shutdown_error}")
                else:
                    raise shutdown_error


def main() -> None:
    if len(sys.argv) == 3 and sys.argv[1] == "--network-worker":
        payload = json.load(sys.stdin)
        if sys.argv[2] == "release":
            sys.stdout.buffer.write(worker_fetch_release())
        elif sys.argv[2] == "archive":
            worker_download_full_archive(payload["asset"], Path(payload["destination"]))
        else:
            raise RuntimeError("Unknown HTTPS worker operation")
        return
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
                FailureSummaryFixture,
                VersionPreflightFixture,
                EvidenceCollectionFixture,
                LauncherRootFinalizationFixture,
                RestartAcceptanceFixture,
                FullArchiveLicenseFixture,
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
        result = run_backend_session(
            binary, root, root / "rpc.log", lambda base: exercise(base, root, state)
        )
        restart = run_backend_session(
            binary,
            root,
            root / "rpc-restart.log",
            lambda base: exercise_restart(base, root, result),
        )
        acceptance = {
            "success": True,
            "cleanup_safe": True,
            **result,
            "restart": {"same_launcher_root": True, "shutdown": "graceful", **restart},
        }
        success_ready = True
    finally:
        if not success_ready:
            acceptance = {
                "success": False,
                "cleanup_safe": False,
                "retained_root": str(root),
            }
            if "version_preflight" in state:
                acceptance["version_preflight"] = state["version_preflight"]
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
                            {
                                "success": False,
                                "cleanup_safe": False,
                                "retained_root": str(root),
                                **(
                                    {"version_preflight": state["version_preflight"]}
                                    if "version_preflight" in state
                                    else {}
                                ),
                            },
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
                if "version_preflight" in state:
                    print(
                        "Torch version preflight: "
                        + json.dumps(state["version_preflight"], sort_keys=True),
                        file=sys.stderr,
                        flush=True,
                    )
                print(f"Retained launcher root: {root}", file=sys.stderr, flush=True)
            finalize_launcher_root(root, cleanup_safe=success_ready)


if __name__ == "__main__":
    main()
