"""Bounded legal-text coverage for the bundled JPEG dependency."""

import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location(
    "generate_notices", Path(__file__).with_name("generate-notices.py")
)
notices = importlib.util.module_from_spec(spec)
spec.loader.exec_module(notices)


class JpegAttributionTests(unittest.TestCase):
    def test_ijg_readme_is_required_and_not_duplicated(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            bundled = root / "libjpeg-turbo"
            bundled.mkdir()
            (bundled / "LICENSE.md").write_text("License references README.ijg")
            with self.assertRaisesRegex(ValueError, "README.ijg"):
                notices.rust_license_files("turbojpeg-sys", root)
            (bundled / "README.ijg").write_text("IJG legal text")
            (bundled / "README.md").write_text("Not legal text")
            files = notices.rust_license_files("turbojpeg-sys", root)
            self.assertEqual(files, [bundled / "LICENSE.md", bundled / "README.ijg"])
            self.assertEqual(
                notices.rust_license_files("unrelated", root), [bundled / "LICENSE.md"]
            )

    def test_wrapper_license_requires_exact_published_revision(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            revision = "a" * 40
            (root / ".cargo_vcs_info.json").write_text(json.dumps({"git": {"sha1": revision}}))
            source = {
                "file": "turbojpeg-sys-LICENSE",
                "source": f"https://raw.githubusercontent.com/honzasp/rust-turbojpeg/{revision}/LICENSE",
            }
            self.assertEqual(
                notices.jpeg_wrapper_license(root, [source]),
                notices.LICENSES / "turbojpeg-sys-LICENSE",
            )
            source["source"] += "?unreviewed=1"
            with self.assertRaisesRegex(ValueError, "revision changed"):
                notices.jpeg_wrapper_license(root, [source])

    def test_checked_wrapper_text_matches_recorded_digest(self):
        sources = json.loads((notices.LICENSES / "sources.json").read_text())
        source = next(item for item in sources if item["file"] == "turbojpeg-sys-LICENSE")
        self.assertEqual(
            notices.digest((notices.LICENSES / source["file"]).read_bytes()),
            source["sha256"],
        )


if __name__ == "__main__":
    unittest.main()
