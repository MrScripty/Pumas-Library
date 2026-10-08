"""Controlled build subprocesses and fixture bytes; no Cargo/model/ORT execution."""

import copy
import json
import os
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import headless_inference as package
import headless_inference_build as subject
from test_headless_inference import fixture


class BuildCandidateTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.repository = self.root / "repository"
        (self.repository / "rust").mkdir(parents=True)
        (self.repository / "rust/Cargo.toml").write_text(
            '[workspace.package]\nversion = "0.8.0-test"\n'
            '[profile.release]\nlto = true\ncodegen-units = 1\nstrip = "debuginfo"\n'
        )
        self.inputs, self.build, self.runtime, self.contract, self.schema, _ = fixture(self.root)
        binary = self.root / "pumas-rpc"
        binary.write_bytes(self.inputs["pumas-rpc"].read_bytes())
        self.inputs["pumas-rpc"] = binary
        notices = self.repository / subject.provenance.ATTRIBUTION
        notices.mkdir(parents=True)
        (notices / "THIRD-PARTY-NOTICES.txt").write_bytes(
            self.inputs["THIRD-PARTY-NOTICES.txt"].read_bytes()
        )
        (self.repository / "LICENSE").write_bytes(self.inputs["LICENSE.txt"].read_bytes())
        self.runtime_inputs = {name: self.inputs[name] for name in self.runtime["files"]}
        self.output = self.root / "output"
        self.calls = []
        self.dirty_after_build = False
        self.changed_info = False
        self.status = 0
        self.built = False
        self.events = []
        for owner, directory, name, kind, features in (
            ("rpc", "pumas-rpc", "pumas-rpc", "bin", ["inference-plugins", "s3"]),
            ("core", "pumas-core", "pumas_library", "lib", subject.CORE_FEATURES),
        ):
            self.events.append(
                {
                    "reason": "compiler-artifact",
                    "manifest_path": str(self.repository / f"rust/crates/{directory}/Cargo.toml"),
                    "target": {"name": name, "kind": [kind]},
                    "features": features,
                    "profile": {
                        "opt_level": "3",
                        "test": False,
                        "debug_assertions": False,
                        "overflow_checks": False,
                        "debuginfo": 0,
                    },
                    "executable": str(self.inputs["pumas-rpc"]) if owner == "rpc" else None,
                    "package_id": f"controlled-{owner}",
                }
            )
        self.events.append({"reason": "build-finished", "success": True})

    def runner(self, command, **kwargs):
        self.calls.append(command)
        if command[:2] == ["cargo", "build"]:
            self.built = True
            self.assertEqual(kwargs["env"]["ORT_SKIP_DOWNLOAD"], "1")
            self.assertEqual(kwargs["env"]["CARGO_BUILD_JOBS"], "1")
            self.assertEqual(kwargs["env"]["PUMAS_SOURCE_REVISION"], "a" * 40)
            kwargs["stdout"].write("\n".join(json.dumps(item) for item in self.events))
            kwargs["stderr"].write("controlled diagnostics\n")
            return SimpleNamespace(returncode=self.status)
        if command[:2] == ["git", "status"]:
            self.assertNotIn("GIT_DIR", kwargs["env"])
            self.assertEqual(kwargs["env"]["ORT_SKIP_DOWNLOAD"], "1")
            stdout = " M src" if self.built and self.dirty_after_build else ""
        elif command == ["git", "rev-parse", "HEAD"]:
            stdout = "a" * 40
        elif command == ["git", "rev-parse", "HEAD^{tree}"]:
            stdout = "b" * 40
        elif command == ["rustc", "-Vv"]:
            stdout = "rustc 1.92.0 (ded5c06cf 2025-12-08)\nhost: x86_64-unknown-linux-gnu\n"
        elif command == [str(self.inputs["pumas-rpc"]), "--build-info"]:
            info = copy.deepcopy(self.contract["expected"]["build_info"])
            if self.changed_info:
                info["source_revision"] = "c" * 40
            stdout = json.dumps(info)
        else:
            stdout = ""
        return SimpleNamespace(stdout=stdout, returncode=0)

    def produce(self):
        with (
            patch.object(subject.provenance, "check_configuration"),
            patch.object(
                subject.provenance, "attribution_binding", return_value={"controlled": True}
            ),
            patch.object(subject.platform, "system", return_value="Linux"),
            patch.object(subject.platform, "machine", return_value="x86_64"),
        ):
            return subject.produce(
                self.repository,
                "linux-x86_64",
                self.contract,
                self.runtime,
                self.runtime_inputs,
                self.schema,
                self.inputs["LICENSE.txt"],
                self.inputs["THIRD-PARTY-NOTICES.txt"],
                self.output,
                runner=self.runner,
                environment={"CARGO_HOME": str(self.root / "cargo")},
            )

    def test_builds_then_assembles_exact_inference_source_cohort(self):
        result = self.produce()
        self.assertEqual(result["qualification"], "unverified_candidate")
        self.assertEqual(result["source"], {"head": "a" * 40, "tree": "b" * 40})
        command = next(command for command in self.calls if command[:2] == ["cargo", "build"])
        self.assertIn(
            command,
            package.production_build_commands(subject.TARGETS["linux-x86_64"]["rust_target"]),
        )
        self.assertNotIn(
            "default", json.loads((self.output / "build-record.json").read_text())["features"]
        )
        self.assertEqual(package.sha256(result["archive"]), result["archive_sha256"])
        manifest = package.extract_verified(
            result["archive"], result["archive_sha256"], self.root / "extract"
        )
        self.assertEqual(manifest["qualification"], "unverified_candidate")
        self.assertTrue((self.output / "build-evidence.json").exists())

    def test_current_07_version_refuses_before_any_build(self):
        self.contract["expected"]["version"] = "0.7.0"
        with self.assertRaisesRegex(ValueError, "v0.8 candidate version required"):
            self.produce()
        self.assertFalse(self.built)

    def test_runtime_byte_mismatch_refuses_before_any_build(self):
        next(iter(self.runtime_inputs.values())).write_bytes(b"changed runtime")
        with self.assertRaisesRegex(ValueError, "runtime bytes mismatch"):
            self.produce()
        self.assertFalse(self.built)

    def test_current_source_version_cannot_be_relabelled_by_08_contract(self):
        manifest = self.repository / "rust/Cargo.toml"
        manifest.write_text(manifest.read_text().replace("0.8.0-test", "0.7.0"))
        with self.assertRaisesRegex(ValueError, "source package version mismatch"):
            self.produce()
        self.assertFalse(self.built)

    def test_core_protocol_pin_cannot_be_independent_of_compiled_rpc_identity(self):
        self.contract["expected"]["core_build_info"]["protocols"].append(
            {"name": "controlled.additional", "versions": [1]}
        )
        with self.assertRaisesRegex(ValueError, "compiled core PumasBuildInfo mismatch"):
            self.produce()
        self.assertFalse(list(self.output.glob("*.tar.gz")))

    def test_default_feature_contamination_refuses_archive(self):
        self.events[0]["features"].append("default")
        with self.assertRaisesRegex(ValueError, "unexpected rpc features"):
            self.produce()
        self.assertFalse(list(self.output.glob("*.tar.gz")))

    def test_compiled_shared_identity_mismatch_refuses_archive(self):
        self.changed_info = True
        with self.assertRaisesRegex(ValueError, "compiled PumasBuildInfo mismatch"):
            self.produce()
        self.assertFalse(list(self.output.glob("*.tar.gz")))

    def test_source_change_refuses_archive(self):
        self.dirty_after_build = True
        with self.assertRaisesRegex(ValueError, "clean committed checkout"):
            self.produce()
        self.assertFalse(list(self.output.glob("*.tar.gz")))

    def test_failed_build_preserves_diagnostics_without_record(self):
        self.status = 1
        with self.assertRaisesRegex(ValueError, "Cargo build failed"):
            self.produce()
        self.assertTrue((self.output / "cargo.stderr.log").exists())
        self.assertFalse((self.output / "build-record.json").exists())

    def test_release_override_refuses_before_build(self):
        with self.assertRaisesRegex(ValueError, "release profile overrides"):
            subject.check_environment(
                {"CARGO_PROFILE_RELEASE_LTO": "false"}, "x86_64-unknown-linux-gnu"
            )

    def test_git_routing_override_refuses_before_source_observation(self):
        for name in ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_CONFIG_COUNT"):
            with self.subTest(name=name):
                with self.assertRaisesRegex(ValueError, "Git routing/config overrides"):
                    subject.check_environment(
                        {name: "/other/pinned/source"}, "x86_64-unknown-linux-gnu"
                    )

    def test_ambient_git_route_is_not_inherited_by_explicit_build_environment(self):
        with patch.dict(os.environ, {"GIT_DIR": "/other/pinned/source"}):
            self.produce()

    def test_compiler_wrapper_overrides_are_refused(self):
        for name in (
            "RUSTC",
            "RUSTC_WRAPPER",
            "RUSTC_WORKSPACE_WRAPPER",
            "CARGO_BUILD_RUSTC",
            "CARGO_BUILD_RUSTC_WRAPPER",
            "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
        ):
            with self.subTest(name=name):
                with self.assertRaisesRegex(ValueError, "compiler/wrapper overrides"):
                    subject.check_environment(
                        {name: "/unreviewed/compiler"}, "x86_64-unknown-linux-gnu"
                    )


if __name__ == "__main__":
    unittest.main()
