"""Synthetic producer subprocess evidence and archive attacks; no Rust build."""

import copy
from contextlib import contextmanager
import io
import json
from pathlib import Path
import tarfile
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

import headless_snapshot as subject


class SnapshotTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.repo, self.producer = self.root / "repo", self.root / "producer"
        (self.repo / "rust").mkdir(parents=True)
        self.producer.mkdir()
        (self.repo / "rust/Cargo.toml").write_text(
            '[workspace.package]\nversion="0.7.0"\n[profile.release]\n'
            'lto=true\ncodegen-units=1\nstrip="debuginfo"\n'
        )
        (self.repo / "package.json").write_text('{"version":"0.7.0"}')
        (self.repo / "LICENSE").write_text("project license\n")
        directory = self.repo / subject.ATTRIBUTION
        directory.mkdir(parents=True)
        (directory / "README.md").write_text("conservative checked attribution\n")
        (directory / "THIRD-PARTY-NOTICES.txt").write_text("third party licenses\n")
        (directory / "inventory.json").write_text(
            json.dumps({"rust_profile": {"default_features": True, "features": []}})
        )
        self.source = {"head": "a" * 40, "tree": "b" * 40}
        self.build_id = "pumas-no-inference-test"
        self.binary = self.root / "pumas-rpc"
        self.binary.write_bytes(b"synthetic executable\n")
        self.info = {
            "build_info_schema_version": 1,
            "component": "pumas-rpc",
            "package_version": "0.7.0",
            "source_revision": self.source["head"],
            "target": subject.TARGET,
            "build_id": self.build_id,
            "compiled_features": ["pumas-library/hf-client"],
            "protocols": [
                {"name": name, "versions": [1]} for name in ("pumas.local-ipc", "pumas.local-http")
            ],
            "schemas": [
                {"name": name, "version": version}
                for name, version in {
                    **subject.package.discovery.CORE_SCHEMAS,
                    "pumas.http-advertisement": 1,
                }.items()
            ],
        }
        self.schema = subject.package.canonical(
            {
                "format": "pumas-desktop-contract-1",
                "dialect": "http://json-schema.org/draft-07/schema#",
                "schemas": {"ActualDtoFixture": {"type": "object"}},
            }
        )
        exporter = copy.deepcopy(self.info)
        exporter["compiled_features"] += [
            "pumas-library/contract-schema",
            "pumas-rpc/export-contract",
        ]
        self.schema_record = {
            "schema_version": 1,
            "source": self.source,
            "export_command": ["/exporter/pumas-rpc", "--export-desktop-contract"],
            "exporter_build_info": exporter,
            "schema_bytes": len(self.schema),
            "schema_sha256": subject.hashlib.sha256(self.schema).hexdigest(),
            "exporter_binary_sha256": "c" * 64,
        }
        self.events = []
        for owner, directory, name, kind in (
            ("rpc", "pumas-rpc", "pumas-rpc", "bin"),
            ("core", "pumas-core", "pumas_library", "lib"),
        ):
            self.events.append(
                {
                    "reason": "compiler-artifact",
                    "manifest_path": str(self.repo / f"rust/crates/{directory}/Cargo.toml"),
                    "target": {"name": name, "kind": [kind]},
                    "features": subject.FEATURES[owner],
                    "package_id": f"synthetic-{owner}",
                    "profile": {
                        "opt_level": "3",
                        "test": False,
                        "debug_assertions": False,
                        "overflow_checks": False,
                        "debuginfo": 0,
                    },
                    "executable": str(self.binary) if owner == "rpc" else None,
                }
            )
        self.events.append({"reason": "build-finished", "success": True})
        self.output = self.root / "output"
        self.calls = []
        self.dirty = False
        self.status = 0

    def runner(self, command, **options):
        self.calls.append(command)
        if command[:2] == ["cargo", "build"]:
            self.assertEqual(command, subject.COMMAND)
            self.assertEqual(options["env"]["ORT_SKIP_DOWNLOAD"], "1")
            self.assertEqual(options["env"]["CARGO_BUILD_JOBS"], "1")
            self.assertEqual(options["env"]["CMAKE_BUILD_PARALLEL_LEVEL"], "1")
            options["stdout"].write("\n".join(json.dumps(event) for event in self.events))
            options["stderr"].write("synthetic diagnostic\n")
            return SimpleNamespace(returncode=self.status)
        if command == [str(self.binary), "--build-info"]:
            self.assertNotIn("PUMAS_BUILD_ID", options["env"])
            return SimpleNamespace(stdout=json.dumps(self.info), returncode=0)
        if command == ["git", "rev-parse", "--show-toplevel"]:
            result = str(options["cwd"])
        elif command[:2] == ["git", "status"]:
            result = " M changed" if self.dirty else ""
        elif command == ["git", "rev-parse", "HEAD"]:
            result = self.source["head"] if options["cwd"] == self.repo else "d" * 40
        elif command == ["git", "rev-parse", "HEAD^{tree}"]:
            result = self.source["tree"] if options["cwd"] == self.repo else "e" * 40
        elif command == ["rustc", "-Vv"]:
            result = f"rustc 1.92.0\nhost: {subject.TARGET}"
        else:
            result = "synthetic checked observation"
        return SimpleNamespace(stdout=result, returncode=0)

    def produce(self, completed=None, environment=None):
        with (
            patch.object(subject.provenance, "check_configuration"),
            patch.object(subject.platform, "system", return_value="Linux"),
            patch.object(subject.platform, "machine", return_value="x86_64"),
        ):
            return subject.produce(
                self.repo,
                self.source,
                self.build_id,
                self.schema,
                self.schema_record,
                self.output,
                completed=completed,
                runner=self.runner,
                environment={} if environment is None else environment,
                producer_repository=self.producer,
            )

    def test_produces_and_extracts_no_inference_actual_version_snapshot(self):
        result = self.produce()
        manifest = subject.extract_verified(
            result["archive"], result["archive_sha256"], self.root / "extract"
        )
        self.assertFalse(manifest["inference_compile_enabled"])
        self.assertEqual(manifest["package_version"], "0.7.0")
        self.assertEqual(manifest["package_source"], self.source)
        self.assertNotEqual(manifest["producer_source"], self.source)
        self.assertEqual({path.name for path in (self.root / "extract").iterdir()}, subject.PAYLOAD)
        info = json.loads((self.root / "extract/rpc-build-info.json").read_text())
        self.assertEqual(info["compiled_features"], ["pumas-library/hf-client"])

    def test_actual_checked_notices_fit_symmetric_legal_payload_bounds(self):
        actual = subject.ROOT / subject.ATTRIBUTION / "THIRD-PARTY-NOTICES.txt"
        expected_sha = "83051c37383eea6699536d23be0c935c602a9bb9941d938b926ea2b1810f2f3c"
        self.assertEqual(actual.stat().st_size, 4_423_904)
        self.assertEqual(subject.package.sha256(actual), expected_sha)
        notices = self.repo / subject.ATTRIBUTION / actual.name
        notices.write_bytes(actual.read_bytes())
        result = self.produce()
        destination = self.root / "extract-real-notices"
        manifest = subject.extract_verified(
            result["archive"], result["archive_sha256"], destination
        )
        self.assertEqual(
            manifest["files"][actual.name], {"bytes": 4_423_904, "sha256": expected_sha}
        )
        self.assertEqual(subject.package.sha256(destination / actual.name), expected_sha)

    def test_oversized_legal_inputs_refuse_before_cargo_and_output_directory(self):
        for name, path in (
            ("license", self.repo / "LICENSE"),
            ("notices", self.repo / subject.ATTRIBUTION / "THIRD-PARTY-NOTICES.txt"),
        ):
            original = path.read_bytes()
            path.write_bytes(b"x" * (subject.MAX_LEGAL_TEXT + 1))
            self.calls.clear()
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "bounded"):
                self.produce()
            self.assertFalse(self.output.exists())
            self.assertFalse(any(command[:2] == ["cargo", "build"] for command in self.calls))
            path.write_bytes(original)

    def test_generated_metadata_is_bounded_before_archive_creation(self):
        result = self.produce()
        core = subject.inference_build.core_projection(self.info)
        for name in ("manifest", "schema-record", "schema"):
            manifest, record, schema = (
                copy.deepcopy(result["manifest"]),
                copy.deepcopy(self.schema_record),
                self.schema,
            )
            if name == "manifest":
                manifest["oversized"] = "x" * (subject.package.MAX_MANIFEST + 1)
            elif name == "schema-record":
                record["export_command"][0] = (
                    "/" + "x" * (subject.package.MAX_MANIFEST + 1) + "/pumas-rpc"
                )
            else:
                schema = subject.package.canonical(
                    {
                        "format": "pumas-desktop-contract-1",
                        "dialect": "http://json-schema.org/draft-07/schema#",
                        "schemas": {
                            "ActualDtoFixture": {"description": "x" * subject.package.MAX_MANIFEST}
                        },
                    }
                )
                record["schema_bytes"] = len(schema)
                record["schema_sha256"] = subject.hashlib.sha256(schema).hexdigest()
            output = self.root / f"oversized-generated-{name}"
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "bounded"):
                subject.assemble(
                    self.binary, self.repo, self.info, core, schema, record, manifest, output
                )
            self.assertFalse(output.exists())

    def test_all_legal_and_metadata_overlimits_refuse_before_extract_destination(self):
        result = self.produce()
        with tarfile.open(result["archive"], "r:gz") as bundle:
            original = {member.name: bundle.extractfile(member).read() for member in bundle}
        for name in sorted(subject.PAYLOAD - {"pumas-rpc"}):
            data = copy.deepcopy(original)
            limit = (
                subject.MAX_LEGAL_TEXT
                if name in subject.LEGAL_TEXT
                else subject.package.MAX_MANIFEST
            )
            data[name] = b"x" * (limit + 1)
            archive = self.root / f"oversized-{name}.tar.gz"
            with tarfile.open(archive, "w:gz", format=tarfile.USTAR_FORMAT) as bundle:
                for member_name, value in data.items():
                    member = tarfile.TarInfo(member_name)
                    member.size = len(value)
                    bundle.addfile(member, io.BytesIO(value))
            destination = self.root / f"extract-oversized-{name}"
            with self.subTest(name=name), self.assertRaises(ValueError):
                subject.extract_verified(archive, subject.package.sha256(archive), destination)
            self.assertFalse(destination.exists())

    def test_oversized_compressed_output_refuses_final_archive_publication(self):
        open_archive = subject.tarfile.open

        @contextmanager
        def enlarged_output(path, *args, **options):
            with open_archive(path, *args, **options) as bundle:
                yield bundle
            # Model a compressed writer exceeding its byte budget, independently
            # of the staged payload's already-passing per-file and total bounds.
            with Path(path).open("ab") as stream:
                stream.write(b"x" * subject.package.MAX_PACKAGE)

        with (
            patch.object(subject.package, "MAX_PACKAGE", 16 * 1024),
            patch.object(subject.tarfile, "open", side_effect=enlarged_output),
        ):
            with self.assertRaisesRegex(ValueError, "bounded nonempty regular"):
                self.produce()
        self.assertFalse(list(self.output.glob("*.tar.gz")))
        self.assertFalse(list(self.output.glob("*.partial")))

    def test_completed_artifact_mode_does_not_repeat_cargo(self):
        log = self.root / "completed.jsonl"
        log.write_text("\n".join(json.dumps(event) for event in self.events))
        result = self.produce(
            completed={
                "binary": str(self.binary),
                "binary_sha256": subject.package.sha256(self.binary),
                "cargo_jsonl": str(log),
                "cargo_jsonl_sha256": subject.package.sha256(log),
                "command": subject.COMMAND,
            }
        )
        self.assertFalse(any(command[:2] == ["cargo", "build"] for command in self.calls))
        self.assertEqual(
            result["manifest"]["compile_origin"]["mode"], "completed_local_build_record"
        )

    def test_turbojpeg_and_native_routes_refuse_before_any_observation(self):
        for name in (
            "TURBOJPEG_SOURCE",
            "X86_64_UNKNOWN_LINUX_GNU_TURBOJPEG_SHARED",
            "CMAKE_TOOLCHAIN_FILE",
            "HOST_CC",
            "CC_x86_64-unknown-linux-gnu",
        ):
            with self.subTest(name=name):
                self.calls.clear()
                with self.assertRaisesRegex(ValueError, "overrides"):
                    self.produce(environment={name: ""})
                self.assertEqual(self.calls, [])

    def test_only_one_cmake_job_is_accepted_and_forced(self):
        self.produce(environment={"CMAKE_BUILD_PARALLEL_LEVEL": "1"})
        for index, value in enumerate(("", "0", "2", "64")):
            self.output = self.root / f"invalid-cmake-{index}"
            self.calls.clear()
            with (
                self.subTest(value=value),
                self.assertRaisesRegex(ValueError, "one native CMake job"),
            ):
                self.produce(environment={"CMAKE_BUILD_PARALLEL_LEVEL": value})
            self.assertEqual(self.calls, [])
        with self.assertRaisesRegex(ValueError, "native compiler/toolchain"):
            subject.native_environment({"CMAKE_BUILD_PARALLEL_LEVEL_x86_64_unknown_linux_gnu": "1"})

    def test_dirty_source_is_refused_before_cargo(self):
        self.dirty = True
        with self.assertRaisesRegex(ValueError, "clean committed"):
            self.produce()
        self.assertFalse(any(command[:2] == ["cargo", "build"] for command in self.calls))

    def test_exporter_and_package_features_are_not_interchangeable(self):
        self.info["compiled_features"].append("pumas-rpc/export-contract")
        with self.assertRaisesRegex(ValueError, "compile features"):
            self.produce()

    def test_broader_exporter_identity_stays_separate_from_package(self):
        exporter = self.schema_record["exporter_build_info"]
        exporter["compiled_features"] += [
            "pumas-rpc/inference-plugins",
            "pumas-library/test-support",
        ]
        exporter["schemas"].append({"name": "pumas.model-operations.image-to-text", "version": 1})
        result = self.produce()
        self.assertEqual(result["manifest"]["artifacts"]["rpc"]["features"], [])
        self.assertFalse(result["manifest"]["inference_compile_enabled"])

    def test_assembler_refuses_enabled_marker_or_inference_features_before_archive(self):
        result = self.produce()
        info = self.info
        core = subject.inference_build.core_projection(info)
        for field in ("marker", "features", "identity"):
            manifest = copy.deepcopy(result["manifest"])
            changed_info = copy.deepcopy(info)
            if field == "marker":
                manifest["inference_compile_enabled"] = True
            elif field == "features":
                manifest["artifacts"]["rpc"]["features"] = ["inference-plugins"]
            else:
                changed_info["compiled_features"].append("pumas-rpc/inference-plugins")
            fresh = self.root / f"output-{field}"
            fresh.mkdir()
            with self.subTest(field=field), self.assertRaises(ValueError):
                subject.assemble(
                    self.binary,
                    self.repo,
                    changed_info,
                    core,
                    self.schema,
                    self.schema_record,
                    manifest,
                    fresh,
                )
            self.assertEqual(list(fresh.iterdir()), [])

    def test_source_or_schema_mismatch_refuses_before_cargo(self):
        self.schema_record["source"] = {"head": "f" * 40, "tree": "b" * 40}
        with self.assertRaisesRegex(ValueError, "schema source mismatch"):
            self.produce()
        self.assertFalse(any(command[:2] == ["cargo", "build"] for command in self.calls))

    def test_unexpected_compile_features_or_profile_are_refused(self):
        for owner in (0, 1):
            original = copy.deepcopy(self.events[owner])
            self.events[owner]["features"] = ["s3"]
            with self.assertRaisesRegex(ValueError, "features"):
                subject.artifact_evidence("\n".join(map(json.dumps, self.events)), self.repo)
            self.events[owner] = copy.deepcopy(original)
            self.events[owner]["profile"]["opt_level"] = "0"
            with self.assertRaisesRegex(ValueError, "profile"):
                subject.artifact_evidence("\n".join(map(json.dumps, self.events)), self.repo)
            self.events[owner] = original

    def test_existing_or_symlink_output_is_refused(self):
        self.output.mkdir()
        with self.assertRaisesRegex(ValueError, "fresh output"):
            self.produce()
        self.output.rmdir()
        self.output.symlink_to(self.root / "elsewhere")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.produce()

    def test_failed_cargo_preserves_diagnostics_without_archive(self):
        self.status = 101
        with self.assertRaisesRegex(ValueError, "Cargo build failed"):
            self.produce()
        self.assertTrue((self.output / "cargo.stderr.log").is_file())
        self.assertFalse(list(self.output.glob("*.tar.gz")))

    def test_wrong_archive_hash_and_existing_extract_are_refused(self):
        result = self.produce()
        destination = self.root / "extract"
        with self.assertRaisesRegex(ValueError, "archive byte hash mismatch"):
            subject.extract_verified(result["archive"], "0" * 64, destination)
        self.assertFalse(destination.exists())
        destination.mkdir()
        with self.assertRaisesRegex(ValueError, "fresh extraction"):
            subject.extract_verified(result["archive"], result["archive_sha256"], destination)

    def test_tampered_payload_refuses_with_rehashed_archive_and_checksum_inventory(self):
        result = self.produce()
        with tarfile.open(result["archive"], "r:gz") as bundle:
            original = {member.name: bundle.extractfile(member).read() for member in bundle}
        for layer in ("checksum", "manifest", "core_identity"):
            data = copy.deepcopy(original)
            if layer == "core_identity":
                core = json.loads(data["core-build-info.json"])
                core["component"] = "pumas-rpc"
                data["core-build-info.json"] = subject.package.canonical(core)
                manifest = json.loads(data["manifest.json"])
                manifest["files"]["core-build-info.json"] = {
                    "bytes": len(data["core-build-info.json"]),
                    "sha256": subject.hashlib.sha256(data["core-build-info.json"]).hexdigest(),
                }
                data["manifest.json"] = subject.package.canonical(manifest)
            else:
                data["THIRD-PARTY-NOTICES.txt"] += b"tampered license\n"
            if layer != "checksum":
                data["SHA256SUMS"] = "".join(
                    f"{subject.hashlib.sha256(data[name]).hexdigest()}  {name}\n"
                    for name in sorted(subject.PAYLOAD - {"SHA256SUMS"})
                ).encode()
            archive = self.root / f"rehashed-{layer}.tar.gz"
            with tarfile.open(archive, "w:gz", format=tarfile.USTAR_FORMAT) as bundle:
                for name, value in data.items():
                    member = tarfile.TarInfo(name)
                    member.size = len(value)
                    bundle.addfile(member, io.BytesIO(value))
            expected = {
                "checksum": "checksum inventory",
                "manifest": "member bytes",
                "core_identity": "core projection",
            }[layer]
            destination = self.root / f"extract-rehashed-{layer}"
            with self.subTest(layer=layer), self.assertRaisesRegex(ValueError, expected):
                subject.extract_verified(archive, subject.package.sha256(archive), destination)
            self.assertFalse(destination.exists())

    def test_extra_link_duplicate_and_extension_members_are_refused(self):
        for kind in ("extra", "link", "duplicate", "extension"):
            archive = self.root / f"attack-{kind}.tar.gz"
            with tarfile.open(archive, "w:gz", format=tarfile.USTAR_FORMAT) as bundle:
                member = tarfile.TarInfo("extra" if kind == "extra" else "pumas-rpc")
                if kind == "link":
                    member.type = tarfile.SYMTYPE
                    member.linkname = "/outside"
                elif kind == "extension":
                    member.type = tarfile.XHDTYPE
                else:
                    member.size = 1
                bundle.addfile(member, io.BytesIO(b"x"))
                if kind == "duplicate":
                    bundle.addfile(member, io.BytesIO(b"x"))
            with self.subTest(kind=kind), self.assertRaises(ValueError):
                subject.extract_verified(
                    archive, subject.package.sha256(archive), self.root / f"extract-{kind}"
                )


if __name__ == "__main__":
    unittest.main()
