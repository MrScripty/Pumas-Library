"""Finite recipe/catalog/preflight controls; no remote fetch or source preparation."""

import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
import zipfile
from unittest.mock import patch

import test_install_verified_wheels as fixtures

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "qualified_catalog", ROOT / "qualified_wheel_catalog.py"
)
catalog = importlib.util.module_from_spec(spec)
spec.loader.exec_module(catalog)


def lock_for(artifacts):
    return (
        "\n".join(f"{a['name']}=={a['version']} --hash=sha256:{a['sha256']}" for a in artifacts)
        + "\n"
    )


def index_rows(artifacts):
    return [{"url": a["url"], "hashes": {"sha256": a["sha256"]}} for a in artifacts]


class QualifiedCatalogTests(unittest.TestCase):
    def setUp(self):
        self.directory = tempfile.TemporaryDirectory()
        self.addCleanup(self.directory.cleanup)
        self.root = Path(self.directory.name)
        self.wheels = self.root / "wheels"
        self.wheels.mkdir()
        self.artifacts = [
            fixtures.make_wheel(
                self.wheels, "root-wheel", requires=["dependency-wheel[fast]>=1,<2"]
            ),
            fixtures.make_wheel(
                self.wheels,
                "dependency-wheel",
                extras=["fast"],
                requires=["leaf-wheel; extra == 'fast'"],
            ),
            fixtures.make_wheel(self.wheels, "leaf-wheel"),
        ]
        for artifact in self.artifacts:
            filename = artifact["url"].rsplit("/", 1)[-1]
            artifact["url"] = "https://files.pythonhosted.org/packages/" + filename
        self.lock = lock_for(self.artifacts)

    def fetch(self, url):
        return index_rows(self.artifacts)

    def invoke(self, artifacts, lock=None):
        lock = self.lock if lock is None else lock
        resolution = {
            "format": "pumas-qualified-wheel-catalog-1",
            "recipe_lock_sha256": hashlib.sha256(lock.encode()).hexdigest(),
            "artifacts": artifacts,
        }
        (self.root / "resolution.json").write_text(json.dumps(resolution))
        (self.root / "lock").write_text(lock)
        (self.root / "preview.json").write_text(
            json.dumps({"requirementsLock": lock, "directArtifacts": []})
        )
        return subprocess.run(
            [
                sys.executable,
                "-I",
                str(ROOT / "install_verified_wheels.py"),
                "--resolution",
                str(self.root / "resolution.json"),
                "--recipe-lock",
                str(self.root / "lock"),
                "--preview",
                str(self.root / "preview.json"),
                "--wheels",
                str(self.wheels),
                "--target",
                str(self.root / "packages"),
                "--output",
                str(self.root / "proof"),
            ],
            capture_output=True,
            text=True,
            timeout=30,
        )

    def test_legacy_noninstall_resolver_starts_without_copied_record_owner(self):
        isolated = self.root / "resolve_runtime.py"
        isolated.write_bytes((ROOT / "resolve_runtime.py").read_bytes())
        completed = subprocess.run(
            [sys.executable, "-I", str(isolated), "--help"],
            capture_output=True,
            text=True,
            timeout=10,
        )
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertFalse((self.root / "wheel_records.py").exists())

    def test_existing_recipe_is_finite_and_hash_pinned_without_network(self):
        original = (ROOT / "runtime" / "requirements.lock").read_text()
        entries = catalog.parse_lock(original)
        self.assertEqual(len(entries), 66)
        self.assertEqual(sum(e["url"] is not None for e in entries), 3)
        self.assertTrue(all(e["hashes"] for e in entries))
        self.assertEqual(
            {e["name"] for e in entries if e["url"]}, {"torch", "torchvision", "nunchaku"}
        )

    def test_complete_named_catalog_and_actual_local_closure_with_extras_install(self):
        with patch.object(
            subprocess, "run", side_effect=AssertionError("catalog must not invoke pip")
        ):
            result = catalog.catalog(self.lock, [], self.fetch)
        self.assertEqual(result["artifacts"], self.artifacts)
        completed = self.invoke(result["artifacts"])
        self.assertEqual(completed.returncode, 0, completed.stderr)
        self.assertEqual(len(list((self.root / "packages").glob("*.dist-info"))), 3)
        self.assertTrue((self.root / "proof" / "installed-files.json").is_file())
        self.assertEqual((self.root / "lock").read_text(), self.lock)
        report = json.loads((self.root / "proof" / "local-pip-report.json").read_text())
        self.assertTrue(
            all(
                item["download_info"]["url"].startswith(self.wheels.as_uri())
                for item in report["install"]
            )
        )

    def test_catalog_never_fetches_dependency_or_payload_urls(self):
        called = []

        def fetch(url):
            called.append(url)
            return self.fetch(url)

        catalog.catalog(self.lock, [], fetch)
        self.assertEqual(len(called), 6)
        self.assertTrue(all(url.startswith(catalog.INDEXES) for url in called))
        self.assertTrue(all(not url.endswith(".whl") for url in called))

    def test_wrong_hash_foreign_source_and_source_archives_never_enter_catalog(self):
        for replacement in [
            {**self.artifacts[0], "sha256": "a" * 64},
            {**self.artifacts[0], "url": "https://example.invalid/root_wheel-1.0-py3-none-any.whl"},
            {
                **self.artifacts[0],
                "url": "https://files.pythonhosted.org/packages/root_wheel-1.0.tar.gz",
            },
        ]:
            with self.subTest(replacement=replacement):
                rows = index_rows([replacement, *self.artifacts[1:]])
                with self.assertRaisesRegex(ValueError, "inconclusive"):
                    catalog.catalog(self.lock, [], lambda url: rows)

    def test_ambiguous_approved_sources_refuse_without_selection_fallback(self):
        duplicate = {
            **self.artifacts[0],
            "url": self.artifacts[0]["url"].replace("/packages/", "/packages/other/"),
        }
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            catalog.catalog(self.lock, [], lambda url: index_rows([*self.artifacts, duplicate]))

    def test_unsupported_lock_options_versions_extras_and_hashes_refuse_before_fetch(self):
        for lock in [
            "--requirement https://example.invalid/hostile\n",
            "x>=1 --hash=sha256:" + "a" * 64,
            "x[extra]==1 --hash=sha256:" + "a" * 64,
            "x==1 --hash=md5:" + "a" * 32,
        ]:
            with (
                self.subTest(lock=lock),
                patch.object(catalog, "fetch_index", side_effect=AssertionError("fetch reached")),
            ):
                with self.assertRaises(ValueError):
                    catalog.catalog(lock, [], lambda url: self.fail("fetch reached"))

    def test_acquired_pin_hash_or_complete_set_difference_refuses_before_install(self):
        variants = [
            self.artifacts[:-1],
            [{**self.artifacts[0], "sha256": "a" * 64}, *self.artifacts[1:]],
            [{**self.artifacts[0], "version": "2.0"}, *self.artifacts[1:]],
        ]
        for artifacts in variants:
            with self.subTest(artifacts=artifacts):
                completed = self.invoke(artifacts)
                self.assertEqual(completed.returncode, 3)
                self.assertFalse((self.root / "packages").exists())

    def test_actual_dependency_urls_even_inactive_refuse_without_pip_or_locator_diagnostic(self):
        for reference in [
            "hidden @ https://user:synthetic@fixture.invalid/hidden.tar.gz",
            "hidden @ git+https://fixture.invalid/repo.git",
            "hidden @ https://fixture.invalid/hidden.whl ; python_version < '0'",
        ]:
            with self.subTest(reference=reference):
                for path in self.wheels.iterdir():
                    path.unlink()
                artifact = fixtures.make_wheel(self.wheels, "root-wheel", requires=[reference])
                artifact["url"] = (
                    "https://files.pythonhosted.org/packages/" + artifact["url"].rsplit("/", 1)[-1]
                )
                completed = self.invoke([artifact], lock_for([artifact]))
                self.assertEqual(completed.returncode, 3, completed.stderr)
                self.assertIn("Dependency direct URLs are unsupported", completed.stderr)
                self.assertNotIn("fixture.invalid", completed.stderr)
                self.assertNotIn("synthetic", completed.stderr)
                self.assertFalse((self.root / "packages").exists())

    def test_actual_metadata_identity_python_and_wheel_tags_refuse_before_install(self):
        for kind in ["identity", "python", "tags"]:
            with self.subTest(kind=kind):
                artifact = fixtures.make_wheel(self.wheels, "root-wheel")
                artifact["url"] = (
                    "https://files.pythonhosted.org/packages/" + artifact["url"].rsplit("/", 1)[-1]
                )
                path = self.wheels / artifact["url"].rsplit("/", 1)[-1]
                with zipfile.ZipFile(path) as archive:
                    entries = {name: archive.read(name) for name in archive.namelist()}
                metadata = next(name for name in entries if name.endswith("/METADATA"))
                wheel = next(name for name in entries if name.endswith("/WHEEL"))
                if kind == "identity":
                    entries[metadata] = entries[metadata].replace(b"Version: 1.0", b"Version: 2.0")
                elif kind == "python":
                    entries[metadata] += b"Requires-Python: >=99\n"
                else:
                    entries[wheel] = entries[wheel].replace(
                        b"Tag: py3-none-any", b"Tag: cp27-none-any"
                    )
                with zipfile.ZipFile(path, "w") as archive:
                    for name, body in entries.items():
                        archive.writestr(name, body)
                artifact["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
                # The fixture's allowed digest matches: actual metadata must reject.
                for other in self.wheels.iterdir():
                    if other != path:
                        other.unlink()
                completed = self.invoke([artifact], lock_for([artifact]))
                self.assertEqual(completed.returncode, 3, completed.stderr)
                self.assertFalse((self.root / "packages").exists())

    def test_missing_active_extra_closure_refuses_even_when_recipe_set_is_complete(self):
        artifacts = self.artifacts[:-1]
        (self.wheels / self.artifacts[-1]["url"].rsplit("/", 1)[-1]).unlink()
        completed = self.invoke(artifacts, lock_for(artifacts))
        self.assertEqual(completed.returncode, 3)
        self.assertFalse((self.root / "packages").exists())

    def test_direct_roots_preserve_fragment_digest_and_exact_source(self):
        entries = catalog.parse_lock((ROOT / "runtime" / "requirements.lock").read_text())
        roots = [
            {"name": e["name"], "version": e["version"], "url": e["url"], "sha256": e["hashes"][0]}
            for e in entries
            if e["url"]
        ]
        roots[-1]["url"] += "#sha256=" + roots[-1]["sha256"]
        catalog.validate_roots(entries, roots)
        for field, value in [
            ("url", "https://example.invalid/root.whl"),
            ("sha256", "a" * 64),
            ("version", "0.0"),
        ]:
            changed = [{**r} for r in roots]
            changed[0][field] = value
            with self.assertRaisesRegex(ValueError, "identity"):
                catalog.validate_roots(entries, changed)

    def test_changed_acquired_bytes_refuse_before_public_pip(self):
        artifact = self.artifacts[0]
        filename = artifact["url"].rsplit("/", 1)[-1]
        (self.wheels / filename).write_bytes(b"changed")
        completed = self.invoke(self.artifacts)
        self.assertEqual(completed.returncode, 3)
        self.assertFalse((self.root / "packages").exists())


if __name__ == "__main__":
    unittest.main()
