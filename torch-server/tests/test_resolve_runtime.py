"""Deterministic resolution checks; no network or large wheels."""

import importlib.util
import base64
import hashlib
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


def native_wheel(version="2.10.0", build="cpu"):
    tag = next(resolver.packaging_tags.sys_tags())
    return f"https://download.pytorch.org/whl/{build}/torch/torch-{version}%2B{build}-{tag}.whl"


def report(
    version="2.10.0+cpu",
    url=None,
    adapter="none",
):
    if url is None:
        url = native_wheel()
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
    def test_copyable_download_sources_accept_any_safe_https_host_and_path(self):
        safe = (
            "https://files.pythonhosted.org/packages/ab/pkg-1.0-py3-none-any.whl",
            "https://download.pytorch.org/whl/cu134/torch-2.14.0.whl",
            "https://download-r2.pytorch.org/whl/cpu/torch-2.14.0.whl",
            "https://github.com/nunchux-ai/nunchaku/releases/download/v1.2.0/nunchaku-1.2.0.whl",
            "https://mirror.example.net:8443/releases/model.safetensors",
        )
        rejected = (
            "http://files.pythonhosted.org/packages/pkg-1.0.whl",
            "https://user:secret@files.pythonhosted.org/packages/pkg-1.0.whl",
            "https://files.pythonhosted.org/packages/pkg-1.0.whl?token=secret",
            "https://files.pythonhosted.org/packages/pkg-1.0.whl?",
            "https://files.pythonhosted.org/packages/pkg-1.0.whl#download",
            "https://files.pythonhosted.org/packages/pkg-1.0.whl#",
            "https://mirror.example.net/releases/model file.bin",
            f"https://mirror.example.net/{'a' * 2048}",
            "https://mirror.example.net:99999/releases/model.safetensors",
            "https:///packages/pkg-1.0.whl",
        )
        for source in safe:
            with self.subTest(source=source):
                self.assertEqual(resolver.copyable_download_source(source), source)
        for source in rejected:
            with self.subTest(source=source):
                self.assertIsNone(resolver.copyable_download_source(source))

    def test_progress_worker_tracks_any_safe_https_download_source(self):
        pip_download = importlib.import_module("pip._internal.network.download")
        pip_cli = importlib.import_module("pip._internal.cli.main")
        response = object()
        source = "https://cdn.example.net/releases/package.whl"
        link = SimpleNamespace(url_without_fragment=source)

        def prepare_download(response, link, progress_bar):
            del response, link, progress_bar
            return iter((b"ab", b"cd"))

        def fake_pip_main(_arguments):
            return list(pip_download._prepare_download(response, link, None)) and 0

        with tempfile.TemporaryDirectory() as directory:
            progress_path = pathlib.Path(directory) / "download-progress.json"
            with (
                patch.object(pip_cli, "main", side_effect=fake_pip_main),
                patch.object(pip_download, "_prepare_download", prepare_download),
                patch.object(pip_download, "is_from_cache", return_value=False),
                patch.object(pip_download, "_get_http_response_size", return_value=4),
            ):
                self.assertEqual(
                    resolver.run_pip_progress_worker(progress_path, ["install", "pkg"]), 0
                )

            progress = json.loads(progress_path.read_text(encoding="utf-8"))
            self.assertEqual(progress["source_url"], source)
            self.assertFalse(progress["active"])
            self.assertEqual(progress["downloaded_bytes"], 4)
            self.assertEqual(progress["total_bytes"], 4)

    def test_progress_worker_reports_speed_during_a_real_chunk_interval(self):
        pip_download = importlib.import_module("pip._internal.network.download")
        pip_cli = importlib.import_module("pip._internal.cli.main")
        response = object()
        source = "https://cdn.example.net/releases/package.whl"
        link = SimpleNamespace(url_without_fragment=source)
        progress_writes = []
        write_progress = resolver._write_download_progress

        def prepare_download(response, link, progress_bar):
            del response, link, progress_bar
            yield b"ab"
            time.sleep(resolver.DOWNLOAD_PROGRESS_INTERVAL_SECONDS + 0.05)
            yield b"cd"

        def fake_pip_main(_arguments):
            return list(pip_download._prepare_download(response, link, None)) and 0

        def capture_progress(*values):
            progress_writes.append(values)
            write_progress(*values)

        with tempfile.TemporaryDirectory() as directory:
            progress_path = pathlib.Path(directory) / "download-progress.json"
            with (
                patch.object(pip_cli, "main", side_effect=fake_pip_main),
                patch.object(pip_download, "_prepare_download", prepare_download),
                patch.object(pip_download, "is_from_cache", return_value=False),
                patch.object(pip_download, "_get_http_response_size", return_value=4),
                patch.object(resolver, "_write_download_progress", side_effect=capture_progress),
            ):
                self.assertEqual(
                    resolver.run_pip_progress_worker(progress_path, ["install", "pkg"]), 0
                )

        measured = [
            sample
            for sample in progress_writes
            if sample[2] and sample[5] is not None
        ]
        self.assertTrue(measured)
        self.assertEqual(measured[-1][1], source)
        self.assertGreater(measured[-1][5], 0)

    def test_progress_worker_does_not_expose_secret_bearing_download_urls(self):
        pip_download = importlib.import_module("pip._internal.network.download")
        pip_cli = importlib.import_module("pip._internal.cli.main")
        response = object()
        link = SimpleNamespace(
            url_without_fragment="https://cdn.example.net/private.whl?token=secret"
        )

        def prepare_download(response, link, progress_bar):
            del response, link, progress_bar
            return iter((b"ab", b"cd"))

        def fake_pip_main(_arguments):
            return list(pip_download._prepare_download(response, link, None)) and 0

        with tempfile.TemporaryDirectory() as directory:
            progress_path = pathlib.Path(directory) / "download-progress.json"
            with (
                patch.object(pip_cli, "main", side_effect=fake_pip_main),
                patch.object(pip_download, "_prepare_download", prepare_download),
                patch.object(pip_download, "is_from_cache", return_value=False),
                patch.object(pip_download, "_get_http_response_size", return_value=4),
            ):
                self.assertEqual(
                    resolver.run_pip_progress_worker(progress_path, ["install", "pkg"]), 0
                )

            progress = json.loads(progress_path.read_text(encoding="utf-8"))
            self.assertIsNone(progress["source_url"])
            self.assertFalse(progress["active"])
            self.assertEqual(progress["downloaded_bytes"], 4)

    def test_download_progress_reports_measured_rate_and_final_byte_count(self):
        timestamps = iter((0.0, 0.5, 1.0))
        writes = []
        with (
            tempfile.TemporaryDirectory() as directory,
            patch.object(
                resolver,
                "_write_download_progress",
                side_effect=lambda *values: writes.append(values),
            ),
        ):
            chunks = resolver._track_download_chunks(
                iter((b"ab", b"cd")),
                "https://files.pythonhosted.org/packages/pkg-1.0.whl",
                4,
                pathlib.Path(directory) / "progress.json",
                clock=lambda: next(timestamps),
            )
            self.assertEqual(list(chunks), [b"ab", b"cd"])

        self.assertEqual(len(writes), 4)
        self.assertEqual(writes[1][2:], (True, 2, 4, 4.0))
        self.assertEqual(writes[-1][2:], (False, 4, 4, None))

    def test_staged_manifest_requires_exact_recorded_file_set_and_hashes(self):
        with tempfile.TemporaryDirectory() as directory:
            target = pathlib.Path(directory)
            metadata = target / "torch-2.10.0.dist-info"
            metadata.mkdir()
            wheel = target / "torch.pth"
            wheel.write_bytes(b"safe staged data")
            digest = (
                base64.urlsafe_b64encode(hashlib.sha256(wheel.read_bytes()).digest())
                .rstrip(b"=")
                .decode()
            )
            (target / "bin").mkdir()
            script = target / "bin" / "torchcmd"
            script.write_bytes(b"#!/bin/sh\n")
            script_digest = (
                base64.urlsafe_b64encode(hashlib.sha256(script.read_bytes()).digest())
                .rstrip(b"=")
                .decode()
            )
            (target / "__pycache__").mkdir()
            bytecode = target / "__pycache__" / "torch.cpython-312.pyc"
            bytecode.write_bytes(b"pip generated")
            record = metadata / "RECORD"
            record.write_text(
                f"torch.pth,sha256={digest},{wheel.stat().st_size}\n"
                f"../../bin/torchcmd,sha256={script_digest},{script.stat().st_size}\n"
                "__pycache__/torch.cpython-312.pyc,,\n"
                "torch-2.10.0.dist-info/RECORD,,\n",
                encoding="utf-8",
            )
            manifest = resolver.installed_file_manifest(target)
            self.assertEqual(
                [item["path"] for item in manifest["files"]],
                ["bin/torchcmd", "torch-2.10.0.dist-info/RECORD", "torch.pth"],
            )
            self.assertFalse(bytecode.exists())
            (target / "unreported.pth").write_text("exec('bad')")
            with self.assertRaisesRegex(ValueError, "unreported"):
                resolver.installed_file_manifest(target)
            (target / "unreported.pth").unlink()
            wheel.write_bytes(b"changed")
            with self.assertRaisesRegex(ValueError, "hash or size"):
                resolver.installed_file_manifest(target)
            wheel.write_bytes(b"safe staged data")
            (target / "linked.pth").symlink_to(wheel)
            with self.assertRaisesRegex(ValueError, "symlink"):
                resolver.installed_file_manifest(target)

    def test_staged_distribution_identity_must_match_pip_report(self):
        with tempfile.TemporaryDirectory() as directory:
            target = pathlib.Path(directory)
            dist = target / "torch-2.10.0+cpu.dist-info"
            dist.mkdir()
            metadata = dist / "METADATA"
            good = "Metadata-Version: 2.1\nName: torch\nVersion: 2.10.0+cpu\n"
            metadata.write_text(good, encoding="utf-8")
            digest = (
                base64.urlsafe_b64encode(hashlib.sha256(metadata.read_bytes()).digest())
                .rstrip(b"=")
                .decode()
            )
            (dist / "RECORD").write_text(
                f"{dist.name}/METADATA,sha256={digest},{metadata.stat().st_size}\n"
                f"{dist.name}/RECORD,,\n",
                encoding="utf-8",
            )
            artifact = [{"name": "torch", "version": "2.10.0+cpu"}]
            self.assertEqual(len(resolver.installed_file_manifest(target, artifact)["files"]), 2)
            metadata.write_text(good.replace("2.10.0+cpu", "1.0"), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "identity differs"):
                resolver.installed_file_manifest(target, artifact)
            metadata.write_text(good, encoding="utf-8")
            wrong_dist = target / "torch-1.0.dist-info"
            dist.rename(wrong_dist)
            with self.assertRaisesRegex(ValueError, "identity differs"):
                resolver.installed_file_manifest(target, artifact)

    def test_staged_manifest_accepts_pip_target_data_files_with_share_paths(self):
        with tempfile.TemporaryDirectory() as directory:
            target = pathlib.Path(directory)
            dist = target / "sympy-1.14.0.dist-info"
            dist.mkdir()
            metadata = dist / "METADATA"
            metadata.write_text(
                "Metadata-Version: 2.1\nName: sympy\nVersion: 1.14.0\n",
                encoding="utf-8",
            )
            executable = target / "bin" / "isympy"
            executable.parent.mkdir()
            executable.write_bytes(b"#!/bin/sh\n")
            man_page = target / "share" / "man" / "man1" / "isympy.1"
            man_page.parent.mkdir(parents=True)
            man_page.write_bytes(b"SymPy interactive shell manual\n")

            def record_row(name, content):
                digest = base64.urlsafe_b64encode(hashlib.sha256(content).digest())
                return f"{name},sha256={digest.rstrip(b'=').decode()},{len(content)}\n"

            record = dist / "RECORD"
            record.write_text(
                record_row("sympy-1.14.0.dist-info/METADATA", metadata.read_bytes())
                + record_row("../../bin/isympy", executable.read_bytes())
                + record_row("../../share/man/man1/isympy.1", man_page.read_bytes())
                + "sympy-1.14.0.dist-info/RECORD,,\n",
                encoding="utf-8",
            )

            manifest = resolver.installed_file_manifest(
                target, [{"name": "sympy", "version": "1.14.0"}]
            )
            self.assertEqual(
                [item["path"] for item in manifest["files"]],
                [
                    "bin/isympy",
                    "share/man/man1/isympy.1",
                    "sympy-1.14.0.dist-info/METADATA",
                    "sympy-1.14.0.dist-info/RECORD",
                ],
            )
            good_record = record.read_text(encoding="utf-8")
            for unsafe_path in ("../../etc/passwd", "../../share/../../outside"):
                with self.subTest(unsafe_path=unsafe_path):
                    record.write_text(
                        good_record.replace(
                            "../../share/man/man1/isympy.1", unsafe_path
                        ),
                        encoding="utf-8",
                    )
                    with self.assertRaisesRegex(ValueError, "escapes its target"):
                        resolver.installed_file_manifest(
                            target, [{"name": "sympy", "version": "1.14.0"}]
                        )
            record.write_text(good_record, encoding="utf-8")

    def test_install_mode_stages_once_and_validates_report_before_writing_lock(self):
        fixture = report()
        commands = []

        def fake_run(command, **_kwargs):
            commands.append(command)
            self.assertIn("--target", command)
            self.assertNotIn("--dry-run", command)
            self.assertIn("--only-binary=:all:", command)
            self.assertIn("torch==2.10.0+cpu", command)
            pathlib.Path(command[command.index("--report") + 1]).write_text(
                json.dumps(fixture), encoding="utf-8"
            )
            target_dir = pathlib.Path(command[command.index("--target") + 1])
            for item in fixture["install"]:
                name = item["metadata"]["name"]
                version = item["metadata"]["version"]
                metadata = target_dir / f"{name}-{version}.dist-info"
                metadata.mkdir()
                metadata_file = metadata / "METADATA"
                metadata_file.write_text(
                    f"Metadata-Version: 2.1\nName: {name}\nVersion: {version}\n",
                    encoding="utf-8",
                )
                digest = (
                    base64.urlsafe_b64encode(hashlib.sha256(metadata_file.read_bytes()).digest())
                    .rstrip(b"=")
                    .decode()
                )
                (metadata / "RECORD").write_text(
                    f"{metadata.name}/METADATA,sha256={digest},{metadata_file.stat().st_size}\n"
                    f"{metadata.name}/RECORD,,\n",
                    encoding="utf-8",
                )
            return SimpleNamespace(returncode=0, stdout="", stderr="")

        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "manifest"
            target = pathlib.Path(directory) / "staged packages"
            progress_file = output / "download-progress.json"
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
                        "--install",
                        "--target",
                        str(target),
                        "--progress-file",
                        str(progress_file),
                        "--output",
                        str(output),
                    ],
                ),
                patch.object(resolver.subprocess, "run", side_effect=fake_run),
            ):
                resolver.main()
            self.assertEqual(len(commands), 1)
            self.assertEqual(commands[0][3], "--_pumas-pip-progress-worker")
            self.assertEqual(commands[0][4], str(progress_file))
            self.assertEqual(commands[0][commands[0].index("--target") + 1], str(target))
            self.assertTrue(target.is_dir())
            self.assertEqual(
                json.loads((output / "resolution.json").read_text())["artifacts"][0]["sha256"],
                "a" * 64,
            )
            self.assertIn("--hash=sha256:" + "a" * 64, (output / "requirements.txt").read_text())
            self.assertEqual(
                len(json.loads((output / "installed-files.json").read_text())["files"]),
                2 * len(fixture["install"]),
            )

    def test_install_mode_rejects_untrusted_or_hashless_report_before_lock_publication(self):
        fixtures = {
            "untrusted origin": report(url="https://example.com/torch.whl"),
            "missing digest": report(),
            "malformed report": {"install": "not a list"},
        }
        fixtures["missing digest"]["install"][0]["download_info"]["archive_info"] = {}
        for name, fixture in fixtures.items():
            with self.subTest(name=name), tempfile.TemporaryDirectory() as directory:
                output = pathlib.Path(directory) / "manifest"
                target = pathlib.Path(directory) / "staged packages"

                def fake_run(command, **_kwargs):
                    pathlib.Path(command[command.index("--report") + 1]).write_text(
                        json.dumps(fixture), encoding="utf-8"
                    )
                    return SimpleNamespace(returncode=0, stdout="", stderr="")

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
                            "--install",
                            "--target",
                            str(target),
                            "--output",
                            str(output),
                        ],
                    ),
                    patch.object(resolver.subprocess, "run", side_effect=fake_run),
                    redirect_stderr(StringIO()),
                ):
                    with self.assertRaises(SystemExit) as exit_result:
                        resolver.main()
                self.assertEqual(exit_result.exception.code, 3)
                self.assertFalse((output / "requirements.txt").exists())
                self.assertFalse((output / "resolution.json").exists())

    def test_install_mode_requires_an_empty_explicit_staged_target(self):
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "manifest"
            target = pathlib.Path(directory) / "staged packages"
            target.mkdir()
            (target / "existing.py").write_text("leave intact", encoding="utf-8")
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
                        "--install",
                        "--target",
                        str(target),
                        "--output",
                        str(output),
                    ],
                ),
                patch.object(resolver.subprocess, "run") as run,
                redirect_stderr(StringIO()),
            ):
                with self.assertRaises(SystemExit) as exit_result:
                    resolver.main()
            self.assertEqual(exit_result.exception.code, 2)
            run.assert_not_called()
            self.assertEqual((target / "existing.py").read_text(), "leave intact")

    def test_install_mode_reports_pip_failure_without_publishing_a_lock(self):
        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "manifest"
            target = pathlib.Path(directory) / "staged packages"
            failed = SimpleNamespace(
                returncode=1,
                stdout="",
                stderr="No matching distribution found for torch==2.10.0+cpu",
            )
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
                        "--install",
                        "--target",
                        str(target),
                        "--output",
                        str(output),
                    ],
                ),
                patch.object(resolver.subprocess, "run", return_value=failed),
                redirect_stderr(StringIO()),
            ):
                with self.assertRaises(SystemExit) as exit_result:
                    resolver.main()
            self.assertEqual(exit_result.exception.code, 4)
            self.assertFalse((output / "requirements.txt").exists())
            self.assertFalse((output / "resolution.json").exists())

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
                        "--torch-wheel",
                        "https://download.pytorch.org/whl/cpu/torch/torch-2.14.0-cp312-cp312-macosx_14_0_arm64.whl",
                        "--output",
                        directory,
                    ],
                ),
                patch.object(resolver.sys, "platform", "darwin"),
                patch.object(resolver.platform, "machine", return_value="arm64"),
                patch.object(resolver.sys, "version_info", SimpleNamespace(major=3, minor=12)),
                patch.object(
                    resolver.packaging_tags,
                    "sys_tags",
                    return_value=["cp312-cp312-macosx_14_0_arm64"],
                ),
                patch.object(resolver.subprocess, "run", return_value=failed) as run,
                redirect_stderr(StringIO()),
            ):
                with self.assertRaises(SystemExit) as exit_result:
                    resolver.main()
        self.assertEqual(exit_result.exception.code, 4)
        self.assertTrue(
            any(
                item.startswith("torch @ https://download.pytorch.org/whl/cpu/")
                for item in run.call_args.args[0]
            )
        )

    def test_cli_pins_discovered_macos_wheel_and_resolves_dependencies(self):
        wheel = (
            "https://download.pytorch.org/whl/cpu/torch/"
            "torch-2.14.0-cp312-cp312-macosx_14_0_arm64.whl"
        )
        digest = "a" * 64
        fixture = report(version="2.14.0", url=wheel)
        commands = []
        cache_dir = pathlib.Path(tempfile.gettempdir()) / "Pumas shared pip cache"

        def fake_run(command, **_kwargs):
            commands.append(command)
            pathlib.Path(command[command.index("--report") + 1]).write_text(
                json.dumps(fixture), encoding="utf-8"
            )
            return SimpleNamespace(returncode=0, stdout="", stderr="")

        with tempfile.TemporaryDirectory() as directory:
            output = pathlib.Path(directory) / "resolver workspace with spaces"
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
                        "--torch-wheel",
                        wheel,
                        "--torch-sha256",
                        digest,
                        "--output",
                        str(output),
                        "--cache-dir",
                        str(cache_dir),
                    ],
                ),
                patch.object(resolver.sys, "platform", "darwin"),
                patch.object(resolver.platform, "machine", return_value="arm64"),
                patch.object(resolver.platform, "platform", return_value="macOS"),
                patch.object(resolver.sys, "version_info", SimpleNamespace(major=3, minor=12)),
                patch.object(
                    resolver.packaging_tags,
                    "sys_tags",
                    return_value=["cp312-cp312-macosx_14_0_arm64"],
                ),
                patch.object(resolver.subprocess, "run", side_effect=fake_run),
            ):
                resolver.main()
            self.assertEqual(len(commands), 1)
            command = commands[0]
            self.assertEqual(command[command.index("--cache-dir") + 1], str(cache_dir))
            self.assertIn(f"torch @ {wheel}#sha256={digest}", command)
            self.assertNotIn("torch==2.14.0", command)
            self.assertTrue(set(resolver.CORE).issubset(command))
            self.assertEqual(
                json.loads((output / "resolution.json").read_text())["torch"],
                "2.14.0",
            )

    def test_cli_rejects_report_that_does_not_match_pinned_wheel(self):
        wheel = "https://download.pytorch.org/whl/cpu/torch/torch-2.10.0%2Bcpu-cp312-cp312-manylinux_2_17_x86_64.whl"
        fixture = report(
            url="https://download.pytorch.org/whl/cpu/torch/torch-2.10.0%2Bcpu-cp312-cp312-manylinux_2_28_x86_64.whl"
        )

        def fake_run(command, **_kwargs):
            pathlib.Path(command[command.index("--report") + 1]).write_text(
                json.dumps(fixture), encoding="utf-8"
            )
            return SimpleNamespace(returncode=0, stdout="", stderr="")

        with tempfile.TemporaryDirectory() as directory:
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
                        "--torch-wheel",
                        wheel,
                        "--output",
                        directory,
                    ],
                ),
                patch.object(
                    resolver.packaging_tags,
                    "sys_tags",
                    return_value=["cp312-cp312-manylinux_2_17_x86_64"],
                ),
                patch.object(resolver.subprocess, "run", side_effect=fake_run),
                redirect_stderr(StringIO()),
            ):
                with self.assertRaises(SystemExit) as exit_result:
                    resolver.main()
            self.assertEqual(exit_result.exception.code, 3)
            self.assertFalse((pathlib.Path(directory) / "requirements.txt").exists())

    def test_cli_rejects_report_with_wrong_hash_for_pinned_wheel(self):
        wheel = native_wheel()
        fixture = report(url=wheel)

        def fake_run(command, **_kwargs):
            pathlib.Path(command[command.index("--report") + 1]).write_text(
                json.dumps(fixture), encoding="utf-8"
            )
            return SimpleNamespace(returncode=0, stdout="", stderr="")

        with tempfile.TemporaryDirectory() as directory:
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
                        "--torch-wheel",
                        wheel,
                        "--torch-sha256",
                        "b" * 64,
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

    def test_cli_requires_discovered_wheel_for_resolution(self):
        with tempfile.TemporaryDirectory() as directory:
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
                patch.object(resolver.subprocess, "run") as run,
                redirect_stderr(StringIO()),
            ):
                with self.assertRaises(SystemExit) as exit_result:
                    resolver.main()
            self.assertEqual(exit_result.exception.code, 2)
            run.assert_not_called()

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
                            "--torch-wheel",
                            native_wheel(),
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

        def assert_utf8_report_read(path, *args, **kwargs):
            if path.name == "pip-resolution.json":
                self.assertEqual(kwargs.get("encoding"), "utf-8")
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
                        "--torch-wheel",
                        native_wheel(),
                        "--output",
                        directory,
                    ],
                ),
                patch.object(resolver.subprocess, "run", side_effect=fake_run),
                patch.object(pathlib.Path, "read_text", assert_utf8_report_read),
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
                        "--torch-wheel",
                        native_wheel("1.12.1", "cu102"),
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
