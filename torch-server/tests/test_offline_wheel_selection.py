"""Selected-packet adversarial controls and optional real pinned public solver."""
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import unittest

import test_wheel_catalog_owner as fixtures

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("offline_selection", ROOT / "offline_wheel_selection.py")
selection = importlib.util.module_from_spec(spec)
spec.loader.exec_module(selection)
UV = os.environ.get("PUMAS_QUALIFIED_UV")
UV_SHA256 = "abdc39eab8b4ad341dca91f3823a23a343fae94bdb22ebdd9e91694415206f2f"


class SelectionTests(unittest.TestCase):
    def setUp(self):
        fixtures.CatalogTests.setUp(self)
        # Synthetic declared floor, supported by this native consumer's tags.
        # Real capture remains 2.41 here and must refuse exact uv projection.
        self.native_observation = copy.deepcopy(self.observation)
        self.observation["target"]["libc"] = {"family": "glibc", "version": "2.40"}

    artifact = fixtures.CatalogTests.artifact
    torch = fixtures.CatalogTests.torch
    request = fixtures.CatalogTests.request
    inspect = fixtures.CatalogTests.inspect

    def prepared(self, artifacts=None, **request_options):
        artifacts = artifacts or [self.torch(requires=["leaf[tools]>=1"]),
                                  self.artifact("leaf", requires=['child ; extra == "tools"'], extras=["tools"]),
                                  self.artifact("child")]
        self.req = self.request(artifacts, **request_options)
        _, self.evidence = self.inspect(self.req, artifacts)
        self.projection = selection.prepare(self.req, self.observation, self.evidence, self.wheels)
        return self.projection

    def lock(self):
        return {"lock-version": "1.0", "created-by": "uv", "requires-python": ">=" + self.observation["target"]["python"],
                "packages": [{"name": c["name"], "version": c["version"],
                              "wheels": [{"url": Path(c["local"]).as_uri(), "hashes": {"sha256": c["sha256"]}}]}
                             for c in self.projection["candidates"]]}

    def checked(self, lock=None):
        return selection.selected_packet(self.req, self.observation, self.evidence,
                                         self.projection, self.lock() if lock is None else lock, self.root)

    def refuses(self, lock, reason):
        with self.assertRaisesRegex(selection.catalog.Refused, reason):
            self.checked(lock)

    def test_actual_selection_and_extras_closure(self):
        self.prepared()
        packet = self.checked()
        self.assertEqual({c["name"] for c in packet["selected"]}, {"torch", "leaf", "child"})
        self.assertEqual(packet["extras"]["leaf"], ["tools"])
        self.assertEqual(packet["catalog_sha256"], selection.digest(self.evidence))

    def test_late_extras_and_cycles_reach_checked_fixed_point(self):
        artifacts = [self.torch(requires=["leaf[alpha]>=1", "parent>=1"]),
                     self.artifact("parent", requires=["leaf[beta]>=1", "torch==2.14.0+cpu"]),
                     self.artifact("leaf", requires=['first; extra == "alpha"', 'second; extra == "beta"'], extras=["alpha", "beta"]),
                     self.artifact("first"), self.artifact("second")]
        self.prepared(artifacts)
        self.assertEqual(self.checked()["extras"]["leaf"], ["alpha", "beta"])
        lock = self.lock()
        lock["packages"] = [p for p in lock["packages"] if p["name"] != "second"]
        self.refuses(lock, "Missing selected closure member")

    def test_inactive_actual_metadata_unprojectable_marker_refuses(self):
        self.req = self.request([self.torch(requires=['leaf; platform_release == "unreachable"'])])
        # Build a separate correctly hashed fixture using the same observations.
        artifact = next(self.source.glob('*/*.whl'))
        row = {"path": artifact, "url": "https://download.pytorch.org/whl/cpu/" + artifact.name,
               "sha256": hashlib.sha256(artifact.read_bytes()).hexdigest()}
        _, self.evidence = self.inspect(self.req, [row])
        with self.assertRaisesRegex(selection.catalog.Refused, "inspection-host leakage"):
            selection.prepare(self.req, self.observation, self.evidence, self.wheels)

    def test_missing_extra_closure_and_unrelated_member_refuse(self):
        self.prepared()
        lock = self.lock()
        lock["packages"] = [p for p in lock["packages"] if p["name"] != "child"]
        self.refuses(lock, "Missing selected closure member")
        self.req["roots"] = ["torch==2.14.0+cpu"]
        torch_fact = next(f for f in self.evidence["facts"] if f["id"] == next(c["id"] for c in self.evidence["candidates"] if c["name"] == "torch"))
        torch_fact["dependencies"] = []
        self.refuses(self.lock(), "Unrelated selected packages")

    def test_undeclared_requested_extra_refuses(self):
        self.prepared([self.torch()])
        self.req["roots"] = ["torch[unknown]==2.14.0+cpu"]
        self.refuses(self.lock(), "Selected extras undeclared")

    def test_original_constraint_rechecked(self):
        self.prepared()
        self.req["constraints"] = ["leaf<1"]
        self.refuses(self.lock(), "original constraint")

    def test_selected_version_requirement_rechecked(self):
        self.prepared()
        self.req["roots"] = ["torch==9"]
        self.refuses(self.lock(), "original requirement")

    def test_local_hash_and_source_substitution_refuse(self):
        self.prepared()
        for change, reason in ((lambda e: e.update(url="https://example.invalid/new.whl"), "escaped local"),
                               (lambda e: e.update(hashes={"sha256": "0" * 64}), "exact original"),
                               (lambda e: e.update(size=1), "size differs")):
            lock = self.lock()
            change(lock["packages"][0]["wheels"][0])
            self.refuses(lock, reason)

    def test_unknown_schema_sources_and_duplicate_packages_refuse(self):
        self.prepared()
        for field, value in (("lock-version", "2.0"), ("environments", [])):
            lock = self.lock()
            lock[field] = value
            self.refuses(lock, "Unsupported public lock")
        for field in ("sdist", "vcs", "directory"):
            lock = self.lock()
            lock["packages"][0][field] = {}
            self.refuses(lock, "Non-wheel")
        lock = self.lock()
        lock["packages"].append(copy.deepcopy(lock["packages"][0]))
        self.refuses(lock, "Ambiguous/ineligible")

    def test_ambiguous_public_lock_files_refuse(self):
        self.prepared()
        lock = self.lock()
        lock["packages"][0]["wheels"] *= 2
        self.refuses(lock, "Ambiguous/ineligible")

    def test_same_version_file_choice_is_not_new_authority(self):
        artifacts = [self.torch(), self.artifact("torch", version="2.14.0+cpu", variant="alternate/", content=b"INERT = 2\n")]
        self.prepared(artifacts)
        lock = self.lock()
        lock["packages"] = lock["packages"][:1]
        self.refuses(lock, "Ambiguous admitted same-version")

    def test_exact_direct_root_preserves_identity_before_coalescing(self):
        original = self.torch()
        alternate = self.artifact("torch", version="2.14.0+cpu", variant="alternate/", content=b"INERT = 2\n")
        direct = {k: original[k] for k in ("name", "url", "sha256", "size")}
        self.prepared([original, alternate], roots=["torch @ " + original["url"]], direct=[direct])
        lock = self.lock()
        lock["packages"] = lock["packages"][:1]
        packet = self.checked(lock)
        self.assertEqual(packet["selected"][0]["url"], original["url"])
        lock["packages"] = self.lock()["packages"][1:]
        self.refuses(lock, "direct root identity differs")

    def test_inactive_row_cannot_add_remote_source(self):
        self.prepared()
        lock = self.lock()
        row = copy.deepcopy(lock["packages"][0])
        row["marker"] = 'sys_platform == "never"'
        row["wheels"][0]["url"] = "https://example.invalid/new.whl"
        lock["packages"].append(row)
        self.refuses(lock, "escaped local")

    def test_lock_python_and_unprojectable_markers_refuse(self):
        self.prepared()
        lock = self.lock()
        lock["requires-python"] = ">=99"
        self.refuses(lock, "Python target differs")
        lock = self.lock()
        lock["packages"][0]["marker"] = 'platform_release == "inactive"'
        self.refuses(lock, "inspection-host leakage")

    def test_incomplete_coverage_refuses_before_solver(self):
        artifacts = [self.torch(requires=["missing>=1"])]
        req = self.request(artifacts)
        plan = selection.catalog.prepare(req, self.observation)
        with self.assertRaises(fixtures.owner.Incomplete):
            self.inspect(req, artifacts)
        with self.assertRaises(selection.catalog.Incomplete):
            selection.prepare(req, self.observation, {"schema": "fake-complete"}, self.wheels)
        self.assertTrue(plan["candidates"])

    def test_changed_catalog_and_acquired_bytes_refuse(self):
        self.prepared()
        forged = copy.deepcopy(self.evidence)
        forged["empty_domains"] = ["forged"]
        with self.assertRaisesRegex(selection.catalog.Refused, "evidence changed"):
            selection.prepare(self.req, self.observation, forged, self.wheels)
        path = Path(self.projection["candidates"][0]["local"])
        path.write_bytes(path.read_bytes() + b"changed")
        with self.assertRaises(selection.catalog.Refused):
            selection.prepare(self.req, self.observation, self.evidence, self.wheels)

    def test_changed_target_or_executable_binding_refuses(self):
        self.prepared()
        for field, value in (("interpreter_sha256", "0" * 64), ("interpreter", "/unapproved/python")):
            observation = copy.deepcopy(self.observation)
            observation[field] = value
            req = copy.deepcopy(self.req)
            req["target_observation_sha256"] = selection.digest(observation)
            with self.assertRaises(selection.catalog.Refused):
                selection.prepare(req, observation, self.evidence, self.wheels)

    def test_real_native_target_without_exact_projection_refuses(self):
        target = selection.catalog.target_owner.WheelTarget(self.native_observation["target"])
        if target.to_dict()["libc"] == {"family": "glibc", "version": "2.41"}:
            with self.assertRaisesRegex(selection.catalog.Refused, "Unsupported public solver libc"):
                selection.resolver_platform(target)

    def test_unsupported_projection_and_inactive_dependency_marker_refuse(self):
        self.prepared()
        target = selection.catalog.target_owner.WheelTarget(self.observation["target"])
        wire = target.to_dict()
        wire["libc"] = {"family": "musl", "version": "1.2"}
        with self.assertRaisesRegex(selection.catalog.Refused, "qualified public solver"):
            selection.resolver_platform(selection.catalog.target_owner.WheelTarget(wire))
        with self.assertRaisesRegex(selection.catalog.Refused, "inspection-host leakage"):
            selection.checked_input('leaf; platform_version == "inactive"')
        with self.assertRaisesRegex(selection.catalog.Refused, "option injection"):
            selection.checked_input("leaf\n--index-url https://example.invalid/")

    def test_yanked_policy_not_silently_lost(self):
        artifact = self.torch()
        req = self.request([artifact])
        body = json.loads(req["observations"][0]["body"])
        body["files"][0]["yanked"] = "upstream withdrawal"
        req["observations"][0]["body"] = json.dumps(body)
        _, evidence = self.inspect(req, [artifact])
        with self.assertRaisesRegex(selection.catalog.Refused, "Yanked source policy"):
            selection.prepare(req, self.observation, evidence, self.wheels)

    @unittest.skipUnless(UV, "requires explicitly supplied qualified public uv executable")
    def test_real_public_uv_isolated_selection_and_resolver_errors(self):
        self.assertEqual(hashlib.sha256(Path(UV).read_bytes()).hexdigest(), UV_SHA256)
        artifacts = [self.torch(requires=["leaf[tools]>=1,<3"]),
                     self.artifact("leaf", version="1", requires=['child ; extra == "tools"'], extras=["tools"]),
                     self.artifact("leaf", version="2", requires=['child ; extra == "tools"'], extras=["tools"]),
                     self.artifact("child")]
        self.prepared(artifacts, constraints=["leaf<2"], roots=["torch @ " + artifacts[0]["url"]],
                      direct=[{k: artifacts[0][k] for k in ("name", "url", "sha256", "size")}])
        directory = self.root / "selection"
        directory.mkdir()
        for leaf in ("home", "cache", "tmp"):
            (directory / leaf).mkdir()
        (directory / "uv.toml").write_text('index-url = "http://127.0.0.1:1/trap"\nfind-links = ["http://127.0.0.1:1/trap"]\n')
        for filename, value in (("request.json", self.req), ("observation.json", self.observation), ("evidence.json", self.evidence)):
            selection.write(self.root / filename, value)
        helper = [sys.executable, "-I", str(ROOT / "offline_wheel_selection.py"), "--request", str(self.root / "request.json"),
                  "--observation", str(self.root / "observation.json"), "--catalog", str(self.root / "evidence.json"),
                  "--wheels", str(self.wheels), "--directory", str(directory)]
        self.assertEqual(subprocess.run(helper, capture_output=True).returncode, 0)
        env = {"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8", "HOME": str(directory / "home"),
               "XDG_CONFIG_HOME": str(directory / "home"), "UV_CACHE_DIR": str(directory / "cache"),
               "TMPDIR": str(directory / "tmp")}
        command = [UV, "--no-config", "--no-cache", "--offline", "--no-python-downloads", "--no-managed-python",
                   "pip", "compile", str(directory / "roots.in"), "--constraints", str(directory / "constraints.in"),
                   "--no-index", "--no-build", "--no-sources", "--keyring-provider", "disabled", "--python", sys.executable,
                   "--python-version", self.projection["python"], "--python-platform", self.projection["platform"],
                   "--format", "pylock.toml", "--generate-hashes", "--no-header", "--no-annotate", "-o", str(directory / "pylock.toml")]
        for link in self.projection["find_links"]:
            command.extend(["--find-links", link])
        # Observe the actual solver process and descendants; no enforced-denial
        # claim is inferred from flags or a parser fixture.
        traced = ["/usr/bin/strace", "-f", "-e", "trace=network,execve", "-o", str(directory / "trace"), *command]
        completed = subprocess.run(traced, cwd=directory, env=env, capture_output=True, timeout=30)
        self.assertEqual(completed.returncode, 0, completed.stderr.decode())
        trace = (directory / "trace").read_text()
        self.assertNotIn("AF_INET", trace)
        self.assertNotIn("AF_INET6", trace)
        self.assertEqual(subprocess.run([*helper, "--check"], capture_output=True).returncode, 0)
        packet = selection.catalog.decode((directory / "selected.json").read_bytes())
        self.assertEqual(next(c["version"] for c in packet["selected"] if c["name"] == "leaf"), "1")
        self.assertEqual(packet["extras"]["leaf"], ["tools"])
        self.assertEqual(list((directory / "cache").iterdir()), [])
        destination = os.environ.get("PUMAS_SELECTION_EVIDENCE_DIR")
        if destination:
            saved = Path(destination)
            saved.mkdir(exist_ok=True)
            for leaf in ("projection.json", "pylock.toml", "selected.json", "roots.in", "constraints.in", "trace"):
                shutil.copyfile(directory / leaf, saved / leaf)
            for leaf in ("request.json", "observation.json", "evidence.json"):
                shutil.copyfile(self.root / leaf, saved / leaf)
            (saved / "uv.stdout").write_bytes(completed.stdout)
            (saved / "uv.stderr").write_bytes(completed.stderr)
            (saved / "invocation.json").write_text(json.dumps({"argv": command, "environment": env,
                "qualified_uv_sha256": UV_SHA256, "synthetic_target": True}, indent=2) + "\n")
        (directory / "constraints.in").write_text("leaf<1\n")
        self.assertNotEqual(subprocess.run(command, cwd=directory, env=env, capture_output=True, timeout=30).returncode, 0)
        fault = command.copy()
        fault[fault.index("--python") + 1] = str(directory / "missing-python")
        self.assertNotEqual(subprocess.run(fault, cwd=directory, env=env, capture_output=True, timeout=30).returncode, 0)
        self.assertEqual(subprocess.run([*helper, "--check"], capture_output=True).returncode, 21)


if __name__ == "__main__":
    unittest.main()
