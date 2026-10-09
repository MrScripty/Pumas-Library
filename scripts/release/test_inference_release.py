"""Synthetic archives and real subprocess environment checks; no model inference."""

import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest.mock import patch

import headless_inference_ci as ci
import onnx_runtime_stage as stage
import packaged_onnx_support as support


class ReleaseTests(unittest.TestCase):
    @unittest.skipUnless(sys.platform == "linux", "Linux packaged subprocess contract")
    def test_environment_does_not_inherit_runtime_or_loader_overrides(self):
        with (
            tempfile.TemporaryDirectory() as tmp,
            patch.dict(
                os.environ,
                {
                    "ORT_DYLIB_PATH": "/ambient/ort.so",
                    "LD_PRELOAD": "/ambient/inject.so",
                    "DYLD_LIBRARY_PATH": "/ambient",
                    "PUMAS_LAUNCHER_ROOT": "/ambient",
                    "PYTHONPATH": "/ambient",
                    "HTTPS_PROXY": "http://ambient",
                },
            ),
        ):
            environment = support.child_environment(Path(tmp))
            output = subprocess.check_output(
                [sys.executable, "-I", "-c", "import os,json;print(json.dumps(dict(os.environ)))"],
                env=environment,
                text=True,
            )
            observed = json.loads(output)
            self.assertFalse(any("ambient" in value for value in observed.values()))
            self.assertNotIn("ORT_DYLIB_PATH", observed)
            self.assertEqual(observed["PUMAS_REGISTRY_DB_PATH"], str(Path(tmp) / "registry.db"))

    def test_stage_hashes_snapshot_and_rejects_drift_links_and_duplicates(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for case in ("valid", "hash", "member", "link", "duplicate"):
                archive = root / f"{case}.tgz"
                data = b"synthetic native bytes, not ORT"
                with tarfile.open(archive, "w:gz") as stream:
                    member = tarfile.TarInfo("upstream/lib/runtime.so")
                    member.size = len(data)
                    if case == "link":
                        member.type = tarfile.SYMTYPE
                        member.linkname = "/outside"
                        member.size = 0
                    stream.addfile(member, io.BytesIO(data))
                    if case == "duplicate":
                        stream.addfile(member, io.BytesIO(data))
                pin = {
                    "member": member.name,
                    "sha256": hashlib.sha256(data).hexdigest(),
                    "bytes": len(data),
                }
                pins = {
                    "version": "1.24.2",
                    "targets": {
                        "linux-x86_64": {
                            "url": "https://example.invalid/synthetic.tgz",
                            "target": "x86_64-unknown-linux-gnu",
                            "archive_sha256": hashlib.sha256(archive.read_bytes()).hexdigest(),
                            "archive_bytes": archive.stat().st_size,
                            "files": {"libonnxruntime.so": pin},
                            "notices": {},
                        }
                    },
                }
                if case == "hash":
                    pins["targets"]["linux-x86_64"]["archive_sha256"] = "0" * 64
                if case == "member":
                    pin["sha256"] = "0" * 64
                destination = root / case
                if case == "valid":
                    result = stage.stage("linux-x86_64", archive, destination, pins)
                    self.assertEqual((destination / "libonnxruntime.so").read_bytes(), data)
                    self.assertEqual(result["files"]["libonnxruntime.so"]["sha256"], pin["sha256"])
                    with self.assertRaises(ValueError):
                        stage.stage("linux-x86_64", archive, destination, pins)
                else:
                    with self.assertRaises(ValueError):
                        stage.stage("linux-x86_64", archive, destination, pins)
                    self.assertFalse(destination.exists())

    def test_current_unbumped_source_cannot_be_relabelled(self):
        with tempfile.TemporaryDirectory() as tmp:
            repository = Path(tmp)
            (repository / "rust").mkdir()
            (repository / "rust/Cargo.toml").write_text('[workspace.package]\nversion="0.7.0"\n')
            with patch.object(
                ci.build, "source_identity", return_value={"head": "a" * 40, "tree": "b" * 40}
            ):
                with self.assertRaisesRegex(ValueError, "v0.8"):
                    ci.contract_for(repository, "linux-x86_64", b"{}", "test")
                (repository / "rust/Cargo.toml").write_text(
                    '[workspace.package]\nversion="0.8.0"\n'
                )
                contract = ci.contract_for(repository, "linux-x86_64", b"{}", "test")
                self.assertEqual(
                    ci.build.core_projection(contract["expected"]["build_info"]),
                    contract["expected"]["core_build_info"],
                )

    @unittest.skipUnless(sys.platform == "linux", "Linux /proc mapping contract")
    def test_linux_mapping_refuses_ambient_or_unobserved_runtime(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            runtime = root / "libonnxruntime.so"
            runtime.write_bytes(b"synthetic")
            with patch.object(
                support, "verify_runtime", return_value={"files": {runtime.name: {}}}
            ):
                for maps in ("", "0000-1000 r-xp 0 00:00 1 /ambient/libonnxruntime.so\n"):
                    with patch.object(Path, "read_text", return_value=maps):
                        with self.assertRaises(ValueError):
                            support.loaded_runtime(123, root)
                stat = runtime.stat()
                maps = f"0000-1000 r-xp 0 {os.major(stat.st_dev):x}:{os.minor(stat.st_dev):x} {stat.st_ino} {runtime}\n"
                with patch.object(Path, "read_text", return_value=maps):
                    self.assertEqual(
                        support.loaded_runtime(123, root)["mapping"],
                        "packaged-file-identity-observed",
                    )


if __name__ == "__main__":
    unittest.main()
