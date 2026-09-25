"""Deterministic resolution checks; no network or large wheels."""

import importlib.util
import json
import pathlib
import re
import tempfile
import time
import unittest
from contextlib import redirect_stderr, redirect_stdout
from io import StringIO
from urllib.request import Request
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
    def test_release_channels_keep_only_canonical_official_cpu_cuda_and_rocm(self):
        links = [
            "cpu/",
            "cu136/",
            "rocm8.0.1/",
            "https://download-r2.pytorch.org/whl/rocm7.14/",
            "cu136/",
            "xpu/",
            "nightly/",
            "cu136-full/",
            "cu117-pypi-cudnn/",
            "cpu-cxx11-abi/",
            "cu137/?unexpected=1",
            "https://download.pytorch.org:invalid/whl/cu139/",
            "https://evil.example/whl/cu138/",
        ]
        self.assertEqual(
            resolver.parse_release_channels(links),
            ["cpu", "cu136", "rocm7.14", "rocm8.0.1"],
        )

    def test_release_options_find_all_exact_wheels_for_each_interpreter(self):
        def links(build):
            return [
                f"https://download.pytorch.org/whl/{build}/torch-2.10.0%2B{build}-cp312-cp312-manylinux_2_28_x86_64.whl#sha256={'a' * 64}",
                f"https://download.pytorch.org/whl/{build}/torch-2.10.0%2B{build}-cp313-cp313-manylinux_2_28_x86_64.whl#sha256={'b' * 64}",
                f"https://download.pytorch.org/whl/{build}/torch-2.10.1%2B{build}-cp312-cp312-manylinux_2_28_x86_64.whl",
                f"https://download.pytorch.org/whl/{build}/torch-2.10.0rc1%2B{build}-cp312-cp312-manylinux_2_28_x86_64.whl",
                f"https://download.pytorch.org/whl/{build}/torch-2.10.0%2B{build}-cp312-cp312-win_amd64.whl",
            ]

        result = resolver.discover_release_options(
            "2.10.0",
            ["3.12", "3.13"],
            root_loader=lambda: ["cpu", "cu136", "rocm8.0.1"],
            index_loader=links,
            tag_loader=lambda python: (
                python,
                {f"cp{python.replace('.', '')}-cp{python.replace('.', '')}-manylinux_2_28_x86_64"},
            ),
        )
        self.assertEqual(result["tag"], "v2.10.0")
        self.assertEqual(result["status"], "matches")
        self.assertTrue(result["completeScan"])
        self.assertEqual(result["checkedChannels"], ["cpu", "cu136", "rocm8.0.1"])
        self.assertEqual(len(result["combinations"]), 6)
        self.assertEqual(
            {item["python"] for item in result["combinations"]}, {"python3.12", "python3.13"}
        )
        self.assertEqual(
            {item["build"] for item in result["combinations"]}, {"cpu", "cu136", "rocm8.0.1"}
        )
        self.assertEqual({item["sha256"] for item in result["combinations"]}, {"a" * 64, "b" * 64})

    def test_release_options_report_partial_matches_and_complete_absence_honestly(self):
        def sometimes_unavailable(build):
            if build == "cu136":
                raise OSError("network unavailable")
            return [
                f"https://download.pytorch.org/whl/{build}/torch-2.10.0%2B{build}-cp312-cp312-manylinux_2_28_x86_64.whl"
            ]

        arguments = {
            "root_loader": lambda: ["cpu", "cu136"],
            "tag_loader": lambda _: ("3.12", {"cp312-cp312-manylinux_2_28_x86_64"}),
        }
        partial = resolver.discover_release_options(
            "2.10.0", ["3.12"], index_loader=sometimes_unavailable, **arguments
        )
        self.assertEqual(partial["status"], "inconclusive")
        self.assertFalse(partial["completeScan"])
        self.assertEqual([item["build"] for item in partial["combinations"]], ["cpu"])
        self.assertEqual(partial["checkedChannels"], ["cpu", "cu136"])
        absent = resolver.discover_release_options(
            "2.10.0", ["3.12"], index_loader=lambda _: [], **arguments
        )
        self.assertEqual(absent["status"], "none")
        self.assertTrue(absent["completeScan"])
        self.assertEqual(absent["combinations"], [])
        root_unavailable = resolver.discover_release_options(
            "2.10.0",
            ["3.12"],
            root_loader=lambda: (_ for _ in ()).throw(OSError("offline")),
            index_loader=lambda _: [],
            tag_loader=arguments["tag_loader"],
        )
        self.assertEqual(root_unavailable["status"], "inconclusive")
        self.assertFalse(root_unavailable["completeScan"])
        self.assertEqual(root_unavailable["checkedChannels"], [])

    def test_release_options_limits_never_claim_complete_scan(self):
        with patch.object(resolver, "MAX_RELEASE_CHANNELS", 2):
            limited = resolver.discover_release_options(
                "2.10.0",
                ["3.12"],
                root_loader=lambda: ["cpu", "cu136", "rocm8.0.1"],
                index_loader=lambda _: [],
                tag_loader=lambda _: ("3.12", {"cp312-cp312-manylinux_2_28_x86_64"}),
            )
        self.assertEqual(limited["status"], "inconclusive")
        self.assertFalse(limited["completeScan"])
        self.assertEqual(limited["checkedChannels"], ["cpu", "cu136"])
        with patch.object(resolver, "MAX_RELEASE_SCAN_SECONDS", 0.001):
            timed_out = resolver.discover_release_options(
                "2.10.0",
                ["3.12"],
                root_loader=lambda: ["cpu"],
                index_loader=lambda _: (time.sleep(0.02), [])[1],
                tag_loader=lambda _: ("3.12", {"cp312-cp312-manylinux_2_28_x86_64"}),
            )
        self.assertEqual(timed_out["status"], "inconclusive")
        self.assertFalse(timed_out["completeScan"])
        self.assertTrue(any("deadline" in issue for issue in timed_out["issues"]))

    def test_release_root_is_bounded_and_untrusted_redirect_is_rejected(self):
        class Response:
            def __enter__(self):
                return self

            def __exit__(self, *_):
                return False

            def geturl(self):
                return "https://download.pytorch.org/whl/"

            def read(self, limit):
                self.limit = limit
                return b"x" * limit

        response = Response()
        request_timeouts = []

        def open_response(_opener, _request, timeout):
            request_timeouts.append(timeout)
            return response

        opener = type("Opener", (), {"open": open_response})()
        with patch.object(resolver, "build_opener", return_value=opener):
            with self.assertRaisesRegex(ValueError, "exceeded"):
                resolver.torch_root_channels()
        self.assertEqual(response.limit, resolver.MAX_RELEASE_ROOT_BYTES + 1)
        self.assertEqual(request_timeouts, [resolver.INDEX_TIMEOUT_SECONDS])
        response.geturl = lambda: "https://download.pytorch.org/whl/cu130/"
        with patch.object(resolver, "build_opener", return_value=opener):
            with self.assertRaisesRegex(ValueError, "untrusted"):
                resolver.torch_root_channels()
        with self.assertRaisesRegex(ValueError, "untrusted"):
            resolver.OfficialDirectoryRedirects().redirect_request(
                Request("https://download.pytorch.org/whl/"),
                None,
                302,
                "Found",
                {},
                "https://evil.example/whl/",
            )
        with self.assertRaisesRegex(ValueError, "untrusted"):
            resolver.OfficialRedirects().redirect_request(
                Request("https://download.pytorch.org/whl/cpu/torch/"),
                None,
                302,
                "Found",
                {},
                "https://download.pytorch.org/whl/cu136/torch/",
            )

    def test_release_options_cli_does_not_require_build_and_preserves_json_shape(self):
        expected = {
            "tag": "v2.10.0",
            "status": "none",
            "completeScan": True,
            "checkedChannels": ["cpu"],
            "combinations": [],
            "issues": [],
        }
        output = StringIO()
        with (
            patch.object(
                resolver.sys,
                "argv",
                [
                    "resolve_runtime.py",
                    "--version",
                    "2.10.0",
                    "--release-options",
                    "--interpreter",
                    "/python312",
                    "--interpreter",
                    "/python313",
                ],
            ),
            patch.object(resolver, "discover_release_options", return_value=expected) as discover,
            redirect_stdout(output),
        ):
            resolver.main()
        discover.assert_called_once_with("2.10.0", ["/python312", "/python313"])
        self.assertEqual(json.loads(output.getvalue()), expected)

    def test_build_vocabulary_matches_rust_manager(self):
        rust_source = (
            ROOT.parent / "rust/crates/pumas-app-manager/src/version_manager/torch_preview.rs"
        ).read_text()
        declaration = re.search(
            r"pub\(super\) const BUILDS: &\[&str\] = &\[(.*?)\];",
            rust_source,
            re.DOTALL,
        )
        self.assertIsNotNone(declaration)
        rust_builds = tuple(re.findall(r'"([^"]+)"', declaration.group(1)))
        self.assertEqual(rust_builds, resolver.BUILDS)

    def test_official_historical_and_current_build_vocabulary(self):
        self.assertEqual(
            resolver.BUILDS,
            (
                "cpu",
                "cu75",
                "cu80",
                "cu90",
                "cu91",
                "cu92",
                "cu100",
                "cu101",
                "cu102",
                "cu110",
                "cu111",
                "cu113",
                "cu115",
                "cu116",
                "cu117",
                "cu118",
                "cu121",
                "cu124",
                "cu126",
                "cu128",
                "cu129",
                "cu130",
                "cu132",
                "cu134",
                "rocm3.7",
                "rocm3.8",
                "rocm3.10",
                "rocm4.0.1",
                "rocm4.1",
                "rocm4.2",
                "rocm4.3.1",
                "rocm4.5.2",
                "rocm5.0",
                "rocm5.1.1",
                "rocm5.2",
                "rocm5.3",
                "rocm5.4.2",
                "rocm5.5",
                "rocm5.6",
                "rocm5.7",
                "rocm6.0",
                "rocm6.1",
                "rocm6.2",
                "rocm6.2.4",
                "rocm6.3",
                "rocm6.4",
                "rocm7.0",
                "rocm7.1",
                "rocm7.2",
                "rocm7.14",
            ),
        )

    def test_rocm_alternatives_follow_version_components(self):
        self.assertEqual(
            resolver.discovery_builds("rocm3.10")[:3],
            ["rocm3.10", "rocm3.8", "rocm4.0.1"],
        )
        self.assertEqual(
            resolver.discovery_builds("rocm8.0.1")[:2],
            ["rocm8.0.1", "rocm7.14"],
        )

    def test_future_canonical_selected_build_is_valid_for_bounded_discovery(self):
        result = resolver.discover_alternatives(
            "v2.10.0",
            "cu136",
            "python3.12",
            ["3.12"],
            index_loader=lambda _: [],
            tag_loader=lambda _: ("3.12", {"cp312-cp312-manylinux_2_28_x86_64"}),
        )
        self.assertEqual(result["checkedBuilds"][:2], ["cu136", "cu134"])
        with self.assertRaisesRegex(ValueError, "supported build"):
            resolver.discover_alternatives(
                "v2.10.0",
                "cu136-full",
                "python3.12",
                ["3.12"],
                index_loader=lambda _: [],
                tag_loader=lambda _: ("3.12", set()),
            )

    def test_historical_build_without_matching_wheel_is_unsupported(self):
        result = resolver.discover_alternatives(
            "v1.12.1",
            "cu102",
            "python3.12",
            ["3.12"],
            index_loader=lambda build: [
                f"https://download.pytorch.org/whl/{build}/"
                f"torch-1.12.1%2B{build}-cp39-cp39-linux_x86_64.whl#sha256={'a' * 64}"
            ],
            tag_loader=lambda _: ("3.12", {"cp312-cp312-manylinux_2_28_x86_64"}),
        )
        self.assertEqual(result["status"], "none")
        self.assertEqual(result["matches"], [])
        self.assertEqual(result["checkedBuilds"][0], "cu102")

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

    def test_resolution_failures_distinguish_torch_dependencies_and_inconclusive_errors(self):
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
            4,
        )
        self.assertEqual(
            resolver.resolution_failure(
                "Could not find a version that satisfies the requirement torch==2.10.0+cpu",
                "2.10.0",
                "cpu",
            )[0],
            4,
        )
        code, message = resolver.resolution_failure(
            "No matching distribution found for torchvision==0.25.0", "2.10.0", "cpu", "flux2"
        )
        self.assertEqual(code, 2)
        self.assertIn("torchvision", message)
        self.assertNotIn("0.25.0", message)
        self.assertEqual(
            resolver.resolution_failure(
                "No matching distribution found for fastapi", "2.10.0", "cpu"
            )[0],
            2,
        )
        code, message = resolver.resolution_failure(
            "No matching distribution found for attacker-controlled-name", "2.10.0", "cpu"
        )
        self.assertEqual(code, 2)
        self.assertNotIn("attacker-controlled-name", message)
        self.assertEqual(
            resolver.resolution_failure(
                "HTTP 503; No matching distribution found for torchvision", "2.10.0", "cpu"
            )[0],
            75,
        )
        self.assertEqual(
            resolver.resolution_failure(
                "Invalid metadata; No matching distribution found for torch==2.10.0+cpu",
                "2.10.0",
                "cpu",
            )[0],
            1,
        )
        self.assertEqual(
            resolver.resolution_failure("metadata-generation-failed", "2.10.0", "cpu")[0],
            1,
        )
        self.assertEqual(
            resolver.resolution_failure(
                "ResolutionImpossible: conflicting dependencies", "2.10.0", "cpu"
            )[0],
            2,
        )
        self.assertEqual(
            resolver.resolution_failure("pip exited unexpectedly", "2.10.0", "cpu")[0],
            1,
        )

    def test_invalid_successful_pip_report_has_distinct_failure_code(self):
        fixtures = {
            "untrusted origin": report(url="https://example.com/torch.whl"),
            "wrong build": report(version="2.10.0+cu128"),
            "missing hash": report(),
            "missing dependency": report(),
        }
        fixtures["missing hash"]["install"][0]["download_info"]["archive_info"] = {}
        fixtures["missing dependency"]["install"].pop()

        def fake_run(command, **_kwargs):
            pathlib.Path(command[command.index("--report") + 1]).write_text(json.dumps(fixture))
            return type("Completed", (), {"returncode": 0, "stdout": "", "stderr": ""})()

        for name, fixture in fixtures.items():
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                with (
                    patch.object(
                        resolver.sys,
                        "argv",
                        [
                            "resolve_runtime.py",
                            "--version",
                            "2.10.0",
                            "--build",
                            "cpu",
                            "--output",
                            directory,
                        ],
                    ),
                    patch.object(resolver.subprocess, "run", side_effect=fake_run),
                    redirect_stderr(StringIO()),
                ):
                    with self.assertRaises(SystemExit) as exit_result:
                        resolver.main()
                self.assertEqual(exit_result.exception.code, 3)
                self.assertFalse((pathlib.Path(directory) / "requirements.txt").exists())
                self.assertFalse((pathlib.Path(directory) / "resolution.json").exists())

    def test_missing_historical_wheel_does_not_publish_install_lock(self):
        failed = type(
            "Completed",
            (),
            {
                "returncode": 1,
                "stdout": "",
                "stderr": "No matching distribution found for torch==1.12.1+cu102",
            },
        )()
        with tempfile.TemporaryDirectory() as directory:
            with (
                patch.object(
                    resolver.sys,
                    "argv",
                    [
                        "resolve_runtime.py",
                        "--version",
                        "1.12.1",
                        "--build",
                        "cu102",
                        "--output",
                        directory,
                    ],
                ),
                patch.object(resolver.subprocess, "run", return_value=failed),
                redirect_stderr(StringIO()),
            ):
                with self.assertRaises(SystemExit) as exit_result:
                    resolver.main()
            self.assertFalse((pathlib.Path(directory) / "requirements.txt").exists())
            self.assertFalse((pathlib.Path(directory) / "resolution.json").exists())
        self.assertEqual(exit_result.exception.code, 4)

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
        self.assertEqual(requested[:2], ["cu130", "cu129"])
        self.assertEqual(result["matches"][0]["build"], "cu129")
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
