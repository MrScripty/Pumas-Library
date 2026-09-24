"""Deterministic resolution checks; no network or large wheels."""

import importlib.util
import pathlib
import unittest
from unittest.mock import patch

ROOT = pathlib.Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("resolve_runtime", ROOT / "resolve_runtime.py")
resolver = importlib.util.module_from_spec(spec)
spec.loader.exec_module(resolver)


def entry(name, version="1.0", url=None, digest="b"):
    if url is None:
        url = f"https://files.pythonhosted.org/packages/{name}-1.0-py3-none-any.whl"
    return {
        "metadata": {"name": name, "version": version},
        "download_info": {"url": url, "archive_info": {"hashes": {"sha256": digest * 64}}},
    }


def report(
    version="2.10.0+cpu",
    url="https://download.pytorch.org/whl/cpu/torch-2.10.0.whl",
    adapter="none",
):
    entries = [entry("torch", version, url, "a"), *(entry(name) for name in resolver.CORE)]
    if adapter != "none":
        build = version.split("+", 1)[1]
        entries.append(
            entry(
                "torchvision",
                f"0.25.0+{build}",
                f"https://download.pytorch.org/whl/{build}/torchvision-0.25.0.whl",
            )
        )
        entries.extend(
            entry(name)
            for name in (
                "diffusers",
                "transformers",
                "accelerate",
                "peft",
                "sentencepiece",
                "protobuf",
            )
        )
    if adapter == "nunchaku":
        entries.append(entry("nunchaku", "1.2.0+torch2.9", resolver.NUNCHAKU_URL, "c"))
        entries[-1]["download_info"]["archive_info"]["hashes"]["sha256"] = resolver.NUNCHAKU_SHA256
    return {"install": entries}


class ResolverTests(unittest.TestCase):
    def test_exact_official_build_and_hashes_are_recorded(self):
        requirements, resolution = resolver.requirements_from_report(report(), "2.10.0", "cpu")
        self.assertEqual(resolution["torch"], "2.10.0+cpu")
        self.assertEqual(resolution["adapter"], "none")
        self.assertEqual(resolution["artifacts"][0]["sha256"], "a" * 64)
        self.assertIn("--hash=sha256:" + "a" * 64, requirements[0])
        self.assertEqual(len(requirements), len(resolution["artifacts"]))

    def test_adapter_artifacts_are_scoped_to_selection(self):
        _, resolution = resolver.requirements_from_report(
            report(adapter="flux2"), "2.10.0", "cpu", "flux2"
        )
        self.assertEqual(resolution["adapter"], "flux2")
        self.assertIn("diffusers", {artifact["name"] for artifact in resolution["artifacts"]})
        with self.assertRaisesRegex(ValueError, "omitted requested packages"):
            resolver.requirements_from_report(report(), "2.10.0", "cpu", "flux2")

    def test_wrong_build_or_untrusted_origin_is_rejected(self):
        for fixture in [
            report(version="2.10.0+cu128"),
            report(url="https://example.com/torch.whl"),
            report(url="https://download.pytorch.org.evil.test/whl/cpu/torch.whl"),
        ]:
            with self.subTest(fixture=fixture):
                with self.assertRaises(ValueError):
                    resolver.requirements_from_report(fixture, "2.10.0", "cpu")

    def test_torchvision_wrong_build_is_rejected(self):
        fixture = report(adapter="flux2")
        fixture["install"][6]["metadata"]["version"] = "0.25.0+cu130"
        with self.assertRaisesRegex(ValueError, "Torchvision build"):
            resolver.requirements_from_report(fixture, "2.10.0", "cpu", "flux2")
        fixture = report(adapter="flux2")
        fixture["install"][6]["download_info"]["url"] = (
            "https://download.pytorch.org/whl/cu130/torchvision-0.25.0.whl"
        )
        with self.assertRaisesRegex(ValueError, "Untrusted"):
            resolver.requirements_from_report(fixture, "2.10.0", "cpu", "flux2")

    def test_untrusted_dependency_and_missing_hash_are_rejected(self):
        fixture = report()
        fixture["install"][1]["download_info"]["url"] = "https://example.com/fastapi.whl"
        with self.assertRaisesRegex(ValueError, "Untrusted"):
            resolver.requirements_from_report(fixture, "2.10.0", "cpu")
        fixture = report()
        fixture["install"][1]["download_info"]["archive_info"] = {}
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            resolver.requirements_from_report(fixture, "2.10.0", "cpu")

    def test_nunchaku_is_limited_to_qualified_abi(self):
        with self.assertRaisesRegex(ValueError, "qualified Nunchaku"):
            resolver.adapter_requirements("nunchaku", "2.10.0", "cu130")
        with (
            patch.object(resolver.sys, "platform", "linux"),
            patch.object(resolver.platform, "machine", return_value="x86_64"),
        ):
            with patch.object(resolver.sys, "version_info", (3, 12, 0)):
                requirements = resolver.adapter_requirements("nunchaku", "2.9.1", "cu130")
        self.assertIn(resolver.NUNCHAKU_URL, requirements[-1])
        fixture = report(
            version="2.9.1+cu130",
            url="https://download-r2.pytorch.org/whl/cu130/torch.whl",
            adapter="nunchaku",
        )
        _, resolution = resolver.requirements_from_report(fixture, "2.9.1", "cu130", "nunchaku")
        self.assertEqual(resolution["adapter"], "nunchaku")

    def test_network_uncertainty_and_missing_torch_are_distinct(self):
        for message in (
            "Connection timed out",
            "certificate verify failed",
            "HTTP 503",
            "Max retries exceeded",
        ):
            with self.subTest(message=message):
                self.assertEqual(resolver.resolution_failure(message, "2.10.0", "cpu")[0], 75)
        self.assertEqual(
            resolver.resolution_failure(
                "No matching distribution found for torch==2.10.0+cpu", "2.10.0", "cpu"
            )[0],
            2,
        )
        self.assertEqual(
            resolver.resolution_failure(
                "No matching distribution found for torchvision", "2.10.0", "cpu"
            )[0],
            2,
        )
        self.assertEqual(
            resolver.resolution_failure(
                "HTTP 503; No matching distribution found for torchvision", "2.10.0", "cpu"
            )[0],
            75,
        )

    def test_discovery_matches_only_exact_binary_wheel_tags_and_caps_results(self):
        requested = []

        def links(build):
            requested.append(build)
            return [
                f"https://download-r2.pytorch.org/whl/{build}/torch-2.10.0%2B{build}-cp312-cp312-manylinux_2_28_x86_64.whl#sha256={'a' * 64}",
                f"https://download-r2.pytorch.org/whl/{build}/torch-2.10.0%2B{build}-cp312-cp312-manylinux_2_28_x86_64.tar.gz",
                "https://evil.test/torch-2.10.0+cpu-cp312-cp312-manylinux_2_28_x86_64.whl",
            ]

        def tags(interpreter):
            return interpreter, {"cp312-cp312-manylinux_2_28_x86_64"}

        result = resolver.discover_alternatives(
            "v2.10.0",
            "cu130",
            "python3.13",
            ["3.12", "3.11", "3.10"],
            index_loader=links,
            tag_loader=tags,
        )
        self.assertEqual(result["status"], "matches")
        self.assertEqual(len(result["matches"]), 3)
        self.assertEqual(len(requested), 1)
        self.assertTrue(result["dependenciesNotChecked"])
        self.assertTrue(result["incomplete"])
        self.assertEqual(result["matches"][0]["build"], "cu130")
        self.assertEqual(result["matches"][0]["sha256"], "a" * 64)

    def test_discovery_network_failure_is_inconclusive_and_never_full_resolution(self):
        def unavailable(_build):
            raise OSError("timeout")

        result = resolver.discover_alternatives(
            "v2.10.0",
            "cu130",
            "python3.12",
            ["3.12"],
            index_loader=unavailable,
            tag_loader=lambda _: ("3.12", {"cp312-cp312-manylinux_2_28_x86_64"}),
        )
        self.assertEqual(result["status"], "inconclusive")
        self.assertEqual(result["matches"], [])
        self.assertEqual(len(result["checkedBuilds"]), resolver.MAX_INDEX_REQUESTS)

    def test_discovery_prefers_selected_build_then_other_build(self):
        requested = []

        def links(build):
            requested.append(build)
            if build == "cu130":
                return []
            return [
                f"https://download.pytorch.org/whl/{build}/torch-2.10.0%2B{build}-cp312-cp312-manylinux_2_28_x86_64.whl"
            ]

        result = resolver.discover_alternatives(
            "v2.10.0",
            "cu130",
            "python3.13",
            ["3.12"],
            index_loader=links,
            tag_loader=lambda _: ("3.12", {"cp312-cp312-manylinux_2_28_x86_64"}),
        )
        self.assertEqual(requested[:2], ["cu130", "cu128"])
        self.assertEqual(result["matches"][0]["build"], "cu128")
        self.assertIsNone(result["matches"][0]["sha256"])

    def test_discovery_rejects_untrusted_and_wrong_build_wheels(self):
        tags = {"cp312-cp312-manylinux_2_28_x86_64"}
        for href in (
            "https://evil.test/whl/cpu/torch-2.10.0%2Bcpu-cp312-cp312-manylinux_2_28_x86_64.whl",
            "https://download.pytorch.org/whl/cu128/torch-2.10.0%2Bcu128-cp312-cp312-manylinux_2_28_x86_64.whl",
            "https://download.pytorch.org/whl/cpu/torch-2.10.0%2Bcpu-cp312-cp312-manylinux_2_28_x86_64.tar.gz",
        ):
            with self.subTest(href=href):
                self.assertIsNone(resolver.wheel_match(href, "2.10.0", "cpu", tags))

    def test_index_read_has_byte_and_time_limits(self):
        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def geturl(self):
                return "https://download.pytorch.org/whl/cpu/torch/"

            def read(self, limit):
                self.limit = limit
                return b"x" * limit

        response = Response()
        opener = type("Opener", (), {"open": lambda self, request, timeout: response})()
        with patch.object(resolver, "build_opener", return_value=opener):
            with self.assertRaisesRegex(ValueError, "exceeded"):
                resolver.torch_index_links("cpu")
        self.assertEqual(response.limit, resolver.MAX_INDEX_BYTES + 1)


if __name__ == "__main__":
    unittest.main()
