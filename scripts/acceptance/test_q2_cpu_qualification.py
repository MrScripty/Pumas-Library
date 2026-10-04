"""Offline failure oracles for the bounded hosted qualification job."""

import hashlib
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

import q2_cpu_qualification as qualification


class QualificationTests(unittest.TestCase):
    def test_insufficient_space_stops_before_source_requests(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with (
                patch.object(qualification.shutil, "disk_usage") as usage,
                patch.object(qualification.urllib.request, "urlopen") as request,
            ):
                usage.return_value.free = qualification.GIB
                with self.assertRaisesRegex(AssertionError, "Need 12 GiB"):
                    qualification.preflight(root, root, {"artifacts": []})
                request.assert_not_called()
            record = json.loads((root / "ci-preflight.json").read_text())
            self.assertFalse(record["allowed"])
            self.assertFalse(record["payload_downloaded"])

    def test_ordinary_preflight_needs_only_disk_and_pinned_sources(self):
        artifact = {"name": "torch", "url": "https://download.pytorch.org/pinned.whl"}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with (
                patch.object(qualification.shutil, "disk_usage") as usage,
                patch.object(qualification.urllib.request, "urlopen") as request,
                patch.object(
                    qualification.shutil,
                    "which",
                    side_effect=AssertionError("No external security facility is required"),
                ),
            ):
                usage.return_value.free = 20 * qualification.GIB
                response = request.return_value.__enter__.return_value
                response.status = 200
                response.headers = {"content-length": "100"}
                qualification.preflight(root, root, {"artifacts": [artifact]})
                self.assertEqual(request.call_count, 1)
                self.assertEqual(request.call_args.args[0].method, "HEAD")
            record = json.loads((root / "ci-preflight.json").read_text())
            self.assertTrue(record["allowed"])
            self.assertFalse(record["payload_downloaded"])

    def test_denied_source_is_not_retried(self):
        artifact = {"name": "torch", "url": "https://download.pytorch.org/pinned.whl"}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with (
                patch.object(qualification.shutil, "disk_usage") as usage,
                patch.object(
                    qualification.urllib.request,
                    "urlopen",
                    side_effect=HTTPError(artifact["url"], 403, "Denied", {}, None),
                ) as request,
            ):
                usage.return_value.free = 20 * qualification.GIB
                with self.assertRaises(HTTPError):
                    qualification.preflight(root, root, {"artifacts": [artifact]})
                self.assertEqual(request.call_count, 1)
            record = json.loads((root / "ci-preflight.json").read_text())
            self.assertFalse(record["allowed"])
            self.assertEqual(record["blocked_artifact"], artifact)

    def test_missing_lengths_cannot_authorize_unbounded_downloads(self):
        for length in (None, "", "unknown", "0", "-1"):
            with self.subTest(length=length), self.assertRaises(AssertionError):
                qualification.resource_budget([{"content_length": length}])
        self.assertEqual(
            qualification.resource_budget([{"content_length": str(qualification.GIB)}]),
            (qualification.GIB, 6 * qualification.GIB),
        )

    def test_failed_local_only_install_cannot_pass(self):
        result = {"success": False}
        with self.assertRaisesRegex(AssertionError, "cannot pass"):
            qualification.complete_result(result, qualification.CPU_RESULT, {"status": "failed"})
        self.assertFalse(result["success"])
        self.assertEqual(result["local_only_install"]["status"], "failed")

    def test_success_claims_real_cpu_and_pip_policy_only(self):
        result = {"success": False}
        qualification.complete_result(result, qualification.CPU_RESULT, {"status": "passed"})
        self.assertTrue(result["success"])
        self.assertFalse(result["os_enforced_network_isolation"])
        self.assertFalse(result["original_child_custody_denial_proof"])

    def test_local_only_worker_is_invoked_directly_with_owned_scratch(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "runtime"
            runtime.mkdir()
            frozen_worker = (
                Path(qualification.__file__).resolve().parents[2]
                / "torch-server/resolve_runtime.py"
            )
            (runtime / "resolve_runtime.py").write_bytes(frozen_worker.read_bytes())
            handoff = root / "handoff.json"
            qualification.write(handoff, {"schema_version": 1, "wheels": []})
            requirements = handoff.with_suffix(".requirements.txt")
            requirements.write_text("retained local hash-locked inputs\n")
            with patch.object(
                qualification.subprocess,
                "run",
                return_value=SimpleNamespace(stdout=json.dumps(qualification.CPU_RESULT)),
            ) as run:
                proof = qualification.local_only_install(root, runtime, handoff, root)
            self.assertEqual(run.call_count, 3)
            worker = run.call_args_list[1]
            self.assertEqual(
                worker.args[0],
                [
                    str(root / "local-only-venv/bin/python"),
                    "-I",
                    str(runtime / "resolve_runtime.py"),
                    "--_pumas-local-wheel-install",
                    str(handoff),
                ],
            )
            self.assertEqual(worker.kwargs["env"]["TMPDIR"], str(root / "local-only-tmp"))
            self.assertNotIn("timeout", worker.kwargs)
            self.assertTrue(worker.kwargs["check"])
            self.assertIn("--no-index", proof["frozen_worker_pip_args"])
            self.assertIn("--require-hashes", proof["frozen_worker_pip_args"])
            self.assertFalse(proof["os_enforced_network_isolation"])
            self.assertEqual(
                (root / "local-hash-locked-requirements.txt").read_bytes(),
                requirements.read_bytes(),
            )

    def test_wrong_tensor_result_cannot_pass(self):
        result = {"success": False}
        with self.assertRaises(AssertionError):
            qualification.complete_result(
                result, {**qualification.CPU_RESULT, "result": 13}, {"status": "passed"}
            )
        self.assertFalse(result["success"])

    def test_closure_drift_preserves_actual_identities_and_fails(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            runtime = root / "torch-versions/v2.14.0"
            runtime.mkdir(parents=True)
            actual = {
                "artifacts": [
                    {
                        "name": "torch",
                        "version": "drift",
                        "url": "https://example.invalid/drift.whl",
                        "sha256": "a" * 64,
                    }
                ]
            }
            qualification.write(runtime / "resolution.json", actual)
            qualification.write(runtime / "runtime.json", {"managed_python": {}})
            with self.assertRaisesRegex(AssertionError, "differs"):
                qualification.local_handoff(root, root, {"artifacts": []}, {})
            self.assertEqual(
                json.loads((root / "actual-wheel-identities.json").read_text()), actual
            )

    def test_receipts_exclude_payloads_and_environment_and_bound_logs(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "payload.whl").write_bytes(b"payload")
            (root / "test-owned-xdg").mkdir()
            (root / "test-owned-xdg/private.json").write_text("private")
            (root / "runtime.json").symlink_to(root / "payload.whl")
            original = b"x" * (128 * 1024)
            (root / "production-install.log").write_bytes(original)
            qualification.write(root / "qualification-result.json", {"success": False})
            destination = root / "receipts"
            qualification.collect_receipts(root, destination)
            self.assertEqual(
                {p.name for p in destination.iterdir()},
                {"production-install.log", "qualification-result.json", "receipt-index.json"},
            )
            self.assertEqual((destination / "production-install.log").stat().st_size, 64 * 1024)
            records = json.loads((destination / "receipt-index.json").read_text())["files"]
            log = next(r for r in records if r["name"] == "production-install.log")
            self.assertTrue(log["truncated_to_tail"])
            self.assertEqual(log["original_sha256"], hashlib.sha256(original).hexdigest())


if __name__ == "__main__":
    unittest.main()
