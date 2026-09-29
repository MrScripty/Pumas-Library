"""Optional adapter diagnostics and core probe status stay separate."""

import importlib.util
import hashlib
import json
import tempfile
from pathlib import Path
from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import patch

spec = importlib.util.spec_from_file_location(
    "probe_runtime", Path(__file__).resolve().parents[1] / "probe_runtime.py"
)
probe_runtime = importlib.util.module_from_spec(spec)
spec.loader.exec_module(probe_runtime)


class AdapterImportProbeTests(TestCase):
    def test_missing_adapter_is_scoped_to_its_feature(self):
        with patch.object(
            probe_runtime.importlib, "import_module", side_effect=ImportError("nunchaku missing")
        ):
            result = probe_runtime.adapter_import_probe(("nunchaku",), ())
        self.assertEqual(result["status"], "unavailable")
        self.assertEqual(result["scope"], "adapter imports")
        self.assertIn("nunchaku missing", result["error"])

    def test_imports_do_not_claim_model_execution(self):
        with (
            patch.object(
                probe_runtime.importlib,
                "import_module",
                return_value=SimpleNamespace(ZImagePipeline=object()),
            ),
            patch.object(probe_runtime.importlib.metadata, "version", return_value="0.37.0"),
        ):
            result = probe_runtime.adapter_import_probe(
                ("diffusers",), (("diffusers", "ZImagePipeline"),)
            )
        self.assertEqual(result["status"], "inconclusive")
        self.assertEqual(result["versions"]["diffusers"], "0.37.0")

    def test_core_failure_is_reported_even_if_optional_adapter_unavailable(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "resolution.json").write_text(
                json.dumps({"torch": "2.10.0+cpu", "adapter": "none", "artifacts": []})
            )
            (root / "serve.py").write_text("# fixture\n")

            async def health():
                return {"status": "ok", "protocol": 3}

            app = SimpleNamespace(
                routes=[SimpleNamespace(), SimpleNamespace(path="/health", endpoint=health)]
            )
            fake_serve = SimpleNamespace(create_app=lambda: app)
            with (
                patch.dict("sys.modules", {"serve": fake_serve}),
                patch.object(
                    probe_runtime.importlib,
                    "import_module",
                    side_effect=ImportError("torch missing"),
                ),
            ):
                result = probe_runtime.probe(root)
        self.assertEqual(result["status"], "failed")
        self.assertEqual(result["capabilities"]["torch_import"]["status"], "failed")
        self.assertEqual(result["capabilities"]["sidecar_app"]["status"], "passed")
        self.assertRegex(result["recorded_at"], r"^\d{4}-\d{2}-\d{2}T")
        self.assertEqual(len(result["context"]["hardware_fingerprint"]), 64)
        canonical_hardware = {
            "cuda": None,
            "hip": None,
            "device_count": 0,
            "devices": [],
            "mps_available": False,
        }
        self.assertEqual(
            result["context"]["hardware_fingerprint"],
            hashlib.sha256(
                json.dumps(canonical_hardware, sort_keys=True, separators=(",", ":")).encode()
            ).hexdigest(),
        )
        self.assertEqual(len(result["context"]["interpreter_sha256"]), 64)
        self.assertEqual(len(result["context"]["distributions_sha256"]), 64)
        self.assertIn("serve.py", result["context"]["runtime_files_sha256"])
        self.assertEqual(result["capabilities"]["flux2_klein"]["status"], "not selected")

    def test_selected_adapter_failure_is_partial_with_passing_core(self):
        class Tensor:
            def __matmul__(self, _other):
                return self

            def tolist(self):
                return [[2.0, 2.0], [2.0, 2.0]]

        fake_torch = SimpleNamespace(
            __version__="2.10.0+cpu",
            ones=lambda *_: Tensor(),
            backends=SimpleNamespace(mps=SimpleNamespace(is_available=lambda: True)),
        )

        async def health():
            return {"status": "ok", "protocol": 3}

        app = SimpleNamespace(routes=[SimpleNamespace(path="/health", endpoint=health)])
        fake_serve = SimpleNamespace(create_app=lambda: app)

        def import_module(name):
            if name == "torch":
                return fake_torch
            raise ImportError(f"{name} missing")

        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "resolution.json").write_text(
                json.dumps({"torch": "2.10.0+cpu", "adapter": "flux2", "artifacts": []})
            )
            (root / "serve.py").write_text("# fixture\n")
            with (
                patch.dict("sys.modules", {"serve": fake_serve}),
                patch.object(probe_runtime.importlib, "import_module", side_effect=import_module),
                patch.object(
                    probe_runtime,
                    "device_probe",
                    return_value=(
                        {"cuda": None, "hip": None, "devices": []},
                        {"status": "unavailable", "scope": "CUDA tensor operation"},
                    ),
                ),
            ):
                result = probe_runtime.probe(root)
        self.assertEqual(result["core_status"], "passed")
        self.assertEqual(result["adapter_status"], "unavailable")
        self.assertEqual(result["status"], "partial")
        self.assertEqual(result["capabilities"]["flux2_klein"]["status"], "unavailable")
        canonical_hardware = {
            "cuda": None,
            "hip": None,
            "device_count": 0,
            "devices": [],
            "mps_available": True,
        }
        expected_fingerprint = hashlib.sha256(
            json.dumps(canonical_hardware, sort_keys=True, separators=(",", ":")).encode()
        ).hexdigest()
        self.assertEqual(result["context"]["hardware_fingerprint"], expected_fingerprint)

    def test_device_failure_is_scoped(self):
        fake_torch = SimpleNamespace(
            version=SimpleNamespace(cuda="13.0", hip=None),
            cuda=SimpleNamespace(
                is_available=lambda: True,
                device_count=lambda: 1,
                get_device_name=lambda _: "GPU",
                get_device_capability=lambda _: (12, 0),
            ),
            ones=lambda *_args, **_kwargs: (_ for _ in ()).throw(RuntimeError("allocation failed")),
        )
        hardware, capability = probe_runtime.device_probe(fake_torch)
        self.assertEqual(capability["status"], "failed")
        self.assertIn("allocation failed", hardware["error"])

    def test_runtime_hashes_track_files_and_skip_venv_and_symlinks(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            (root / "serve.py").write_bytes(b"first")
            (root / "loaders").mkdir()
            (root / "loaders" / "module.py").write_bytes(b"second")
            (root / "nested").mkdir()
            (root / "nested" / "not_shipped.py").write_bytes(b"other")
            (root / "venv").mkdir()
            (root / "venv" / "ignored.py").write_bytes(b"ignored")
            (root / "venv" / "lib").mkdir()
            (root / "venv" / "lib" / "package.py").write_bytes(b"package")
            try:
                (root / "linked.py").symlink_to(root / "serve.py")
            except OSError as error:
                # Windows hosted runners may lack symlink privilege; keep the
                # file-hash assertions useful without weakening link coverage
                # on systems that can create the fixture.
                if getattr(error, "winerror", None) != 1314:
                    raise
            hashes = probe_runtime.runtime_file_hashes(root)
        self.assertEqual(
            hashes,
            {
                "loaders/module.py": hashlib.sha256(b"second").hexdigest(),
                "serve.py": hashlib.sha256(b"first").hexdigest(),
            },
        )

    def test_driver_query_is_bounded_and_normalized(self):
        with patch.object(
            probe_runtime.subprocess,
            "run",
            return_value=SimpleNamespace(returncode=0, stdout="580.82.07\n580.82.07\n"),
        ) as run:
            self.assertEqual(probe_runtime.driver_versions(), ["580.82.07"])
        self.assertEqual(run.call_args.kwargs["timeout"], 5)

    def test_all_distribution_versions_include_packages_outside_lock(self):
        distributions = [
            SimpleNamespace(metadata={"Name": "Extra_Package"}, version="1.2.3"),
            SimpleNamespace(metadata={"Name": "torch"}, version="2.10.0+cpu"),
        ]
        with patch.object(
            probe_runtime.importlib.metadata,
            "distributions",
            return_value=distributions,
        ):
            self.assertEqual(
                probe_runtime.all_distribution_versions(),
                {"extra-package": "1.2.3", "torch": "2.10.0+cpu"},
            )
