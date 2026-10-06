"""Approved target evidence at the packet/local-consumer boundary; tiny wheels only."""
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import test_install_verified_wheels as fixtures

owner = fixtures.consumer.target_owner()


def packet(observation, artifacts):
    markers = observation["target"]["markers"]
    return owner.bind_resolution({"interpreter": observation["interpreter"],
        "python": markers["python_version"], "implementation": "cpython",
        "machine": markers["platform_machine"], "artifacts": artifacts}, observation)


class ObservationTests(unittest.TestCase):
    def setUp(self):
        self.observation = owner.capture_observation()
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.wheels = self.root / "wheels"
        self.wheels.mkdir()
        self.artifacts = [fixtures.make_wheel(self.wheels, "root-wheel")]
        self.resolution = packet(self.observation, self.artifacts)
        self.approved = self.root / "approved.json"
        self.input = self.root / "resolution.json"
        self.packages, self.output = self.root / "packages", self.root / "proof"

    def command(self, *, approval=True, python=sys.executable):
        self.input.write_text(json.dumps(self.resolution))
        self.approved.write_text(json.dumps(self.observation))
        args = [python, "-I", str(fixtures.ROOT / "install_verified_wheels.py"),
                "--resolution", str(self.input), "--wheels", str(self.wheels),
                "--target", str(self.packages), "--output", str(self.output)]
        return [*args, "--target-observation", str(self.approved)] if approval else args

    def refuse(self, *, approval=True):
        args = self.command(approval=approval)
        with patch.object(sys, "argv", ["consumer", *args[3:]]), patch.object(fixtures.consumer.subprocess, "run", side_effect=AssertionError("pip must not start")):
            with self.assertRaises(SystemExit) as refusal:
                fixtures.consumer.main()
            self.assertEqual(refusal.exception.code, 3)
        self.assertFalse(self.packages.exists())
        self.assertFalse(self.output.exists())

    def test_actual_selected_interpreter_observer_and_canonical_projection(self):
        completed = subprocess.run([sys.executable, "-I", str(fixtures.ROOT / "wheel_target.py"), "--observe"], capture_output=True, check=True)
        observed = json.loads(completed.stdout)
        self.assertEqual(observed, self.observation)
        owner.checked_observation(observed).require_native_consumer()
        self.assertEqual(observed["interpreter_sha256"], hashlib.sha256(Path(sys.executable).read_bytes()).hexdigest())
        self.assertEqual(owner.observation_digest({"z": "é", "a": {"b": True, "a": None}}),
                         hashlib.sha256('{"a":{"a":null,"b":true},"z":"é"}'.encode()).hexdigest())

    def test_binding_preserves_artifact_identity_and_snapshots_approval(self):
        self.assertEqual(self.resolution["artifacts"], self.artifacts)
        self.assertEqual(owner.resolution_target(self.resolution, self.observation).to_dict(), self.observation["target"])
        changed = copy.deepcopy(self.observation)
        changed["target"]["markers"]["platform_release"] += "-changed"
        with self.assertRaisesRegex(ValueError, "differs"):
            owner.resolution_target(self.resolution, changed)

    def test_packet_cannot_replace_target_and_recompute_its_own_acceptance(self):
        replacement = copy.deepcopy(self.observation)
        replacement["target"]["markers"]["platform_release"] += "-changed"
        self.resolution = packet(replacement, self.artifacts)
        self.refuse()

    def test_missing_null_or_removed_context_never_defaults_to_legacy(self):
        self.refuse(approval=False)
        baseline = copy.deepcopy(self.resolution)
        for key in ("wheel_target", "wheel_target_observation_sha256"):
            self.resolution = copy.deepcopy(baseline)
            self.resolution.pop(key)
            self.refuse()
            self.resolution = copy.deepcopy(baseline)
            self.resolution[key] = None
            self.refuse()
        self.resolution = {"artifacts": self.artifacts}
        self.refuse()  # Approval presence forbids downgrading both binding fields.

    def test_unsupported_or_incomplete_owner_context_refuses(self):
        baseline = copy.deepcopy(self.observation)
        for change in (lambda x: x.update(schema="future"), lambda x: x.pop("interpreter_sha256"),
                       lambda x: x["target"].update(os="plan9"), lambda x: x["target"].update(abi="cp312d"),
                       lambda x: x["target"]["markers"].pop("platform_release")):
            self.observation = copy.deepcopy(baseline)
            change(self.observation)
            self.resolution["wheel_target"] = self.observation["target"]
            self.resolution["wheel_target_observation_sha256"] = owner.observation_digest(self.observation)
            self.refuse()

    def test_changed_interpreter_bytes_or_packet_path_refuse_before_pip(self):
        self.observation["interpreter_sha256"] = "0" * 64
        self.resolution = packet(self.observation, self.artifacts)
        self.refuse()
        self.observation = owner.capture_observation()
        self.resolution = packet(self.observation, self.artifacts)
        self.resolution["interpreter"] += "-changed"
        self.refuse()

    def test_approved_native_target_survives_provider_to_venv_handoff(self):
        venv = self.root / "venv"
        subprocess.run([sys.executable, "-I", "-m", "venv", str(venv)], capture_output=True, check=True)
        python = venv / ("Scripts/python.exe" if sys.platform == "win32" else "bin/python")
        # Windows venv redirectors can differ from the provider executable. They
        # need approval of the actual selected interpreter, never a hash waiver.
        if hashlib.sha256(python.read_bytes()).hexdigest() != self.observation["interpreter_sha256"]:
            observed = subprocess.run([str(python), "-I", str(fixtures.ROOT / "wheel_target.py"), "--observe"], capture_output=True, check=True)
            self.observation = json.loads(observed.stdout)
            self.resolution = packet(self.observation, self.artifacts)
        completed = subprocess.run(self.command(python=str(python)), capture_output=True)
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        self.assertEqual(self.resolution["interpreter"], self.observation["interpreter"])
        self.assertTrue(json.loads((self.output / "installed-files.json").read_text())["files"])
        report = json.loads((self.output / "local-pip-report.json").read_text())
        self.assertTrue(all(item["download_info"]["url"].startswith("file:") for item in report["install"]))

    def test_absent_context_remains_explicit_legacy_consumption(self):
        self.resolution = {"artifacts": self.artifacts}
        completed = subprocess.run(self.command(approval=False), capture_output=True)
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        self.assertTrue((self.output / "installed-files.json").is_file())


if __name__ == "__main__":
    unittest.main()
