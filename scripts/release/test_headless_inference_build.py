"""Controlled build subprocesses and fixture bytes; no Cargo/model/ORT execution."""

import copy
from contextlib import chdir
import json
import os
import shutil
import subprocess
from pathlib import Path
from types import SimpleNamespace
import tempfile
import unittest
from unittest.mock import patch

import headless_inference as package
import headless_inference_build as subject
from test_headless_inference import fixture


class BuildCandidateTests(unittest.TestCase):
    def test_all_turbojpeg_routes_refuse_before_any_subprocess_even_when_empty(self):
        names = (
            "STATIC",
            "DYNAMIC",
            "SHARED",
            "SOURCE",
            "LIB_DIR",
            "LIB_PATH",
            "INCLUDE_DIR",
            "INCLUDE_PATH",
            "BINDING",
        )
        for prefix in ("", "X86_64_UNKNOWN_LINUX_GNU_", "AARCH64_APPLE_DARWIN_"):
            for name in names:
                for value in ("", "0", "1", "/outside/library"):
                    with self.subTest(prefix=prefix, name=name, value=value):
                        self.calls.clear()
                        with self.assertRaisesRegex(ValueError, "TurboJPEG routing/binding"):
                            self.produce(environment={f"{prefix}TURBOJPEG_{name}": value})
                        self.assertEqual(self.calls, [])

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
        self.reported_root = self.repository
        self.root_after_build = None
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
            self.cargo_environment = dict(kwargs["env"])
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
        elif command == ["git", "rev-parse", "--show-toplevel"]:
            stdout = str(
                self.root_after_build
                if self.built and self.root_after_build
                else self.reported_root
            )
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

    def produce(self, environment=None, real_configuration=False):
        with (
            patch.object(
                subject.provenance,
                "check_configuration",
                wraps=subject.provenance.check_configuration if real_configuration else None,
            ) as configuration_check,
            patch.object(
                subject.provenance, "attribution_binding", return_value={"controlled": True}
            ),
            patch.object(subject.platform, "system", return_value="Linux"),
            patch.object(subject.platform, "machine", return_value="x86_64"),
        ):
            self.configuration_check = configuration_check
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
                environment=environment
                if environment is not None
                else {"CARGO_HOME": str(self.root / "cargo")},
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

    def test_compiled_rpc_vision_schema_projects_to_existing_core_identity(self):
        self.contract["expected"]["build_info"]["schemas"].append(
            {"name": "pumas.model-operations.image-to-text", "version": 1}
        )
        evidence = self.produce()
        self.assertEqual(
            evidence["compiled_core_projection"], self.contract["expected"]["core_build_info"]
        )
        self.assertIn(
            {"name": "pumas.model-operations.image-to-text", "version": 1},
            evidence["compiled_build_info"]["schemas"],
        )

    def test_unknown_rpc_schema_or_vision_version_is_not_stripped(self):
        for index, schema in enumerate(
            (
                {"name": "pumas.model-operations.image-to-text", "version": 2},
                {"name": "pumas.unreviewed-rpc-schema", "version": 1},
            )
        ):
            with self.subTest(schema=schema):
                rpc = copy.deepcopy(self.contract["expected"]["build_info"])
                rpc["schemas"].append(schema)
                projection = subject.core_projection(rpc)
                self.assertIn(schema, projection["schemas"])
                self.assertNotEqual(projection, self.contract["expected"]["core_build_info"])
                self.output = self.root / f"schema-refusal-{index}"
                self.contract["expected"]["build_info"]["schemas"].append(schema)
                with self.assertRaisesRegex(ValueError, "compiled core PumasBuildInfo mismatch"):
                    self.produce()
                self.contract["expected"]["build_info"]["schemas"].pop()
                self.assertFalse(list(self.output.glob("*.tar.gz")))

    def test_vision_schema_without_compiled_feature_is_not_stripped(self):
        rpc = copy.deepcopy(self.contract["expected"]["build_info"])
        rpc["compiled_features"].remove("pumas-rpc/inference-plugins")
        schema = {"name": "pumas.model-operations.image-to-text", "version": 1}
        rpc["schemas"].append(schema)
        self.assertIn(schema, subject.core_projection(rpc)["schemas"])

    def test_relative_cargo_home_is_frozen_for_pre_post_checks_and_build(self):
        launch_directory = self.root / "launch-directory"
        launch_directory.mkdir()
        cargo_home = self.repository / "relative-cargo-home"
        with chdir(launch_directory):
            self.produce(environment={"CARGO_HOME": "relative-cargo-home"}, real_configuration=True)
        expected_home = str(cargo_home.resolve())
        self.assertEqual(self.configuration_check.call_count, 2)
        for call in self.configuration_check.call_args_list:
            self.assertEqual(call.args[0], self.repository)
            self.assertEqual(call.args[1]["CARGO_HOME"], expected_home)
        self.assertEqual(self.cargo_environment["CARGO_HOME"], expected_home)

    def test_relative_cargo_home_new_config_is_refused_after_build(self):
        launch_directory = self.root / "launch-directory"
        launch_directory.mkdir()
        cargo_home = self.repository / "relative-cargo-home"
        original_runner = self.runner

        def introduce_config(command, **options):
            if command[:2] == ["cargo", "build"]:
                cargo_home.mkdir()
                (cargo_home / "config.toml").write_text(
                    "[profile.release]\nlto = false\ncodegen-units = 16\n"
                )
            return original_runner(command, **options)

        self.runner = introduce_config
        with chdir(launch_directory):
            with self.assertRaisesRegex(ValueError, "active Cargo config files"):
                self.produce(
                    environment={"CARGO_HOME": "relative-cargo-home"}, real_configuration=True
                )
        self.assertTrue(self.built)
        self.assertFalse(list(self.output.glob("*.tar.gz")))

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

    def test_different_git_root_refuses_before_build(self):
        self.reported_root = self.root / "other-checkout"
        self.reported_root.mkdir()
        with self.assertRaisesRegex(ValueError, "Git working root differs"):
            self.produce()
        self.assertFalse(self.built)

    def test_unavailable_git_root_refuses_before_build(self):
        self.reported_root = self.root / "missing-checkout"
        with self.assertRaisesRegex(ValueError, "Git working root unavailable"):
            self.produce()
        self.assertFalse(self.built)

    def test_changed_git_root_after_build_refuses_archive(self):
        self.root_after_build = self.root / "other-checkout"
        self.root_after_build.mkdir()
        with self.assertRaisesRegex(ValueError, "Git working root differs"):
            self.produce()
        self.assertFalse(list(self.output.glob("*.tar.gz")))

    def test_real_git_core_worktree_redirect_refuses_before_cargo(self):
        environment = {
            name: value for name, value in os.environ.items() if not name.startswith("GIT_")
        }

        def git(*arguments):
            return subprocess.run(
                ["git", *arguments],
                cwd=self.repository,
                env=environment,
                check=True,
                capture_output=True,
                text=True,
            ).stdout.strip()

        git("init")
        git("config", "user.name", "MrScripty")
        git("config", "user.email", "TheEnvironmentGuy@protonmail.com")
        git("add", ".")
        git("commit", "-m", "Controlled root-binding fixture")
        expected = self.contract["expected"]
        expected["source_commit"], expected["source_tree"] = (
            git("rev-parse", "HEAD"),
            git("rev-parse", "HEAD^{tree}"),
        )
        for name in ("build_info", "core_build_info"):
            expected[name]["source_revision"] = expected["source_commit"]
        observed = self.root / "observed-checkout"
        shutil.copytree(self.repository, observed, ignore=shutil.ignore_patterns(".git"))
        git("config", "core.worktree", str(observed))
        manifest = self.repository / "rust/Cargo.toml"
        manifest.write_text(manifest.read_text() + "\n# Changed actual build input\n")
        self.assertEqual(git("status", "--porcelain"), "")
        self.assertEqual(Path(git("rev-parse", "--show-toplevel")), observed)
        original_runner = self.runner

        def real_git_runner(command, **options):
            if command[0] == "git":
                return subprocess.run(command, **options)
            if command[:2] == ["cargo", "build"]:
                self.built = True
                raise AssertionError("Cargo must never be invoked for redirected Git root")
            return original_runner(command, **options)

        self.runner = real_git_runner
        with self.assertRaisesRegex(ValueError, "Git working root differs"):
            self.produce()
        self.assertFalse(self.built)

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

    def test_relative_cargo_home_uses_build_directory_for_config_refusal(self):
        launch_directory = self.root / "launch-directory"
        launch_directory.mkdir()
        cargo_home = self.repository / "relative-cargo-home"
        cargo_home.mkdir()
        (cargo_home / "config.toml").write_text(
            "[profile.release]\nlto = false\ncodegen-units = 16\n"
        )
        original_runner = self.runner

        def refuse_cargo(command, **options):
            if command[:2] == ["cargo", "build"]:
                self.built = True
                raise AssertionError(
                    "Cargo reached despite active build-directory CARGO_HOME config"
                )
            return original_runner(command, **options)

        with (
            chdir(launch_directory),
            patch.object(
                subject.provenance, "attribution_binding", return_value={"controlled": True}
            ),
            patch.object(subject.platform, "system", return_value="Linux"),
            patch.object(subject.platform, "machine", return_value="x86_64"),
        ):
            with self.assertRaisesRegex(ValueError, "active Cargo config files"):
                subject.produce(
                    self.repository,
                    "linux-x86_64",
                    self.contract,
                    self.runtime,
                    self.runtime_inputs,
                    self.schema,
                    self.inputs["LICENSE.txt"],
                    self.inputs["THIRD-PARTY-NOTICES.txt"],
                    self.output,
                    runner=refuse_cargo,
                    environment={"CARGO_HOME": "relative-cargo-home"},
                )
        self.assertFalse(self.built)

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


class RealGitRootTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.repository = self.root / "repository"
        self.repository.mkdir()
        self.environment = {
            name: value for name, value in os.environ.items() if not name.startswith("GIT_")
        }
        self.git("init")
        self.git("config", "user.name", "MrScripty")
        self.git("config", "user.email", "TheEnvironmentGuy@protonmail.com")
        (self.repository / "tracked-input").write_text("Controlled Git fixture\n")
        self.git("add", ".")
        self.git("commit", "-m", "Controlled Git identity fixture")
        self.expected = {
            "head": self.git("rev-parse", "HEAD"),
            "tree": self.git("rev-parse", "HEAD^{tree}"),
        }

    def runner(self, command, **options):
        options["env"] = self.environment
        return subprocess.run(command, **options)

    def git(self, *arguments):
        return self.runner(
            ["git", *arguments], cwd=self.repository, check=True, capture_output=True, text=True
        ).stdout.strip()

    def test_valid_linked_worktree_keeps_real_root_identity(self):
        linked = self.root / "linked-worktree"
        self.git("worktree", "add", "--detach", str(linked), "HEAD")
        self.assertEqual(subject.source_identity(linked, self.runner), self.expected)

    def test_valid_symlink_alias_keeps_real_root_identity(self):
        alias = self.root / "alias"
        try:
            alias.symlink_to(self.repository, target_is_directory=True)
        except OSError as error:
            self.skipTest(f"native directory symlink unavailable: {error}")
        self.assertEqual(subject.source_identity(alias, self.runner), self.expected)


if __name__ == "__main__":
    unittest.main()
