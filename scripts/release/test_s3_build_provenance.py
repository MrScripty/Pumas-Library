"""Synthetic subprocess/artifact controls; never execute Cargo or a binary."""

import copy
import json
import importlib.util
from pathlib import Path
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch

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


if __name__ == "__main__":
    unittest.main()
