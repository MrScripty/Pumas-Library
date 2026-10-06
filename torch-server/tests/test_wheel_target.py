"""Explicit cross-target preflight; no remote wheels, resolver or package code."""
import copy
import importlib.util
from pathlib import Path
import sys
import sysconfig
import unittest
from unittest.mock import patch

import test_install_verified_wheels as fixtures

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("target_owner", ROOT / "wheel_target.py")
owner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(owner)
fixture_spec = importlib.util.spec_from_file_location("target_fixture", ROOT.parent / "docs/plans/artifact-acquisition/reports/torch-explicit-target-2026-10-06/fixture_target.py")
factory = importlib.util.module_from_spec(fixture_spec)
fixture_spec.loader.exec_module(factory)


class TargetTests(unittest.TestCase):
    def test_complete_markers_and_tags_ignore_inspection_host(self):
        for system in ("linux", "windows", "macos"):
            with self.subTest(system=system), patch.object(sysconfig, "get_config_var", side_effect=AssertionError("host ABI probe")), patch.object(owner.tags_api, "sys_tags", side_effect=AssertionError("host tag probe")):
                target = owner.WheelTarget(factory.target(system, python="3.12.7"))
                self.assertEqual(target.markers["python_full_version"], "3.12.7")
                with patch.object(owner.markers_api, "default_environment", return_value={key: "inspection-host" for key in owner.MARKER_KEYS}):
                    marker = owner.markers_api.Marker('python_full_version == "3.12.7" and implementation_version == "3.12.7"')
                    self.assertTrue(marker.evaluate(target.marker_environment()))
                    self.assertTrue(owner.markers_api.Marker('platform_release == "' + target.markers["platform_release"] + '"').evaluate(target.marker_environment()))

    def test_python_patch_specifiers_and_explicit_normal_abi(self):
        target = owner.WheelTarget(factory.target("windows", python="3.12.7"))
        self.assertFalse(target.allows_python(">=3.12.8"))
        self.assertTrue(target.allows_python(">=3.12.7,<3.13"))
        for tag, compatible in (("cp312-cp312-win_amd64", True), ("cp311-abi3-win_amd64", True),
                                ("cp311-cp311-win_amd64", False), ("cp312-cp312d-win_amd64", False),
                                ("cp313-cp313t-win_amd64", False), ("py3-none-any", True)):
            self.assertEqual(target.supports(owner.tags_api.parse_tag(tag)), compatible, tag)

    def test_linux_libc_floors_aliases_and_foreign_wheels(self):
        for family, version, yes, no in (("glibc", "2.17", "cp312-cp312-manylinux2014_x86_64", "cp312-cp312-manylinux_2_28_x86_64"),
                                         ("musl", "1.2", "cp312-cp312-musllinux_1_1_x86_64", "cp312-cp312-manylinux_2_17_x86_64")):
            target = owner.WheelTarget(factory.target("linux", python="3.12.7", libc=family, libc_version=version))
            self.assertTrue(target.supports(owner.tags_api.parse_tag(yes)))
            self.assertFalse(target.supports(owner.tags_api.parse_tag(no)))
            self.assertFalse(target.supports(owner.tags_api.parse_tag("cp312-cp312-win_amd64")))
        document = factory.target("linux", python="3.12.7")
        document["native_linux_tag"] = False
        self.assertFalse(owner.WheelTarget(document).supports(owner.tags_api.parse_tag("cp312-cp312-linux_x86_64")))

    def test_macos_arm64_deployment_and_universal2(self):
        target = owner.WheelTarget(factory.target("macos", python="3.12.7"))
        for tag, compatible in (("cp312-cp312-macosx_11_0_arm64", True), ("cp312-cp312-macosx_14_0_universal2", True),
                                ("cp312-cp312-macosx_15_0_arm64", False), ("cp312-cp312-macosx_14_0_x86_64", False)):
            self.assertEqual(target.supports(owner.tags_api.parse_tag(tag)), compatible)

    def test_unknown_incomplete_contradictory_targets_fail_closed(self):
        baseline = factory.target("linux", python="3.12.7")
        variants = [None, {}, "linux", {**baseline, "os": "plan9"}, {**baseline, "arch": "aarch64"},
                    {**baseline, "python": "3.12"}, {**baseline, "python": "3.12.7rc1"},
                    {**baseline, "abi": "cp312d"}, {**baseline, "abi": "cp313t"},
                    {**baseline, "libc": None}, {**baseline, "libc": {"family": "unknown", "version": "2.17"}},
                    {**baseline, "libc": {"family": "glibc", "version": "3.0"}},
                    {**baseline, "markers": {}}]
        for key in owner.MARKER_KEYS:
            changed = copy.deepcopy(baseline)
            changed["markers"].pop(key)
            variants.append(changed)
        for variant in variants:
            with self.subTest(variant=variant), self.assertRaises(owner.UnsupportedTarget):
                owner.WheelTarget(variant)
        wrong = copy.deepcopy(baseline)
        wrong["markers"]["python_full_version"] = "3.12.14"
        with self.assertRaises(owner.UnsupportedTarget):
            owner.WheelTarget(wrong)

    def test_wire_and_marker_data_cannot_mutate_validated_target(self):
        document = factory.target("windows", python="3.12.7")
        target = owner.WheelTarget(document)
        document["markers"]["sys_platform"] = "linux"
        target.to_dict()["markers"]["sys_platform"] = "linux"
        self.assertEqual(target.markers["sys_platform"], "win32")
        with self.assertRaises(TypeError):
            target.markers["sys_platform"] = "linux"

    def test_explicit_consumer_preflight_uses_full_target_markers(self):
        with fixtures.tempfile.TemporaryDirectory() as directory:
            wheels = Path(directory)
            artifact = fixtures.make_wheel(wheels, "root-wheel", requires=['absent-wheel ; python_full_version >= "3.12.8"', 'absent-windows ; sys_platform == "win32"'])
            with patch.object(fixtures.consumer, "default_environment", side_effect=AssertionError("inspection marker probe")), patch.object(fixtures.consumer, "sys_tags", side_effect=AssertionError("inspection tag probe")):
                lines, _ = fixtures.consumer.local_requirements([artifact], wheels, wheel_target=factory.target("linux", python="3.12.7"))
                self.assertEqual(len(lines), 1)
                for target in (factory.target("linux", python="3.12.14"), factory.target("windows", python="3.12.7")):
                    with self.assertRaisesRegex(ValueError, "required dependency"):
                        fixtures.consumer.local_requirements([artifact], wheels, wheel_target=target)

    def test_foreign_explicit_target_cannot_install_in_native_consumer(self):
        foreign = "linux" if sys.platform == "win32" else "windows"
        with fixtures.tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = fixtures.make_wheel(root, "root-wheel")
            with patch.object(fixtures.consumer.subprocess, "run", side_effect=AssertionError("pip must not run")):
                with self.assertRaisesRegex(ValueError, "native consumer"):
                    fixtures.consumer.install([artifact], root, root / "output", root / "proof", wheel_target=factory.target(foreign, python="3.12.7"))
            self.assertFalse((root / "output").exists())

    def test_native_capture_reuses_public_observed_target_tags(self):
        target = owner.capture_native()
        self.assertEqual(set(target.tags), set(owner.tags_api.sys_tags()))
        self.assertEqual(dict(target.markers), owner.markers_api.default_environment())
        target.require_native_consumer()

    def test_actual_local_install_validates_an_explicit_native_target(self):
        with fixtures.tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            wheels = root / "wheels"
            wheels.mkdir()
            artifact = fixtures.make_wheel(wheels, "root-wheel")
            target = owner.capture_native().to_dict()
            result = fixtures.consumer.install([artifact], wheels, root / "packages", root / "proof", wheel_target=target)
            self.assertTrue(result["files"])

    def test_cli_explicit_missing_or_foreign_target_refuses_before_pip(self):
        import json
        with fixtures.tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            artifact = fixtures.make_wheel(root, "root-wheel")
            resolution = root / "resolution.json"
            for target in (None, {}, factory.target("windows" if sys.platform != "win32" else "linux", python="3.12.7")):
                resolution.write_text(json.dumps({"artifacts": [artifact], "wheel_target": target}))
                command = ["consumer", "--resolution", str(resolution), "--wheels", str(root),
                           "--target", str(root / "packages"), "--output", str(root / "proof")]
                with patch.object(sys, "argv", command), patch.object(fixtures.consumer.subprocess, "run", side_effect=AssertionError("pip must not run")):
                    with self.assertRaises(SystemExit) as refusal:
                        fixtures.consumer.main()
                    self.assertEqual(refusal.exception.code, 3)
                self.assertFalse((root / "packages").exists())


if __name__ == "__main__":
    unittest.main()
