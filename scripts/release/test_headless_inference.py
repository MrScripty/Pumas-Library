"""Packaging/admission tests. Controlled bytes/process, never inference evidence."""

import copy
import hashlib
import io
import json
import os
import shutil
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest import mock

import headless_inference as package


def fixture(root, target="linux-x86_64"):
    definition = package.TARGETS[target]
    schema = package.canonical({"fixture": "controlled-contract-not-Pumas"})
    expected = {
        "version": "0.8.0-test",
        "source_commit": "a" * 40,
        "source_tree": "b" * 40,
        "build_id": "controlled-fixture",
        "protocol_version": 1,
        "schema_sha256": hashlib.sha256(schema).hexdigest(),
        "inference_enabled": True,
        "features": ["inference-plugins", "s3"],
        "modalities": ["fixture_modality"],
    }
    contract = {
        "schema_version": 1,
        "expected": expected,
        "requests": [
            {"path": "/fixture-handshake", "bindings": {name: "/" + name for name in expected}}
        ],
    }
    binary = root / "binary"
    binary.write_bytes(b"controlled-binary-not-a-Pumas-release")
    native = root / "native"
    native.write_bytes(b"controlled-runtime-not-ORT")
    license_file = root / "license"
    license_file.write_text("controlled project license fixture\n")
    notices = root / "notices"
    notices.write_text("controlled runtime notice fixture\n")
    build = {
        "target": definition["rust_target"],
        "host": definition["rust_target"],
        "profile": "release",
        "features": expected["features"],
        "source": {"head": expected["source_commit"], "tree": expected["source_tree"]},
        "build_id": expected["build_id"],
        "version": expected["version"],
        "inference_enabled": True,
        "binary_sha256": package.sha256(binary),
        "command": ["controlled-fixture"],
        "rustc": "controlled-fixture-not-a-real-Rust-build",
    }
    runtime = {
        "target": definition["rust_target"],
        "version": "1.24.2",
        "loader_entry": definition["runtime_entry"],
        "source_archive_sha256": "c" * 64,
        "files": {
            definition["runtime_entry"]: {
                "sha256": package.sha256(native),
                "bytes": native.stat().st_size,
            }
        },
    }
    inputs = {
        definition["binary"]: binary,
        definition["runtime_entry"]: native,
        "LICENSE.txt": license_file,
        "THIRD-PARTY-NOTICES.txt": notices,
    }
    output = root / f"pumas-rpc-inference-{expected['version']}-{target}.{definition['format']}"
    return inputs, build, runtime, contract, schema, output


class ArchiveTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)

    def test_roundtrip_both_archive_formats_and_no_ambient_payload(self):
        for target in package.TARGETS:
            with self.subTest(target=target):
                root = self.root / target
                root.mkdir()
                inputs, build, runtime, contract, schema, output = fixture(root, target)
                (root / "credentials-not-an-input").write_text("must never be copied")
                manifest, digest = package.assemble(
                    inputs, build, runtime, contract, schema, output
                )
                extracted = root / "consumer"
                observed = package.extract_verified(output, digest, extracted)
                self.assertEqual(manifest, observed)
                self.assertEqual(
                    set(observed["files"]) | {"manifest.json", "SHA256SUMS"},
                    {path.name for path in extracted.iterdir()},
                )
                self.assertEqual(observed["qualification"], "unverified_candidate")

    def test_feature_revision_schema_runtime_and_content_mismatches_refused(self):
        cases = [
            ("features", lambda build, runtime, contract: build.update(features=["s3"])),
            ("source", lambda build, runtime, contract: build["source"].update(head="d" * 40)),
            ("runtime", lambda build, runtime, contract: runtime.update(version="1.24.3")),
            (
                "inference",
                lambda build, runtime, contract: contract["expected"].update(
                    inference_enabled=False
                ),
            ),
            (
                "schema",
                lambda build, runtime, contract: contract["expected"].update(
                    schema_sha256="e" * 64
                ),
            ),
            ("binary", lambda build, runtime, contract: build.update(binary_sha256="f" * 64)),
        ]
        for name, mutate in cases:
            with self.subTest(name=name):
                root = self.root / name
                root.mkdir()
                inputs, build, runtime, contract, schema, output = fixture(root)
                mutate(build, runtime, contract)
                with self.assertRaises(ValueError):
                    package.assemble(inputs, build, runtime, contract, schema, output)
                self.assertFalse(output.exists())

    def test_missing_extra_symlink_and_case_collision_inputs_refused(self):
        for name in ["missing", "extra", "symlink", "case", "reserved"]:
            root = self.root / name
            root.mkdir()
            inputs, build, runtime, contract, schema, output = fixture(root)
            if name == "missing":
                del inputs["libonnxruntime.so"]
            elif name == "extra":
                inputs["unexpected.dll"] = inputs["pumas-rpc"]
            elif name == "case":
                inputs["PUMAS-RPC"] = inputs["pumas-rpc"]
            elif name == "reserved":
                inputs["build-record.json"] = inputs["pumas-rpc"]
            else:
                link = root / "link"
                link.symlink_to(inputs["pumas-rpc"])
                inputs["pumas-rpc"] = link
            with self.subTest(name=name), self.assertRaises(ValueError):
                package.assemble(inputs, build, runtime, contract, schema, output)

    def test_trusted_hash_and_archive_hash_mismatch_before_execution(self):
        path = self.root / "record.json"
        path.write_text('{"schema_version":1}')
        with self.assertRaisesRegex(ValueError, "trusted metadata hash mismatch"):
            package.trusted_json(path, "0" * 64)
        self.assertEqual(package.trusted_json(path, package.sha256(path)), {"schema_version": 1})
        inputs, build, runtime, contract, schema, output = fixture(self.root)
        package.assemble(inputs, build, runtime, contract, schema, output)
        with self.assertRaisesRegex(ValueError, "archive SHA256 mismatch"):
            package.extract_verified(output, "0" * 64, self.root / "consumer")
        self.assertFalse((self.root / "consumer").exists())

    def test_archive_traversal_duplicate_and_links_refused(self):
        for kind in ["traversal", "duplicate", "symlink"]:
            archive = self.root / (kind + ".tar.gz")
            with tarfile.open(archive, "w:gz") as stream:
                member = tarfile.TarInfo("../escape" if kind == "traversal" else "duplicate")
                member.size = 1
                if kind == "symlink":
                    member.type = tarfile.SYMTYPE
                    member.linkname = "/outside"
                    stream.addfile(member)
                else:
                    stream.addfile(member, io.BytesIO(b"x"))
                    if kind == "duplicate":
                        stream.addfile(member, io.BytesIO(b"x"))
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                package.extract_verified(archive, package.sha256(archive), self.root / kind)
            self.assertFalse((self.root / "escape").exists())

    def test_zip_symlink_refused_and_duplicate_json_refused(self):
        archive = self.root / "bad.zip"
        with zipfile.ZipFile(archive, "w") as stream:
            member = zipfile.ZipInfo("link")
            member.external_attr = 0o120777 << 16
            stream.writestr(member, "/outside")
        with self.assertRaises(ValueError):
            package.extract_verified(archive, package.sha256(archive), self.root / "consumer")
        with self.assertRaisesRegex(ValueError, "duplicate JSON key"):
            package.decode(b'{"schema_version":1,"schema_version":2}')
        with self.assertRaisesRegex(ValueError, "nonfinite JSON number"):
            package.decode(b'{"value":NaN}')

    def test_checksum_collision_and_invalid_schema_refused(self):
        inputs, build, runtime, contract, schema, output = fixture(self.root)
        checksum = output.with_name(output.name + ".sha256")
        checksum.write_text("existing trusted checksum")
        with self.assertRaisesRegex(ValueError, "overwrite archive checksum"):
            package.assemble(inputs, build, runtime, contract, schema, output)
        self.assertFalse(output.exists())
        self.assertEqual(checksum.read_text(), "existing trusted checksum")
        checksum.unlink()
        with self.assertRaises(json.JSONDecodeError):
            package.assemble(inputs, build, runtime, contract, b"invalid schema", output)
        self.assertFalse(output.exists())

    def test_member_count_and_pointer_admission_bounds(self):
        archive = self.root / "too-many.tar.gz"
        with tarfile.open(archive, "w:gz") as stream:
            for index in range(131):
                member = tarfile.TarInfo(f"member-{index}")
                member.size = 1
                stream.addfile(member, io.BytesIO(b"x"))
        with self.assertRaisesRegex(ValueError, "bounded archive member count"):
            package.extract_verified(archive, package.sha256(archive), self.root / "consumer")
        _, _, _, contract, _, _ = fixture(self.root)
        contract["requests"][0]["bindings"]["version"] = "/invalid~2escape"
        with self.assertRaisesRegex(ValueError, "pointer escape"):
            package.validate_contract(contract)
        with self.assertRaisesRegex(ValueError, "array index"):
            package.pointer(["item"], "/-1")

    def test_metadata_and_archive_path_replacement_cannot_change_verified_bytes(self):
        path = self.root / "record.json"
        path.write_bytes(b'{"identity":"trusted"}')
        digest = package.sha256(path)
        read_metadata = package.metadata_bytes

        def replace_after_read(source):
            data = read_metadata(source)
            path.write_bytes(b'{"identity":"replacement"}')
            return data

        with mock.patch.object(package, "metadata_bytes", side_effect=replace_after_read):
            self.assertEqual(package.trusted_json(path, digest), {"identity": "trusted"})
        if os.name == "nt":
            # Windows denies replacing an open archive. POSIX permits this race.
            return
        inputs, build, runtime, contract, schema, output = fixture(self.root)
        _, digest = package.assemble(inputs, build, runtime, contract, schema, output)
        file_digest = hashlib.file_digest
        replaced = False

        def replace_after_hash(stream, algorithm):
            nonlocal replaced
            result = file_digest(stream, algorithm)
            if not replaced:
                replaced = True
                replacement = output.with_name("replacement")
                replacement.write_bytes(b"untrusted replacement archive")
                replacement.replace(output)
            return result

        with mock.patch.object(package.hashlib, "file_digest", side_effect=replace_after_hash):
            observed = package.extract_verified(output, digest, self.root / "consumer")
        self.assertEqual(observed["build"], build)

    def test_oversized_first_tar_member_rejected_before_skipping_payload(self):
        archive = self.root / "oversized.tar.gz"
        with tarfile.open(archive, "w:gz") as stream:
            member = tarfile.TarInfo("oversized")
            member.size = package.MAX_PACKAGE + 1
            # A header alone is sufficient: rejection must precede seeking/decompression.
            stream.fileobj.write(member.tobuf())
        with self.assertRaisesRegex(ValueError, "archive payload exceeds bounded size"):
            package.extract_verified(archive, package.sha256(archive), self.root / "consumer")

    def test_tar_extension_header_rejected_before_reading_its_body(self):
        archive = self.root / "extension.tar.gz"
        with tarfile.open(archive, "w:gz") as stream:
            member = tarfile.TarInfo("extended")
            member.type = tarfile.XHDTYPE
            member.size = package.MAX_PACKAGE + 1
            stream.fileobj.write(member.tobuf())
        with self.assertRaisesRegex(ValueError, "nonregular tar header"):
            package.extract_verified(archive, package.sha256(archive), self.root / "consumer")

    def test_payload_mutation_even_with_new_outer_digest_refused(self):
        inputs, build, runtime, contract, schema, output = fixture(self.root, "windows-x86_64")
        package.assemble(inputs, build, runtime, contract, schema, output)
        changed = self.root / "changed.zip"
        with zipfile.ZipFile(output) as source, zipfile.ZipFile(changed, "w") as sink:
            for name in source.namelist():
                data = source.read(name)
                if name == "pumas-rpc.exe":
                    data = b"x" * len(data)
                sink.writestr(name, data)
        with self.assertRaisesRegex(ValueError, "payload hash mismatch"):
            package.extract_verified(changed, package.sha256(changed), self.root / "consumer")
        self.assertFalse((self.root / "consumer").exists())


@unittest.skipUnless(
    sys.platform == "linux" and shutil.which("cc"),
    "controlled native process requires Linux and cc; not platform qualification",
)
class ConsumerProcessTests(unittest.TestCase):
    def test_exact_extracted_process_starts_matches_and_stops(self):
        self.run_process_fixture()

    def test_live_mismatched_schema_refused_after_exact_extraction(self):
        self.run_process_fixture({"schema_sha256": "f" * 64})

    def test_live_boolean_integer_and_revision_mismatches_refused(self):
        for mutation in (
            {"inference_enabled": 1},
            {"protocol_version": True},
            {"source_commit": "d" * 40},
        ):
            with self.subTest(mutation=mutation):
                self.run_process_fixture(mutation)

    def run_process_fixture(self, mismatch=None):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            inputs, build, runtime, contract, schema, output = fixture(root)
            response = copy.deepcopy(contract["expected"])
            if mismatch:
                response.update(mismatch)
            source = Path(__file__).with_name("fixtures") / "headless-rpc-fixture.c"
            binary = inputs["pumas-rpc"]
            # Argument-array invocation: fixture JSON is never passed through a shell.
            subprocess.run(
                [
                    "cc",
                    "-std=c11",
                    "-D_POSIX_C_SOURCE=200809L",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DFIXTURE_RESPONSE=" + json.dumps(json.dumps(response)),
                    str(source),
                    "-o",
                    str(binary),
                ],
                check=True,
                capture_output=True,
            )
            build["binary_sha256"] = package.sha256(binary)
            manifest, digest = package.assemble(inputs, build, runtime, contract, schema, output)
            extracted = root / "consumer"
            package.extract_verified(output, digest, extracted)
            if mismatch:
                with self.assertRaisesRegex(
                    ValueError, "live schema/build/features/modalities mismatch"
                ):
                    package.smoke(extracted, manifest, contract, timeout=5)
            else:
                ambient = {
                    "LD_AUDIT": "must-not-load",
                    "DYLD_INSERT_LIBRARIES": "must-not-load",
                    "ORT_DYLIB_PATH": "must-not-load",
                    "PUMAS_FIXTURE_AMBIENT": "must-not-read",
                }
                with mock.patch.dict(os.environ, ambient):
                    evidence = package.smoke(extracted, manifest, contract, timeout=5)
                self.assertEqual(evidence["compatibility"], "passed")
                self.assertEqual(evidence["runtime_loading"], "unqualified")
                self.assertEqual(evidence["real_inference"], "unqualified")


if __name__ == "__main__":
    unittest.main()
