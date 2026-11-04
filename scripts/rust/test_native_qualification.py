#!/usr/bin/env python3
"""Self-test for the native qualification runner's matching and exit handling."""

import subprocess
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

sys.path.insert(0, str(Path(__file__).resolve().parent))

import native_qualification as nq  # noqa: E402

LISTING = """\
integration_tests::http_admission_rejects_untrusted_sources_before_rpc_dispatch: test
http_transport::tests::drains: test
http_transport::tests::ignored_case: test

3 tests, 0 benchmarks
"""
IGNORED = "http_transport::tests::ignored_case: test\n"


def completed(stdout="", returncode=0):
    return subprocess.CompletedProcess([], returncode, stdout=stdout, stderr="")


class MatchingTests(unittest.TestCase):
    def test_host_names_follow_the_runner_os(self):
        self.assertEqual(nq.host_name("Darwin"), "macOS")
        self.assertEqual(nq.host_name("Windows"), "Windows")
        self.assertEqual(nq.host_name("Linux"), "Linux")

    def test_listing_keeps_only_test_names(self):
        self.assertEqual(len(nq.listed_tests(LISTING)), 3)
        self.assertIn("http_transport::tests::drains", nq.listed_tests(LISTING))

    def test_a_filter_matching_only_ignored_tests_is_unmatched(self):
        runnable = nq.listed_tests(LISTING) - nq.listed_tests(IGNORED)
        self.assertEqual(
            nq.unmatched_filters(["ignored_case", "drains"], runnable), ["ignored_case"]
        )

    def test_workspace_custody_and_cleanup_run_on_every_native_host(self):
        for text in ("acquisition::workspace::tests", "native_"):
            hosts = {
                item.hosts for group in nq.GROUPS for item in group.filters if item.text == text
            }
            self.assertEqual(hosts, {nq.ALL}, text)

    def test_every_shard_runs_something_on_every_host(self):
        for shard in nq.SHARDS:
            for host in nq.HOSTS:
                filters = [
                    text
                    for group in nq.GROUPS
                    if group.shard == shard
                    for text in nq.active_filters(group, host)
                ]
                self.assertTrue(filters, f"{shard} on {host}")


class RunGroupTests(unittest.TestCase):
    group = nq.Group(
        "default-features",
        "RPC",
        "pumas-rpc",
        nq.TESTS,
        (nq.f("http_transport::tests"), nq.f("http_admission_rejects")),
    )

    def test_unmatched_filter_is_refused_with_its_name(self):
        with patch.object(nq, "run_cargo", side_effect=[completed("drains: test\n"), completed()]):
            with self.assertRaises(SystemExit) as refused:
                nq.run_group(self.group, "Linux")
        self.assertIn("'http_admission_rejects'", str(refused.exception))
        self.assertIn("renamed or removed", str(refused.exception))

    def test_failing_tests_fail_the_group(self):
        with (
            patch.object(nq, "run_cargo", side_effect=[completed(LISTING), completed(IGNORED)]),
            patch.object(nq.subprocess, "run", return_value=completed(returncode=101)) as run,
        ):
            with self.assertRaises(SystemExit) as failed:
                nq.run_group(self.group, "Linux")
        self.assertIn("exit status 101", str(failed.exception))
        command = run.call_args.args[0]
        self.assertEqual(command[-2:], ["http_transport::tests", "http_admission_rejects"])

    def test_listing_failure_is_reported(self):
        with patch.object(nq, "run_cargo", return_value=completed(returncode=101)):
            with self.assertRaises(SystemExit) as failed:
                nq.run_group(self.group, "Linux")
        self.assertIn("Listing tests failed", str(failed.exception))


if __name__ == "__main__":
    unittest.main()
