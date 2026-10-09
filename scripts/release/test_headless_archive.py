"""Archive byte/pin regressions with synthetic payloads, not native OS builds."""

import hashlib
import importlib.util
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch
import zipfile

SPEC = importlib.util.spec_from_file_location(
    "headless_archive", Path(__file__).with_name("make-headless-archive.py")
)
subject = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(subject)


class ArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.stage = self.root / "stage"
        self.stage.mkdir()
        self.payload = {
            "pumas-rpc": b"synthetic executable\n",
            "LICENSE.txt": b"project license\n",
            "THIRD-PARTY-NOTICES.txt": b"third party licenses\n",
        }
        for name, data in self.payload.items():
            (self.stage / name).write_bytes(data)

    def command(self, *arguments):
        return subprocess.run(
            [sys.executable, str(Path(subject.__file__)), *arguments],
            capture_output=True,
            timeout=15,
        )

    def test_tar_pin_survives_location_permissions_and_timestamp_changes(self):
        first, second = self.root / "first.tar.gz", self.root / "second.tar.gz"
        for path in self.stage.iterdir():
            path.chmod(0o600)
            os.utime(path, (111, 111))
        subject.write_archive(self.stage, first)
        for path in self.stage.iterdir():
            path.chmod(0o777)
            os.utime(path, (999999, 999999))
        subject.write_archive(self.stage, second)
        self.assertEqual(first.read_bytes(), second.read_bytes())
        self.assertEqual(first.read_bytes()[3:8], b"\0" * 5)  # No filename flag/mtime.
        with tarfile.open(first) as archive:
            self.assertEqual(archive.getnames(), sorted(self.payload))
            for member in archive:
                self.assertTrue(member.isfile())
                self.assertEqual(archive.extractfile(member).read(), self.payload[member.name])
                self.assertEqual((member.uid, member.gid, member.mtime), (0, 0, 0))
                self.assertEqual((member.uname, member.gname, member.pax_headers), ("", "", {}))
                self.assertEqual(member.mode, 0o755 if member.name == "pumas-rpc" else 0o644)

    def test_zip_format_fixture_is_stable_not_windows_native_qualification(self):
        (self.stage / "pumas-rpc").rename(self.stage / "pumas-rpc.exe")
        first, second = self.root / "first.zip", self.root / "second.zip"
        subject.write_archive(self.stage, first)
        for path in self.stage.iterdir():
            path.chmod(0o777)
            os.utime(path, (999999, 999999))
        subject.write_archive(self.stage, second)
        self.assertEqual(first.read_bytes(), second.read_bytes())
        with zipfile.ZipFile(first) as archive:
            self.assertEqual(len(archive.infolist()), 3)
            for member in archive.infolist():
                self.assertEqual(member.date_time, (1980, 1, 1, 0, 0, 0))
                self.assertEqual(member.create_system, 3)
                self.assertEqual((member.extra, member.comment), (b"", b""))

    def test_wrong_or_malformed_binary_pin_creates_no_output_directory(self):
        for pin in ("0" * 64, "bad", "A" * 64):
            output = self.root / ("output-" + pin)
            result = self.command(
                "--binary",
                str(self.stage / "pumas-rpc"),
                "--binary-sha256",
                pin,
                "--os",
                "linux",
                "--output-dir",
                str(output),
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(output.exists())

    def test_cli_admits_exact_pin_preserves_payload_and_refuses_overwrite(self):
        output = self.root / "output"
        pin = hashlib.sha256(self.payload["pumas-rpc"]).hexdigest()
        arguments = (
            "--binary",
            str(self.stage / "pumas-rpc"),
            "--binary-sha256",
            pin,
            "--os",
            "linux",
            "--output-dir",
            str(output),
        )
        result = self.command(*arguments)
        self.assertEqual(result.returncode, 0, result.stderr.decode())
        archive = next(output.glob("*.tar.gz"))
        original = archive.read_bytes()
        self.assertIn(hashlib.sha256(original).hexdigest().encode(), result.stdout)
        with tarfile.open(archive) as bundle:
            self.assertEqual(set(bundle.getnames()), set(self.payload))
            self.assertEqual(bundle.extractfile("pumas-rpc").read(), self.payload["pumas-rpc"])
        again = self.command(*arguments)
        self.assertNotEqual(again.returncode, 0)
        self.assertEqual(archive.read_bytes(), original)

    def test_alias_and_wrong_basename_refuse_without_creating_output(self):
        renamed = self.root / "wrong-name"
        renamed.write_bytes(self.payload["pumas-rpc"])
        linked = self.root / "pumas-rpc"
        binaries = [renamed]
        # Windows symlink privileges are not a fixture prerequisite.
        if os.name != "nt":
            linked.symlink_to(self.stage / "pumas-rpc")
            binaries.append(linked)
        for binary in binaries:
            output = self.root / (binary.name + "-output")
            result = self.command(
                "--binary",
                str(binary),
                "--os",
                "linux",
                "--output-dir",
                str(output),
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(output.exists())

    def test_failed_assembly_removes_only_its_exclusively_created_output(self):
        output = self.root / "failed.tar.gz"
        with patch.object(
            subject.tarfile.TarFile, "addfile", side_effect=OSError("fixture failure")
        ):
            with self.assertRaises(OSError):
                subject.write_archive(self.stage, output)
        self.assertFalse(output.exists())
        output.write_bytes(b"retained existing archive")
        with self.assertRaises(FileExistsError):
            subject.write_archive(self.stage, output)
        self.assertEqual(output.read_bytes(), b"retained existing archive")

    def test_final_output_flush_close_failure_removes_new_archive(self):
        output = self.root / "late-close.tar.gz"
        real_open = Path.open

        class LateClose:
            def __init__(self, stream):
                self.stream = stream

            def __enter__(self):
                return self.stream

            def __exit__(self, *_):
                self.stream.close()
                raise OSError("injected final flush/close failure")

        def opened(path, *arguments, **options):
            stream = real_open(path, *arguments, **options)
            return LateClose(stream) if path == output else stream

        with patch.object(Path, "open", opened):
            with self.assertRaisesRegex(OSError, "final flush/close"):
                subject.write_archive(self.stage, output)
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
