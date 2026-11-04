"""Supplemental managed-CPython license texts satisfy only verified, declared gaps."""

import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "managed_python_supplemental", Path(__file__).with_name("managed_python_supplemental.py")
)
supplemental = importlib.util.module_from_spec(spec)
spec.loader.exec_module(supplemental)

ROOT = Path(__file__).resolve().parents[2]
RELEASE = "20260901"
LINUX = "x86_64-unknown-linux-gnu"
WINDOWS = "x86_64-pc-windows-msvc"
MACOS = "aarch64-apple-darwin"
TARGETS = {LINUX, WINDOWS, MACOS}
CPYTHON = "licenses/LICENSE.cpython.txt"
ZSTD = "licenses/LICENSE.zstd.txt"
ZLIB_NG = "licenses/LICENSE.zlib-ng.txt"


def write_record(
    directory, declared_path, content, *, name=None, targets=(LINUX,), signed=None, overrides=None
):
    directory.mkdir(parents=True, exist_ok=True)
    if name is None:
        name = declared_path.removeprefix("licenses/LICENSE.").removesuffix(".txt") + "-LICENSE"
    (directory / name).write_bytes(content)
    signed = content if signed is None else signed
    record = {
        "schema_version": 1,
        "declared_path": declared_path,
        "supplemental_reason": supplemental.REASON,
        "note": "Fixture text supplied outside the archive.",
        "python_build_standalone_release": RELEASE,
        "pin_authority": (
            f"https://github.com/astral-sh/python-build-standalone/blob/{RELEASE}"
            "/pythonbuild/downloads.py"
        ),
        "targets": sorted(targets),
        "component": name,
        "version": "1.0",
        "license": "BSD-3-Clause",
        "source_archive": {
            "url": "https://example.invalid/source.tar.gz",
            "size": 10,
            "sha256": "b" * 64,
        },
        "path_in_source_archive": "source/LICENSE",
        "file": name,
        "size": len(signed),
        "sha256": hashlib.sha256(signed).hexdigest(),
        **(overrides or {}),
    }
    (directory / (name + supplemental.RECORD_SUFFIX)).write_text(json.dumps(record))
    return record


class SupplementalLicenseTests(unittest.TestCase):
    def load(self, directory):
        return supplemental.load_records(directory, RELEASE, TARGETS)

    def test_verified_text_satisfies_its_absent_declared_path(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            write_record(directory, ZSTD, b"zstd terms\n", targets=(LINUX, WINDOWS))
            records = self.load(directory)
            for target in (LINUX, WINDOWS):
                selected = supplemental.select(
                    records, target, {CPYTHON, ZSTD}, {CPYTHON}, strict=True
                )
                self.assertEqual([item["declared_path"] for item in selected], [ZSTD])
                self.assertEqual(selected[0]["text_path"].read_bytes(), b"zstd terms\n")

    def test_mismatched_hash_or_size_is_refused(self):
        for changes in ({"signed": b"original\n"}, {"overrides": {"size": 99}}):
            with self.subTest(changes=changes), tempfile.TemporaryDirectory() as tmp:
                write_record(Path(tmp), ZSTD, b"altered\n", **changes)
                with self.assertRaisesRegex(ValueError, "Changed managed CPython supplemental"):
                    self.load(Path(tmp))

    def test_unsupplied_or_mismatched_absent_path_still_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            write_record(directory, ZSTD, b"zstd terms\n", targets=(LINUX,))
            records = self.load(directory)
            for target, declared in (
                (LINUX, {CPYTHON, ZSTD, ZLIB_NG}),
                (WINDOWS, {CPYTHON, ZSTD}),
                (LINUX, {CPYTHON, "licenses/LICENSE.other.txt"}),
            ):
                for strict in (False, True):
                    with self.subTest(target=target, declared=declared, strict=strict):
                        with self.assertRaisesRegex(ValueError, "absent from the full archive"):
                            supplemental.select(records, target, declared, {CPYTHON}, strict=strict)

    def test_strict_selection_refuses_undeclared_or_archived_supplemental_paths(self):
        with tempfile.TemporaryDirectory() as tmp:
            directory = Path(tmp)
            write_record(directory, ZSTD, b"zstd terms\n")
            records = self.load(directory)
            for declared, archived in (({CPYTHON}, {CPYTHON}), ({CPYTHON, ZSTD}, {CPYTHON, ZSTD})):
                with self.assertRaisesRegex(ValueError, "not needed"):
                    supplemental.select(records, LINUX, declared, archived, strict=True)
                self.assertEqual(
                    supplemental.select(records, LINUX, declared, archived, strict=False), []
                )

    def test_records_bind_release_target_shape_and_reject_orphans_or_overlaps(self):
        invalid = (
            {"python_build_standalone_release": "20261001"},
            {"targets": ["riscv64-unknown-linux-gnu"]},
            {"targets": [LINUX, WINDOWS]},
            {"declared_path": "../LICENSE.zstd.txt"},
            {"supplemental_reason": "guess"},
            {"source_archive": {"url": "http://example.invalid/a", "size": 1, "sha256": "b" * 64}},
            {"extra": True},
        )
        for changes in invalid:
            with self.subTest(changes=changes), tempfile.TemporaryDirectory() as tmp:
                write_record(Path(tmp), ZSTD, b"terms\n", overrides=changes)
                with self.assertRaisesRegex(ValueError, "Invalid managed CPython supplemental"):
                    self.load(Path(tmp))
        with tempfile.TemporaryDirectory() as tmp:
            write_record(Path(tmp), ZSTD, b"terms\n")
            (Path(tmp) / "unexplained-LICENSE").write_bytes(b"terms\n")
            with self.assertRaisesRegex(ValueError, "without provenance"):
                self.load(Path(tmp))
        with tempfile.TemporaryDirectory() as tmp:
            write_record(Path(tmp), ZSTD, b"terms\n", name="zstd-a-LICENSE")
            write_record(Path(tmp), ZSTD, b"terms\n", name="zstd-b-LICENSE")
            with self.assertRaisesRegex(ValueError, "Duplicate supplemental license"):
                self.load(Path(tmp))

    def test_checked_in_records_cover_the_retained_archive_gaps(self):
        records = self.load(ROOT / supplemental.DIRECTORY)
        evidence = (
            ROOT / "docs/plans/torch-cross-platform-runtime-management/reports"
            "/managed-python-license-collection"
        )
        for folder, target, absent in (
            ("linux-x86_64-cpython-3.14.7", LINUX, [ZLIB_NG, ZSTD]),
            ("macos-arm64-cpython-3.14.7", MACOS, [ZLIB_NG, ZSTD]),
            ("windows-x86_64-cpython-3.14.7", WINDOWS, [ZSTD]),
        ):
            with self.subTest(target=target):
                retained = evidence / folder / "managed-python-licenses"
                manifest = json.loads((retained / "full-archive-manifest.json").read_bytes())
                python = json.loads((retained / "full-archive/PYTHON.json").read_bytes())
                declared = supplemental.declared_license_paths(python)
                archived = {item["path"] for item in manifest["files"]}
                self.assertEqual(sorted(declared - archived), absent)
                selected = supplemental.select(records, target, declared, archived, strict=True)
                self.assertEqual([item["declared_path"] for item in selected], absent)


if __name__ == "__main__":
    unittest.main()
