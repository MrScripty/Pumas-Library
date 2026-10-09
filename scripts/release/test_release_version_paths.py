"""Candidate versions select matching notices and artifact metadata."""

import importlib.util
import json
from pathlib import Path
import tarfile
import tempfile
import unittest
from unittest.mock import patch


def module(name):
    spec = importlib.util.spec_from_file_location(name, Path(__file__).with_name(name + ".py"))
    result = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(result)
    return result


class VersionPathsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.version = "0.8.0-rc.1"
        (self.root / "package.json").write_text(json.dumps({"version": self.version}))
        self.notices = self.root / "docs/release-attribution" / self.version
        self.notices.mkdir(parents=True)
        (self.notices / "THIRD-PARTY-NOTICES.txt").write_text("candidate legal text")
        (self.root / "LICENSE").write_text("project legal text")

    def test_headless_archive_uses_source_version_and_refuses_relabel(self):
        producer = module("make-headless-archive")
        binary = self.root / "pumas-rpc"
        binary.write_text("synthetic bytes, never executed")
        output = self.root / "out"
        argv = [
            "make-headless-archive.py",
            "--binary",
            str(binary),
            "--output-dir",
            str(output),
            "--os",
            "linux",
        ]
        with patch.object(producer, "ROOT", self.root), patch("sys.argv", argv):
            producer.main()
        archive = next(output.glob("*.tar.gz"))
        self.assertIn(self.version, archive.name)
        with tarfile.open(archive) as handle:
            self.assertEqual(
                handle.extractfile("THIRD-PARTY-NOTICES.txt").read(), b"candidate legal text"
            )
        with (
            patch.object(producer, "ROOT", self.root),
            patch("sys.argv", argv + ["--version", "0.7.0"]),
            self.assertRaisesRegex(SystemExit, "match the source manifest"),
        ):
            producer.main()

    def test_linux_metadata_preserves_candidate_version_everywhere(self):
        metadata = module("write-linux-metadata")
        inventory = {
            "input_sha256": {},
            "notices_sha256": metadata.hash_file(self.notices / "THIRD-PARTY-NOTICES.txt"),
            "packages": [],
        }
        (self.notices / "inventory.json").write_text(json.dumps(inventory))
        output = self.root / "out"
        output.mkdir()
        extracted = self.root / "extracted"
        for name, relative in (
            (f"Pumas.Library-{self.version}.AppImage", "appimage/squashfs-root"),
            (f"pumas-library-electron_{self.version}_amd64.deb", "deb/opt/Pumas Library"),
        ):
            (output / name).write_text("synthetic artifact, never executed")
            resources = extracted / relative / "resources"
            resources.mkdir(parents=True)
            (resources / "THIRD-PARTY-NOTICES.txt").write_text("candidate legal text")
        with (
            patch.object(metadata, "ROOT", self.root),
            patch.object(metadata.subprocess, "check_output", side_effect=["a" * 40, b""]),
        ):
            metadata.write_metadata(output, extracted)
        for path in output.glob("*.spdx.json"):
            self.assertEqual(
                json.loads(path.read_text())["packages"][0]["versionInfo"], self.version
            )
        self.assertEqual(len(list(output.glob("*.spdx.json"))), 2)
        self.assertTrue((output / f"THIRD-PARTY-NOTICES-{self.version}.txt").is_file())
        provenance = output / f"pumas-library-{self.version}.provenance.jsonl"
        self.assertTrue(provenance.is_file())
        self.assertTrue(
            all(
                self.version in item["name"]
                for item in json.loads(provenance.read_text())["subject"]
            )
        )
        self.assertFalse(any("0.7.0" in path.name for path in output.iterdir()))


if __name__ == "__main__":
    unittest.main()
