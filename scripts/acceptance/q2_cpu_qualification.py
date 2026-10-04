"""Hosted Linux CI recipe; frozen production source, disposable root, no mirrors."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import urllib.request
from urllib.parse import urlsplit

CANDIDATE = "1e83cfc95f9bc0f9f0e37d514371bd1f4364f9d8"
CANDIDATE_TREE = "bfe9937961b43feb77e50cfb25c83a45236f0f98"
MANIFEST = "docs/plans/torch-cross-platform-runtime-management/reports/v2.14.0-linux-current-source-cpu-rpc-restart-acceptance/resolution.json"
MANIFEST_SHA256 = "5d186ed7f2fe7573720ecc9d5ca7397f2aa9abc811cf842401d387b4e4d08cd4"
RUNTIME_METADATA = str(Path(MANIFEST).with_name("runtime.json"))
RUNTIME_METADATA_SHA256 = "a03ae995e9bb9e471a1dcc6bab1869a82da5517302a28ffdc4c7cd328401d322"
LOCAL_WORKER_SHA256 = "e20fe54c25adb2b7d80cad8b8858b0c7eeb76b05cbdbb599e954700b16f59ee0"
GIB = 1024**3
CPU = "import json,torch; t=torch.arange(1,4,dtype=torch.int64,device='cpu'); print(json.dumps({'version':torch.__version__,'device':str(t.device),'result':int((t*t).sum().item())}))"
CPU_RESULT = {"version": "2.14.0+cpu", "device": "cpu", "result": 14}
RECEIPTS = (
    "input-validation.json",
    "ci-preflight.json",
    "actual-wheel-identities.json",
    "custody.json",
    "requirements.txt",
    "resolution.json",
    "runtime.json",
    "probe.json",
    "local-hash-locked-requirements.txt",
    "qualification-result.json",
    "production-install.log",
    "local-only-install.log",
)


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def sha(path):
    value = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(64 * 1024), b""):
            value.update(chunk)
    return value.hexdigest()


def inputs(repo):
    assert (
        subprocess.check_output(["git", "-C", str(repo), "rev-parse", "HEAD"], text=True).strip()
        == CANDIDATE
    )
    assert not subprocess.check_output(["git", "-C", str(repo), "diff", "HEAD", "--"])
    assert (
        subprocess.check_output(
            ["git", "-C", str(repo), "rev-parse", "HEAD^{tree}"], text=True
        ).strip()
        == CANDIDATE_TREE
    )
    assert sha(repo / MANIFEST) == MANIFEST_SHA256
    assert sha(repo / RUNTIME_METADATA) == RUNTIME_METADATA_SHA256
    assert sha(repo / "torch-server/resolve_runtime.py") == LOCAL_WORKER_SHA256
    manifest = json.loads((repo / MANIFEST).read_text())
    assert (manifest["release"], manifest["build"], manifest["python"], manifest["adapter"]) == (
        "2.14.0",
        "cpu",
        "3.14",
        "none",
    )
    assert len(manifest["artifacts"]) == 25
    for artifact in manifest["artifacts"]:
        url = urlsplit(artifact["url"])
        assert url.scheme == "https" and url.hostname in (
            "download.pytorch.org",
            "download-r2.pytorch.org",
            "files.pythonhosted.org",
        )
        assert not (url.username or url.password or url.query or url.fragment)
    return manifest


def resource_budget(observations):
    lengths = [o["content_length"] for o in observations]
    assert all(
        length is not None and str(length).isdigit() and int(length) > 0 for length in lengths
    ), "Cannot bound downloads without every pinned wheel Content-Length"
    known_bytes = sum(map(int, lengths))
    return known_bytes, max(3 * GIB, 5 * known_bytes + GIB)


def preflight(repo, output, manifest):
    # A normal hosted runner needs separate build headroom; never free its unrelated caches.
    observations = []
    artifact = None
    try:
        assert shutil.disk_usage(repo).free >= 12 * GIB, (
            "Need 12 GiB free for fresh Rust build plus runtime; use an adequately provisioned ordinary runner"
        )
        for artifact in manifest["artifacts"]:
            with urllib.request.urlopen(
                urllib.request.Request(artifact["url"], method="HEAD"), timeout=20
            ) as response:
                observations.append(
                    {
                        "name": artifact["name"],
                        "url": artifact["url"],
                        "status": response.status,
                        "content_length": response.headers.get("content-length"),
                    }
                )
                assert response.status == 200
        known_bytes, budget = resource_budget(observations)
        assert shutil.disk_usage(repo).free >= 9 * GIB + budget, (
            "Insufficient fresh build plus measured runtime headroom"
        )
    except Exception as error:
        write(
            output / "ci-preflight.json",
            {
                "allowed": False,
                "manifest_sha256": MANIFEST_SHA256,
                "observations": observations,
                "blocked_artifact": artifact,
                "error": str(error),
                "payload_downloaded": False,
            },
        )
        raise  # No retry, alternate source, proxy change, or large download.
    write(
        output / "ci-preflight.json",
        {
            "allowed": True,
            "manifest_sha256": MANIFEST_SHA256,
            "observations": observations,
            "known_payload_bytes": known_bytes,
            "missing_lengths": 0,
            "runtime_budget_bytes": budget,
            "fresh_build_minimum_free_bytes": 9 * GIB + budget,
            "bootstrap_and_managed_python_estimate_bytes": GIB,
            "ram_estimate_gib": [2, 4],
            "build_jobs": 2,
            "payload_downloaded": False,
        },
    )


def identity(artifact):
    return (artifact["name"], artifact["version"], artifact["url"], artifact["sha256"].lower())


def local_handoff(root, output, expected, expected_python):
    runtime = root / "torch-versions/v2.14.0"
    resolution = json.loads((runtime / "resolution.json").read_text())
    write(output / "actual-wheel-identities.json", resolution)
    shutil.copyfile(runtime / "runtime.json", output / "runtime.json")
    assert sorted(map(identity, resolution["artifacts"])) == sorted(
        map(identity, expected["artifacts"])
    ), (
        "Resolved closure differs from the reviewed repository evidence; retain it for review, do not silently substitute"
    )
    managed = json.loads((runtime / "runtime.json").read_text())["managed_python"]
    assert managed["provider"] == expected_python["provider"], "Managed provider identity drift"
    assert managed["distribution"] == expected_python["distribution"], (
        "Managed CPython distribution drift"
    )
    executable = Path(managed["executable"]["path"])
    assert executable.resolve().is_relative_to(root.resolve())
    assert (
        sha(executable)
        == managed["executable"]["sha256"]
        == expected_python["executable"]["sha256"]
    ), "Retained CPython executable hash drift"
    custody = json.loads((root / "custody.json").read_text())
    assert len(custody["records"]) == 1
    record = next(iter(custody["records"].values()))
    receipt = custody["receipts"][record["id"]]
    assert record["phase"]["state"] == "adopted" and receipt["owner"] == "runtime.torch"
    assert receipt["acquisition_id"] == record["id"]
    assert receipt["use_lease"] == record["phase"]["lease"]
    assert receipt["demand"] == record["demand"] and receipt["workspace"] == record["workspace"]
    assert receipt["payload"]["interpreter_sha256"] == expected_python["executable"]["sha256"]
    assert (
        receipt["manifest"] == record["manifest"] and receipt["verified_files"] == record["files"]
    )
    relative = Path(record["workspace"]["relative_target"])
    assert not relative.is_absolute() and ".." not in relative.parts
    directory = root / "torch-versions" / relative
    assert directory.resolve().is_relative_to(root.resolve()) and not directory.is_symlink()
    wheels = []
    assert len(record["manifest"]["files"]) == len(resolution["artifacts"]) == len(record["files"])
    for artifact, selected, verified in zip(
        resolution["artifacts"], record["manifest"]["files"], record["files"]
    ):
        path = directory / selected["logical_path"]
        assert not path.is_symlink() and path.is_file()
        assert path.resolve().is_relative_to(directory.resolve())
        assert verified["path"] == selected["logical_path"]
        assert (
            selected["source_key"]
            == f"{artifact['name']}:{artifact['version']}:{artifact['sha256']}"
        )
        assert sha(path) == artifact["sha256"].lower() == verified["sha256"]
        assert path.stat().st_size == verified["bytes"]
        wheels.append(
            {
                "name": artifact["name"],
                "version": artifact["version"],
                "path": str(path),
                "sha256": artifact["sha256"],
            }
        )
    handoff = root / "offline-local-wheels.json"
    write(handoff, {"schema_version": 1, "wheels": wheels})
    for name in (
        "requirements.txt",
        "resolution.json",
        "pip-resolution.json",
        "probe.json",
    ):
        if (runtime / name).is_file():
            shutil.copyfile(runtime / name, output / name)
    shutil.copyfile(root / "custody.json", output / "custody.json")
    return runtime, handoff


def local_only_install(root, runtime, handoff, output):
    """Run the unchanged local-wheel worker directly; this is pip policy evidence."""
    assert sha(runtime / "resolve_runtime.py") == LOCAL_WORKER_SHA256, "Local worker source drift"
    python = runtime / "venv/bin/python"
    local = root / "local-only-venv"
    scratch = root / "local-only-tmp"
    scratch.mkdir()
    environment = os.environ.copy()
    environment["TMPDIR"] = str(scratch)
    subprocess.run([str(python), "-I", "-m", "venv", str(local)], check=True, env=environment)
    local_python = local / "bin/python"
    command = [
        str(local_python),
        "-I",
        str(runtime / "resolve_runtime.py"),
        "--_pumas-local-wheel-install",
        str(handoff),
    ]
    # Await the product worker and its pip children. The workflow bounds the
    # entire disposable run to 45 minutes; no separate parent-only kill timeout.
    with (output / "local-only-install.log").open("w") as log:
        subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True, env=environment)
    result = subprocess.run(
        [str(local_python), "-I", "-c", CPU],
        capture_output=True,
        text=True,
        check=True,
        timeout=60,
        env=environment,
    )
    proof = json.loads(result.stdout)
    assert proof == CPU_RESULT
    shutil.copyfile(
        handoff.with_suffix(".requirements.txt"), output / "local-hash-locked-requirements.txt"
    )
    return {
        "status": "passed",
        "scope": "additional empty-venv install using retained local wheels and pip no-index policy after production settlement",
        "cpu_operation": proof,
        "command": command,
        "frozen_worker_sha256": LOCAL_WORKER_SHA256,
        "os_enforced_network_isolation": False,
        "original_child_custody_denial_proof": False,
        "pip_args_evidence": "derived from the exact frozen worker source, not a separate process-argv observation",
        "frozen_worker_pip_args": [
            str(local_python),
            "-I",
            "-m",
            "pip",
            "--isolated",
            "install",
            "--no-index",
            "--no-deps",
            "--require-hashes",
            "--only-binary=:all:",
            "--no-cache-dir",
            "--disable-pip-version-check",
            "-r",
            str(handoff.with_suffix(".requirements.txt")),
        ],
    }


def complete_result(result, proof, local):
    assert proof == CPU_RESULT
    result.update(
        real_production_cpu_operation=proof,
        local_only_install=local,
        os_enforced_network_isolation=False,
        original_child_custody_denial_proof=False,
    )
    assert local["status"] == "passed", "Local-only installation failed; qualification cannot pass"
    result["success"] = True


def collect_receipts(output, destination):
    """Allowlisted small records; never traverse an installation or upload payloads."""
    assert not destination.exists(), "Receipt destination must be fresh"
    destination.mkdir(parents=True)
    index = {
        "commit": CANDIDATE,
        "workflow_revision": os.environ.get("GITHUB_SHA"),
        "run_id": os.environ.get("GITHUB_RUN_ID"),
        "files": [],
        "AC11": "open pending independent review",
        "AC12": "open pending independent review",
    }
    for name in RECEIPTS:
        path = output / name
        if not path.is_file() or path.is_symlink():
            continue
        size = path.stat().st_size
        record = {"name": name, "original_bytes": size, "original_sha256": sha(path)}
        if name.endswith(".log"):
            with path.open("rb") as log:
                log.seek(max(0, size - 64 * 1024))
                data = log.read()
            record["truncated_to_tail"] = size > len(data)
        elif size <= 512 * 1024:
            data = path.read_bytes()
        else:
            record["omitted"] = "Exceeds 512 KiB per-record bound; original remains runner-local"
            index["files"].append(record)
            continue
        (destination / name).write_bytes(data)
        record.update(exported_bytes=len(data), exported_sha256=hashlib.sha256(data).hexdigest())
        index["files"].append(record)
    write(destination / "receipt-index.json", index)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("check-inputs", "preflight", "run", "receipts"))
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--binary", type=Path)
    parser.add_argument("--receipts", type=Path)
    args = parser.parse_args()
    repo = args.repo.resolve()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    if args.mode == "receipts":
        assert args.receipts is not None, "--receipts is required"
        collect_receipts(output, args.receipts.resolve())
        return
    expected = inputs(repo)
    if args.mode == "check-inputs":
        write(
            output / "input-validation.json",
            {
                "commit": CANDIDATE,
                "tree": CANDIDATE_TREE,
                "manifest_sha256": MANIFEST_SHA256,
                "runtime_metadata_sha256": RUNTIME_METADATA_SHA256,
                "local_worker_sha256": LOCAL_WORKER_SHA256,
                "wheel_count": 25,
                "network_requests": 0,
                "real_installation": "not run",
            },
        )
        return
    if args.mode == "preflight":
        preflight(repo, output, expected)
        return
    plan = json.loads((output / "ci-preflight.json").read_text())
    assert plan["allowed"] and plan["manifest_sha256"] == MANIFEST_SHA256
    assert shutil.disk_usage(repo).free >= plan["runtime_budget_bytes"]
    assert args.binary is not None and args.binary.is_file()
    root = Path(tempfile.mkdtemp(prefix="pumas-q2-real-cpu-", dir=output.parent))
    environment = os.environ.copy()
    environment["XDG_CONFIG_HOME"] = str(output / "test-owned-xdg")
    result = {
        "commit": CANDIDATE,
        "tree": CANDIDATE_TREE,
        "manifest_sha256": MANIFEST_SHA256,
        "runtime_metadata_sha256": RUNTIME_METADATA_SHA256,
        "qualification_recipe_sha256": sha(Path(__file__)),
        "driver_binary_sha256": sha(args.binary),
        "workflow_revision": os.environ.get("GITHUB_SHA"),
        "run_id": os.environ.get("GITHUB_RUN_ID"),
        "root": str(root),
        "success": False,
        "os_enforced_network_isolation": False,
        "original_child_custody_denial_proof": False,
        "AC11": "open pending independent review",
        "AC12": "open pending independent review",
    }
    try:
        with (output / "production-install.log").open("w") as log:
            try:
                subprocess.run(
                    [str(args.binary.resolve()), str(root)],
                    cwd=repo,
                    env=environment,
                    stdout=log,
                    stderr=subprocess.STDOUT,
                    check=True,
                )
            finally:
                if (root / "custody.json").is_file():
                    shutil.copyfile(root / "custody.json", output / "custody.json")
        expected_python = json.loads((repo / RUNTIME_METADATA).read_text())["managed_python"]
        runtime, handoff = local_handoff(root, output, expected, expected_python)
        python = runtime / "venv/bin/python"
        proof = json.loads(
            subprocess.check_output([str(python), "-I", "-c", CPU], text=True, timeout=60)
        )
        complete_result(result, proof, local_only_install(root, runtime, handoff, output))
        result["cleanup"] = (
            "owned successful test root removed after both production owners drained"
        )
        shutil.rmtree(root)
    except BaseException as error:
        result.update(success=False, error=str(error), retained_root=str(root))
        raise
    finally:
        write(output / "qualification-result.json", result)


if __name__ == "__main__":
    main()
