"""Real tiny wheel consumption controls; no resolver/network/runtime downloads."""

import base64
import hashlib
import importlib.util
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location(
    "install_verified_wheels", ROOT / "install_verified_wheels.py"
)
consumer = importlib.util.module_from_spec(spec)
spec.loader.exec_module(consumer)


def make_wheel(root, name, *, requires=(), extras=(), version="1.0", content=b"VALUE = 7\n"):
    filename = f"{name.replace('-', '_')}-{version}-py3-none-any.whl"
    metadata = f"Metadata-Version: 2.1\nName: {name}\nVersion: {version}\n"
    metadata += "".join(f"Requires-Dist: {value}\n" for value in requires)
    metadata += "".join(f"Provides-Extra: {value}\n" for value in extras)
    directory = f"{name.replace('-', '_')}-{version}.dist-info"
    files = {
        f"{name.replace('-', '_')}.py": content,
        f"{directory}/METADATA": metadata.encode(),
        f"{directory}/WHEEL": b"Wheel-Version: 1.0\nGenerator: pumas-fixture\nRoot-Is-Purelib: true\nTag: py3-none-any\n",
    }
    rows = []
    for path, data in files.items():
        digest = base64.urlsafe_b64encode(hashlib.sha256(data).digest()).decode().rstrip("=")
        rows.append(f"{path},sha256={digest},{len(data)}\n")
    rows.append(f"{directory}/RECORD,,\n")
    files[f"{directory}/RECORD"] = "".join(rows).encode()
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as wheel:
        for path, data in files.items():
            wheel.writestr(path, data)
    body = buffer.getvalue()
    (root / filename).write_bytes(body)
    return {
        "name": name,
        "version": version,
        "url": f"https://fixture.invalid/{filename}",
        "sha256": hashlib.sha256(body).hexdigest(),
    }


class LocalWheelTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.wheels = self.root / "wheels"
        self.wheels.mkdir()
        self.target, self.output = self.root / "packages", self.root / "proof"

    def refuse(self, artifacts):
        with patch.object(
            consumer.subprocess, "run", side_effect=AssertionError("pip must not start")
        ):
            with self.assertRaises(ValueError):
                consumer.install(artifacts, self.wheels, self.target, self.output)
        self.assertFalse(self.target.exists())
        self.assertFalse((self.output / "installed-files.json").exists())

    def test_actual_dependent_wheels_install_with_exact_local_report_and_record_proof(self):
        artifacts = [
            make_wheel(self.wheels, "root-wheel", requires=["dependency-wheel==1.0"]),
            make_wheel(self.wheels, "dependency-wheel"),
        ]
        manifest = consumer.install(artifacts, self.wheels, self.target, self.output)
        self.assertTrue(manifest["files"])
        self.assertEqual(
            {p.name for p in self.target.glob("*.dist-info")},
            {"root_wheel-1.0.dist-info", "dependency_wheel-1.0.dist-info"},
        )
        requirements = (self.output / "local-requirements.txt").read_text()
        self.assertIn(" @ file://", requirements)
        self.assertNotIn("https://", requirements)
        report = json.loads((self.output / "local-pip-report.json").read_text())
        self.assertTrue(
            all(item["download_info"]["url"].startswith("file:") for item in report["install"])
        )

    def test_missing_required_closure_refuses_before_pip(self):
        self.refuse([make_wheel(self.wheels, "root-wheel", requires=["missing-wheel==1.0"])])

    def test_wrong_dependency_version_refuses_before_pip(self):
        self.refuse(
            [
                make_wheel(self.wheels, "root-wheel", requires=["dependency-wheel>=2"]),
                make_wheel(self.wheels, "dependency-wheel"),
            ]
        )

    def test_missing_selected_file_refuses_before_pip(self):
        artifact = make_wheel(self.wheels, "root-wheel")
        next(self.wheels.iterdir()).unlink()
        self.refuse([artifact])

    def test_alternate_same_name_version_bytes_refuse_before_pip(self):
        original = make_wheel(self.wheels, "root-wheel")
        make_wheel(self.wheels, "root-wheel", content=b"SUBSTITUTED = True\n")
        self.refuse([original])

    def test_hidden_direct_url_dependency_refuses_before_pip(self):
        self.refuse(
            [
                make_wheel(
                    self.wheels,
                    "root-wheel",
                    requires=["missing-wheel @ https://fixture.invalid/hidden.whl"],
                )
            ]
        )

    def test_inactive_direct_url_is_also_refused(self):
        self.refuse(
            [
                make_wheel(
                    self.wheels,
                    "root-wheel",
                    requires=[
                        'missing-wheel @ https://fixture.invalid/hidden.whl ; python_version < "0"'
                    ],
                )
            ]
        )

    def test_inactive_marker_does_not_require_a_missing_distribution(self):
        artifacts = [
            make_wheel(
                self.wheels, "root-wheel", requires=['missing-wheel==1.0; python_version < "0"']
            )
        ]
        lines, _ = consumer.local_requirements(artifacts, self.wheels)
        self.assertEqual(len(lines), 1)

    def test_transitive_extras_require_the_full_activated_closure(self):
        root = make_wheel(self.wheels, "root-wheel", requires=["dependency-wheel[images]==1.0"])
        dependency = make_wheel(
            self.wheels,
            "dependency-wheel",
            extras=["images"],
            requires=['image-wheel==1.0; extra == "images"'],
        )
        self.refuse([root, dependency])
        image = make_wheel(self.wheels, "image-wheel")
        lines, _ = consumer.local_requirements([root, dependency, image], self.wheels)
        self.assertEqual(len(lines), 3)

    def test_required_extra_must_be_provided(self):
        self.refuse(
            [
                make_wheel(self.wheels, "root-wheel", requires=["dependency-wheel[missing]==1.0"]),
                make_wheel(self.wheels, "dependency-wheel"),
            ]
        )

    def test_extra_unselected_file_refuses_before_pip(self):
        artifact = make_wheel(self.wheels, "root-wheel")
        make_wheel(self.wheels, "unselected-wheel")
        self.refuse([artifact])

    def test_duplicate_distribution_refuses_before_pip(self):
        artifact = make_wheel(self.wheels, "root-wheel")
        self.refuse([artifact, artifact])

    def test_linked_wheel_refuses_before_pip(self):
        artifact = make_wheel(self.wheels, "root-wheel")
        path = next(self.wheels.iterdir())
        outside = self.root / "external.whl"
        path.rename(outside)
        path.symlink_to(outside)
        self.refuse([artifact])

    def test_selected_interpreter_rejects_incompatible_wheel_tags(self):
        artifact = make_wheel(self.wheels, "root-wheel")
        old = next(self.wheels.iterdir())
        new = old.with_name(old.name.replace("py3-none-any", "cp299-cp299-win_amd64"))
        old.rename(new)
        artifact["url"] = f"https://fixture.invalid/{new.name}"
        self.refuse([artifact])

    def test_actual_pip_report_must_have_supported_version(self):
        for version in (None, "2", 1):
            with self.subTest(version=version), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                wheels = root / "wheels"
                wheels.mkdir()
                artifacts = [make_wheel(wheels, "root-wheel")]
                run = consumer.subprocess.run

                def changed_report(*args, **kwargs):
                    result = run(*args, **kwargs)
                    report = root / "proof/local-pip-report.json"
                    data = json.loads(report.read_text())
                    if version is None:
                        data.pop("version")
                    else:
                        data["version"] = version
                    report.write_text(json.dumps(data))
                    return result

                with patch.object(consumer.subprocess, "run", side_effect=changed_report):
                    with self.assertRaisesRegex(
                        ValueError, "Unsupported local pip installation report version"
                    ):
                        consumer.install(artifacts, wheels, root / "packages", root / "proof")
                self.assertFalse((root / "proof/installed-files.json").exists())

    def test_distribution_canonicalization_accepts_existing_dotted_names(self):
        artifact = make_wheel(self.wheels, "dotted.name")
        lines, _ = consumer.local_requirements([artifact], self.wheels)
        self.assertTrue(lines[0].startswith("dotted-name @ file:"))


if __name__ == "__main__":
    unittest.main()
