"""Synthetic checker packets, actual public-pip installs; Rust qualifies live authority."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

import test_offline_wheel_selection as selection_fixtures

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("consume_selected", ROOT / "consume_selected_wheels.py")
selected = importlib.util.module_from_spec(spec)
spec.loader.exec_module(selected)


class SelectedConsumerTests(unittest.TestCase):
    setUp = selection_fixtures.SelectionTests.setUp
    artifact = selection_fixtures.SelectionTests.artifact
    torch = selection_fixtures.SelectionTests.torch
    request = selection_fixtures.SelectionTests.request
    inspect = selection_fixtures.SelectionTests.inspect
    prepared = selection_fixtures.SelectionTests.prepared
    lock = selection_fixtures.SelectionTests.lock

    def packet(self):
        # Preserve original direct URL and a root extra; alternatives stay in
        # the catalog, while only the independently checked subset reaches pip.
        original = self.torch(requires=['leaf; extra == "tools"'], extras=["tools"])
        artifacts = [original, self.artifact("leaf", version="1"), self.artifact("leaf", version="2")]
        self.prepared(artifacts, roots=["torch[tools] @ " + original["url"]],
                      constraints=["leaf<2"], direct=[{k: original[k] for k in ("name", "url", "sha256", "size")}])
        lock = self.lock()
        lock["packages"] = [p for p in lock["packages"] if p["name"] != "leaf" or p["version"] == "1"]
        self.directory = self.root / "selection"
        self.directory.mkdir()
        for name, key in (("roots.in", "roots"), ("constraints.in", "constraints")):
            (self.directory / name).write_text("\n".join(self.projection[key]) + "\n")
        (self.directory / "projection.json").write_text(json.dumps(self.projection))
        body = 'lock-version = "1.0"\ncreated-by = "uv"\nrequires-python = ' + json.dumps(lock["requires-python"]) + '\n'
        for item in lock["packages"]:
            wheel = item["wheels"][0]
            body += '\n[[packages]]\nname = ' + json.dumps(item["name"]) + '\nversion = ' + json.dumps(item["version"])
            body += '\n[[packages.wheels]]\nurl = ' + json.dumps(wheel["url"])
            body += '\n[packages.wheels.hashes]\nsha256 = ' + json.dumps(wheel["hashes"]["sha256"]) + '\n'
        (self.directory / "pylock.toml").write_text(body)
        packet = selected.selection.selected_packet(self.req, self.observation, self.evidence, self.projection, lock, self.directory)
        packet["lock_sha256"] = hashlib.sha256(body.encode()).hexdigest()
        self.target, self.output = self.root / "packages", self.root / "proof"
        return packet

    def consume(self, packet, *, verify_only=False):
        return selected.consume(self.req, self.observation, self.evidence, self.wheels,
                                self.directory, packet, self.target, self.output, verify_only=verify_only)

    def test_actual_exact_subset_install_original_direct_root_extra_report_record_and_no_ip_traffic(self):
        packet = self.packet()
        run = subprocess.run
        trace = self.root / "pip-network.trace"
        calls = []

        def traced(command, **kwargs):
            calls.append(command)
            return run(["/usr/bin/strace", "-f", "-e", "trace=network,execve", "-o", str(trace), *command], **kwargs)

        with patch.dict(os.environ, {"PIP_INDEX_URL": "http://127.0.0.1:1/trap",
                                    "PIP_REQUIREMENT": "http://127.0.0.1:1/injected",
                                    "PIP_FIND_LINKS": "http://127.0.0.1:1/alternatives"}):
            with patch.object(selected.consumer.subprocess, "run", side_effect=traced):
                manifest = self.consume(packet)
        self.assertEqual(len(calls), 1)
        for flag in ("--isolated", "--no-index", "--no-deps", "--require-hashes", "--only-binary=:all:"):
            self.assertIn(flag, calls[0])
        # Public pip's vendored transport probes IPv6 support by binding ::1.
        # Observe actual retrieval calls, not socket creation or egress denial.
        for line in trace.read_text().splitlines():
            if any(call in line for call in ("connect(", "sendto(", "sendmsg(", "sendmmsg(")):
                self.assertNotIn("AF_INET", line)
        if destination := os.environ.get("PUMAS_SELECTED_CONSUMER_EVIDENCE"):
            destination = Path(destination)
            destination.mkdir(parents=True, exist_ok=True)
            (destination / "python-pip-network.trace").write_bytes(trace.read_bytes())
            (destination / "python-pip-invocation.json").write_text(json.dumps(calls[0]))
        self.assertEqual(manifest, self.consume(packet, verify_only=True))
        self.assertEqual(packet["extras"]["torch"], ["tools"])
        self.assertEqual(len(list(self.target.glob("*.dist-info"))), 2)
        report = json.loads((self.output / "local-pip-report.json").read_text())
        self.assertEqual({r["metadata"]["name"] for r in report["install"]}, {"torch", "leaf"})
        self.assertEqual({r["download_info"]["url"] for r in report["install"]},
                         {Path(c["local"]).as_uri() for c in packet["selected"]})
        self.assertTrue(all(c["url"].startswith("https:") for c in packet["selected"]))

    def test_root_extra_missing_actual_dependency_refuses_before_pip(self):
        packet = self.packet()
        root = next(c for c in packet["selected"] if c["name"] == "torch")
        with patch.object(selected.consumer.subprocess, "run", side_effect=AssertionError("pip must not start")):
            with self.assertRaisesRegex(ValueError, "omits"):
                selected.consumer.install([root], self.wheels, self.target, self.output,
                    local_paths=[root["local"]], requested_extras={"torch": ["tools"]})
        self.assertFalse(self.target.exists())

    def test_actual_report_target_url_hash_version_duplicate_and_member_mutations_refuse(self):
        packet = self.packet()
        self.consume(packet)
        report_path = self.output / "local-pip-report.json"
        original = report_path.read_bytes()
        for mutation in ("target", "url", "hash", "version", "duplicate", "schema", "source-kind"):
            with self.subTest(mutation=mutation):
                report = json.loads(original)
                item = report["install"][0]
                if mutation == "target":
                    report["environment"]["python_full_version"] = "1.0"
                if mutation == "url":
                    item["download_info"]["url"] = "https://example.invalid/wheel.whl"
                if mutation == "hash":
                    item["download_info"]["archive_info"]["hashes"]["sha256"] = "0" * 64
                if mutation == "version":
                    item["metadata"]["version"] = "999"
                if mutation == "duplicate":
                    report["install"].append(copy.deepcopy(item))
                if mutation == "schema":
                    report["version"] = "2"
                if mutation == "source-kind":
                    item["is_direct"] = False
                report_path.write_text(json.dumps(report))
                with self.assertRaises(ValueError):
                    self.consume(packet, verify_only=True)
        report_path.write_bytes(original)
        for path in (self.target / "torch.py", next(self.target.glob("torch-*.dist-info/RECORD")),
                     self.output / "installed-files.json", self.output / "local-requirements.txt"):
            with self.subTest(path=path.name):
                before = path.read_bytes()
                path.write_bytes(before + b"changed")
                with self.assertRaises(ValueError):
                    self.consume(packet, verify_only=True)
                path.write_bytes(before)
        self.consume(packet, verify_only=True)

    def test_selected_packet_extras_subset_local_path_and_source_cannot_be_substituted(self):
        packet = self.packet()
        for mutation in ("extras", "local", "source", "subset"):
            changed = copy.deepcopy(packet)
            if mutation == "extras":
                changed["extras"]["torch"] = []
            if mutation == "local":
                changed["selected"][0]["local"] = "/tmp/unapproved.whl"
            if mutation == "source":
                changed["selected"][0]["url"] = "https://example.invalid/substitute.whl"
            if mutation == "subset":
                changed["selected"].pop()
            with self.subTest(mutation=mutation), patch.object(selected.consumer.subprocess, "run", side_effect=AssertionError("pip must not start")):
                with self.assertRaises(ValueError):
                    self.consume(changed)
        self.assertFalse(self.target.exists())

    def test_changed_original_producer_and_lock_refuse_before_install(self):
        packet = self.packet()
        changed = copy.deepcopy(self.observation)
        changed["interpreter_sha256"] = "0" * 64
        with self.assertRaises(ValueError):
            selected.consume(self.req, changed, self.evidence, self.wheels, self.directory, packet, self.target, self.output)
        lock = self.directory / "pylock.toml"
        lock.write_bytes(lock.read_bytes() + b"# changed retained lock\n")
        with self.assertRaises(ValueError):
            self.consume(packet)
        self.assertFalse(self.target.exists())


if __name__ == "__main__":
    unittest.main()
