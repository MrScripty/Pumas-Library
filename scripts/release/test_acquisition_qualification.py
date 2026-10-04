"""Lightweight oracles for the non-empty acquisition qualification runner."""

import importlib.util
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location(
    "acquisition_runner", Path(__file__).with_name("test-acquisition-integration.py")
)
runner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(runner)


class QualificationTests(unittest.TestCase):
    def test_nonempty_listing_and_execution(self):
        listing = "tests::acquisition_integration_custody: test\n"
        count = runner.verify_listing(listing, 1)
        runner.verify_execution(
            "test result: ok. 1 passed; 0 failed; 0 ignored; 2 filtered out;", count
        )

    def test_empty_or_other_filters_refuse(self):
        for listing in ["", "tests::unrelated: test\n"]:
            with self.assertRaises(RuntimeError):
                runner.verify_listing(listing, 1)

    def test_ignored_incomplete_or_failed_execution_refuses(self):
        for result in [
            "test result: ok. 0 passed; 0 failed; 0 ignored;",
            "test result: ok. 0 passed; 0 failed; 1 ignored;",
            "test result: FAILED. 0 passed; 1 failed; 0 ignored;",
            "test result: ok. 1 passed; 0 failed; 0 ignored;",
        ]:
            with self.assertRaises(RuntimeError):
                runner.verify_execution(result, 2)


if __name__ == "__main__":
    unittest.main()
