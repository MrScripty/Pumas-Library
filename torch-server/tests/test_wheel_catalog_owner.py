"""Actual inert wheel/complete-observation controls. No provider or resolver."""
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

from test_install_verified_wheels import make_wheel

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("wheel_catalog_owner", ROOT / "wheel_catalog_owner.py")
owner = importlib.util.module_from_spec(spec)
spec.loader.exec_module(owner)


class CatalogTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / "source"
        self.source.mkdir()
        self.wheels = self.root / "wheels"
        self.wheels.mkdir()
        self.observation = owner.target_owner.capture_observation()

    def artifact(self, name, *, version="1.0", requires=(), extras=(), variant="", content=b"INERT = 1\n"):
        folder = self.source / str(len(list(self.source.iterdir())))
        folder.mkdir()
        row = make_wheel(folder, name, version=version, requires=requires, extras=extras, content=content)
        filename = row["url"].rsplit("/", 1)[-1]
        row["path"] = folder / filename
        row["size"] = row["path"].stat().st_size
        prefix = "https://download.pytorch.org/whl/cpu/" if name == "torch" else "https://files.pythonhosted.org/packages/"
        row["url"] = prefix + variant + filename
        return row

    def request(self, artifacts, *, roots=None, constraints=(), release="2.14.0", build="cpu", direct=()):
        rows = {}
        for artifact in artifacts:
            rows.setdefault(artifact["name"], []).append({"filename": artifact["url"].rsplit("/", 1)[-1],
                "url": artifact["url"], "hashes": {"sha256": artifact["sha256"]}, "size": artifact["size"]})
        observations = []
        for name, files in rows.items():
            for repo in ("pytorch", "pypi"):
                base = f"https://download.pytorch.org/whl/{build}/" if repo == "pytorch" else "https://pypi.org/simple/"
                observations.append({"repository": repo, "project": name, "url": base + name + "/",
                    "status": 200, "observed_at": "2026-10-06T12:00:00Z", "body": json.dumps({
                        "meta": {"api-version": "1.3"}, "name": name,
                        "files": files if repo == ("pytorch" if name == "torch" else "pypi") else []})})
        return {"schema": "pumas.wheel-catalog-request.v1", "selection": "synthetic-selection-1",
                "release": release, "build": build, "roots": roots or [f"torch=={release}+{build}"],
                "constraints": list(constraints), "direct_roots": list(direct),
                "target_observation_sha256": owner.digest(self.observation), "observations": observations}

    def inspect(self, request, artifacts):
        plan = owner.prepare(request, self.observation)
        for candidate in plan["candidates"]:
            row = next(a for a in artifacts if a["url"] == candidate["url"] and a["sha256"] == candidate["sha256"])
            folder = self.wheels / candidate["id"]
            folder.mkdir()
            shutil.copyfile(row["path"], folder / candidate["filename"])
        result = owner.inspect(request, self.observation, plan, self.wheels)
        return plan, result

    def torch(self, **kwargs):
        return self.artifact("torch", version="2.14.0+cpu", **kwargs)

    def mutate(self, artifact, change):
        with zipfile.ZipFile(artifact["path"]) as archive:
            members = {m.filename: archive.read(m) for m in archive.infolist()}
        change(members)
        with zipfile.ZipFile(artifact["path"], "w", zipfile.ZIP_DEFLATED) as archive:
            for name, body in members.items():
                archive.writestr(name, body)
        artifact["size"] = artifact["path"].stat().st_size
        artifact["sha256"] = hashlib.sha256(artifact["path"].read_bytes()).hexdigest()

    def test_full_alternative_ranges_late_extras_markers_cycle_reach_fixed_point(self):
        artifacts = [self.torch(requires=["branch>=1", "switch==1"]),
            self.artifact("branch", version="1", requires=["dependency<2", 'gpu-dependency; extra == "gpu"'], extras=["gpu"]),
            self.artifact("branch", version="2", requires=["dependency>=2"]),
            self.artifact("switch", requires=["branch[gpu]>=1", "cycle==1"]),
            self.artifact("cycle", requires=["switch==1", 'unobserved; python_version < "3.0"']),
            self.artifact("dependency", version="1"), self.artifact("dependency", version="2"),
            self.artifact("gpu-dependency")]
        request = self.request(artifacts)
        with patch("socket.socket", side_effect=AssertionError("No socket permitted")), patch("subprocess.run", side_effect=AssertionError("No solver permitted")):
            plan, result = self.inspect(request, artifacts)
        self.assertEqual(len(plan["candidates"]), 8)
        self.assertEqual(result["reachable"]["dependency"], ["dependency<2", "dependency>=2"])
        self.assertEqual(result["extras"]["branch"], ["", "gpu"])
        self.assertIn("gpu-dependency", result["reachable"])
        self.assertNotIn("unobserved", result["reachable"])
        self.assertFalse(result["empty_domains"])
        self.assertEqual(result["request_sha256"], owner.digest(request))

    def test_second_request_uses_own_release_and_build(self):
        artifact = self.artifact("torch", version="2.15.0+cu130")
        artifact["url"] = artifact["url"].replace("/cpu/", "/cu130/")
        plan, result = self.inspect(self.request([artifact], release="2.15.0", build="cu130"), [artifact])
        self.assertEqual(plan["candidates"][0]["version"], "2.15.0+cu130")
        self.assertIn("torch==2.15.0+cu130", result["reachable"]["torch"])

    def test_second_repository_failure_refuses_even_with_usable_first(self):
        artifacts = [self.torch()]
        request = self.request(artifacts)
        request["observations"][1]["status"] = 503
        with self.assertRaises(owner.Incomplete):
            owner.prepare(request, self.observation)

    def test_omitted_dependency_or_repository_never_completes(self):
        for missing in ("project", "repository"):
            with self.subTest(missing=missing):
                shutil.rmtree(self.wheels)
                self.wheels.mkdir()
                artifacts = [self.torch(requires=["dependency>=1"])]
                if missing == "repository":
                    artifacts += [self.artifact("dependency")]
                request = self.request(artifacts)
                if missing == "repository":
                    request["observations"] = [o for o in request["observations"] if not (o["project"] == "dependency" and o["repository"] == "pypi")]
                with self.assertRaises(owner.Incomplete):
                    self.inspect(request, artifacts)

    def test_typed_404_absence_is_evidence_never_fallback_signal(self):
        artifacts = [self.torch(requires=["missing>=1"])]
        request = self.request(artifacts)
        for repo in ("pytorch", "pypi"):
            base = "https://download.pytorch.org/whl/cpu/" if repo == "pytorch" else "https://pypi.org/simple/"
            request["observations"].append({"repository": repo, "project": "missing", "url": base + "missing/", "status": 404, "body": "", "observed_at": "2026-10-06T12:00:00Z"})
        _, result = self.inspect(request, artifacts)
        self.assertEqual(result["empty_domains"], ["missing"])
        self.assertNotIn("fallback", result)

    def test_same_filename_versions_are_separate_exact_direct_identity(self):
        first, second = self.torch(content=b"INERT = 1\n"), self.torch(variant="other/", content=b"INERT = 2\n")
        direct = [{k: first[k] for k in ("name", "url", "sha256", "size")}]
        request = self.request([first, second], roots=["torch @ " + first["url"]], direct=direct)
        plan, result = self.inspect(request, [first, second])
        self.assertEqual(len({c["id"] for c in plan["candidates"]}), 2)
        self.assertEqual(len({c["filename"] for c in plan["candidates"]}), 1)
        self.assertEqual(result["reachable"]["torch"], ["torch @ " + first["url"]])
        changed = copy.deepcopy(request)
        changed["direct_roots"][0]["sha256"] = second["sha256"]
        with self.assertRaises(owner.Refused):
            owner.prepare(changed, self.observation)

    def test_source_redirect_tracking_and_credential_urls_refuse_before_body(self):
        artifact = self.torch()
        for mutation in ("redirect", "tracking", "payload", "credentials", "build"):
            with self.subTest(mutation=mutation):
                request = self.request([artifact])
                if mutation == "redirect":
                    request["observations"][0]["url"] = "https://unknown.invalid/simple/torch/"
                else:
                    page = json.loads(request["observations"][0]["body"])
                    if mutation == "tracking":
                        page["next"] = "https://unknown.invalid/next"
                    else:
                        page["files"][0]["url"] = {"payload": "https://unknown.invalid/a.whl", "credentials": "https://secret:token@download.pytorch.org/whl/cpu/a.whl", "build": artifact["url"].replace("/cpu/", "/cu130/")}[mutation]
                    request["observations"][0]["body"] = json.dumps(page)
                with self.assertRaises(owner.Refused):
                    owner.prepare(request, self.observation)

    def test_original_explicit_constraints_filter_without_narrowing_alternatives(self):
        artifacts = [self.torch(requires=["dependency>=1"]), self.artifact("dependency", version="1"), self.artifact("dependency", version="2")]
        plan, _ = self.inspect(self.request(artifacts, constraints=["dependency<2"]), artifacts)
        self.assertEqual([c["version"] for c in plan["candidates"] if c["name"] == "dependency"], ["1"])
        self.assertEqual(len(plan["exclusions"]), 1)

    def test_sdist_or_vcs_root_refuses_and_sdist_row_never_fetches(self):
        artifact = self.torch()
        request = self.request([artifact], roots=["torch @ git+https://unknown.invalid/source"], direct=[])
        with self.assertRaises(owner.Refused):
            owner.prepare(request, self.observation)
        request = self.request([artifact])
        page = json.loads(request["observations"][0]["body"])
        page["files"].append({"filename": "torch-2.14.0.tar.gz", "url": "https://unknown.invalid/build", "hashes": {}})
        request["observations"][0]["body"] = json.dumps(page)
        plan, _ = self.inspect(request, [artifact])
        self.assertEqual(len(plan["candidates"]), 1)
        self.assertEqual(plan["exclusions"][0]["reason"], "wheel-only")

    def test_inactive_dependency_url_refuses_actual_metadata(self):
        artifacts = [self.torch(requires=['evil @ https://unknown.invalid/build.tar.gz ; python_version < "3.0"'])]
        with self.assertRaisesRegex(owner.Refused, "including inactive"):
            self.inspect(self.request(artifacts), artifacts)

    def test_missing_size_and_budget_overflow_cannot_complete(self):
        artifacts = [self.torch()]
        request = self.request(artifacts)
        page = json.loads(request["observations"][0]["body"])
        del page["files"][0]["size"]
        request["observations"][0]["body"] = json.dumps(page)
        with self.assertRaises(owner.Incomplete):
            owner.prepare(request, self.observation)
        for limit in ("MAX_PROJECTS", "MAX_OBSERVATIONS", "MAX_PAGE", "MAX_METADATA", "MAX_ROWS", "MAX_CANDIDATES", "MAX_PAYLOAD"):
            with self.subTest(limit=limit), patch.object(owner, limit, 0), self.assertRaises((owner.Incomplete, owner.Refused)):
                owner.prepare(self.request(artifacts), self.observation)

    def test_zip_member_expansion_edges_and_deadline_overflow_are_incomplete(self):
        for limit in ("MAX_MEMBERS", "MAX_EXPANDED", "MAX_CATALOG_EXPANDED", "MAX_SECONDS", "MAX_EDGES"):
            with self.subTest(limit=limit):
                shutil.rmtree(self.wheels)
                self.wheels.mkdir()
                artifacts = [self.torch(requires=["dependency==1"]), self.artifact("dependency")]
                with patch.object(owner, limit, 0), self.assertRaises(owner.Incomplete):
                    self.inspect(self.request(artifacts), artifacts)

    def test_actual_python_metadata_overrides_optional_index_hint(self):
        artifact = self.torch()
        self.mutate(artifact, lambda members: members.update({next(n for n in members if n.endswith("/METADATA")): next(v for n, v in members.items() if n.endswith("/METADATA")) + b"Requires-Python: <3.0\n"}))
        _, result = self.inspect(self.request([artifact]), [artifact])
        self.assertEqual(result["empty_domains"], ["torch"])
        self.assertFalse(result["facts"][0]["eligible"])

    def test_optional_index_python_hint_contradicting_actual_metadata_refuses(self):
        artifacts = [self.torch()]
        request = self.request(artifacts)
        page = json.loads(request["observations"][0]["body"])
        page["files"][0]["requires-python"] = ">=3.10"
        request["observations"][0]["body"] = json.dumps(page)
        with self.assertRaisesRegex(owner.Refused, "contradicts"):
            self.inspect(request, artifacts)

    def test_incompatible_filename_excluded_and_exact_output_budget_refuses(self):
        artifacts = [self.torch()]
        request = self.request(artifacts)
        page = json.loads(request["observations"][0]["body"])
        row = copy.deepcopy(page["files"][0])
        row["filename"] = row["filename"].replace("py3-none-any", "cp39-cp39-win_amd64")
        row["url"] = row["url"].replace("py3-none-any", "cp39-cp39-win_amd64")
        page["files"].append(row)
        request["observations"][0]["body"] = json.dumps(page)
        plan, _ = self.inspect(request, artifacts)
        self.assertEqual(len(plan["candidates"]), 1)
        self.assertEqual(plan["exclusions"][0]["reason"], "target-or-explicit-constraint")
        with patch.object(owner, "MAX_OUTPUT", 0), self.assertRaises(owner.Incomplete):
            owner.prepare(request, self.observation)

    def test_duplicate_protocol_fields_and_pagination_cannot_assert_complete(self):
        request = self.request([self.torch()])
        for body in ('{"meta":{},"meta":{},"name":"torch","files":[]}',
                     '{"meta":{"api-version":"1.3"},"name":"torch","files":[],"next":"/next"}'):
            with self.subTest(body=body), self.assertRaises(owner.Refused):
                request["observations"][0]["body"] = body
                owner.prepare(request, self.observation)

    def test_actual_wheel_tags_identity_and_unsafe_namespace_refuse(self):
        for change in ("tags", "identity", "namespace"):
            with self.subTest(change=change):
                shutil.rmtree(self.wheels)
                self.wheels.mkdir()
                artifact = self.torch()
                def mutate(members):
                    if change == "namespace":
                        members["../outside"] = b"unsafe"
                    else:
                        key = next(n for n in members if n.endswith("/WHEEL" if change == "tags" else "/METADATA"))
                        members[key] = members[key].replace(b"py3-none-any", b"cp39-none-any") if change == "tags" else members[key].replace(b"Name: torch", b"Name: other")
                self.mutate(artifact, mutate)
                with self.assertRaises(owner.Refused):
                    self.inspect(self.request([artifact]), [artifact])

    def test_changed_target_plan_bytes_namespace_and_new_upstream_do_not_renew_scope(self):
        artifacts = [self.torch()]
        request = self.request(artifacts)
        plan, result = self.inspect(request, artifacts)
        retained = copy.deepcopy(result)
        for change in ("target", "plan", "body", "namespace", "new-page"):
            with self.subTest(change=change):
                selected_request, selected_plan, selected_observation = copy.deepcopy(request), copy.deepcopy(plan), copy.deepcopy(self.observation)
                candidate = plan["candidates"][0]
                local = self.wheels / candidate["id"] / candidate["filename"]
                original = local.read_bytes()
                if change == "target":
                    selected_observation["interpreter_sha256"] = "0" * 64
                elif change == "plan":
                    selected_plan["candidates"] = []
                elif change == "body":
                    local.write_bytes(b"changed")
                elif change == "namespace":
                    (self.wheels / "unapproved").mkdir()
                else:
                    selected_request["observations"][0]["observed_at"] = "2026-10-06T12:01:00Z"
                with self.assertRaises(owner.Refused):
                    owner.inspect(selected_request, selected_observation, selected_plan, self.wheels)
                local.write_bytes(original)
                if change == "namespace":
                    (self.wheels / "unapproved").rmdir()
        self.assertEqual(result, retained)

    def test_yanked_prerelease_rows_retained_without_solver_policy_decision(self):
        artifacts = [self.torch(requires=["dependency>=1"]), self.artifact("dependency", version="2.0rc1")]
        request = self.request(artifacts)
        row = next(o for o in request["observations"] if o["repository"] == "pypi" and o["project"] == "dependency")
        page = json.loads(row["body"])
        page["files"][0]["yanked"] = "provider reason"
        row["body"] = json.dumps(page)
        plan, _ = self.inspect(request, artifacts)
        self.assertEqual(plan["candidates"][1]["yanked"], "provider reason")
        self.assertEqual(plan["candidates"][1]["version"], "2.0rc1")

    def test_cli_errors_redact_untrusted_input_and_never_use_legacy_exit_codes(self):
        request = self.request([self.torch()])
        request["observations"][0]["url"] = "https://SECRET-CONTROL:SECRET-TOKEN@unknown.invalid/"
        request_path, observation_path, output = self.root / "request.json", self.root / "observation.json", self.root / "output.json"
        request_path.write_text(json.dumps(request))
        observation_path.write_text(json.dumps(self.observation))
        result = subprocess.run([sys.executable, "-I", str(ROOT / "wheel_catalog_owner.py"), "--request", str(request_path), "--observation", str(observation_path), "--output", str(output)], capture_output=True, text=True)
        self.assertEqual(result.returncode, 21)
        self.assertNotIn("SECRET", result.stdout + result.stderr)
        self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
