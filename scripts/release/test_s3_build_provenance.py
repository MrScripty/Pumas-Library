"""Synthetic subprocess/artifact controls; never execute Cargo or a binary."""

import copy
import json
import importlib.util
from pathlib import Path
import subprocess
import sqlite3
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import s3_build_provenance as subject


class ProvenanceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="s3-provenance-test-")
        self.addCleanup(self.temporary.cleanup)
        self.directory = Path(self.temporary.name)
        self.repository = self.directory / "repository"
        self.repository.mkdir()
        (self.repository / "rust").mkdir()
        (self.repository / "rust/Cargo.toml").write_text(
            '[profile.release]\nlto = true\ncodegen-units = 1\nstrip = "debuginfo"\n'
        )
        self.notices = self.repository / subject.ATTRIBUTION
        self.notices.mkdir(parents=True)
        (self.notices / "README.md").write_text("Synthetic S3 attribution fixture")
        (self.notices / "THIRD-PARTY-NOTICES.txt").write_text("Synthetic SDK and XML notices")
        self.inventory = {
            "rust_profile": {
                "package": "pumas-rpc",
                "default_features": True,
                "features": ["s3"],
                "targets": [subject.TARGET, "aarch64-apple-darwin", "x86_64-pc-windows-msvc"],
            },
            "packages": [{"name": "aws-sdk-s3"}, {"name": "roxmltree"}],
            "notices_sha256": subject.digest(self.notices / "THIRD-PARTY-NOTICES.txt"),
        }
        self.write_inventory()
        self.binary = self.directory / "target/release/pumas-rpc"
        self.binary.parent.mkdir(parents=True)
        self.binary.write_bytes(b"synthetic production executable; NEVER EXECUTED")
        self.profile = {
            "opt_level": "3",
            "debuginfo": 0,
            "debug_assertions": False,
            "overflow_checks": False,
            "test": False,
        }
        self.events = []
        for owner, crate, manifest, name, kind, features in (
            ("rpc", "pumas-rpc", "pumas-rpc", "pumas-rpc", "bin", subject.RPC_FEATURES),
            ("core", "pumas-library", "pumas-core", "pumas_library", "lib", subject.CORE_FEATURES),
        ):
            self.events.append(
                {
                    "reason": "compiler-artifact",
                    "package_id": f"path+file:///fixture#{crate}@0.7.0",
                    "manifest_path": str(self.repository / f"rust/crates/{manifest}/Cargo.toml"),
                    "target": {"name": name, "kind": [kind]},
                    "features": features,
                    "profile": self.profile.copy(),
                    "executable": str(self.binary) if owner == "rpc" else None,
                }
            )
            filename = "bin-pumas-rpc" if owner == "rpc" else "lib-pumas_library"
            path = self.binary.parent / f".fingerprint/{crate}-synthetic/{filename}.json"
            path.parent.mkdir(parents=True)
            path.write_text(
                json.dumps({"features": json.dumps(features), "profile": 1234, "rustflags": []})
            )
        self.events.append({"reason": "build-finished", "success": True})
        self.calls = []
        self.head, self.tree = "a" * 40, "b" * 40
        self.dirty = False
        self.changed = False
        self.built = False
        self.build_status = 0
        self.output = self.directory / "evidence/build.json"

    def write_inventory(self):
        (self.notices / "inventory.json").write_text(json.dumps(self.inventory))

    def runner(self, command, **options):
        self.calls.append(command)
        if command == subject.COMMAND:
            self.built = True
            options["stdout"].write("\n".join(json.dumps(event) for event in self.events))
            options["stderr"].write("synthetic retained build diagnostic\n")
            return SimpleNamespace(
                returncode=self.build_status,
            )
        if command[:2] == ["git", "status"]:
            stdout = " M rust/Cargo.toml" if self.dirty else ""
        elif command == ["git", "rev-parse", "HEAD"]:
            stdout = "c" * 40 if self.changed and self.built else self.head
        elif command == ["git", "rev-parse", "HEAD^{tree}"]:
            stdout = self.tree
        elif command == ["rustc", "-Vv"]:
            stdout = f"rustc synthetic\nhost: {subject.TARGET}\n"
        elif command == ["cargo", "-V"]:
            stdout = "cargo synthetic"
        elif command[0] == "node":
            # Simulates the authoritative checker. The production helper always
            # invokes that checker; these fixtures do not reproduce its policy.
            inventory = json.loads((self.notices / "inventory.json").read_text())
            if not any(item["name"] == "aws-sdk-s3" for item in inventory["packages"]):
                raise subprocess.CalledProcessError(
                    1, command, stderr="missing selected SDK closure"
                )
            stdout = ""
        elif command[1:] == ["scripts/release/check-dependency-features.py", "--s3"]:
            stdout = "synthetic feature checker passed"
        else:
            self.fail(f"unmocked subprocess command: {command!r}")
        return SimpleNamespace(stdout=stdout, stderr="", returncode=0)

    def build(self):
        return subject.build_provenance(
            self.repository,
            self.output,
            self.runner,
            environment={"CARGO_HOME": str(self.directory / "cargo-home")},
        )

    def validate(self, record):
        return subject.validate_provenance(
            record, self.binary, self.repository, self.head, self.runner
        )

    def test_fixed_build_and_artifact_record(self):
        record = self.build()
        self.assertEqual(record, json.loads(self.output.read_text()))
        self.assertEqual(record["binary_sha256"], subject.digest(self.binary))
        self.assertEqual(record["artifacts"]["core"]["features"], subject.CORE_FEATURES)
        self.assertEqual(record["source"], {"head": self.head, "tree": self.tree})
        self.assertIn(subject.COMMAND, self.calls)
        self.assertFalse(any(str(self.binary) == command[0] for command in self.calls))
        self.assertIn(
            ["node", "scripts/release/check-attribution.cjs", "--features", "s3"], self.calls
        )
        self.assertTrue(
            any("check-dependency-features.py" in " ".join(command) for command in self.calls)
        )
        self.assertEqual(set(record["attribution"]["sha256"]), set(subject.ATTRIBUTION_FILES))
        self.assertEqual(self.validate(record), record)
        with self.assertRaisesRegex(ValueError, "overwrite"):
            self.build()

    def test_binary_hash_and_source_binding(self):
        record = self.build()
        for field in ("head", "tree"):
            with self.subTest(field=field):
                changed = copy.deepcopy(record)
                changed["source"][field] = "d" * 40
                with self.assertRaisesRegex(ValueError, "source head/tree"):
                    self.validate(changed)
        with self.assertRaisesRegex(ValueError, "source head/tree"):
            subject.validate_provenance(record, self.binary, self.repository, "d" * 40, self.runner)
        self.binary.write_bytes(b"different arbitrary executable")
        with self.assertRaisesRegex(ValueError, "binary SHA"):
            self.validate(record)

    def test_production_artifact_features_and_profiles_are_required(self):
        original = copy.deepcopy(self.events)
        for owner, features in (
            (0, ["s3"]),
            (0, subject.RPC_FEATURES + ["test-support"]),
            (1, subject.CORE_FEATURES + ["test-support"]),
            (1, ["hf-client", "s3"]),
        ):
            with self.subTest(owner=owner, features=features):
                self.events = copy.deepcopy(original)
                self.events[owner]["features"] = features
                with self.assertRaisesRegex(ValueError, "unexpected .* features"):
                    subject.artifact_evidence(
                        "\n".join(map(json.dumps, self.events)), self.repository
                    )
        for key, bad in (("test", True), ("opt_level", "0"), ("debug_assertions", True)):
            with self.subTest(profile=key):
                self.events = copy.deepcopy(original)
                self.events[0]["profile"][key] = bad
                with self.assertRaisesRegex(ValueError, "artifact profile"):
                    subject.artifact_evidence(
                        "\n".join(map(json.dumps, self.events)), self.repository
                    )

    def test_missing_or_ambiguous_artifacts_fail(self):
        for events in (self.events[1:], self.events + [self.events[0]], self.events[:-1]):
            with self.subTest(events=len(events)), self.assertRaises(ValueError):
                subject.artifact_evidence("\n".join(map(json.dumps, events)), self.repository)

    def test_dirty_or_changed_source_never_emits_provenance(self):
        self.dirty = True
        with self.assertRaisesRegex(ValueError, "clean committed"):
            self.build()
        self.assertFalse(self.built)
        self.dirty, self.changed = False, True
        with self.assertRaisesRegex(ValueError, "source changed"):
            self.build()
        self.assertFalse(self.output.exists())
        self.assertTrue(self.output.with_suffix(".cargo.stderr.log").exists())

    def test_flag_and_release_overrides_refused_before_build(self):
        for environment in (
            {"RUSTFLAGS": "--cfg test"},
            {"CARGO_ENCODED_RUSTFLAGS": "bad"},
            {"CARGO_BUILD_RUSTFLAGS": "bad"},
            {"CARGO_PROFILE_RELEASE_LTO": "false"},
            {"CARGO_BUILD_TARGET": "aarch64-apple-darwin"},
        ):
            with self.subTest(environment=environment), self.assertRaises(ValueError):
                subject.build_provenance(self.repository, self.output, self.runner, environment)
        self.assertFalse(self.built)
        (self.repository / "rust/Cargo.toml").write_text("[profile.release]\nlto = false\n")
        with self.assertRaisesRegex(ValueError, "release profile changed"):
            self.build()
        self.assertFalse(self.built)

    def test_old_or_omitted_notice_inputs_fail(self):
        record = self.build()
        (self.notices / "THIRD-PARTY-NOTICES.txt").write_text("old reader-only notices")
        with self.assertRaisesRegex(ValueError, "text mismatch"):
            self.validate(record)
        self.inventory["notices_sha256"] = subject.digest(self.notices / "THIRD-PARTY-NOTICES.txt")
        self.write_inventory()
        with self.assertRaisesRegex(ValueError, "attribution binding"):
            self.validate(record)
        self.inventory["rust_profile"]["features"] = []
        self.write_inventory()
        with self.assertRaisesRegex(ValueError, "profile mismatch"):
            subject.attribution_binding(self.repository)

    def test_authoritative_checker_rejects_missing_sdk_and_missing_readme(self):
        self.inventory["packages"] = [{"name": "roxmltree"}]
        self.write_inventory()
        with self.assertRaises(subprocess.CalledProcessError):
            self.build()
        self.assertFalse(self.built)
        (self.notices / "README.md").unlink()
        with self.assertRaises(FileNotFoundError):
            subject.attribution_binding(self.repository)

    def test_failed_build_retains_original_diagnostics(self):
        self.build_status = 1
        with self.assertRaisesRegex(ValueError, "diagnostics preserved"):
            self.build()
        self.assertFalse(self.output.exists())
        self.assertEqual(
            self.output.with_suffix(".cargo.stderr.log").read_text(),
            "synthetic retained build diagnostic\n",
        )

    def test_validator_refuses_wrong_command_profile_and_features(self):
        record = self.build()
        for key, value in (
            ("command", subject.COMMAND + ["--no-default-features"]),
            ("default_features", False),
            ("profile", "dev"),
            ("target", "aarch64-apple-darwin"),
        ):
            with self.subTest(key=key):
                changed = copy.deepcopy(record)
                changed["build"][key] = value
                with self.assertRaisesRegex(ValueError, "build profile"):
                    self.validate(changed)
        record["artifacts"]["core"]["features"].append("test-support")
        with self.assertRaisesRegex(ValueError, "artifact features"):
            self.validate(record)

    def test_fingerprint_flags_are_not_silently_accepted(self):
        path = self.binary.parent / ".fingerprint/pumas-rpc-synthetic/bin-pumas-rpc.json"
        fingerprint = json.loads(path.read_text())
        fingerprint["rustflags"] = ["--cfg", "test"]
        path.write_text(json.dumps(fingerprint))
        with self.assertRaisesRegex(ValueError, "fingerprint reports"):
            self.build()
        self.assertFalse(self.output.exists())

    def test_active_cargo_configs_are_refused_without_contents(self):
        for directory in (
            self.repository / ".cargo",
            self.directory / ".cargo",
            self.directory / "cargo-home",
        ):
            directory.mkdir(exist_ok=True)
            for name in ("config", "config.toml"):
                with self.subTest(directory=directory.name, name=name):
                    path = directory / name
                    path.write_text("synthetic sensitive content; never parsed or printed")
                    with self.assertRaisesRegex(ValueError, "active Cargo config files") as caught:
                        subject.build_provenance(
                            self.repository,
                            self.output,
                            self.runner,
                            {"CARGO_HOME": str(self.directory / "cargo-home")},
                        )
                    self.assertNotIn("sensitive", str(caught.exception))
                    self.assertFalse(self.built)
                    path.unlink()
        with self.assertRaisesRegex(ValueError, "target Rust flag"):
            subject.check_environment({"CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_RUSTFLAGS": "bad"})

    def test_turbojpeg_routes_refuse_before_s3_cargo_even_when_empty(self):
        for prefix in ("", "X86_64_UNKNOWN_LINUX_GNU_", "AARCH64_APPLE_DARWIN_"):
            for suffix in (
                "SOURCE",
                "STATIC",
                "DYNAMIC",
                "SHARED",
                "BINDING",
                "LIB_DIR",
                "LIB_PATH",
                "INCLUDE_DIR",
                "INCLUDE_PATH",
            ):
                for value in ("", "0", "1", "/outside"):
                    with self.subTest(prefix=prefix, suffix=suffix, value=value):
                        with self.assertRaisesRegex(ValueError, "TurboJPEG routing/binding"):
                            subject.build_provenance(
                                self.repository,
                                self.output,
                                self.runner,
                                {f"{prefix}TURBOJPEG_{suffix}": value},
                            )
                        self.assertFalse(self.built)
                        self.assertFalse(self.output.exists())


class InstalledInputsTests(unittest.TestCase):
    def setUp(self):
        path = Path(__file__).with_name("qualify-s3-installed.py")
        spec = importlib.util.spec_from_file_location("installed_s3", path)
        self.harness = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(self.harness)
        self.temporary = tempfile.TemporaryDirectory(prefix="s3-installed-inputs-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)

    def test_sigv4_query_order_does_not_change_signed_identity(self):
        # Fixed synthetic HMAC vector for the canonical query below. The test
        # does not calculate its own expected signature using the oracle.
        headers = {
            "host": "localhost:443",
            "x-amz-content-sha256": "UNSIGNED-PAYLOAD",
            "x-amz-date": "20260101T000000Z",
            "x-amz-security-token": self.harness.TOKEN,
            "Authorization": (
                f"AWS4-HMAC-SHA256 Credential={self.harness.ACCESS}/"
                "20260101/fixture-region/s3/aws4_request, "
                "SignedHeaders=host;x-amz-content-sha256;x-amz-date;x-amz-security-token, "
                "Signature=8f26803a48d7b13979af61f661f89b824603af92b7d8b70b94cc520c82a55ff6"
            ),
        }
        handler = SimpleNamespace(
            command="GET", headers=headers, server=SimpleNamespace(token=True)
        )
        for query in (
            "versionId=weights-v1&x-id=GetObject",
            "x-id=GetObject&versionId=weights-v1",
        ):
            with self.subTest(query=query):
                handler.path = f"/fixture-bucket/models/shared?{query}"
                self.assertTrue(self.harness.valid_signature(handler, self.harness.SECRET))
                self.assertFalse(self.harness.valid_signature(handler, "wrong-synthetic-secret"))
        headers["x-amz-security-token"] = "wrong-synthetic-token"
        self.assertFalse(self.harness.valid_signature(handler, self.harness.SECRET))
        headers["x-amz-security-token"] = self.harness.TOKEN
        handler.path = "/fixture-bucket/models/shared?x-id=GetObject&versionId=changed"
        self.assertFalse(self.harness.valid_signature(handler, self.harness.SECRET))

    def test_archive_inputs_select_complete_s3_profile_and_exact_member_set(self):
        with patch.object(self.harness, "ROOT", self.root):
            inputs = self.harness.package_inputs(self.root / "binary")
        self.assertEqual(
            [name for _, name in inputs],
            [
                "pumas-rpc",
                "LICENSE.txt",
                "THIRD-PARTY-NOTICES.txt",
                "ATTRIBUTION-README.md",
                "ATTRIBUTION-inventory.json",
            ],
        )
        self.assertTrue(all(subject.ATTRIBUTION in str(path) for path, _ in inputs[2:]))
        installed = self.root / "installed"
        installed.mkdir()
        expected = {}
        for _, name in inputs:
            (installed / name).write_text(f"synthetic {name}")
            expected[name] = subject.digest(installed / name)
        (installed / "qualification.json").write_text("synthetic qualification metadata")
        self.harness.verify_installed_inputs(installed, expected)
        (installed / "credential.txt").write_text("synthetic; must never be packaged")
        with self.assertRaisesRegex(AssertionError, "member set"):
            self.harness.verify_installed_inputs(installed, expected)
        (installed / "credential.txt").unlink()
        (installed / "ATTRIBUTION-inventory.json").unlink()
        with self.assertRaisesRegex(AssertionError, "member set"):
            self.harness.verify_installed_inputs(installed, expected)

    def test_replaced_notice_bytes_fail_after_extraction(self):
        expected = {}
        for name in ["pumas-rpc", "THIRD-PARTY-NOTICES.txt"]:
            (self.root / name).write_text(f"synthetic {name}")
            expected[name] = subject.digest(self.root / name)
        (self.root / "qualification.json").write_text("synthetic metadata")
        (self.root / "THIRD-PARTY-NOTICES.txt").write_text("historical or default notices")
        with self.assertRaisesRegex(AssertionError, "input hash"):
            self.harness.verify_installed_inputs(self.root, expected)

    def test_copy_is_bound_to_build_notice_hashes_not_new_stage_hashes(self):
        with patch.object(self.harness, "ROOT", self.root):
            inputs = self.harness.package_inputs(self.root / "binary")
            for path, name in inputs:
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(f"synthetic {name}")
            provenance = {
                "binary_sha256": subject.digest(inputs[0][0]),
                "attribution": {
                    "sha256": {path.name: subject.digest(path) for path, _ in inputs[2:]}
                },
            }
            stage = self.root / "stage"
            stage.mkdir()
            expected = self.harness.stage_verified_inputs(stage, inputs[0][0], provenance)
            self.assertEqual(
                expected["THIRD-PARTY-NOTICES.txt"],
                provenance["attribution"]["sha256"]["THIRD-PARTY-NOTICES.txt"],
            )
            inputs[2][0].write_text("replacement notices after build validation")
            with self.assertRaisesRegex(AssertionError, "differs from build"):
                self.harness.stage_verified_inputs(stage, inputs[0][0], provenance)


class InstalledProcessLossTests(unittest.TestCase):
    setUp = InstalledInputsTests.setUp

    def custody_fixture(self):
        self.operation = "00000000-0000-4000-8000-000000000001"
        acquisition = "00000000-0000-4000-8000-000000000002"
        self.store = self.root / "launcher-data/downloads.json"
        workspace_name = f".s3-import-{self.operation}"
        self.workspace = self.root / "launcher-data" / workspace_name
        self.workspace.mkdir(parents=True)
        self.partial = self.workspace / "weights.gguf.part"
        self.partial.write_bytes(self.harness.WEIGHTS[:1])
        (self.root / "custody-sentinel").write_bytes(b"owned fixture sentinel")
        self.models = self.root / "shared-resources/models"
        self.models.mkdir(parents=True)
        with sqlite3.connect(self.models / "models.db") as db:
            db.execute("CREATE TABLE models (id TEXT, path TEXT, metadata_json TEXT)")
        self.document = {
            "schema_version": 7,
            "consumer_receipts": {},
            "acquisitions": {
                acquisition: {
                    "id": acquisition,
                    "demand": {"consumer": "model.s3.workflow", "operation": self.operation},
                    "phase": {"state": "transferring"},
                    "files": [],
                    "workspace": {
                        "root_identity": "owned-fixture-root",
                        "relative_target": workspace_name,
                    },
                    "manifest": {
                        "schema_version": 1,
                        "source": {
                            "provider": "s3",
                            "source_id": "owned-fixture",
                            "revision": {
                                "authority": "s3.explicit_versions",
                                "value": "owned-pin",
                                "strength": "immutable",
                            },
                        },
                        "files": [
                            {
                                "logical_path": "weights.gguf",
                                "source_key": json.dumps(["models/shared", "weights-v1"]),
                                "expected_size": len(self.harness.WEIGHTS),
                                "expected_sha256": {
                                    "authority": "caller.sha256",
                                    "value": self.harness.hashlib.sha256(
                                        self.harness.WEIGHTS
                                    ).hexdigest(),
                                },
                                "verification": "sha256",
                            }
                        ],
                    },
                }
            },
        }
        self.store.write_text(json.dumps(self.document))
        self.progress = {
            "status": "running",
            "progress": {"phase": "acquiring", "downloaded_for_current_file": "1"},
        }
        return self.harness.custody_snapshot(self.root, self.operation)

    def test_boundary_requires_live_progress_and_actual_prefix(self):
        before = self.custody_fixture()
        self.harness.verify_crash_boundary(self.progress, before)
        for phase, count, status in (
            ("acquiring", "0", "running"),
            ("acquiring", "2", "running"),
            ("finalizing", "1", "running"),
            ("acquiring", "1", "finished"),
        ):
            progress = {
                "status": status,
                "progress": {"phase": phase, "downloaded_for_current_file": count},
            }
            with self.subTest(phase=phase, count=count, status=status):
                with self.assertRaisesRegex(AssertionError, "live acquiring"):
                    self.harness.verify_crash_boundary(progress, before)
        self.partial.write_bytes(b"")
        with self.assertRaisesRegex(AssertionError, "exactly one byte"):
            self.harness.custody_snapshot(self.root, self.operation)

    def test_ready_verified_or_receipt_state_refuses_crash_boundary(self):
        self.custody_fixture()
        record = next(iter(self.document["acquisitions"].values()))
        for field, value, message in (
            ("phase", {"state": "files_ready"}, "not transferring"),
            ("files", [{"bytes": 1}], "premature byte proof"),
        ):
            original = record[field]
            record[field] = value
            self.store.write_text(json.dumps(self.document))
            with self.assertRaisesRegex(AssertionError, message):
                self.harness.custody_snapshot(self.root, self.operation)
            record[field] = original
        self.document["consumer_receipts"] = {"unexpected": {}}
        self.store.write_text(json.dumps(self.document))
        with self.assertRaisesRegex(AssertionError, "premature byte proof"):
            self.harness.custody_snapshot(self.root, self.operation)

    def test_actual_inode_and_complete_document_changes_refuse_cold_proof(self):
        before = self.custody_fixture()
        self.partial.rename(self.workspace / "retained-original-partial")
        self.partial.write_bytes(self.harness.WEIGHTS[:1])
        after = self.harness.custody_snapshot(self.root, self.operation)
        with self.assertRaisesRegex(AssertionError, "exact retained custody"):
            self.harness.verify_cold_custody(before, after, [], [], {"success": True, "models": {}})
        self.document["unrelated_partition"] = {"changed": True}
        self.store.write_text(json.dumps(self.document))
        changed = self.harness.custody_snapshot(self.root, self.operation)
        with self.assertRaisesRegex(AssertionError, "exact retained custody"):
            self.harness.verify_cold_custody(
                after, changed, [], [], {"success": True, "models": {}}
            )

    def test_source_replay_or_public_model_refuses_cold_proof(self):
        before = self.custody_fixture()
        with self.assertRaisesRegex(AssertionError, "replayed source"):
            self.harness.verify_cold_custody(
                before, before, [], [{"method": "HEAD"}], {"success": True, "models": {}}
            )
        with self.assertRaisesRegex(AssertionError, "published a model"):
            self.harness.verify_cold_custody(
                before, before, [], [], {"success": True, "models": {"unexpected": {}}}
            )

    def test_read_only_index_observation_sees_wal_and_receipt_refusal(self):
        self.custody_fixture()
        with sqlite3.connect(self.models / "models.db") as db:
            db.execute("PRAGMA journal_mode=WAL")
            db.execute("INSERT INTO models VALUES ('unexpected', 'unexpected', '{}')")
            db.commit()
            with self.assertRaisesRegex(AssertionError, "indexed model"):
                self.harness.custody_snapshot(self.root, self.operation)
            db.execute("DELETE FROM models")
            db.commit()
        (self.models / ".pumas_import_publication.json").write_text("{}")
        with self.assertRaisesRegex(AssertionError, "publication receipt"):
            self.harness.custody_snapshot(self.root, self.operation)

    def test_full_or_completed_input_refuses_partial_custody(self):
        self.custody_fixture()
        self.partial.write_bytes(self.harness.WEIGHTS)
        with self.assertRaisesRegex(AssertionError, "exactly one byte"):
            self.harness.custody_snapshot(self.root, self.operation)
        self.partial.write_bytes(self.harness.WEIGHTS[:1])
        (self.workspace / "weights.gguf").write_bytes(self.harness.WEIGHTS)
        with self.assertRaisesRegex(AssertionError, "completed input"):
            self.harness.custody_snapshot(self.root, self.operation)

    def mock_backend(self):
        backend = self.harness.Backend.__new__(self.harness.Backend)
        backend.process = Mock(pid=111)
        backend.process.poll.return_value = None
        backend.process.wait.return_value = -self.harness.signal.SIGKILL
        backend.reader = Mock()
        backend.reader.is_alive.return_value = False
        backend.output, backend.overflow = bytearray(), False
        return backend

    def test_kill_requires_owned_live_group_and_sigkill_status(self):
        backend = self.mock_backend()
        with patch.object(self.harness.os, "killpg") as kill:
            self.assertEqual(backend.kill(), -self.harness.signal.SIGKILL)
            kill.assert_called_once_with(111, self.harness.signal.SIGKILL)
        backend.process.wait.assert_called_once_with(5)
        backend.reader.join.assert_called_once_with(5)
        backend = self.mock_backend()
        backend.process.poll.return_value = 0
        with patch.object(self.harness.os, "killpg") as kill:
            with self.assertRaisesRegex(AssertionError, "already stopped"):
                backend.kill()
            kill.assert_not_called()
        backend = self.mock_backend()
        backend.process.wait.return_value = 0
        with patch.object(self.harness.os, "killpg"):
            with self.assertRaisesRegex(AssertionError, "exit from SIGKILL"):
                backend.kill()

    def test_reader_join_overflow_and_secret_refusals(self):
        backend = self.mock_backend()
        backend.reader.is_alive.return_value = True
        with self.assertRaisesRegex(AssertionError, "reader failed to drain"):
            backend.finish_capture()
        backend.reader.is_alive.return_value = False
        backend.overflow = True
        with self.assertRaisesRegex(AssertionError, "log exceeded"):
            backend.finish_capture()
        backend.overflow = False
        backend.output.extend(self.harness.TOKEN.encode())
        with self.assertRaisesRegex(AssertionError, "credential escaped"):
            backend.finish_capture()


if __name__ == "__main__":
    unittest.main()
