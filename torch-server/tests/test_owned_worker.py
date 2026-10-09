"""Installed bootstrap/isolation qualification; controlled workers, no real ASR."""

import asyncio
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from owned_worker import REQUIRED_CODE, REQUIRED_MODEL  # noqa: E402

CONTROLLED = ("tests/owned_channel_fixture.py", "tests/owned_worker_fixture.py")


@unittest.skipUnless(sys.platform == "linux", "Inherited Linux directory bootstrap")
class OwnedWorkerTests(unittest.IsolatedAsyncioTestCase):
    def stage(self):
        temp = Path(self.enterContext(tempfile.TemporaryDirectory()))
        code, packages, model, ambient = (
            temp / name for name in ("code", "packages", "model", "ambient")
        )
        for root in (code, packages, model, ambient):
            root.mkdir()
        for name in (*REQUIRED_CODE, *CONTROLLED):
            destination = code / name
            destination.parent.mkdir(exist_ok=True)
            shutil.copyfile(ROOT / name, destination)
        for name in REQUIRED_MODEL:
            (model / name).write_bytes(b"\x00" if name == "model.safetensors" else b"{}")
        return code, packages, model, ambient

    def launch(self, roots, *, controlled=False, extra=(), isolated=True):
        code, packages, model, ambient = roots
        fds = [
            os.open(root, os.O_RDONLY | (os.O_DIRECTORY if root.is_dir() else 0))
            for root in (code, packages, model)
        ]
        entry = code / ("tests/owned_worker_fixture.py" if controlled else "owned_worker.py")
        flags = ["-I", "-B", "-S"] if isolated else ["-B", "-S"]
        command = [sys.executable, *flags, str(entry)]
        for name, fd in zip(("code", "packages", "model"), fds):
            command.extend([f"--{name}-root-fd", str(fd)])
        process = subprocess.Popen(
            [*command, *extra],
            pass_fds=fds,
            cwd=ambient,
            env={**os.environ, "PYTHONPATH": str(ambient)},
            stdin=subprocess.PIPE,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )
        for fd in fds:
            os.close(fd)
        self.addAsyncCleanup(self.stop, process)
        return process

    async def stop(self, process):
        if process.stdin and not process.stdin.closed:
            process.stdin.close()
        try:
            await asyncio.to_thread(process.wait, timeout=3)
        except subprocess.TimeoutExpired:
            process.kill()
            await asyncio.to_thread(process.wait, timeout=3)
        for stream in (process.stdout, process.stderr):
            stream.close()

    async def call(self, process, ident, operation, payload):
        raw = json.dumps(
            {"version": 1, "exchange_id": ident, "operation": operation, "payload": payload}
        ).encode()
        process.stdin.write(struct.pack("!I", len(raw)) + raw)
        process.stdin.flush()

        def read():
            size = struct.unpack("!I", process.stdout.read(4))[0]
            self.assertLessEqual(size, 64 * 1024)
            return json.loads(process.stdout.read(size))

        result = await asyncio.wait_for(asyncio.to_thread(read), 3)
        self.assertEqual((result["exchange_id"], result["operation"]), (ident, operation))
        return result

    async def refusal(self, roots, code, **kwargs):
        process = self.launch(roots, **kwargs)
        process.stdin.close()
        await asyncio.to_thread(process.wait, timeout=3)
        self.assertEqual(process.returncode, 2)
        self.assertEqual(process.stdout.read(), b"")
        self.assertEqual(process.stderr.read().strip(), code.encode())

    async def test_actual_bootstrap_controlled_selected_load_use_unload(self):
        roots = self.stage()
        process = self.launch(roots, controlled=True)
        hello = (await self.call(process, 1, "hello", {}))["result"]
        self.assertFalse(hello["production_available"])
        runtime = hello["runtime_instance_id"]
        loaded = (
            await self.call(
                process,
                2,
                "load",
                {
                    "runtime_instance_id": runtime,
                    "model_id": "library/speech",
                    "source_id": "fixture-selected",
                },
            )
        )["result"]
        self.assertEqual(loaded["state"], "ready")
        identity = {"runtime_instance_id": runtime, "slot": loaded["slot"]}
        request = {
            "contract_version": 1,
            "request_id": "original",
            "model": "library/speech",
            "capability": "audio_transcription",
            "input": {
                "kind": "audio",
                "encoding": "pcm_s16le",
                "sample_rate_hz": 16000,
                "channels": 1,
                "sample_count": 1,
                "data_base64": "AAA=",
            },
            "output": "text",
            "options": {"kind": "audio", "language": "en"},
        }
        started = (await self.call(process, 3, "use", {**identity, "request": request}))["result"]
        settled = (
            await self.call(
                process,
                4,
                "status",
                {**identity, "operation_id": started["operation_id"], "wait_for_settlement": True},
            )
        )["result"]
        self.assertEqual(
            (settled["state"], settled["cleanup"], settled["finish_reason"]),
            ("completed", "confirmed", "stop"),
        )
        self.assertIn("calls=1;pcm=0000", settled["text"])
        retired = (await self.call(process, 5, "unload", identity))["result"]
        self.assertEqual((retired["state"], retired["cleanup"]), ("retired", "confirmed"))
        await self.call(process, 6, "close", {"runtime_instance_id": runtime})
        process.stdin.close()
        await asyncio.to_thread(process.wait, timeout=3)
        self.assertEqual(process.returncode, 0, process.stderr.read())
        self.assertEqual(process.stdout.read(), b"")

    async def test_shipping_unready_cannot_import_torch_from_environment_or_cwd(self):
        roots = self.stage()
        (roots[3] / "torch.py").write_text("raise AssertionError('ambient module executed')")
        await self.refusal(roots, "installed_runtime_unready")

    async def test_installed_sidecar_venv_is_unselected_and_cannot_supply_imports(self):
        roots = self.stage()
        venv = roots[0] / "venv"
        venv.mkdir()
        (venv / "__init__.py").write_text("raise AssertionError('unselected code executed')")
        (venv / "escape.py").write_text("raise AssertionError('unselected module executed')")
        (venv / "python").symlink_to(sys.executable)
        (venv / "unselected.pth").write_text("raise AssertionError('unselected hook executed')")
        packages = venv / "lib/python3.12/site-packages"
        packages.mkdir(parents=True)
        (packages / "selected_package.py").write_text("VALUE = 'selected'\n")
        roots = (roots[0], packages, roots[2], roots[3])
        process = self.launch(roots, controlled=True, extra=("--assert-excluded-venv",))
        hello = (await self.call(process, 1, "hello", {}))["result"]
        self.assertFalse(hello["production_available"])
        process.stdin.close()
        await asyncio.to_thread(process.wait, timeout=3)
        self.assertEqual(process.returncode, 0, process.stderr.read())
        self.assertEqual(process.stdout.read(), b"")

    async def test_linked_or_non_directory_code_role_venv_refuses(self):
        for linked in (False, True):
            roots = self.stage()
            venv = roots[0] / "venv"
            if linked:
                venv.symlink_to(roots[1], target_is_directory=True)
            else:
                venv.write_text("not a directory")
            await self.refusal(roots, "unsupported_runtime_member", controlled=True)

    async def test_channel_eof_retains_import_roots_through_native_owner_drain(self):
        roots = self.stage()
        (roots[0] / "retained_probe.py").write_text("VALUE = 'retained'\n")
        process = self.launch(
            roots,
            controlled=True,
            extra=("--assert-retained-after-close", "--hold-use-until-cancel"),
        )
        hello = (await self.call(process, 1, "hello", {}))["result"]
        loaded = (
            await self.call(
                process,
                2,
                "load",
                {
                    "runtime_instance_id": hello["runtime_instance_id"],
                    "model_id": "library/speech",
                    "source_id": "fixture-selected",
                },
            )
        )["result"]
        request = {
            "contract_version": 1,
            "request_id": "EOF-owner",
            "model": "library/speech",
            "capability": "audio_transcription",
            "input": {
                "kind": "audio",
                "encoding": "pcm_s16le",
                "sample_rate_hz": 16000,
                "channels": 1,
                "sample_count": 1,
                "data_base64": "AAA=",
            },
            "output": "text",
            "options": {"kind": "audio"},
        }
        await self.call(
            process,
            3,
            "use",
            {
                "runtime_instance_id": hello["runtime_instance_id"],
                "slot": loaded["slot"],
                "request": request,
            },
        )
        marker = await asyncio.wait_for(asyncio.to_thread(process.stderr.readline), 3)
        self.assertEqual(marker.strip(), b"controlled use entered")
        process.stdin.close()
        await asyncio.to_thread(process.wait, timeout=3)
        self.assertEqual(process.returncode, 0)
        self.assertEqual(process.stdout.read(), b"")
        self.assertIn(b"controlled roots retained after channel close", process.stderr.read())

    async def test_shipping_factory_with_controlled_installed_modules_keeps_gate_unavailable(self):
        roots = self.stage()
        (roots[1] / "torch.py").write_text(
            "import os\nos.write(1, b'import diagnostic\\n')\nclass device: pass\n"
        )
        (roots[1] / "psutil.py").write_text("# controlled import closure only\n")
        process = self.launch(roots)
        hello = (await self.call(process, 1, "hello", {}))["result"]
        self.assertFalse(hello["production_available"])
        refused = await self.call(
            process,
            2,
            "load",
            {
                "runtime_instance_id": hello["runtime_instance_id"],
                "model_id": "library/speech",
                "source_id": "fixture-selected",
            },
        )
        self.assertEqual(
            refused["error"], {"code": "native_runtime_unqualified", "effect": "not_admitted"}
        )
        process.stdin.close()
        await asyncio.to_thread(process.wait, timeout=3)
        self.assertEqual(process.returncode, 0)
        self.assertEqual(process.stdout.read(), b"")
        self.assertIn(b"import diagnostic", process.stderr.read())

    async def test_invalid_or_non_directory_descriptor_refuses(self):
        await self.refusal(
            self.stage(), "invalid_bootstrap_descriptor", extra=("--model-root-fd", "-1")
        )
        code, packages, model, ambient = self.stage()
        file = ambient / "not-directory"
        file.write_text("not authority")
        await self.refusal((code, packages, file, ambient), "invalid_bootstrap_descriptor")

    async def test_missing_or_linked_bootstrap_members_refuse(self):
        for linked in (False, True):
            roots = self.stage()
            member = roots[0] / "speech_operations.py"
            member.unlink()
            if linked:
                member.symlink_to(ROOT / "speech_operations.py")
            await self.refusal(
                roots, "unsupported_runtime_member" if linked else "missing_bootstrap_member"
            )

    async def test_automatic_site_hooks_and_bytecode_are_inert_and_not_model_inputs(self):
        for name in ("evil.pth", "sitecustomize.py", "usercustomize.py", "torch.pyc"):
            roots = self.stage()
            (roots[1] / name).write_text("raise AssertionError('hook executed')")
            process = self.launch(roots, controlled=True)
            hello = (await self.call(process, 1, "hello", {}))["result"]
            self.assertFalse(hello["production_available"])
            await self.call(
                process, 2, "close", {"runtime_instance_id": hello["runtime_instance_id"]}
            )
            process.stdin.close()
            await asyncio.to_thread(process.wait, timeout=3)
            self.assertEqual(process.returncode, 0, process.stderr.read())
            (roots[2] / name).write_text("unselected model member")
            await self.refusal(roots, "unsupported_runtime_member", controlled=True)

    async def test_unknown_or_missing_model_members_refuse_before_factory(self):
        roots = self.stage()
        (roots[2] / "config.json").unlink()
        await self.refusal(roots, "unsupported_model_read_set", controlled=True)
        roots = self.stage()
        (roots[2] / "external.py").write_text("raise AssertionError()")
        await self.refusal(roots, "unsupported_model_read_set", controlled=True)

    async def test_nonisolated_invocation_refuses(self):
        await self.refusal(self.stage(), "unsupported_bootstrap", isolated=False)

    async def test_cli_cannot_install_qualification_or_select_native_factory(self):
        process = self.launch(self.stage(), extra=("--qualified", "true"))
        process.stdin.close()
        await asyncio.to_thread(process.wait, timeout=3)
        self.assertEqual(process.returncode, 2)
        self.assertEqual(process.stdout.read(), b"")
        self.assertIn(b"unrecognized arguments", process.stderr.read())


class SourceOnlyImportTests(unittest.TestCase):
    def test_selected_source_ignores_otherwise_valid_tainted_bytecode(self):
        import importlib.util
        import py_compile
        from owned_worker import _SourceOnlyLoader

        with tempfile.TemporaryDirectory() as directory:
            source = Path(directory) / "selected.py"
            source.write_text("VALUE = 'evil'\n")
            py_compile.compile(
                str(source),
                doraise=True,
                invalidation_mode=py_compile.PycInvalidationMode.UNCHECKED_HASH,
            )
            self.assertTrue(Path(importlib.util.cache_from_source(str(source))).is_file())
            source.write_text("VALUE = 'good'\n")
            code = _SourceOnlyLoader("selected", str(source)).get_code("selected")
            namespace = {}
            exec(code, namespace)
            self.assertEqual(namespace["VALUE"], "good")

    def test_inert_namespace_components_cannot_supply_imports(self):
        from owned_worker import _inert_import_member

        for path in ("x.pth/module.py", "__pycache__/module.py", "sitecustomize.py/nested.py"):
            self.assertTrue(_inert_import_member(path))
        self.assertFalse(_inert_import_member("normal/package.py"))
