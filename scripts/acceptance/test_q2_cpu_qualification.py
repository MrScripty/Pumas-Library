"""Offline failure oracles for the bounded hosted qualification job."""

import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
from urllib.error import HTTPError

import q2_cpu_qualification as qualification


class QualificationTests(unittest.TestCase):
    def test_missing_network_namespace_stops_before_source_requests(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with (
                patch.object(
                    qualification, "sandbox_preflight", return_value={"status": "blocked"}
                ),
                patch.object(qualification.urllib.request, "urlopen") as request,
            ):
                with self.assertRaisesRegex(AssertionError, "Network isolation unavailable"):
                    qualification.preflight(root, root, {"artifacts": []})
                request.assert_not_called()
            record = json.loads((root / "ci-preflight.json").read_text())
            self.assertFalse(record["allowed"])
            self.assertFalse(record["payload_downloaded"])

    def test_denied_source_is_not_retried(self):
        artifact = {"name": "torch", "url": "https://download.pytorch.org/pinned.whl"}
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with (
                patch.object(qualification, "sandbox_preflight", return_value={"status": "passed"}),
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

    def test_blocked_offline_install_cannot_pass(self):
        result = {"success": False}
        with self.assertRaisesRegex(AssertionError, "cannot pass"):
            qualification.complete_result(result, qualification.CPU_RESULT, {"status": "blocked"})
        self.assertFalse(result["success"])
        self.assertEqual(result["no_network"]["status"], "blocked")

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
