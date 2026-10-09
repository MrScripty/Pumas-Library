"""Legal discovery controls for the vendored JPEG codec (no Cargo calls)."""

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


class VendoredJpegLegalTests(unittest.TestCase):
    def test_bundled_ijg_readme_is_required_and_collected_without_general_readmes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            native = root / "libjpeg-turbo"
            native.mkdir()
            bsd = native / "LICENSE.md"
            bsd.write_text("BSD terms reference README.ijg")
            ijg = native / "README.ijg"
            ijg.write_text("IJG copyright and redistribution terms")
            (root / "README.md").write_text("wrapper instructions")
            (root / "README.ijg").write_text("unrelated file")
            self.assertEqual(notices.rust_license_files("turbojpeg-sys", root), [bsd, ijg])
            self.assertEqual(notices.rust_license_files("another-package", root), [bsd])
            ijg.unlink()
            with self.assertRaisesRegex(ValueError, "Missing bundled JPEG legal text"):
                notices.rust_license_files("turbojpeg-sys", root)
            ijg.write_text("IJG copyright and redistribution terms")
            bsd.unlink()
            with self.assertRaisesRegex(ValueError, "Missing bundled JPEG legal text"):
                notices.rust_license_files("turbojpeg-sys", root)

    def test_wrapper_text_is_bound_to_published_crate_revision(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            revision = "a" * 40
            (root / ".cargo_vcs_info.json").write_text(json.dumps({"git": {"sha1": revision}}))
            sources = [
                {
                    "file": "turbojpeg-sys-LICENSE",
                    "source": f"https://raw.githubusercontent.com/honzasp/rust-turbojpeg/{revision}/LICENSE",
                }
            ]
            self.assertEqual(
                notices.jpeg_wrapper_license(root, sources),
                notices.LICENSES / "turbojpeg-sys-LICENSE",
            )
            sources[0]["source"] = sources[0]["source"].replace(revision, "b" * 40)
            with self.assertRaisesRegex(ValueError, "Upstream license revision changed"):
                notices.jpeg_wrapper_license(root, sources)


if __name__ == "__main__":
    unittest.main()
