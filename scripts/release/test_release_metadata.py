"""Complete synthetic artifact inventories; no actual installer/model acceptance."""

import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location(
    "release_metadata", Path(__file__).with_name("write-release-metadata.py")
)
metadata = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(metadata)


class MetadataTests(unittest.TestCase):
    def fixture(self, root):
        repository, output = root / "repo", root / "candidate"
        output.mkdir()
        for name, data in {
            "package.json": b'{"version":"0.8.0-rc.1"}',
            "rust/Cargo.lock": b"locked Rust fixture",
            "pnpm-lock.yaml": b"locked JS fixture",
        }.items():
            path = repository / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
        for name in (
            "LICENSE",
            "scripts/release/artifact-plan.json",
            "scripts/release/onnx-runtime-pins.json",
            "scripts/release/licenses/onnxruntime-LICENSE",
            "scripts/release/licenses/onnxruntime-ThirdPartyNotices.txt",
        ):
            path = repository / name
            path.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(ROOT / name, path)
        for profile in ("", "-s3"):
            base = repository / f"docs/release-attribution/0.8.0-rc.1{profile}"
            base.mkdir(parents=True)
            notices = b"test-only collected attribution fixture" + profile.encode()
            (base / "THIRD-PARTY-NOTICES.txt").write_bytes(notices)
            (base / "inventory.json").write_text(
                json.dumps(
                    {
                        "input_sha256": {
                            "package.json": hashlib.sha256(
                                (repository / "package.json").read_bytes()
                            ).hexdigest()
                        },
                        "notices_sha256": hashlib.sha256(notices).hexdigest(),
                        "packages": [
                            {
                                "name": "test-source" + profile,
                                "version": "1.0",
                                "scope": "test-only input",
                                "license_declared": "MIT",
                                "source": "NOASSERTION",
                            }
                        ],
                    }
                )
            )
        plan = json.loads((repository / "scripts/release/artifact-plan.json").read_text())
        names = [
            entry["filename"].replace("{version}", "0.8.0-rc.1") for entry in plan["artifacts"]
        ]
        self.assertEqual(len(names), 16)
        for name in names:
            (output / name).write_bytes(b"test-only non-installable archive: " + name.encode())
        return repository, output, set(names)

    def test_every_final_artifact_and_metadata_byte_is_described(self):
        with tempfile.TemporaryDirectory() as temporary:
            repository, output, names = self.fixture(Path(temporary))
            metadata.write_metadata(
                output,
                "test-build",
                {"head": "a" * 40, "tree": "b" * 40, "builder": "test-only"},
                repository,
            )
            documents = list(output.glob("*.spdx.json"))
            self.assertEqual(len(documents), 16)
            for path in documents:
                document = json.loads(path.read_text())
                artifact = output / document["name"]
                self.assertFalse(document["packages"][0]["filesAnalyzed"])
                self.assertEqual(
                    document["packages"][0]["checksums"][0]["checksumValue"],
                    hashlib.sha256(artifact.read_bytes()).hexdigest(),
                )
            provenance = json.loads(next(output.glob("*.provenance.jsonl")).read_text())
            self.assertEqual({entry["name"] for entry in provenance["subject"]}, names)
            self.assertTrue(
                provenance["predicate"]["buildDefinition"]["internalParameters"]["unsigned"]
            )
            checksums = dict(
                line.split("  ", 1)[::-1]
                for line in (output / "checksums-sha256.txt").read_text().splitlines()
            )
            self.assertEqual(
                set(checksums), {p.name for p in output.iterdir()} - {"checksums-sha256.txt"}
            )
            self.assertEqual(len(checksums), 35)
            for name, sha in checksums.items():
                self.assertEqual(sha, hashlib.sha256((output / name).read_bytes()).hexdigest())
            combined = (output / "THIRD-PARTY-NOTICES-0.8.0-rc.1.txt").read_bytes()
            native = repository / "scripts/release/licenses"
            for name in ("onnxruntime-LICENSE", "onnxruntime-ThirdPartyNotices.txt"):
                lf = (native / name).read_bytes().replace(b"\r\n", b"\n")
                self.assertIn(lf, combined)
                self.assertIn(lf.replace(b"\n", b"\r\n"), combined)
            with self.assertRaisesRegex(ValueError, "fresh artifact-plan"):
                metadata.write_metadata(output, "test-build", {}, repository)

    def test_missing_extra_empty_and_symlink_artifacts_fail_before_metadata(self):
        for mutation in ("missing", "extra", "empty", "symlink"):
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as temporary:
                repository, output, names = self.fixture(Path(temporary))
                artifact = output / sorted(names)[0]
                if mutation == "missing":
                    artifact.unlink()
                elif mutation == "extra":
                    (output / "stale.exe").write_bytes(b"stale")
                elif mutation == "empty":
                    artifact.write_bytes(b"")
                else:
                    if not hasattr(Path, "symlink_to"):
                        self.skipTest("filesystem has no symlink support")
                    artifact.unlink()
                    try:
                        artifact.symlink_to(output / sorted(names)[1])
                    except OSError:
                        # Native Windows runners may lack symlink privilege;
                        # the other required mutation cases still execute.
                        self.skipTest("native filesystem denied symlink creation")
                with self.assertRaises(ValueError):
                    metadata.write_metadata(output, "test", {}, repository)
                self.assertFalse(list(output.glob("*.spdx.json")))

    def test_stale_attribution_fails_before_metadata(self):
        with tempfile.TemporaryDirectory() as temporary:
            repository, output, _ = self.fixture(Path(temporary))
            (repository / "package.json").write_text('{"version":"0.8.0-rc.1","changed":true}')
            with self.assertRaisesRegex(ValueError, "attribution inputs changed"):
                metadata.write_metadata(output, "test", {}, repository)
            self.assertFalse(list(output.glob("*.spdx.json")))

    def test_native_license_cannot_substitute_for_third_party_terms(self):
        with tempfile.TemporaryDirectory() as temporary:
            repository, output, _ = self.fixture(Path(temporary))
            base = repository / "scripts/release/licenses"
            shutil.copyfile(
                base / "onnxruntime-LICENSE", base / "onnxruntime-ThirdPartyNotices.txt"
            )
            with self.assertRaisesRegex(ValueError, "native notice differs"):
                metadata.write_metadata(output, "test", {}, repository)
            self.assertFalse(list(output.glob("*.spdx.json")))

    def test_unrepresented_platform_notice_cannot_be_silently_dropped(self):
        with tempfile.TemporaryDirectory() as temporary:
            repository, output, _ = self.fixture(Path(temporary))
            path = repository / "scripts/release/onnx-runtime-pins.json"
            pins = json.loads(path.read_text())
            pins["targets"]["windows-x86_64"]["notices"]["ONNX-NOTICES.txt"]["sha256"] = "f" * 64
            path.write_text(json.dumps(pins))
            with self.assertRaisesRegex(ValueError, "native notice differs"):
                metadata.write_metadata(output, "test", {}, repository)
            self.assertFalse(list(output.glob("*.spdx.json")))


if __name__ == "__main__":
    unittest.main()
