"""Packaging/admission tests. Controlled bytes/process, never inference evidence."""

import copy
import hashlib
import io
import json
import os
import shutil
import signal
import time
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
        "schema_sha256": hashlib.sha256(schema).hexdigest(),
        "inference_enabled": True,
        "features": ["inference-plugins", "s3"],
    }
    core = {
        "build_info_schema_version": 1,
        "component": "pumas-library",
        "package_version": expected["version"],
        "build_id": expected["build_id"],
        "source_revision": expected["source_commit"],
        "target": definition["rust_target"],
        "compiled_features": [
            "pumas-library/hf-client",
            "pumas-library/process-manager",
            "pumas-library/gpu-monitor",
            "pumas-library/onnx-runtime",
            "pumas-library/s3",
        ],
        "protocols": [{"name": "pumas.local-ipc", "versions": [1]}],
        "schemas": [
            {"name": name, "version": version}
            for name, version in package.discovery.CORE_SCHEMAS.items()
        ],
    }
    rpc = copy.deepcopy(core)
    rpc["component"] = "pumas-rpc"
    rpc["compiled_features"] += ["pumas-rpc/inference-plugins", "pumas-rpc/s3"]
    rpc["protocols"].append({"name": "pumas.local-http", "versions": [1]})
    rpc["schemas"].append({"name": "pumas.http-advertisement", "version": 1})
    rpc["schemas"].append({"name": "pumas.http-admission-fence", "version": 1})
    rpc["schemas"].append({"name": "pumas.http-owner-retention", "version": 1})
    expected.update(build_info=rpc, core_build_info=core)
    contract = {"schema_version": 2, "expected": expected}
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
        # Declarative admission fixtures, never evidence that Cargo ran.
        "command": [
            "cargo",
            "build",
            "--locked",
            "--offline",
            "--manifest-path",
            "rust/Cargo.toml",
            "-p",
            "pumas-rpc",
            "--release",
            "--no-default-features",
            "--features",
            "s3,inference-plugins",
            "--target",
            definition["rust_target"],
        ],
        "rustc": "rustc 0.0.0 (0000000 2000-01-01)",
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

    def test_canary_extra_metadata_rejected_before_serialization(self):
        canary = "DUMMY_METADATA_CANARY_NOT_A_CREDENTIAL"
        cases = {
            "build": lambda build, runtime, contract: build.update(unexpected_secret=canary),
            "source": lambda build, runtime, contract: build["source"].update(
                unexpected_secret=canary
            ),
            "runtime": lambda build, runtime, contract: runtime.update(unexpected_secret=canary),
            "runtime_item": lambda build, runtime, contract: runtime["files"][
                "libonnxruntime.so"
            ].update(unexpected_secret=canary),
            "contract": lambda build, runtime, contract: contract.update(unexpected_secret=canary),
            "expected": lambda build, runtime, contract: contract["expected"].update(
                unexpected_secret=canary
            ),
            "rpc_build": lambda build, runtime, contract: contract["expected"]["build_info"].update(
                unexpected_secret=canary
            ),
            "schema": lambda build, runtime, contract: contract["expected"]["core_build_info"][
                "schemas"
            ][0].update(unexpected_secret=canary),
        }
        for name, mutate in cases.items():
            with self.subTest(name=name):
                root = self.root / name
                root.mkdir()
                inputs, build, runtime, contract, schema, output = fixture(root)
                mutate(build, runtime, contract)
                with mock.patch.object(package, "canonical", wraps=package.canonical) as serialize:
                    with self.assertRaises(ValueError):
                        package.assemble(inputs, build, runtime, contract, schema, output)
                    self.assertFalse(
                        any(canary in json.dumps(call.args[0]) for call in serialize.call_args_list)
                    )
                self.assertFalse(output.exists())
                self.assertFalse(output.with_name(output.name + ".partial").exists())

    def test_metadata_snapshot_survives_input_mutation_during_staging(self):
        inputs, build, runtime, contract, schema, output = fixture(self.root)
        original_build, original_runtime, original_contract = copy.deepcopy(
            (build, runtime, contract)
        )
        canary = "DUMMY_METADATA_CANARY_NOT_A_CREDENTIAL"
        copy_payload = shutil.copyfile
        written = []
        write_bytes = Path.write_bytes

        def mutate_inputs(source, destination):
            # Model another caller retaining the input dictionaries while a
            # potentially large native payload is staged after admission.
            build["unexpected_secret"] = canary
            build["source"]["unexpected_secret"] = canary
            build["features"].append(canary)
            build["command"].append(canary)
            runtime["unexpected_secret"] = canary
            runtime["files"]["libonnxruntime.so"]["unexpected_secret"] = canary
            contract["unexpected_secret"] = canary
            contract["expected"]["build_info"]["compiled_features"].append(canary)
            contract["expected"]["core_build_info"]["schemas"][0]["name"] = canary
            return copy_payload(source, destination)

        def capture_write(path, data):
            written.append(data)
            return write_bytes(path, data)

        with mock.patch.object(package.shutil, "copyfile", side_effect=mutate_inputs):
            with mock.patch.object(Path, "write_bytes", capture_write):
                manifest, digest = package.assemble(
                    inputs, build, runtime, contract, schema, output
                )
        self.assertTrue(written)
        self.assertTrue(all(canary.encode() not in data for data in written))
        observed = package.extract_verified(output, digest, self.root / "consumer")
        self.assertEqual(observed, manifest)
        self.assertEqual(observed["build"], original_build)
        self.assertEqual(observed["runtime"], original_runtime)
        self.assertEqual(observed["compatibility"], original_contract)

    def test_public_metadata_projection_preserves_values_without_container_aliases(self):
        _, build, runtime, contract, _, _ = fixture(self.root)
        supplied = (build, runtime, contract)
        admitted = package.admitted_metadata(
            build, runtime, contract, package.TARGETS["linux-x86_64"]
        )
        self.assertEqual(admitted, supplied)

        def container_ids(value):
            if isinstance(value, dict):
                return {id(value)} | set().union(*(container_ids(item) for item in value.values()))
            if isinstance(value, (list, tuple)):
                return {id(value)} | set().union(*(container_ids(item) for item in value))
            return set()

        self.assertFalse(container_ids(supplied) & container_ids(admitted))

    def test_cli_pinned_metadata_roundtrip_from_other_working_directory(self):
        inputs, build, runtime, contract, schema, output = fixture(self.root)
        records = {"build-record": build, "runtime-record": runtime, "contract": contract}
        command = [sys.executable, str(Path(package.__file__).resolve()), "assemble"]
        for name, data in records.items():
            path = self.root / (name + ".json")
            path.write_bytes(package.canonical(data))
            command.extend(["--" + name, str(path), "--" + name + "-sha256", package.sha256(path)])
        input_map = self.root / "inputs.json"
        input_map.write_bytes(package.canonical({name: str(path) for name, path in inputs.items()}))
        schema_path = self.root / "schema.json"
        schema_path.write_bytes(schema)
        command.extend(
            ["--inputs", str(input_map), "--schema", str(schema_path), "--output", str(output)]
        )
        invalid = list(command)
        invalid[invalid.index("--build-record-sha256") + 1] = "0" * 64
        refused = subprocess.run(invalid, cwd=self.root, capture_output=True, text=True, timeout=10)
        self.assertNotEqual(refused.returncode, 0)
        self.assertIn("trusted metadata hash mismatch", refused.stderr)
        self.assertFalse(output.exists())
        assembled = subprocess.run(
            command, cwd=self.root, capture_output=True, text=True, timeout=10
        )
        self.assertEqual(assembled.returncode, 0, assembled.stderr)
        report = json.loads(assembled.stdout)
        self.assertEqual(report["qualification"], "unverified_candidate")
        observed = package.extract_verified(
            output, report["archive_sha256"], self.root / "consumer"
        )
        self.assertEqual(observed["build"], build)
        self.assertEqual(observed["runtime"], runtime)
        self.assertEqual(observed["compatibility"], contract)

    def test_manifest_and_payload_item_extra_canary_fields_refused(self):
        inputs, build, runtime, contract, schema, output = fixture(self.root)
        manifest, _ = package.assemble(inputs, build, runtime, contract, schema, output)
        for location in ("manifest", "payload_item"):
            with self.subTest(location=location):
                candidate = copy.deepcopy(manifest)
                item = candidate if location == "manifest" else candidate["files"]["pumas-rpc"]
                item["unexpected_secret"] = "DUMMY_METADATA_CANARY_NOT_A_CREDENTIAL"
                with self.assertRaisesRegex(ValueError, "unexpected or missing"):
                    package.check_manifest(candidate)

    def test_only_reviewed_offline_locked_production_argv_admitted(self):
        for target in package.TARGETS:
            for index, argv in enumerate(
                package.production_build_commands(package.TARGETS[target]["rust_target"])
            ):
                with self.subTest(target=target, command=index):
                    root = self.root / f"{target}-{index}"
                    root.mkdir()
                    inputs, build, runtime, contract, schema, output = fixture(root, target)
                    build["command"] = argv
                    manifest, digest = package.assemble(
                        inputs, build, runtime, contract, schema, output
                    )
                    observed = package.extract_verified(output, digest, root / "consumer")
                    self.assertEqual(observed["build"]["command"], argv)
                    self.assertEqual(manifest["qualification"], "unverified_candidate")
        for name in (
            "extra_secret",
            "missing_offline",
            "missing_locked",
            "test_build",
            "wrong_target",
            "shell_command",
        ):
            with self.subTest(rejected=name):
                root = self.root / name
                root.mkdir()
                inputs, build, runtime, contract, schema, output = fixture(root)
                if name == "extra_secret":
                    build["command"].append("--dummy-secret=DUMMY_METADATA_CANARY_NOT_A_CREDENTIAL")
                elif name in ("missing_offline", "missing_locked"):
                    build["command"].remove("--" + name.removeprefix("missing_"))
                elif name == "test_build":
                    build["command"][1] = "test"
                elif name == "wrong_target":
                    build["command"][-1] = "aarch64-apple-darwin"
                else:
                    build["command"] = " ".join(build["command"])
                with self.assertRaisesRegex(
                    ValueError, "reviewed offline locked production command"
                ):
                    package.assemble(inputs, build, runtime, contract, schema, output)
                self.assertFalse(output.exists())

    def test_actual_cargo_json_build_command_preserved_verbatim(self):
        inputs, build, runtime, contract, schema, output = fixture(self.root)
        actual = [
            "cargo",
            "build",
            "--locked",
            "--offline",
            "--manifest-path",
            "rust/Cargo.toml",
            "-p",
            "pumas-rpc",
            "--bin",
            "pumas-rpc",
            "--no-default-features",
            "--features",
            "s3,inference-plugins",
            "--target",
            "x86_64-unknown-linux-gnu",
            "--release",
            "--message-format=json-render-diagnostics",
        ]
        build["command"] = actual
        _, digest = package.assemble(inputs, build, runtime, contract, schema, output)
        observed = package.extract_verified(output, digest, self.root / "consumer")
        self.assertEqual(observed["build"]["command"], actual)

    def test_exported_metadata_strings_and_types_are_bounded(self):
        cases = {
            "build_id": lambda build, runtime, contract: contract["expected"].update(
                build_id="x" * 129
            ),
            "toolchain_canary": lambda build, runtime, contract: build.update(
                rustc=build["rustc"] + " DUMMY_METADATA_CANARY_NOT_A_CREDENTIAL"
            ),
            "provisional_request": lambda build, runtime, contract: contract.update(
                requests=[{"path": "/fixture-handshake", "bindings": {}}]
            ),
            "feature": lambda build, runtime, contract: contract["expected"]["build_info"].update(
                compiled_features=["x" * 129]
            ),
            "protocol": lambda build, runtime, contract: contract["expected"]["build_info"][
                "protocols"
            ][0].update(versions=[True]),
            "runtime_item_type": lambda build, runtime, contract: runtime["files"][
                "libonnxruntime.so"
            ].update(bytes=True),
            "extra_feature": lambda build, runtime, contract: contract["expected"][
                "features"
            ].append("unreviewed_feature"),
        }
        for name, mutate in cases.items():
            with self.subTest(name=name):
                root = self.root / name
                root.mkdir()
                inputs, build, runtime, contract, schema, output = fixture(root)
                mutate(build, runtime, contract)
                with self.assertRaises(ValueError):
                    package.assemble(inputs, build, runtime, contract, schema, output)
                self.assertFalse(output.exists())

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
            package.read_pinned_json(path, "0" * 64)
        self.assertEqual(
            package.read_pinned_json(path, package.sha256(path)), {"schema_version": 1}
        )
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

    def test_member_count_and_provisional_projection_refused(self):
        archive = self.root / "too-many.tar.gz"
        with tarfile.open(archive, "w:gz") as stream:
            for index in range(131):
                member = tarfile.TarInfo(f"member-{index}")
                member.size = 1
                stream.addfile(member, io.BytesIO(b"x"))
        with self.assertRaisesRegex(ValueError, "bounded archive member count"):
            package.extract_verified(archive, package.sha256(archive), self.root / "consumer")
        _, _, _, contract, _, _ = fixture(self.root)
        contract["requests"] = [{"path": "/fixture-handshake", "bindings": {}}]
        with self.assertRaisesRegex(ValueError, "compatibility contract fields"):
            package.validate_contract(contract)

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
            self.assertEqual(package.read_pinned_json(path, digest), {"identity": "trusted"})
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


def description_fixture(contract):
    expected = contract["expected"]
    return {
        "advertisement_schema_version": 1,
        "service_generation": "controlled-http-generation",
        "endpoint": "__ENDPOINT__",
        "build_info": copy.deepcopy(expected["build_info"]),
        "instance": {
            "discovery_schema_version": 1,
            "registry_library_id": "controlled-library-id",
            "library_root": "__ROOT__",
            "generation": "2026-10-08T11:00:00Z",
            "pumas_version": expected["version"],
            "build_info": copy.deepcopy(expected["core_build_info"]),
            "protocols": [{"name": "pumas.local-ipc", "versions": [1]}],
            "capabilities": [
                "model.query@1",
                "model.get.local@1",
                "model.selector@1",
                "artifact.resolve@1",
            ],
            "model_ref_schema_version": 1,
            "selector_schema_version": 1,
        },
    }


@unittest.skipUnless(
    sys.platform == "linux" and shutil.which("cc"),
    "controlled native process requires Linux and cc; not platform qualification",
)
class ConsumerProcessTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)

    def prepare(self, mutation=None, mode=0):
        inputs, build, runtime, contract, schema, output = fixture(self.root)
        response = description_fixture(contract)
        if mutation:
            mutation(response)
        source = Path(__file__).with_name("fixtures") / "headless-rpc-fixture.c"
        binary = inputs["pumas-rpc"]
        subprocess.run(
            [
                "cc",
                "-std=c11",
                "-D_POSIX_C_SOURCE=200809L",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DFIXTURE_RESPONSE=" + json.dumps(json.dumps(response)),
                "-DFIXTURE_MODE=" + str(mode),
                str(source),
                "-o",
                str(binary),
            ],
            check=True,
            capture_output=True,
        )
        build["binary_sha256"] = package.sha256(binary)
        manifest, digest = package.assemble(inputs, build, runtime, contract, schema, output)
        extracted = self.root / "consumer"
        package.extract_verified(output, digest, extracted)
        return extracted, manifest, contract

    def start_borrowed(self, extracted):
        root = self.root / 'existing "owner"'
        root.mkdir()
        registry = root / "registry.db"
        environment = package.clean_environment()
        environment["PUMAS_REGISTRY_DB_PATH"] = str(registry)
        log = (root / "fixture.log").open("wb")
        process = subprocess.Popen(
            [str(extracted / "pumas-rpc"), "--launcher-root", str(root), "--port", "0"],
            env=environment,
            stdout=log,
        )

        def stop():
            if process.poll() is None:
                process.send_signal(signal.SIGINT)
                process.wait(timeout=5)
            log.close()

        self.addCleanup(stop)
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            if registry.exists():
                return root, registry, process
            self.assertIsNone(process.poll())
            time.sleep(0.01)
        self.fail("controlled owner did not start")

    def test_exact_extracted_owner_starts_matches_and_stops(self):
        extracted, manifest, contract = self.prepare()
        ambient = {
            "LD_AUDIT": "must-not-load",
            "DYLD_INSERT_LIBRARIES": "must-not-load",
            "ORT_DYLIB_PATH": "must-not-load",
            "PUMAS_FIXTURE_AMBIENT": "must-not-read",
        }
        with mock.patch.dict(os.environ, ambient):
            evidence = package.smoke(extracted, manifest, contract, timeout=5)
        self.assertEqual(evidence["access"], "owned")
        self.assertEqual(evidence["compatibility"], "passed")
        self.assertEqual(evidence["runtime_loading"], "unqualified")
        self.assertEqual(evidence["real_inference"], "unqualified")

    def test_compatible_attach_reuses_owner_without_stopping_it(self):
        extracted, manifest, contract = self.prepare()
        root, registry, process = self.start_borrowed(extracted)
        before = (root / "fixture-observation.json").read_bytes()
        evidence = package.attach(extracted, manifest, contract, root, registry, timeout=5)
        self.assertEqual(evidence["access"], "borrowed")
        self.assertEqual(evidence["observation"], "point_in_time")
        self.assertIsNone(process.poll())
        self.assertEqual((root / "fixture-observation.json").read_bytes(), before)

    def test_incompatible_attach_does_not_start_or_stop_owner(self):
        extracted, manifest, contract = self.prepare(
            lambda description: description["build_info"].update(source_revision="f" * 40)
        )
        root, registry, process = self.start_borrowed(extracted)
        with self.assertRaisesRegex(ValueError, "live PumasBuildInfo mismatch"):
            package.attach(extracted, manifest, contract, root, registry, timeout=5)
        self.assertIsNone(process.poll())

    def test_missing_owner_is_refused_without_starting(self):
        extracted, manifest, contract = self.prepare()
        root = self.root / "unowned"
        root.mkdir()
        registry = self.root / "empty-registry"
        registry.touch()
        with self.assertRaisesRegex(ValueError, "authenticated local owner observation refused"):
            package.attach(extracted, manifest, contract, root, registry, timeout=5)
        self.assertEqual(list(root.iterdir()), [])

    def test_missing_owner_identity_refused_and_owned_child_stops(self):
        extracted, manifest, contract = self.prepare(
            lambda description: description.pop("instance")
        )
        with self.assertRaisesRegex(ValueError, "HttpServiceDescription fields"):
            package.smoke(extracted, manifest, contract, timeout=5)

    def test_forced_stop_is_failure_and_exact_launched_child_is_reaped(self):
        extracted, manifest, contract = self.prepare(mode=5)
        original_wait = subprocess.Popen.wait
        stopped = []

        def short_wait(process, timeout=None):
            result = original_wait(process, timeout=min(timeout, 0.2) if timeout else timeout)
            stopped.append(process)
            return result

        with mock.patch.object(subprocess.Popen, "wait", short_wait):
            with self.assertRaisesRegex(ValueError, "required forced shutdown"):
                package.smoke(extracted, manifest, contract, timeout=5)
        killed = [process for process in stopped if process.returncode == -signal.SIGKILL]
        self.assertEqual(len(killed), 1)
        self.assertIsNotNone(killed[0].poll())

    def test_observer_timeout_leaves_borrowed_owner_running(self):
        extracted, manifest, contract = self.prepare(mode=6)
        root, registry, process = self.start_borrowed(extracted)
        with self.assertRaises(subprocess.TimeoutExpired):
            package.attach(extracted, manifest, contract, root, registry, timeout=0.2)
        self.assertIsNone(process.poll())

    def test_startup_failures_and_redirect_refused(self):
        for mode, message in (
            (1, "exited before readiness"),
            (2, "did not announce readiness"),
            (3, "invalid announced port"),
            (4, "redirects refused"),
        ):
            with self.subTest(mode=mode), tempfile.TemporaryDirectory() as temporary:
                original = self.root
                self.root = Path(temporary)
                try:
                    extracted, manifest, contract = self.prepare(mode=mode)
                    with self.assertRaisesRegex(ValueError, message):
                        package.smoke(extracted, manifest, contract, timeout=0.3)
                finally:
                    self.root = original


class OwnerContractTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.root = Path(self.temporary.name)
        self.addCleanup(self.temporary.cleanup)
        _, _, _, contract, _, _ = fixture(self.root)
        self.expected = contract["expected"]
        self.description = description_fixture(contract)
        self.description["instance"]["library_root"] = str(self.root)
        self.description["endpoint"] = "http://127.0.0.1:43210"

    def test_missing_incompatible_and_wrong_context_are_refused(self):
        cases = [
            lambda item: item.update(advertisement_schema_version=True),
            lambda item: item.update(service_generation=""),
            lambda item: item.update(endpoint="https://127.0.0.1:43210"),
            lambda item: item["instance"].update(registry_library_id=""),
            lambda item: item["instance"].update(generation=""),
            lambda item: item["instance"].update(library_root="/wrong-root"),
            lambda item: item["instance"].update(model_ref_schema_version=True),
            lambda item: item["instance"].pop("build_info"),
            lambda item: item["build_info"].update(source_revision=None),
            lambda item: item["build_info"].update(build_info_schema_version=True),
            lambda item: item["instance"].update(capabilities=[]),
        ]
        for mutation in cases:
            with self.subTest(mutation=mutation):
                value = copy.deepcopy(self.description)
                mutation(value)
                with self.assertRaises(ValueError):
                    package.discovery.validate_description(value, self.expected, self.root)

    def test_root_binding_uses_existing_directory_identity(self):
        equivalent_path = os.path.join(str(self.root), ".")
        self.assertNotEqual(equivalent_path, str(self.root))
        self.assertTrue(package.discovery.same_library_root(equivalent_path, self.root))
        self.description["instance"]["library_root"] = equivalent_path
        package.discovery.validate_description(self.description, self.expected, self.root)

        other = self.root / "different-directory"
        other.mkdir()
        regular_file = self.root / "not-a-directory"
        regular_file.touch()
        for observed in (
            str(other),
            str(regular_file),
            str(self.root / "missing"),
            ".",
            "",
            None,
            "invalid\0path",
        ):
            with self.subTest(observed=observed):
                self.assertFalse(package.discovery.same_library_root(observed, self.root))
                self.description["instance"]["library_root"] = observed
                with self.assertRaisesRegex(ValueError, "library owner context"):
                    package.discovery.validate_description(
                        self.description, self.expected, self.root
                    )

    def test_root_binding_refuses_identity_lookup_errors(self):
        for error in (OSError("unavailable"), ValueError("invalid path")):
            with self.subTest(error=error), mock.patch.object(Path, "samefile", side_effect=error):
                self.assertFalse(package.discovery.same_library_root(str(self.root), self.root))

    def test_root_binding_does_not_case_fold_distinct_directories(self):
        upper, lower = self.root / "CaseSensitiveRoot", self.root / "casesensitiveroot"
        upper.mkdir()
        lower.mkdir(exist_ok=True)
        if upper.samefile(lower):
            self.skipTest("host filesystem treats these spellings as the same directory")
        self.assertFalse(package.discovery.same_library_root(str(upper), lower))

    @unittest.skipUnless(os.name == "nt", "requires native Windows filesystem identity")
    def test_native_windows_extended_length_owner_root(self):
        # Rust's canonicalize may return this extended-length spelling while
        # Python resolves the ordinary selected input without that prefix.
        ordinary = str(self.root.resolve(strict=True))
        self.assertFalse(
            ordinary.startswith("\\\\?\\"), "test needs an ordinary temporary-directory spelling"
        )
        extended = (
            "\\\\?\\UNC\\" + ordinary[2:] if ordinary.startswith("\\\\") else "\\\\?\\" + ordinary
        )
        self.assertNotEqual(ordinary, extended)
        self.assertTrue(Path(ordinary).samefile(extended))
        self.description["instance"]["library_root"] = extended
        package.discovery.validate_description(self.description, self.expected, Path(ordinary))
        self.assertTrue(package.discovery.same_library_root(ordinary, Path(extended)))

        different = self.root / "different-native-directory"
        different.mkdir()
        with self.assertRaisesRegex(ValueError, "library owner context"):
            package.discovery.validate_description(self.description, self.expected, different)
        for field in ("registry_library_id", "generation"):
            changed = copy.deepcopy(self.description)
            changed["instance"][field] = ""
            with (
                self.subTest(field=field),
                self.assertRaisesRegex(ValueError, "library owner context"),
            ):
                package.discovery.validate_description(changed, self.expected, Path(ordinary))

    def test_generic_fixture_projection_is_not_an_owner(self):
        with self.assertRaisesRegex(ValueError, "HttpServiceDescription fields"):
            package.discovery.validate_description(self.expected, self.expected, self.root)

    def test_decode_rejects_duplicate_or_oversized_owner_json(self):
        for body in (b'{"instance":1,"instance":2}', b" " * (64 * 1024 + 1), b'{"x":NaN}'):
            with self.subTest(body=body[:80]), self.assertRaises(ValueError):
                package.discovery.decode(body)

    def test_endpoint_is_numeric_loopback_only(self):
        for endpoint in (
            "http://localhost:12",
            "http://[::1%25eth0]:12",
            " http://127.0.0.1:12",
            "http://127.0.0.1:12\n",
            "http://127.0.0.1:0",
            "http://127.0.0.1:12/path",
            "http://user@127.0.0.1:12",
            "http://127.0.0.1:12?token=x",
            "http://192.0.2.1:12",
            "http://127.0.0.1:12#fragment",
        ):
            with self.subTest(endpoint=endpoint), self.assertRaises(ValueError):
                package.discovery.endpoint(endpoint)
        self.assertEqual(package.discovery.endpoint("http://[::1]:12"), "http://[::1]:12")

    def test_http_and_authenticated_generation_changes_refused(self):
        for stage in ("http", "after"):
            with self.subTest(stage=stage):
                changed = copy.deepcopy(self.description)
                changed["service_generation"] = "new-generation"
                response = mock.MagicMock()
                response.__enter__.return_value = response
                response.status = 200
                response.read.return_value = package.canonical(
                    changed if stage == "http" else self.description
                )
                opener = mock.Mock()
                opener.open.return_value = response
                with (
                    mock.patch.object(
                        package.discovery,
                        "observe",
                        side_effect=[
                            self.description,
                            changed if stage == "after" else self.description,
                        ],
                    ),
                    mock.patch.object(
                        package.discovery.urllib.request, "build_opener", return_value=opener
                    ),
                ):
                    with self.assertRaisesRegex(ValueError, "changed"):
                        package.discovery.verify_owner("unused", self.root, {}, self.expected)
                self.assertEqual(
                    opener.open.call_args.args[0], "http://127.0.0.1:43210/.well-known/pumas"
                )


if __name__ == "__main__":
    unittest.main()
