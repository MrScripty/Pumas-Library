"""Installed Python import closure only; no Rust execution or native qualification."""

import os
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest


REPOSITORY = Path(__file__).resolve().parents[2]
EMBEDDED_SIDECAR = REPOSITORY / "rust/crates/pumas-core/src/runtime_profiles/torch_sidecar.rs"

# Run in a fresh isolated interpreter. Only Torch is substituted: startup must
# import every actual local module from the temporary tree built from the
# sidecar table compiled into Pumas, the tree every Torch runtime launches.
# No inference, listener, download, model load or repository import is permitted.
IMPORT_PROBE = r"""
import importlib
from pathlib import Path
import sys
import types

root = Path(sys.argv[1]).resolve()
assert sys.flags.isolated
sys.dont_write_bytecode = True
sys.path.insert(0, str(root))
assert not {"serve", "model_manager", "audio_api"}.intersection(sys.modules)
torch = types.ModuleType("torch")
class Device:
    def __init__(self, value):
        self.type = str(value).split(":", 1)[0]
        self.value = value
    def __str__(self):
        return str(self.value)
torch.device = Device
torch.cuda = types.SimpleNamespace(is_available=lambda: False, device_count=lambda: 0)
torch.backends = types.SimpleNamespace(mps=types.SimpleNamespace(is_available=lambda: False))
sys.modules["torch"] = torch
serve = importlib.import_module("serve")
app = serve.create_app()
assert serve.TORCH_PROTOCOL == 3
assert serve.TORCH_CAPABILITIES == ("image_generation", "audio_transcription")
for name in (
    "serve", "control_api", "device_manager", "model_manager", "audio_api",
    "audio_input", "audio_contract", "image_api", "openai_api", "diffusion", "validation",
):
    module = sys.modules[name]
    assert Path(module.__file__).resolve().is_relative_to(root), (name, module.__file__)
assert "loaders.cohere_asr_loader" not in sys.modules
print("EMBEDDED_IMAGE_TEXT_IMPORT_CLOSURE_OK")
"""


class EmbeddedRuntimeImportTests(unittest.TestCase):
    def embedded_python_sources(self):
        source = EMBEDDED_SIDECAR.read_text()
        # Read the actual embedded-file table, rather than maintaining a second
        # hand-picked runtime file list that could omit a transitive import.
        table = source.split("const SIDECAR_FILES: &[(&str, &str)] = &[", 1)[1]
        table = table.split("\n];", 1)[0]
        included = re.findall(
            r'\(\s*"([^"\n]+)"\s*,\s*include_str!\("([^"\n]+)"\)\s*,?\s*\)', table
        )
        paths = {
            name: (EMBEDDED_SIDECAR.parent / relative).resolve() for name, relative in included
        }
        expected_python = set(re.findall(r'\(\s*"([^"\n]+\.py)"\s*,', table))
        actual_python = {name: path for name, path in paths.items() if name.endswith(".py")}
        self.assertEqual(
            set(actual_python), expected_python, "Unrecognized Python embedding syntax"
        )
        self.assertIn("serve.py", actual_python)
        self.assertIn("model_manager.py", actual_python)
        for name in (
            "audio_api.py",
            "audio_input.py",
            "audio_contract.py",
            "loaders/cohere_asr_loader.py",
            "runtime_compat.py",
            "resolve_runtime.py",
            "probe_runtime.py",
        ):
            self.assertIn(name, actual_python)
        # Every sidecar source is embedded; none is left behind.
        sidecar = REPOSITORY / "torch-server"
        shipped = {
            path.relative_to(sidecar).as_posix()
            for path in [*sidecar.glob("*.py"), *(sidecar / "loaders").glob("*.py")]
        }
        self.assertEqual(set(actual_python), shipped)
        self.assertEqual(paths["LICENSE"], REPOSITORY / "LICENSE")
        return actual_python

    def probe(self, root):
        # Deliberately offer the checkout as PYTHONPATH: -I must ignore it, even
        # when an embedded file is missing. CWD is the temporary install tree.
        return subprocess.run(
            [sys.executable, "-I", "-c", IMPORT_PROBE, str(root)],
            cwd=root,
            env={**os.environ, "PYTHONPATH": str(REPOSITORY / "torch-server")},
            capture_output=True,
            text=True,
            timeout=15,
            check=False,
        )

    def stage(self, root):
        for name, source in self.embedded_python_sources().items():
            destination = root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(source.read_bytes())

    def test_embedded_image_text_startup_imports_only_installed_local_modules(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.stage(root)
            result = self.probe(root)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("EMBEDDED_IMAGE_TEXT_IMPORT_CLOSURE_OK", result.stdout)

    def test_missing_binding_cannot_fall_back_to_repository_pythonpath(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            self.stage(root)
            (root / "audio_input.py").unlink()
            result = self.probe(root)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("No module named 'audio_input'", result.stderr)
        self.assertNotIn("EMBEDDED_IMAGE_TEXT_IMPORT_CLOSURE_OK", result.stdout)


if __name__ == "__main__":
    unittest.main()
