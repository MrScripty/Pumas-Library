"""Hosted Linux CI recipe; frozen production source, disposable root, no mirrors."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import socket
import subprocess
import sys
import tempfile
import threading
import urllib.request
from urllib.parse import urlsplit

CANDIDATE = "1e83cfc95f9bc0f9f0e37d514371bd1f4364f9d8"
CANDIDATE_TREE = "bfe9937961b43feb77e50cfb25c83a45236f0f98"
MANIFEST = "docs/plans/torch-cross-platform-runtime-management/reports/v2.14.0-linux-current-source-cpu-rpc-restart-acceptance/resolution.json"
MANIFEST_SHA256 = "5d186ed7f2fe7573720ecc9d5ca7397f2aa9abc811cf842401d387b4e4d08cd4"
RUNTIME_METADATA = str(Path(MANIFEST).with_name("runtime.json"))
RUNTIME_METADATA_SHA256 = "a03ae995e9bb9e471a1dcc6bab1869a82da5517302a28ffdc4c7cd328401d322"
GIB = 1024**3
CPU = "import json,torch; t=torch.arange(1,4,dtype=torch.int64,device='cpu'); print(json.dumps({'version':torch.__version__,'device':str(t.device),'result':int((t*t).sum().item())}))"
CPU_RESULT = {"version": "2.14.0+cpu", "device": "cpu", "result": 14}
RECEIPTS = (
    "input-validation.json",
    "sandbox-preflight.json",
    "ci-preflight.json",
    "actual-wheel-identities.json",
    "custody.json",
    "requirements.txt",
    "resolution.json",
    "runtime.json",
    "probe.json",
    "network-control.json",
    "local-hash-locked-requirements.txt",
    "qualification-result.json",
    "production-install.log",
    "offline-install.log",
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


def sandbox_preflight(output):
    """Check already available, unprivileged isolation before any large downloads."""
    result = {"status": "blocked", "host_net_namespace": os.readlink("/proc/self/ns/net")}
    bwrap = shutil.which("bwrap")
    if bwrap is None:
        result["reason"] = "bwrap is not installed on this runner"
    else:
        with socket.socket() as listener:
            listener.bind(("127.0.0.1", 0))
            listener.listen()
            listener.settimeout(2)
            port = listener.getsockname()[1]
            with socket.create_connection(("127.0.0.1", port), timeout=2):
                pass
            connection, _ = listener.accept()
            connection.close()
            probe = (
                "import json,os,socket; r={'net_namespace':os.readlink('/proc/self/ns/net')};\n"
                f"try: socket.create_connection(('127.0.0.1',{port}),timeout=2); r['connected']=True\n"
                "except OSError: r['connected']=False\nprint(json.dumps(r))"
            )
            check = subprocess.run(
                [
                    bwrap,
                    "--unshare-net",
                    "--unshare-pid",
                    "--die-with-parent",
                    "--ro-bind",
                    "/",
                    "/",
                    "--dev",
                    "/dev",
                    "--proc",
                    "/proc",
                    "--",
                    sys.executable,
                    "-I",
                    "-c",
                    probe,
                ],
                capture_output=True,
                text=True,
                timeout=10,
            )
            result.update(returncode=check.returncode, stderr=check.stderr)
            if check.returncode:
                result["reason"] = (
                    "Existing runner policy denies the process-local network namespace"
                )
            else:
                proof = json.loads(check.stdout)
                assert (
                    not proof["connected"]
                    and proof["net_namespace"] != result["host_net_namespace"]
                )
                result.update(status="passed", proof=proof, positive_control=True)
    if result["status"] != "passed":
        result["required_authority"] = (
            "Owner review of an ordinary runner with working unprivileged bwrap is required; this job does not install tools or change host security/network policy"
        )
    write(output / "sandbox-preflight.json", result)
    return result


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
        assert sandbox_preflight(output)["status"] == "passed", (
            "Network isolation unavailable; qualification is blocked before payload downloads"
        )
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


def offline_install(root, runtime, handoff, output):
    """Additional real package-only proof, after the production use has settled."""
    listener = socket.socket()
    listener.bind(("127.0.0.1", 0))
    listener.listen()
    listener.settimeout(0.1)
    port = listener.getsockname()[1]
    observed = []
    done = threading.Event()

    def monitor():
        while not done.is_set():
            try:
                connection, _ = listener.accept()
                observed.append("connection")
                connection.close()
            except socket.timeout:
                continue

    observer = threading.Thread(target=monitor)
    observer.start()
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=2):
            pass
        bwrap = shutil.which("bwrap")
        if bwrap is None:
            return {
                "status": "blocked",
                "reason": "bwrap unavailable; no installation sandbox attempted",
            }
        scratch = root / "offline-tmp"
        scratch.mkdir()
        sandbox = [
            bwrap,
            "--unshare-net",
            "--unshare-pid",
            "--die-with-parent",
            "--ro-bind",
            "/",
            "/",
            "--dev",
            "/dev",
            "--proc",
            "/proc",
            "--bind",
            str(root),
            str(root),
            "--setenv",
            "TMPDIR",
            str(scratch),
            "--",
        ]
        probe = (
            "import json,os,socket; result={'net_namespace':os.readlink('/proc/self/ns/net')};\ntry: socket.create_connection(('127.0.0.1',"
            + str(port)
            + "),timeout=2); result['connected']=True\nexcept OSError as e: result.update(connected=False,error=str(e))\nprint(json.dumps(result))"
        )
        python = runtime / "venv/bin/python"
        control = subprocess.run(
            [*sandbox, str(python), "-I", "-c", probe], capture_output=True, text=True, timeout=10
        )
        write(
            output / "network-control.json",
            {
                "args": sandbox,
                "returncode": control.returncode,
                "stdout": control.stdout,
                "stderr": control.stderr,
                "parent_net_namespace": os.readlink("/proc/self/ns/net"),
            },
        )
        if control.returncode:
            return {
                "status": "blocked",
                "reason": control.stderr,
                "scope": "existing namespace unavailable; no bypass",
            }
        check = json.loads(control.stdout)
        assert not check["connected"] and check["net_namespace"] != os.readlink("/proc/self/ns/net")
        offline = root / "offline-venv"
        subprocess.run([str(python), "-I", "-m", "venv", str(offline)], check=True, timeout=60)
        offline_python = offline / "bin/python"
        command = [
            *sandbox,
            str(offline_python),
            "-I",
            str(runtime / "resolve_runtime.py"),
            "--_pumas-local-wheel-install",
            str(handoff),
        ]
        with (output / "offline-install.log").open("w") as log:
            subprocess.run(command, stdout=log, stderr=subprocess.STDOUT, check=True, timeout=600)
        result = subprocess.run(
            [*sandbox, str(offline_python), "-I", "-c", CPU],
            capture_output=True,
            text=True,
            check=True,
            timeout=60,
        )
        proof = json.loads(result.stdout)
        assert proof == CPU_RESULT
        # Drain the monitor before asserting its held-source observation below.
        done.set()
        observer.join(timeout=2)
        assert not observer.is_alive() and observed == ["connection"]
        shutil.copyfile(
            handoff.with_suffix(".requirements.txt"), output / "local-hash-locked-requirements.txt"
        )
        return {
            "status": "passed",
            "scope": "additional empty-venv package-only installation, not network isolation of the original production publication",
            "cpu_operation": proof,
            "command": command,
            "observed_connections": len(observed),
            "actual_pip_args": [
                str(offline_python),
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
    finally:
        done.set()
        observer.join(timeout=2)
        listener.close()


def complete_result(result, proof, offline):
    assert proof == CPU_RESULT
    result.update(real_production_cpu_operation=proof, no_network=offline)
    assert offline["status"] == "passed", (
        "Local-only no-network installation is blocked; qualification cannot pass"
    )
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
        complete_result(result, proof, offline_install(root, runtime, handoff, output))
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
