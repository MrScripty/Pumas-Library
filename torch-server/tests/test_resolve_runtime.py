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
from types import SimpleNamespace
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
    def test_bootstrap_platform_tags_keep_only_the_native_host(self):
        cases = (
            ("win32", "AMD64", ["win_amd64", "win32"], ["win_amd64"], "windows"),
            (
                "darwin",
                "arm64",
                ["macosx_15_0_arm64", "macosx_15_0_universal2", "macosx_15_0_x86_64"],
                ["macosx_15_0_arm64", "macosx_15_0_universal2"],
                "macos",
            ),
            (
                "linux",
                "x86_64",
                ["manylinux_2_28_x86_64", "linux_aarch64"],
                ["manylinux_2_28_x86_64"],
                "linux",
            ),
        )
        for system, machine, platforms, expected, target in cases:
            with self.subTest(system=system):
                completed = type(
                    "Completed",
                    (),
                    {
                        "returncode": 0,
                        "stdout": json.dumps(
                            {
                                "python": "3.12",
                                "platform": system,
                                "machine": machine,
                                "implementation": "cpython",
                                "platforms": platforms,
                            }
                        ),
                    },
                )()
                with patch.object(resolver.subprocess, "run", return_value=completed) as run:
                    self.assertEqual(
                        resolver.bootstrap_platform_tags("/bootstrap"), (target, expected)
                    )
                self.assertEqual(run.call_args.args[0][0], "/bootstrap")

    def test_candidate_tags_match_cpython_314_on_exact_native_platform(self):
        for target, platform_tag in (
            ("windows", "win_amd64"),
            ("macos", "macosx_15_0_arm64"),
            ("linux", "manylinux_2_28_x86_64"),
        ):
            with self.subTest(target=target):
                tags = resolver.candidate_cpython_tags("3.14", target, [platform_tag])
                self.assertIn(f"cp314-cp314-{platform_tag}", tags)
                self.assertTrue(all(tag.endswith(f"-{platform_tag}") for tag in tags))
        with self.assertRaises(ValueError):
            resolver.candidate_cpython_tags("3.14", "macos", ["macosx_15_0_x86_64"])
        with self.assertRaises(ValueError):
            resolver.candidate_cpython_tags("3.14", "windows", ["manylinux_2_28_x86_64"])

    def test_release_options_candidate_mode_scans_without_candidate_interpreters(self):
        for target, platform_tag, build, version_suffix in (
            ("windows", "win_amd64", "cu130", "%2Bcu130"),
            ("macos", "macosx_15_0_arm64", "cpu", ""),
            ("linux", "manylinux_2_28_x86_64", "cpu", "%2Bcpu"),
        ):
            with self.subTest(target=target):

                def wheel(minor):
                    return f"torch-2.14.0{version_suffix}-cp3{minor}-cp3{minor}-{platform_tag}.whl"

                links = [wheel(13), wheel(14), "torch-2.14.0%2Bcpu-cp314-cp314-win32.whl"]
                with patch.object(
                    resolver, "interpreter_tags", side_effect=AssertionError("candidate probed")
                ):
                    result = resolver.discover_release_options(
                        "2.14.0",
                        ["/bootstrap"],
                        root_loader=lambda: [build],
                        index_loader=lambda _: links,
                        tag_loader=lambda _: (_ for _ in ()).throw(
                            AssertionError("candidate probed")
                        ),
                        python_candidates=["3.14", "3.13"],
                        platform_loader=lambda _: (target, [platform_tag]),
                    )
                self.assertEqual(result["status"], "matches")
                self.assertTrue(result["completeScan"])
                self.assertEqual(
                    [combination["python"] for combination in result["combinations"]],
                    ["python3.14", "python3.13"],
                )
                self.assertEqual(len(result["combinations"]), 2)

    def test_release_options_rejects_malformed_or_prerelease_candidates(self):
        for candidate in (
            "3.14rc1",
            "3.14.0",
            "3.014",
            "python3.14",
            "2.7",
            "3.-1",
            "",
            "3.9",
            "3.0",
            "3.09",
        ):
            with self.subTest(candidate=candidate):
                with self.assertRaises(ValueError):
                    resolver.discover_release_options(
                        "2.14.0", ["/bootstrap"], python_candidates=[candidate]
                    )
        with self.assertRaises(ValueError):
            resolver.discover_release_options(
                "2.14.0", ["/bootstrap", "/other"], python_candidates=["3.14"]
            )
        self.assertEqual(resolver.candidate_python_version("3.10"), (3, 10))
        self.assertEqual(resolver.candidate_python_version("3.99"), (3, 99))

    def test_candidate_catalog_truncation_and_foreign_platform_are_inconclusive(self):
        common = {
            "root_loader": lambda: ["cpu"],
            "index_loader": lambda _: [],
            "platform_loader": lambda _: ("windows", ["win_amd64"]),
        }
        with patch.object(resolver, "MAX_RELEASE_CANDIDATES", 2):
            truncated = resolver.discover_release_options(
                "2.14.0", ["/bootstrap"], python_candidates=["3.12", "3.13", "3.14"], **common
            )
        self.assertEqual(truncated["status"], "inconclusive")
        self.assertFalse(truncated["completeScan"])
        self.assertTrue(any("first 2" in issue for issue in truncated["issues"]))
        foreign = resolver.discover_release_options(
            "2.14.0",
            ["/bootstrap"],
            python_candidates=["3.14"],
            root_loader=lambda: self.fail("foreign platform must stop before index scan"),
            platform_loader=lambda _: ("macos", ["macosx_15_0_x86_64"]),
        )
        self.assertEqual(foreign["status"], "inconclusive")
        self.assertFalse(foreign["completeScan"])
        self.assertEqual(foreign["combinations"], [])

    def test_release_options_cli_passes_repeated_candidates_and_one_bootstrap(self):
        expected = {
            "tag": "v2.14.0",
            "status": "none",
            "completeScan": True,
            "checkedChannels": ["cpu"],
            "combinations": [],
            "issues": [],
        }
        with (
            patch.object(
                resolver.sys,
                "argv",
                [
                    "resolve_runtime.py",
                    "--version",
                    "2.14.0",
                    "--release-options",
                    "--interpreter",
                    "/bootstrap",
                    "--python-candidate",
                    "3.13",
                    "--python-candidate",
                    "3.14",
                ],
            ),
            patch.object(resolver, "discover_release_options", return_value=expected) as discover,
            redirect_stdout(StringIO()) as output,
        ):
            resolver.main()
        discover.assert_called_once_with(
            "2.14.0", ["/bootstrap"], python_candidates=["3.13", "3.14"]
        )
        self.assertEqual(json.loads(output.getvalue()), expected)

    def test_native_target_accepts_cpython_314_without_a_minor_allowlist(self):
        for system, machine, expected in (
            ("win32", "AMD64", "windows"),
            ("darwin", "arm64", "macos"),
            ("linux", "x86_64", "linux"),
        ):
            with self.subTest(system=system):
                self.assertEqual(
                    resolver.native_target(system, machine, "3.14", "cpython"), expected
                )
        for system, machine, python, implementation in (
            ("win32", "x86", "3.14", "cpython"),
            ("darwin", "x86_64", "3.14", "cpython"),
            ("linux", "aarch64", "3.14", "cpython"),
            ("win32", "AMD64", "3.14", "pypy"),
            ("darwin", "arm64", "3.14rc1", "cpython"),
            ("darwin", "arm64", "2.7", "cpython"),
        ):
            with self.subTest(
                system=system, machine=machine, python=python, implementation=implementation
            ):
                with self.assertRaises(ValueError):
                    resolver.native_target(system, machine, python, implementation)

    def test_cpython_314_candidate_tags_remain_native(self):
        for system, machine, wheel_tag in (
            ("win32", "AMD64", "cp314-cp314-win_amd64"),
            ("darwin", "arm64", "cp314-cp314-macosx_15_0_arm64"),
        ):
            with self.subTest(system=system):
                completed = type(
                    "Completed",
                    (),
                    {
                        "returncode": 0,
                        "stdout": json.dumps(
                            {
                                "python": "3.14",
                                "platform": system,
                                "machine": machine,
                                "implementation": "cpython",
                                "tags": [wheel_tag, "py3-none-any"],
                            }
                        ),
                    },
                )()
                with patch.object(resolver.subprocess, "run", return_value=completed):
                    self.assertEqual(resolver.interpreter_tags("python"), ("3.14", {wheel_tag}))

    def test_native_interpreter_tags_include_windows_and_macos_but_reject_rosetta(self):
        cases = (
            ("win32", "AMD64", "cp312-cp312-win_amd64", True),
            ("darwin", "arm64", "cp312-cp312-macosx_14_0_arm64", True),
            ("darwin", "x86_64", "cp312-cp312-macosx_14_0_x86_64", False),
            ("win32", "x86", "cp312-cp312-win32", False),
            ("linux", "x86_64", "cp312-cp312-manylinux_2_28_x86_64", True),
        )
        for system, machine, wheel_tag, accepted in cases:
            with self.subTest(system=system, machine=machine):
                completed = type(
                    "Completed",
                    (),
                    {
                        "returncode": 0,
                        "stdout": json.dumps(
                            {
                                "python": "3.12",
                                "platform": system,
                                "machine": machine,
                                "implementation": "cpython",
                                "tags": [wheel_tag, "py3-none-any"],
                            }
                        ),
                    },
                )()
                with patch.object(resolver.subprocess, "run", return_value=completed):
                    if accepted:
                        self.assertEqual(resolver.interpreter_tags("python"), ("3.12", {wheel_tag}))
                    else:
                        with self.assertRaises(ValueError):
                            resolver.interpreter_tags("python")

    def test_native_wheels_match_exact_release_and_architecture(self):
        windows = {"cp312-cp312-win_amd64"}
        mac = {"cp312-cp312-macosx_14_0_arm64"}
        self.assertIsNotNone(
            resolver.wheel_match(
                "torch-2.14.0%2Bcpu-cp312-cp312-win_amd64.whl", "2.14.0", "cpu", windows
            )
        )
        self.assertIsNotNone(
            resolver.wheel_match(
                "torch-2.14.0%2Bcu130-cp312-cp312-win_amd64.whl", "2.14.0", "cu130", windows
            )
        )
        plain = resolver.wheel_match(
            "torch-2.14.0-cp312-cp312-macosx_14_0_arm64.whl", "2.14.0", "cpu", mac
        )
        self.assertEqual(plain["torch"], "2.14.0")
        for href, build, tags in (
            ("torch-2.14.0-cp312-cp312-win_amd64.whl", "cpu", windows),
            ("torch-2.14.0-cp312-cp312-macosx_14_0_x86_64.whl", "cpu", mac),
            ("torch-2.14.1-cp312-cp312-macosx_14_0_arm64.whl", "cpu", mac),
            ("https://evil.test/torch-2.14.0-cp312-cp312-macosx_14_0_arm64.whl", "cpu", mac),
        ):
            with self.subTest(href=href):
                self.assertIsNone(resolver.wheel_match(href, "2.14.0", build, tags))

    def test_legacy_discovery_accepts_cpython_314_without_claiming_dependencies(self):
        result = resolver.discover_alternatives(
            "v2.14.0",
            "cu130",
            "python3.14",
            ["python3.14"],
            index_loader=lambda build: (
                ["torch-2.14.0-cp314-cp314-macosx_15_0_arm64.whl"] if build == "cpu" else []
            ),
            tag_loader=lambda _: ("3.14", {"cp314-cp314-macosx_15_0_arm64"}),
        )
        self.assertEqual(result["status"], "matches")
        self.assertEqual(result["matches"][0]["build"], "cpu")
        self.assertTrue(result["dependenciesNotChecked"])

    def test_macos_cpu_report_records_plain_torch_distribution_version(self):
        fixture = report(
            version="2.14.0",
            url="https://download.pytorch.org/whl/cpu/torch-2.14.0-cp312-cp312-macosx_14_0_arm64.whl",
        )
        with (
            patch.object(resolver.sys, "platform", "darwin"),
            patch.object(resolver.platform, "machine", return_value="arm64"),
            patch.object(resolver.sys, "version_info", SimpleNamespace(major=3, minor=12)),
        ):
            _, resolution = resolver.requirements_from_report(fixture, "2.14.0", "cpu")
        self.assertEqual(resolution["release"], "2.14.0")
        self.assertEqual(resolution["torch"], "2.14.0")
        self.assertEqual(resolution["build"], "cpu")
        with (
            patch.object(resolver.sys, "platform", "linux"),
            patch.object(resolver.platform, "machine", return_value="x86_64"),
        ):
            with self.assertRaises(ValueError):
                resolver.requirements_from_report(fixture, "2.14.0", "cpu")

    def test_macos_cpu_cli_requests_plain_version_and_classifies_missing_wheel(self):
        failed = type(
            "Completed",
            (),
            {
                "returncode": 1,
                "stdout": "",
                "stderr": "No matching distribution found for torch==2.14.0",
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
                        "2.14.0",
                        "--build",
                        "cpu",
                        "--output",
                        directory,
                    ],
                ),
                patch.object(resolver.sys, "platform", "darwin"),
                patch.object(resolver.platform, "machine", return_value="arm64"),
                patch.object(resolver.sys, "version_info", SimpleNamespace(major=3, minor=12)),
                patch.object(resolver.subprocess, "run", return_value=failed) as run,
                redirect_stderr(StringIO()),
            ):
                with self.assertRaises(SystemExit) as exit_result:
                    resolver.main()
        self.assertEqual(exit_result.exception.code, 4)
        self.assertIn("torch==2.14.0", run.call_args.args[0])
        self.assertNotIn("torch==2.14.0+cpu", run.call_args.args[0])

    def test_release_options_match_native_windows_and_macos_wheels(self):
        fixtures = (
            ("cp312-cp312-win_amd64", "torch-2.14.0%2Bcu130-cp312-cp312-win_amd64.whl", "cu130"),
            (
                "cp312-cp312-macosx_14_0_arm64",
                "torch-2.14.0-cp312-cp312-macosx_14_0_arm64.whl",
                "cpu",
            ),
        )
        for tag, wheel, build in fixtures:
            with self.subTest(tag=tag):
                result = resolver.discover_release_options(
                    "2.14.0",
                    ["python"],
                    root_loader=lambda: [build],
                    index_loader=lambda _: [f"https://download.pytorch.org/whl/{build}/{wheel}"],
                    tag_loader=lambda _: ("3.12", {tag}),
                )
                self.assertEqual(result["status"], "matches")
                self.assertEqual(len(result["combinations"]), 1)
                self.assertEqual(result["combinations"][0]["build"], build)

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

    def test_cli_reads_utf8_pip_report_and_publishes_exact_artifacts(self):
        fixture = report()
        fixture["install"][1]["metadata"]["summary"] = "Ready” 🚀"
        expected_requirements, expected_resolution = resolver.requirements_from_report(
            fixture, "2.10.0", "cpu"
        )
        original_read_text = pathlib.Path.read_text
        original_write_text = pathlib.Path.write_text
        written_text_options = {}

        def read_with_windows_default(path, *args, **kwargs):
            if path.name == "pip-resolution.json" and kwargs.get("encoding") is None:
                return path.read_bytes().decode("cp1252")
            return original_read_text(path, *args, **kwargs)

        def record_write_encoding(path, data, *args, **kwargs):
            written_text_options[path.name] = (kwargs.get("encoding"), kwargs.get("newline"))
            return original_write_text(path, data, *args, **kwargs)

        def fake_run(command, **_kwargs):
            report_path = pathlib.Path(command[command.index("--report") + 1])
            report_path.write_bytes(json.dumps(fixture, ensure_ascii=False).encode("utf-8"))
            return SimpleNamespace(returncode=0, stdout="", stderr="")

        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory)
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
                patch.object(pathlib.Path, "read_text", read_with_windows_default),
                patch.object(pathlib.Path, "write_text", record_write_encoding),
            ):
                resolver.main()
            self.assertEqual(
                (output / "requirements.txt").read_bytes(),
                ("\n".join(expected_requirements) + "\n").encode("utf-8"),
            )
            self.assertEqual(
                (output / "resolution.json").read_bytes(),
                (json.dumps(expected_resolution, indent=2) + "\n").encode("utf-8"),
            )
        self.assertEqual(
            written_text_options,
            {"requirements.txt": ("utf-8", "\n"), "resolution.json": ("utf-8", "\n")},
        )

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
