"""Run a bounded untrained lifecycle check on an exact produced Linux archive.

This uses the producing job's own bytes. It is not pretrained Nomic acceptance,
model-quality evidence, native closure qualification, or a release authorization.
"""

import argparse
import os
from pathlib import Path
import signal
import subprocess
import sys

import headless_inference as package
from synthetic_onnx_fixture import create_fixture


def run_lifecycle(command, output):
    process = subprocess.Popen(
        command, stdout=output, stderr=subprocess.STDOUT, start_new_session=True
    )
    try:
        returncode = process.wait(timeout=120)
    except subprocess.TimeoutExpired:
        # The verifier and its owned Pumas child share this new process group.
        # Stop both on timeout, before reaping the verifier and releasing its PID.
        try:
            os.killpg(process.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        process.wait(timeout=5)
        raise
    package.require(returncode == 0, "synthetic lifecycle or shutdown failed; log preserved")


def bind_result(expected_source, evidence, manifest, fixture, result):
    expected = manifest["compatibility"]["expected"]
    package.require(
        evidence["source"]["head"] == expected_source == expected["source_commit"]
        and evidence["source"]["tree"] == expected["source_tree"],
        "synthetic lifecycle source identity differs from candidate",
    )
    package.require(
        result["binary_sha256"] == manifest["files"]["pumas-rpc"]["sha256"],
        "synthetic lifecycle binary differs from the verified archive",
    )
    package.require(
        all(result.get(name) == "passed" for name in ("import", "load", "unload"))
        and type(result.get("embedding_dimensions")) is int
        and result["embedding_dimensions"] == 256
        and result.get("finite") is True
        and result.get("runtime", {}).get("mapping") == "packaged-file-identity-observed",
        "synthetic lifecycle result is incomplete",
    )
    return {
        "schema_version": 1,
        "qualification": "synthetic_execution_only",
        "pretrained_model_acceptance": False,
        "source": evidence["source"],
        "archive_sha256": evidence["archive_sha256"],
        "fixture": fixture,
        "lifecycle": result,
        "graceful_shutdown": "passed",
        "system_dependency_closure": "unqualified",
    }


def verify(candidate, expected_source):
    package.require(sys.platform == "linux", "synthetic packaged verification requires Linux")
    candidate = Path(candidate).resolve()
    evidence = package.decode((candidate / "build-evidence.json").read_bytes())
    archive = Path(evidence["archive"]).resolve()
    package.require(archive.parent == candidate, "candidate archive must remain in its output")
    extracted = candidate / "synthetic-extracted"
    manifest = package.extract_verified(archive, evidence["archive_sha256"], extracted)
    package.require(manifest["target"] == "linux-x86_64", "native Linux lifecycle required")
    package.require(
        manifest["compatibility"]["expected"]["source_commit"] == expected_source,
        "unexpected candidate source before execution",
    )
    fixture_root = candidate / "synthetic-fixture"
    fixture = create_fixture(fixture_root)
    log = candidate / "synthetic-packaged-lifecycle.log"
    with log.open("wb") as output:
        run_lifecycle(
            [
                sys.executable,
                str(Path(__file__).with_name("verify-packaged-onnx.py")),
                str(extracted / "pumas-rpc"),
                str(fixture_root),
            ],
            output,
        )
    # The child exits successfully only after its actual Pumas process has
    # unloaded and shut down cleanly; a printed result alone is insufficient.
    package.require(log.stat().st_size <= 1024 * 1024, "bounded lifecycle output required")
    results = [
        package.decode(line) for line in log.read_bytes().splitlines() if line.startswith(b"{")
    ]
    package.require(len(results) == 1, "one complete lifecycle result required")
    record = bind_result(expected_source, evidence, manifest, fixture, results[0])
    (candidate / "synthetic-lifecycle-evidence.json").write_bytes(package.canonical(record))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate-dir", type=Path, required=True)
    parser.add_argument("--expected-source", required=True)
    args = parser.parse_args()
    verify(args.candidate_dir, args.expected_source)
