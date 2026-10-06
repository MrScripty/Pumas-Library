"""Actual shared acquisition/public uv, then inert independent lock controls.

Keep the genuine uv lock and source declarations unchanged. Modified copies are
checker inputs only; no install, new source access or resolver fallback follows.
"""
import argparse
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import tomllib

from packaging.requirements import Requirement


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--harness", type=Path, required=True)
    parser.add_argument("--uv", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    directory = args.output.resolve()
    directory.mkdir(parents=True)
    report = Path(__file__).resolve().parent.parent
    evaluator = report / "torch-offline-catalog-2026-10-06/evaluate_offline.py"
    owner = load("successor_owner", evaluator)
    factory = load("trusted_inert_factory", report / "torch-public-uv-evaluation-2026-10-06/evaluate_uv.py")
    root = report.parents[3]
    frozen_source = subprocess.check_output([
        "git", "show", "9eda2944eb9b8ccbaef89d3bcd125742e6086be6:"
        "docs/plans/artifact-acquisition/reports/torch-offline-catalog-2026-10-06/evaluate_offline.py",
    ], cwd=root)
    frozen_path = directory / "frozen_checker.py"
    frozen_path.write_bytes(frozen_source)
    baseline = load("frozen_checker", frozen_path)
    source = owner.Source()
    try:
        candidates = []
        for index, tag in enumerate(("py3-none-any", "cp312-cp312-manylinux_2_17_x86_64")):
            filename, body = factory.wheel("root", tag=tag)
            path = "/owner/" + filename
            source.routes[path] = (200, {}, body)
            candidates.append({"id": f"candidate-{index}", "name": "root", "version": "1.0",
                               "filename": filename, "url": source.base + path,
                               "sha256": owner.digest(body), "bytes": len(body)})
        original = candidates[0]
        raw_root = "root @ " + original["url"] + "#sha256=" + original["sha256"]
        spec = {"case": "exact-direct-root", "candidates": candidates,
                "approved_urls": [c["url"] for c in candidates], "target": owner.fixture_target("linux", python="3.12.7"),
                "complete_declaration": True, "roots": [raw_root],
                "catalog_digest": owner.digest(owner.canonical(candidates))}
        declaration = (json.dumps(spec, indent=2) + "\n").encode()
        spec_path = directory / "owner-spec.json"
        spec_path.write_bytes(declaration)
        completed = subprocess.run([str(args.harness.resolve()), str(spec_path), sys.executable,
                                    str(evaluator), str(args.uv.resolve()), str(root / "torch-server/tooling/packaging.zip")],
                                   env={"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8"}, capture_output=True, timeout=90)
        (directory / "harness.stdout").write_bytes(completed.stdout)
        (directory / "harness.stderr").write_bytes(completed.stderr)
        owner.require(completed.returncode == 0, "shared acquisition harness failed")
        outcome = json.loads((directory / "outcome.json").read_text())
        payload = outcome["payload"]
        owner.require(payload["status"] == "selected", "real public uv did not select an exact direct root")
        lock_path = directory / "pylock.toml"
        original_lock = lock_path.read_bytes()
        projection = (directory / "projection.json").read_bytes()
        lock = tomllib.loads(original_lock.decode())
        environment, tags = owner.context(spec["target"])
        inspected = [owner.inspect_wheel(c, directory / "acquisition/catalog", tags, python=spec["target"]["python"]) for c in candidates]
        approved, alternative = inspected
        owner.require(all(c["eligible"] for c in inspected), "both same-version wheels must be eligible")
        owner.require(approved["sha256"] != alternative["sha256"], "substitution must change actual bytes/hash")
        binding = [(raw_root, owner.artifact_identity(approved))]
        roots = [Requirement("root==1.0")]
        controls = []

        def check(name, document, requirements, bindings, *, reason=None, old_accepts=True):
            (directory / (name + ".lock-input.json")).write_text(json.dumps(document, indent=2) + "\n")
            legacy_accepted = False
            try:
                baseline.selected_packet(document, inspected, directory, environment, requirements)
                legacy_accepted = True
            except baseline.Refused:
                pass
            accepted = False
            error = None
            try:
                selected = owner.selected_packet(document, inspected, directory, environment, requirements, bindings)
                accepted = True
            except owner.Refused as refusal:
                error = str(refusal)
            assertions = {"expected_legacy_outcome": legacy_accepted == old_accepts,
                          "expected_successor_outcome": accepted == (reason is None),
                          "specific_reason": reason is None or reason in (error or ""),
                          "immutable_uv_output": lock_path.read_bytes() == original_lock,
                          "immutable_projection": (directory / "projection.json").read_bytes() == projection,
                          "immutable_original_url_hash": spec_path.read_bytes() == declaration,
                          "no_new_source_request": len(source.requests) == 2}
            if accepted and any(owner.active(Requirement(raw), (), environment) for raw, _identity in bindings):
                assertions["exact_approved_selection"] = selected[0]["id"] == original["id"] and selected[0]["sha256"] == original["sha256"]
            controls.append({"case": name, "legacy_accepted": legacy_accepted, "successor_accepted": accepted,
                             "reason": error, "assertions": assertions,
                             "failed_assertions": [k for k, value in assertions.items() if not value]})

        check("actual-uv-approved-root", lock, roots, binding)
        substituted = copy.deepcopy(lock)
        substituted["packages"][0]["archive"] = {"path": alternative["local"], "hashes": {"sha256": alternative["sha256"]}}
        check("admitted-same-version-lock-substitution", substituted, roots, binding,
              reason="selected direct root does not match exact approved artifact identity/hash")
        bad_hash = copy.deepcopy(substituted)
        bad_hash["packages"][0]["archive"]["hashes"]["sha256"] = original["sha256"]
        check("different-wheel-with-original-hash", bad_hash, roots, binding,
              reason="lock member has no exact original-source mapping", old_accepts=False)
        ambiguous = copy.deepcopy(lock)
        ambiguous["packages"][0].pop("archive")
        ambiguous["packages"][0]["wheels"] = [{"url": Path(c["local"]).as_uri(), "hashes": {"sha256": c["sha256"]}} for c in inspected]
        check("ambiguous-compatible-root-files", ambiguous, roots, binding,
              reason="ambiguous/ineligible exact selected wheel", old_accepts=False)
        alternative_root = "root @ " + alternative["url"] + "#sha256=" + alternative["sha256"]
        check("conflicting-direct-roots-not-coalesced", lock, roots * 2,
              [*binding, (alternative_root, owner.artifact_identity(alternative))],
              reason="selected direct root does not match exact approved artifact identity/hash")
        inactive = raw_root + ' ; sys_platform == "win32"'
        empty = {"lock-version": "1.0", "created-by": "uv", "packages": []}
        check("inactive-direct-root-marker", empty, [Requirement('root==1.0 ; sys_platform == "win32"')],
              [(inactive, owner.artifact_identity(approved))])
        receipt = json.loads((directory / "completion-receipt.json").read_text())
        pipeline_checks = {
            "actual_uv_exact_candidate": payload["selected"][0]["id"] == original["id"]
                and payload["selected"][0]["url"] == original["url"]
                and payload["selected"][0]["sha256"] == original["sha256"],
            "actual_uv_no_ip_io": payload["observation"]["ip_operations"] == [],
            "held_using": payload["held_use_record"]["phase"]["state"] == "using",
            "exact_receipt": receipt["manifest"] == payload["held_use_record"]["manifest"]
                and receipt["verified_files"] == payload["held_use_record"]["files"],
            "settled_adopted": all(r["phase"]["state"] == "adopted" for r in outcome["settled_records"]),
            "only_owner_sources": [source.base + r["path"] for r in source.requests] == [c["url"] for c in candidates],
            "no_credentials": all(not r["authorization"] for r in source.requests),
        }
        evidence = {"controls": controls, "pipeline_checks": pipeline_checks,
                    "approved": candidates[0], "alternative": candidates[1], "requests": source.requests,
                    "uv_output_sha256": hashlib.sha256(original_lock).hexdigest(),
                    "projection_sha256": hashlib.sha256(projection).hexdigest(),
                    "frozen_checker_sha256": hashlib.sha256(frozen_source).hexdigest(),
                    "failed_assertions": sum(len(c["failed_assertions"]) for c in controls)
                        + sum(not passed for passed in pipeline_checks.values())}
        (directory / "binding-evidence.json").write_text(json.dumps(evidence, indent=2) + "\n")
        print(json.dumps(evidence, indent=2))
        owner.require(evidence["failed_assertions"] == 0, "direct-root binding controls failed")
    finally:
        source.close()


if __name__ == "__main__":
    main()
