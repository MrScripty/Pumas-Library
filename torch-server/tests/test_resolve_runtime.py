"""Deterministic artifact-resolution checks; no network or large wheels."""

import importlib.util
import pathlib
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("resolve_runtime", ROOT / "resolve_runtime.py")
resolver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(resolver)


def report(version="2.10.0+cpu", url="https://download.pytorch.org/whl/cpu/torch/torch.whl"):
    return {
        "install": [
            {
                "metadata": {"name": "torch", "version": version},
                "download_info": {"url": url, "archive_info": {"hashes": {"sha256": "a" * 64}}},
            },
            {
                "metadata": {"name": "fastapi", "version": "1.0"},
                "download_info": {
                    "url": "https://files.pythonhosted.org/packages/fastapi.whl",
                    "archive_info": {"hashes": {"sha256": "b" * 64}},
                },
            },
        ]
    }


class ResolverTests(unittest.TestCase):
    def test_exact_official_build_and_hashes_are_recorded(self):
        requirements, resolution = resolver.requirements_from_report(report(), "2.10.0", "cpu")
        self.assertEqual(resolution["torch"], "2.10.0+cpu")
        self.assertEqual(resolution["artifacts"][0]["sha256"], "a" * 64)
        self.assertIn("--hash=sha256:" + "a" * 64, requirements[0])

    def test_wrong_build_or_unofficial_artifact_is_rejected(self):
        for fixture in [
            report(version="2.10.0+cu128"),
            report(url="https://example.com/torch.whl"),
        ]:
            with self.subTest(fixture=fixture):
                with self.assertRaises(ValueError):
                    resolver.requirements_from_report(fixture, "2.10.0", "cpu")

    def test_unhashed_dependency_is_rejected(self):
        fixture = report()
        fixture["install"][1]["download_info"]["archive_info"] = {}
        with self.assertRaises(ValueError):
            resolver.requirements_from_report(fixture, "2.10.0", "cpu")


if __name__ == "__main__":
    unittest.main()
