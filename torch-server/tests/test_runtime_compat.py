"""The runtime compatibility check names every gap and covers the startup imports."""

import ast
import importlib.metadata
import importlib.util
import io
import json
import sys
from contextlib import redirect_stdout
from pathlib import Path
from types import SimpleNamespace
from unittest import TestCase
from unittest.mock import patch

SIDECAR = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("runtime_compat", SIDECAR / "runtime_compat.py")
runtime_compat = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runtime_compat)


def startup_third_party_imports() -> set[str]:
    """Top-level third-party modules imported, at module level, from serve.py on."""
    local = {path.stem for path in SIDECAR.glob("*.py")} | {"loaders"}
    pending, seen, third_party = ["serve"], set(), set()
    while pending:
        name = pending.pop()
        if name in seen:
            continue
        seen.add(name)
        tree = ast.parse((SIDECAR / f"{name}.py").read_text())
        for node in tree.body:
            if isinstance(node, ast.Import):
                modules = [alias.name for alias in node.names]
            elif isinstance(node, ast.ImportFrom) and node.level == 0 and node.module:
                modules = [node.module]
            else:
                continue
            for module in modules:
                top = module.split(".", 1)[0]
                if top in local:
                    pending.append(top)
                elif top != "__future__" and top not in sys.stdlib_module_names:
                    third_party.add(top)
    return third_party


class StartupRequirementTests(TestCase):
    def test_declared_requirements_are_exactly_the_startup_imports(self):
        declared = {name for name, _, _ in runtime_compat.STARTUP_REQUIREMENTS}
        self.assertEqual(declared, startup_third_party_imports())


class CheckTests(TestCase):
    def check_with(self, installed: dict[str, str], importable=None):
        importable = set(installed) if importable is None else importable

        def version(distribution):
            if distribution not in installed:
                raise importlib.metadata.PackageNotFoundError(distribution)
            return installed[distribution]

        def find_spec(name):
            return SimpleNamespace(name=name) if name in importable else None

        with (
            patch.object(runtime_compat.importlib.metadata, "version", side_effect=version),
            patch.object(runtime_compat.importlib.util, "find_spec", side_effect=find_spec),
        ):
            return runtime_compat.check()

    def current(self) -> dict[str, str]:
        return {
            "torch": "2.9.1+cu130",
            "fastapi": "0.110",
            "pydantic": "2.12.0",
            "uvicorn": "0.24.0",
            "psutil": "7.0.0",
        }

    def test_floors_and_newer_releases_are_accepted(self):
        self.assertEqual(self.check_with(self.current()), [])

    def test_each_gap_is_named(self):
        installed = self.current()
        del installed["uvicorn"]
        installed["fastapi"] = "0.100.0"
        installed["pydantic"] = "1.10.13"
        problems = self.check_with(installed)
        self.assertEqual(len(problems), 3, problems)
        self.assertIn("uvicorn is not installed", problems)
        self.assertTrue(any("fastapi 0.100.0 is older than 0.109.1" in p for p in problems))
        self.assertTrue(any("pydantic 1.10.13 is older than 2.0" in p for p in problems))

    def test_unimportable_and_unreadable_versions_are_named(self):
        installed = self.current() | {"psutil": "unknown"}
        problems = self.check_with(installed, importable=set(installed) - {"torch"})
        self.assertEqual(len(problems), 2, problems)
        self.assertIn("module torch cannot be found", problems[0])
        self.assertIn("not a release number", problems[1])

    def test_old_interpreter_is_refused_before_packages_are_read(self):
        with patch.object(runtime_compat.sys, "version_info", (3, 9, 18)):
            problems = runtime_compat.check()
        self.assertEqual(
            problems, ["Python 3.9 is older than 3.10, the oldest this sidecar supports"]
        )

    def test_main_prints_one_json_report(self):
        output = io.StringIO()
        with (
            patch.object(runtime_compat, "check", return_value=["uvicorn is not installed"]),
            redirect_stdout(output),
        ):
            runtime_compat.main()
        self.assertEqual(
            json.loads(output.getvalue()),
            {"compatible": False, "problems": ["uvicorn is not installed"]},
        )
