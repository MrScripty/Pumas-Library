"""Qualify the actual RPC binary using tiny, disposable legacy library fixtures.

Never opens a user library or downloads model weights. Uses only stdlib tools.
"""
import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import time
import urllib.request
import uuid


def author_fixture(root: Path):
    library = root / "shared-resources/models"
    library.mkdir(parents=True)
    library_id = str(uuid.uuid4())
    (library / ".pumas-library-id.json").write_text(json.dumps({"schema_version": 1, "library_id": library_id}))
    document = {"schema_version": 7, "acquisitions": {}, "consumer_receipts": {},
                "downloads": [], "queue_admissions": {}, "released_queue_admissions": {},
                "admission_attempts": {}, "recovery_revocations": {}, "lifecycle_quarantines": {}}
    cases, payloads = [], []
    for name, count in (("two-files", 2), ("three-files", 3), ("llada-33-files", 33), ("paused-a", 1), ("paused-b", 1)):
        download_id, attempt_id = str(uuid.uuid4()), str(uuid.uuid4())
        old = f"diffusion/acme/{name}"
        new = f"diffusion/vit/{name}"
        files = [f"component/weights-{n}.safetensors" for n in range(count)]
        paused = name.startswith("paused")
        snapshot = {"download_id": download_id, "repo_id": f"acme/{name}", "filename": files[0],
                    "filenames": files, "dest_dir": str(library / old), "total_bytes": count * 7,
                    "status": "paused" if paused else "error", "revision": None,
                    "download_request": {"repo_id": f"acme/{name}", "family": "acme", "official_name": name,
                                         "model_type": "diffusion", "filenames": files},
                    "created_at": "2026-09-13T00:00:00Z", "known_sha256": None, "huggingface_evidence": None}
        document["downloads"].append(snapshot)
        document["queue_admissions"][download_id] = {"attempt_id": attempt_id, "domain": "ambient",
            "destination": {"library_root": f"uuid:{library_id}", "relative_target": old},
            "requested_payload_files": files, "execution_files": files,
            "position": {"ordinal": 0, "predecessor": None}}
        if paused:
            continue
        target = library / new
        (target / "component").mkdir(parents=True)
        (target / "metadata.json").write_text(json.dumps({"model_id": new, "model_type": "diffusion",
            "family": "vit", "architecture_family": "vit", "official_name": name, "cleaned_name": name,
            "repo_id": f"acme/{name}", "expected_files": files}))
        for name_in_model in files:
            file = target / name_in_model
            file.write_bytes(b"fixture")
            payloads.append(file)
        cases.append((download_id, attempt_id, new))
    (root / "launcher-data").mkdir()
    (root / "launcher-data/downloads.json").write_text(json.dumps(document))
    (root / "pumas-authored-location-fixture.json").write_text(json.dumps({"fixtureKind":"pumas.download-location-reconciliation","version":1}))
    return cases, payloads


def observe(files):
    return [(file.stat().st_dev, file.stat().st_ino, file.stat().st_size, file.stat().st_mtime_ns) for file in files]


def qualify(binary: Path, parent: Path):
    with tempfile.TemporaryDirectory(prefix="pumas-location-acceptance-", dir=parent) as temporary:
        root = Path(temporary)
        cases, files = author_fixture(root)
        observations = observe(files)
        log = root / "rpc.log"
        environment = {**os.environ, "XDG_CONFIG_HOME": str(root / "config"), "APPDATA": str(root / "config"),
                       "PUMAS_REGISTRY_DB_PATH": str(root / "registry.db")}
        with log.open("wb") as output:
            process = subprocess.Popen([str(binary), "--launcher-root", str(root), "--port", "0"],
                                       cwd=root, env=environment, stdout=output, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + 45
                while time.monotonic() < deadline:
                    match = re.search(r"RPC_PORT=(\d+)", log.read_text(errors="replace"))
                    if process.poll() is not None:
                        raise RuntimeError("Disposable backend failed startup: " + log.read_text(errors="replace")[-2500:])
                    if match:
                        break
                    time.sleep(0.1)
                else:
                    raise RuntimeError("Disposable backend readiness timeout")
                transport = urllib.request.build_opener(urllib.request.ProxyHandler({}))
                request_id = 0

                def call(method, params):
                    nonlocal request_id
                    request_id += 1
                    request = urllib.request.Request(f"http://127.0.0.1:{match[1]}/rpc", headers={"Content-Type": "application/json"},
                        data=json.dumps({"jsonrpc": "2.0", "id": request_id, "method": method, "params": params}).encode())
                    with transport.open(request, timeout=15) as response:
                        value = json.load(response)
                    assert "error" not in value, value
                    return value["result"]

                store = root / "launcher-data/downloads.json"
                initial = json.loads(store.read_text())
                for download_id, attempt_id, model_id in cases:
                    before = store.read_bytes()
                    report = call("inspect_retained_download", {"download_id": download_id})
                    assert report["status"] == "existing_local_candidate", report
                    assert store.read_bytes() == before, "Inspection must not publish metadata"
                    candidate = next(candidate for candidate in report["candidates"] if candidate["modelId"] == model_id)
                    request = {"downloadId": download_id, "attemptId": attempt_id, "candidateModelId": model_id,
                        "expectedObservation": candidate["observation"], "operationId": str(uuid.uuid4()),
                        "acceptance": "keep_existing_local_files", "acceptStorageUpgrade": report["requiresFormatUpgrade"]}
                    stale = {**request, "expectedObservation": "0" * 64}
                    assert call("resolve_retained_download", stale)["status"] == "stale_observation"
                    assert store.read_bytes() == before, "Stale preview must not publish or upgrade"
                    result = call("resolve_retained_download", request)
                    assert result["status"] == "resolved", result
                    assert call("resolve_retained_download", request)["status"] == "resolved"
                    assert observe(files) == observations, "Model files must remain at identical identities and stats"
                    assert call("inspect_retained_download", {"download_id": download_id})["status"] == "resolved"
                final = json.loads(store.read_text())
                assert final["schema_version"] == 7 and final["hf_model_partition_version"] == 6
                assert final["acquisitions"] == initial["acquisitions"]
                assert final["consumer_receipts"] == initial["consumer_receipts"]
                for original in initial["downloads"]:
                    identity = original["download_id"]
                    if original["status"] == "error":
                        assert final["retained_download_resolutions"][identity]["original"] == original
                        assert final["released_queue_admissions"][identity] == initial["queue_admissions"][identity]
                        assert not Path(original["dest_dir"]).exists()
                    else:
                        assert original in final["downloads"]
                        assert final["queue_admissions"][identity] == initial["queue_admissions"][identity]
                assert len(call("list_model_downloads", {})["downloads"]) == 2
                print("PASS: actual RPC resolves 2/3/33-file cases, preserves files/Error/history/paused claims, and refreshes immediately")
            finally:
                process.send_signal(signal.SIGINT)
                process.wait(timeout=15)
                assert process.returncode == 0, "Disposable backend must shut down cleanly"

        # Reopen the same disposable library and reobserve its durable history.
        with log.open("wb") as output:
            process = subprocess.Popen([str(binary), "--launcher-root", str(root), "--port", "0"],
                                       cwd=root, env=environment, stdout=output, stderr=subprocess.STDOUT)
            try:
                deadline = time.monotonic() + 45
                while time.monotonic() < deadline:
                    match = re.search(r"RPC_PORT=(\d+)", log.read_text(errors="replace"))
                    if process.poll() is not None:
                        raise RuntimeError("Reopened fixture failed startup")
                    if match:
                        break
                    time.sleep(0.1)
                else:
                    raise RuntimeError("Reopened fixture readiness timeout")
                for download_id, _, _ in cases:
                    assert call("inspect_retained_download", {"download_id": download_id})["status"] == "resolved"
                assert len(call("list_model_downloads", {})["downloads"]) == 2
                assert observe(files) == observations
                print("PASS: real process reopen preserves resolved history, files and unrelated paused downloads")
            finally:
                process.send_signal(signal.SIGINT)
                process.wait(timeout=15)
                assert process.returncode == 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rpc_binary", type=Path)
    parser.add_argument("--temporary-parent", type=Path, default=Path(os.environ.get("TMPDIR", tempfile.gettempdir())))
    args = parser.parse_args()
    qualify(args.rpc_binary.resolve(), args.temporary_parent.resolve())
