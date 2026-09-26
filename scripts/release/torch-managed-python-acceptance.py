#!/usr/bin/env python3
"""Exercise a native CPU Torch install through a real, isolated pumas-rpc binary.

Usage: torch-managed-python-acceptance.py /path/to/pumas-rpc
This downloads a managed Python and Torch wheels into a disposable launcher root.
"""

import argparse
from contextlib import contextmanager
import hashlib
import io
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
import tarfile
import tempfile
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
            (runtime / "runtime.json").write_text("fixture runtime", encoding="utf-8")
            (root / "rpc.log").write_text("backend log", encoding="utf-8")
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
                {"rpc.log", "acceptance.json", "managed-python-licenses", *INSTALL_REPORTS},
            )
            self.assertEqual(json.loads((output / "acceptance.json").read_text()), result)
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
    log = root / "rpc.log"
    if log.is_file():
        shutil.copyfile(log, output / log.name)
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
                EvidenceCollectionFixture,
                LauncherRootFinalizationFixture,
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
