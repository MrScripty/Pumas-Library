import importlib.util
from pathlib import Path
import tempfile
import unittest

spec = importlib.util.spec_from_file_location("location_acceptance", Path(__file__).with_name("download_location_reconciliation.py"))
module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(module)


class FixtureTests(unittest.TestCase):
    def test_authored_shapes_have_exact_selected_files_and_no_original_destinations(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            cases, files = module.author_fixture(root)
            self.assertEqual(len(cases), 3)
            self.assertEqual(len(files), 38)
            self.assertEqual(len({file for file in files}), 38)
            self.assertTrue(all(file.read_bytes() == b"fixture" for file in files))
            data = module.json.loads((root / "launcher-data/downloads.json").read_text())
            self.assertEqual([len(entry["filenames"]) for entry in data["downloads"]], [2, 3, 33, 1, 1])
            self.assertTrue(all(not Path(entry["dest_dir"]).exists() for entry in data["downloads"]))
            self.assertEqual(data["consumer_receipts"], {})
            self.assertEqual(data["acquisitions"], {})


if __name__ == "__main__":
    unittest.main()
