"""Fake RPC, process and mapping fixtures; no runtime download, ORT or model inference."""

import hashlib
import importlib.util
import copy
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import Mock, patch

import headless_inference_ci as ci
import packaged_onnx_support as support
import synthetic_onnx_fixture as synthetic
import verify_synthetic_packaged_onnx as synthetic_check

STAGE_SPEC = importlib.util.spec_from_file_location(
    "desktop_stage", Path(__file__).with_name("stage-rpc.py")
)
desktop = importlib.util.module_from_spec(STAGE_SPEC)
STAGE_SPEC.loader.exec_module(desktop)


class ReleaseTests(unittest.TestCase):
    def test_child_environment_passes_only_the_github_api_token(self):
        ambient = {"PUMAS_GITHUB_TOKEN": "ghs_fixture", "GITHUB_TOKEN": "x", "HF_TOKEN": "y"}
        with patch.dict(os.environ, ambient):
            environment = support.child_environment(tempfile.gettempdir())
        self.assertEqual(environment["PUMAS_GITHUB_TOKEN"], "ghs_fixture")
        self.assertNotIn("GITHUB_TOKEN", environment)
        self.assertNotIn("HF_TOKEN", environment)
        with patch.dict(os.environ, {"PUMAS_GITHUB_TOKEN": ""}):
            self.assertNotIn("PUMAS_GITHUB_TOKEN", support.child_environment(tempfile.gettempdir()))

    def test_only_shared_libraries_count_as_a_mapped_onnx_runtime(self):
        # ONNX Runtime 1.31 maps its own cache database; it is not a library.
        for name, expected in [
            ("/x/onnx-runtime-versions/v1.31.1+cpu/lib/libonnxruntime.so", True),
            ("/x/lib/libonnxruntime.so.1.31.1", True),
            ("/x/lib/libonnxruntime.dylib", True),
            ("C:/x/lib/onnxruntime.dll", True),
            ("/home/u/.cache/Microsoft/DeveloperTools/.onnxruntime/onnxruntime.db-shm", False),
            ("/x/onnxruntime.db", False),
            ("/x/lib/libother.so", False),
        ]:
            self.assertEqual(support.is_runtime_library(name), expected, name)

    def test_desktop_staging_packages_only_the_backend_never_onnx_runtime(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source, target = root / "source", root / "stage"
            source.mkdir()
            target.mkdir()
            binary = "pumas-rpc.exe" if os.name == "nt" else "pumas-rpc"
            (target / "sentinel").write_bytes(b"prior usable stage")
            with self.assertRaises(ValueError):
                desktop.stage_rpc(source, target)
            self.assertEqual((target / "sentinel").read_bytes(), b"prior usable stage")
            (source / binary).write_bytes(b"test RPC, not executable")
            # ONNX Runtime is downloaded by the desktop at runtime; libraries
            # beside the build output must never be staged into a package.
            for name in [
                "onnxruntime.dll",
                "libonnxruntime.so",
                "libonnxruntime.dylib",
                "ONNX-LICENSE.txt",
            ]:
                (source / name).write_bytes(b"must never copy")
            desktop.stage_rpc(source, target)
            self.assertEqual({p.name for p in target.iterdir()}, {binary})

    def test_windows_module_paths_drop_the_verbatim_prefix(self):
        # A module loaded through a canonical path is reported as \\?\D:\...
        self.assertEqual(
            support.without_verbatim_prefix("\\\\?\\D:\\a\\onnxruntime.dll"),
            "D:\\a\\onnxruntime.dll",
        )
        self.assertEqual(
            support.without_verbatim_prefix("\\\\?\\UNC\\host\\share\\ort.dll"),
            "\\\\host\\share\\ort.dll",
        )
        self.assertEqual(support.without_verbatim_prefix("D:\\a\\ort.dll"), "D:\\a\\ort.dll")

    def test_native_target_rejects_unsupported_host_instead_of_assuming_linux(self):
        with (
            patch.object(support.platform, "system", return_value="Linux"),
            patch.object(support.platform, "machine", return_value="aarch64"),
        ):
            with self.assertRaisesRegex(ValueError, "supported native host"):
                support.native_target()

    @unittest.skipIf(sys.platform == "linux", "native Windows/macOS OS observer")
    def test_real_native_module_observer_can_inspect_its_own_process(self):
        paths = support.native_modules(os.getpid(), support.native_target())
        self.assertTrue(paths)
        self.assertTrue(any(path.is_file() for path in paths))

    def test_forced_shutdown_and_nonzero_exit_never_pass(self):
        process = Mock(returncode=-9)
        process.poll.return_value = None
        process.wait.side_effect = [subprocess.TimeoutExpired("owned RPC", 20), -9]
        with self.assertRaisesRegex(RuntimeError, "forced shutdown"):
            support.stop_process(process)
        process.kill.assert_called_once()
        process = Mock(returncode=1)
        process.poll.return_value = 1
        with self.assertRaisesRegex(ValueError, "shut down cleanly"):
            support.stop_process(process)
        process = Mock()
        process.poll.return_value = None
        process.send_signal.side_effect = OSError("test-only unavailable console")
        with self.assertRaises(OSError):
            support.stop_process(process)
        process.kill.assert_called_once()
        process.wait.assert_called_once_with(timeout=10)

    def test_synthetic_fixture_is_deterministic_untrained_and_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / "fixture"
            record = synthetic.create_fixture(root)
            self.assertIs(record["pretrained_model_acceptance"], False)
            self.assertIn("untrained", record["scope"])
            self.assertEqual(
                record["files"]["onnx/model_fp16.onnx"]["sha256"],
                "3639dc4b3eca8394bc178e96f322d90ca76b93999e409b9f173d6c65094791f5",
            )
            for name, entry in record["files"].items():
                self.assertEqual(
                    hashlib.sha256((root / name).read_bytes()).hexdigest(), entry["sha256"]
                )
                self.assertEqual((root / name).stat().st_size, entry["bytes"])
            with self.assertRaises(FileExistsError):
                synthetic.create_fixture(root)

    @unittest.skipUnless(sys.platform == "linux", "Linux owned process group")
    def test_synthetic_verifier_timeout_stops_its_process_group(self):
        process = Mock(pid=12345)
        process.wait.side_effect = [subprocess.TimeoutExpired("fixture", 120), -9]
        with (
            patch.object(synthetic_check.subprocess, "Popen", return_value=process) as spawn,
            patch.object(synthetic_check.os, "killpg") as stop,
        ):
            with self.assertRaises(subprocess.TimeoutExpired):
                synthetic_check.run_lifecycle(["fixture"], io.BytesIO())
            self.assertTrue(spawn.call_args.kwargs["start_new_session"])
            stop.assert_called_once_with(12345, synthetic_check.signal.SIGKILL)
        process = Mock()
        process.wait.return_value = 1
        with patch.object(synthetic_check.subprocess, "Popen", return_value=process):
            with self.assertRaisesRegex(ValueError, "shutdown failed"):
                synthetic_check.run_lifecycle(["fixture"], io.BytesIO())

    def test_combined_rpc_schemas_are_complete_and_separate_from_core(self):
        with tempfile.TemporaryDirectory() as tmp:
            repository = Path(tmp)
            (repository / "rust").mkdir()
            (repository / "rust/Cargo.toml").write_text(
                '[workspace.package]\nversion="0.8.0-rc.1"\n'
            )
            with patch.object(
                ci.build, "source_identity", return_value={"head": "a" * 40, "tree": "b" * 40}
            ):
                for target in ("linux-x86_64", "windows-x86_64", "macos-arm64"):
                    contract = ci.contract_for(repository, target, b"{}", "test")
                    expected = contract["expected"]
                    rpc, core = expected["build_info"], expected["core_build_info"]
                    added = {item["name"] for item in rpc["schemas"]} - {
                        item["name"] for item in core["schemas"]
                    }
                    wanted = {
                        "pumas.http-advertisement",
                        "pumas.http-admission-fence",
                        "pumas.model-operations.image-to-text",
                    }
                    if target == "linux-x86_64":
                        wanted.add("pumas.http-owner-retention")
                    self.assertEqual(added, wanted)
                    self.assertEqual(ci.build.core_projection(rpc), core)
                    for name in wanted:
                        missing = copy.deepcopy(contract)
                        missing["expected"]["build_info"]["schemas"] = [
                            item for item in rpc["schemas"] if item["name"] != name
                        ]
                        with self.assertRaises(ValueError):
                            ci.package.validate_contract(missing)
                        wrong_version = copy.deepcopy(contract)
                        for item in wrong_version["expected"]["build_info"]["schemas"]:
                            if item["name"] == name:
                                item["version"] = 2
                        with self.assertRaises(ValueError):
                            ci.package.validate_contract(wrong_version)
                        leaked = copy.deepcopy(contract)
                        leaked["expected"]["core_build_info"]["schemas"].append(
                            {"name": name, "version": 1}
                        )
                        with self.assertRaises(ValueError):
                            ci.package.validate_contract(leaked)
                    unknown = copy.deepcopy(rpc)
                    unknown["schemas"].append({"name": "pumas.future-rpc-only", "version": 1})
                    self.assertNotEqual(ci.build.core_projection(unknown), core)

    @unittest.skipUnless(sys.platform == "linux", "Linux packaged subprocess contract")
    def test_environment_does_not_inherit_runtime_or_loader_overrides(self):
        with (
            tempfile.TemporaryDirectory() as tmp,
            patch.dict(
                os.environ,
                {
                    "ORT_DYLIB_PATH": "/ambient/ort.so",
                    "LD_PRELOAD": "/ambient/inject.so",
                    "DYLD_LIBRARY_PATH": "/ambient",
                    "PUMAS_LAUNCHER_ROOT": "/ambient",
                    "PYTHONPATH": "/ambient",
                    "HTTPS_PROXY": "http://ambient",
                },
            ),
        ):
            environment = support.child_environment(Path(tmp))
            output = subprocess.check_output(
                [sys.executable, "-I", "-c", "import os,json;print(json.dumps(dict(os.environ)))"],
                env=environment,
                text=True,
            )
            observed = json.loads(output)
            self.assertFalse(any("ambient" in value for value in observed.values()))
            self.assertNotIn("ORT_DYLIB_PATH", observed)
            self.assertEqual(observed["PUMAS_REGISTRY_DB_PATH"], str(Path(tmp) / "registry.db"))

    def test_current_unbumped_source_cannot_be_relabelled(self):
        with tempfile.TemporaryDirectory() as tmp:
            repository = Path(tmp)
            (repository / "rust").mkdir()
            (repository / "rust/Cargo.toml").write_text('[workspace.package]\nversion="0.7.0"\n')
            with patch.object(
                ci.build, "source_identity", return_value={"head": "a" * 40, "tree": "b" * 40}
            ):
                with self.assertRaisesRegex(ValueError, "v0.8"):
                    ci.contract_for(repository, "linux-x86_64", b"{}", "test")
                (repository / "rust/Cargo.toml").write_text(
                    '[workspace.package]\nversion="0.8.0"\n'
                )
                contract = ci.contract_for(repository, "linux-x86_64", b"{}", "test")
                self.assertEqual(
                    ci.build.core_projection(contract["expected"]["build_info"]),
                    contract["expected"]["core_build_info"],
                )


def release(tag, prerelease=False):
    return {"tagName": tag, "prerelease": prerelease}


class FakeRpc:
    """Scripted JSON-RPC results; records every call. Never a real Pumas."""

    def __init__(self, results):
        self.results = {method: list(values) for method, values in results.items()}
        self.calls = []

    def __call__(self, method, params=None):
        self.calls.append((method, params))
        values = self.results[method]
        return values.pop(0) if len(values) > 1 else values[0]

    def methods(self):
        return [method for method, _ in self.calls]


class FakeClock:
    def __init__(self):
        self.now = 0.0

    def __call__(self):
        return self.now

    def sleep(self, seconds):
        self.now += seconds


TAG = "v1.31.1+cpu"
DONE = {"tag": TAG, "completedAt": "2026-11-12T00:00:00Z", "success": True}
QUIET = {"log": lambda *args, **kwargs: None}


class InstalledRuntimeTests(unittest.TestCase):
    def test_selects_the_newest_stable_cpu_build_by_version_not_listing_order(self):
        listing = {
            "success": True,
            "versions": [
                release("v1.31.1+cuda13"),
                release("v1.32.0+cpu", prerelease=True),
                release("v1.9.9+cpu"),
                release(TAG),
                release("v1.30.0+cpu"),
                release("v1.31.1+cuda12"),
                {"name": "no tag"},
            ],
        }
        self.assertEqual(support.select_cpu_release(listing), TAG)

    def test_listing_refusals_carry_their_reason(self):
        for listing, reason in (
            ({"success": False, "rate_limited": True, "retry_after_secs": 30}, "retry after 30"),
            ({"success": False, "error": "offline"}, "offline"),
            ({"success": True, "versions": [release("v1.31.1+cuda12")]}, "v1.31.1\\+cuda12"),
            ({"success": True, "versions": []}, "no stable \\+cpu"),
            (None, "unexpected ONNX Runtime listing"),
        ):
            with self.subTest(listing=listing), self.assertRaisesRegex(ValueError, reason):
                support.select_cpu_release(listing)

    def install(self, rpc, timeout=10):
        clock = FakeClock()
        return support.install_runtime(
            rpc, TAG, timeout=timeout, interval=1, clock=clock, sleep=clock.sleep
        )

    def test_install_waits_for_completed_progress_then_confirms_installation(self):
        rpc = FakeRpc(
            {
                "install_version": [{"success": True, "message": "started"}],
                "get_installation_progress": [{"tag": TAG, "stage": "download"}, DONE],
                "get_installed_versions": [{"success": True, "versions": [TAG]}],
            }
        )
        self.assertEqual(self.install(rpc)["tag"], TAG)
        self.assertEqual(rpc.calls[0], ("install_version", {"app_id": "onnx-runtime", "tag": TAG}))
        self.assertEqual(
            rpc.methods(),
            [
                "install_version",
                "get_installation_progress",
                "get_installation_progress",
                "get_installed_versions",
            ],
        )

    def test_install_accepts_a_cleared_progress_record_once_installed(self):
        rpc = FakeRpc(
            {
                "install_version": [{"success": True, "message": "started"}],
                "get_installation_progress": [None],
                "get_installed_versions": [
                    {"success": True, "versions": []},
                    {"success": True, "versions": [TAG]},
                ],
            }
        )
        self.assertEqual(self.install(rpc)["installed_versions"], [TAG])

    def test_install_failures_refuse_with_the_reason(self):
        cases = {
            "refused": (
                {"install_version": [{"success": False, "error": "disk full"}]},
                "refused to install.*disk full",
            ),
            "failed": (
                {
                    "install_version": [{"success": True}],
                    "get_installation_progress": [
                        {**DONE, "success": False, "error": "checksum mismatch", "logPath": "x"}
                    ],
                },
                "installation failed: checksum mismatch",
            ),
            "other": (
                {
                    "install_version": [{"success": True}],
                    "get_installation_progress": [{"tag": "v1.30.0+cpu"}],
                },
                "another ONNX Runtime installation",
            ),
            "timeout": (
                {
                    "install_version": [{"success": True}],
                    "get_installation_progress": [{"tag": TAG, "stage": "download"}],
                },
                "did not finish within 10 seconds; last stage download",
            ),
            "missing": (
                {
                    "install_version": [{"success": True}],
                    "get_installation_progress": [DONE],
                    "get_installed_versions": [{"success": True, "versions": []}],
                },
                "reported complete but is not installed",
            ),
        }
        for name, (results, reason) in cases.items():
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, reason):
                self.install(FakeRpc(results))

    def lifecycle_results(self, **changes):
        return {
            "get_available_versions": [{"success": True, "versions": [release(TAG)]}],
            "install_version": [{"success": True}],
            "get_installation_progress": [DONE],
            "get_installed_versions": [{"success": True, "versions": [TAG]}],
            "upsert_runtime_profile": [{"success": True, "profile_id": "onnx-verification"}],
            "launch_runtime_profile": [{"success": True, "ready": True}],
            "import_model_in_place": [{"success": True, "model_id": "embedding/fixture/m"}],
            "serve_model": [{"loaded": True}],
            "unserve_model": [{"unloaded": True}],
            **changes,
        }

    def test_lifecycle_pins_a_managed_cpu_profile_to_the_installed_version(self):
        rpc = FakeRpc(self.lifecycle_results())
        posts, observed = [], []

        def post(path, body):
            posts.append((path, body))
            return {"data": [{"embedding": [0.5] * 256}]}

        def observe(tag):
            # The runtime must be observed while the model is still served.
            observed.append((tag, rpc.methods()[-1]))
            return {"mapping": "pumas-installed-runtime-observed", "runtime_version": tag}

        result = support.exercise_installed_runtime(rpc, post, Path("/model"), observe, **QUIET)
        self.assertEqual(
            rpc.methods(),
            [
                "get_available_versions",
                "install_version",
                "get_installation_progress",
                "get_installed_versions",
                "upsert_runtime_profile",
                "launch_runtime_profile",
                "import_model_in_place",
                "serve_model",
                "unserve_model",
            ],
        )
        calls = dict(rpc.calls)
        self.assertEqual(calls["get_available_versions"]["app_id"], "onnx-runtime")
        self.assertEqual(
            calls["upsert_runtime_profile"]["profile"],
            {
                "profile_id": "onnx-verification",
                "provider": "onnx_runtime",
                "provider_mode": "onnx_serve",
                "management_mode": "managed",
                "name": "Installer verification",
                "device": {"mode": "cpu"},
                "runtime_version": TAG,
            },
        )
        self.assertEqual(calls["launch_runtime_profile"], {"profile_id": "onnx-verification"})
        served = calls["serve_model"]["request"]["config"]
        self.assertEqual(
            (served["profile_id"], served["device_mode"]), ("onnx-verification", "cpu")
        )
        self.assertEqual(posts[0][0], "/v1/embeddings")
        self.assertEqual(observed, [(TAG, "serve_model")])
        self.assertEqual(result["runtime_version"], TAG)
        for name in ("runtime_install", "import", "load", "unload"):
            self.assertEqual(result[name], "passed")

    def test_lifecycle_refuses_profile_launch_and_nonfinite_failures(self):
        good = [0.5] * 256
        for name, changes, vector, reason in (
            (
                "profile",
                {"upsert_runtime_profile": [{"success": False, "error": "bad version"}]},
                good,
                "refused the ONNX runtime profile.*bad version",
            ),
            (
                "launch",
                {"launch_runtime_profile": [{"success": False, "error": "not installed"}]},
                good,
                "did not launch: not installed",
            ),
            ("nonfinite", {}, [float("nan")] * 256, "finite"),
            ("dimensions", {}, [0.5] * 8, "256"),
        ):
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, reason):
                support.exercise_installed_runtime(
                    FakeRpc(self.lifecycle_results(**changes)),
                    lambda path, body, vector=vector: {"data": [{"embedding": vector}]},
                    Path("/model"),
                    lambda tag: {},
                    **QUIET,
                )

    def test_runtime_beside_the_rpc_is_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name in ("pumas-rpc", "LICENSE.txt", "THIRD-PARTY-NOTICES.txt", "manifest.json"):
                (root / name).write_bytes(b"controlled")
            support.require_no_bundled_runtime(root)
            for name in (
                "libonnxruntime.so.1.31.1",
                "onnxruntime.dll",
                "libllama.so",
                "ggml-cpu.dll",
                "torch_cpu.dll",
            ):
                runtime = root / name
                runtime.write_bytes(b"controlled bytes, not a runtime")
                with (
                    self.subTest(name=name),
                    self.assertRaisesRegex(ValueError, name.replace(".", "\\.")),
                ):
                    support.require_no_bundled_runtime(root)
                runtime.unlink()

    def test_runtime_telemetry_storage_is_refused(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "cache" / "Microsoft" / "DeveloperTools").mkdir(parents=True)
            (root / "cache" / "unrelated" / ".onnxruntime").mkdir(parents=True)
            support.require_no_runtime_telemetry_storage(root)
            storage = root / "cache" / "Microsoft" / "DeveloperTools" / ".onnxruntime"
            storage.mkdir()
            (storage / "onnxruntime.db").write_bytes(b"controlled")
            with self.assertRaisesRegex(ValueError, "telemetry storage"):
                support.require_no_runtime_telemetry_storage(root)

    def test_process_tree_follows_descendants_only(self):
        parents = {1: 0, 10: 1, 11: 10, 12: 11, 13: 1, 20: 99, 21: 20, 30: 30}
        self.assertEqual(support.process_tree(10, parents), {10, 11, 12})
        self.assertEqual(support.process_tree(30, parents), {30})

    def test_linux_parent_map_reads_stat_with_parenthesized_names(self):
        with tempfile.TemporaryDirectory() as temporary:
            proc = Path(temporary)
            for pid, text in {
                "10": "10 (pumas-rpc) S 1 10 10",
                "11": "11 (onnx host (x) y) S 10 10 10",
                "self": "ignored",
            }.items():
                (proc / pid).mkdir()
                (proc / pid / "stat").write_text(text)
            self.assertEqual(support.linux_parent_map(proc), {10: 1, 11: 10})

    @unittest.skipUnless(sys.platform == "linux", "Linux /proc mapping contract")
    def test_linux_mappings_bind_device_and_inode(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            library = root / "libonnxruntime.so.1.31.1"
            library.write_bytes(b"controlled bytes, not ORT")
            other = root / "libc.so.6"
            other.write_bytes(b"controlled")
            stat = library.stat()
            device = f"{os.major(stat.st_dev):x}:{os.minor(stat.st_dev):x}"
            (root / "7").mkdir()
            maps = root / "7" / "maps"
            maps.write_text(
                f"0000-1000 r-xp 0 {device} {stat.st_ino} {library}\n"
                f"1000-2000 r-xp 0 {device} {other.stat().st_ino} {other}\n"
                "2000-3000 rw-p 0 00:00 0\n"
            )
            self.assertEqual(support.linux_mapped_runtimes(7, root), {library})
            maps.write_text(f"0000-1000 r-xp 0 {device} {stat.st_ino + 1} {library}\n")
            with self.assertRaisesRegex(ValueError, "identity changed"):
                support.linux_mapped_runtimes(7, root)

    def test_runtime_must_load_from_the_pumas_installed_version(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            version = root / "onnx-runtime-versions" / TAG / "lib"
            version.mkdir(parents=True)
            installed = version / "libonnxruntime.so.1.31.1"
            installed.write_bytes(b"controlled bytes, not ORT")
            ambient = root / "ambient" / "libonnxruntime.so"
            ambient.parent.mkdir()
            ambient.write_bytes(b"controlled ambient bytes")
            parents = {100: 1, 101: 100, 200: 1}

            def observe(mappings, target="linux-x86_64"):
                with (
                    patch.object(support, "parent_map", return_value=parents),
                    patch.object(
                        support,
                        "mapped_runtimes",
                        side_effect=lambda pid, _: mappings.get(pid, set()),
                    ),
                ):
                    return support.installed_runtime(100, root, TAG, target)

            # The out-of-process host (a descendant) maps the installed version.
            evidence = observe({101: {installed}})
            self.assertEqual(evidence["mapping"], "pumas-installed-runtime-observed")
            self.assertEqual(evidence["runtime_version"], TAG)
            self.assertEqual(
                evidence["libraries"],
                {
                    "lib/libonnxruntime.so.1.31.1": hashlib.sha256(
                        installed.read_bytes()
                    ).hexdigest()
                },
            )
            self.assertEqual(evidence["identity_observation"], "mapped-device-inode")
            self.assertEqual(
                observe({101: {installed}}, "macos-arm64")["identity_observation"],
                "native-module-path",
            )
            # Unrelated processes are never attributed to the owned tree.
            with self.assertRaisesRegex(ValueError, "no ONNX Runtime library was observed"):
                observe({200: {installed}})
            with self.assertRaisesRegex(ValueError, "outside the Pumas-installed"):
                observe({100: {ambient}, 101: {installed}})
            with self.assertRaisesRegex(ValueError, "is missing"):
                support.installed_runtime(100, root, "v9.9.9+cpu", "linux-x86_64")

    def test_vanished_child_is_skipped_but_not_counted(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            version = root / "onnx-runtime-versions" / TAG
            version.mkdir(parents=True)
            installed = version / "onnxruntime.dll"
            installed.write_bytes(b"controlled")

            def mapped(pid, target):
                if pid == 101:
                    raise FileNotFoundError(pid)
                return {installed}

            with (
                patch.object(support, "parent_map", return_value={101: 100}),
                patch.object(support, "mapped_runtimes", side_effect=mapped),
            ):
                evidence = support.installed_runtime(100, root, TAG, "windows-x86_64")
            self.assertEqual(evidence["observing_process_count"], 1)

    def test_synthetic_receipt_binds_binary_source_and_installed_runtime(self):
        source = "a" * 40
        evidence = {"source": {"head": source, "tree": "b" * 40}, "archive_sha256": "c" * 64}
        manifest = {
            "compatibility": {"expected": {"source_commit": source, "source_tree": "b" * 40}},
            "files": {"pumas-rpc": {"sha256": "d" * 64}},
        }
        result = {
            "binary_sha256": "d" * 64,
            "bundled_runtimes": [],
            "runtime_install": "passed",
            "runtime_version": TAG,
            "import": "passed",
            "load": "passed",
            "unload": "passed",
            "embedding_dimensions": 256,
            "finite": True,
            "runtime": {"mapping": "pumas-installed-runtime-observed", "runtime_version": TAG},
        }
        record = synthetic_check.bind_result(source, evidence, manifest, {}, result)
        self.assertIs(record["pretrained_model_acceptance"], False)
        self.assertEqual(record["archive_sha256"], evidence["archive_sha256"])
        self.assertEqual(record["qualification"], "synthetic_execution_only")
        self.assertEqual(record["runtime_version"], TAG)
        for changed in (
            {**result, "binary_sha256": "e" * 64},
            {**result, "finite": False},
            {**result, "unload": "failed"},
            {**result, "runtime_install": "skipped"},
            {**result, "bundled_runtimes": ["libonnxruntime.so"]},
            {**result, "runtime_version": "v1.31.1+cuda12"},
            {**result, "runtime": {"mapping": "packaged-file-identity-observed"}},
            {**result, "runtime": {**result["runtime"], "runtime_version": "v1.30.0+cpu"}},
        ):
            with self.assertRaises(ValueError):
                synthetic_check.bind_result(source, evidence, manifest, {}, changed)
        with self.assertRaises(ValueError):
            synthetic_check.bind_result("f" * 40, evidence, manifest, {}, result)


if __name__ == "__main__":
    unittest.main()
