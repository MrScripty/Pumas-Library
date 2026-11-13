#!/usr/bin/env python3
"""Verify a native packaged RPC with a supplied Nomic-layout ONNX fixture.

Usage: verify-packaged-onnx.py /path/to/packaged/pumas-rpc /path/to/nomic-model

The package bundles no runtime. Through the RPC's own JSON-RPC API this installs
the newest stable `+cpu` ONNX Runtime with Pumas's version manager (a network
download), pins a managed CPU profile to it, then imports, serves, embeds and
unloads the fixture. The fixture is copied into an isolated library and is never
modified. CI only: never run from local unit tests.
"""

import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

from headless_inference import sha256
import packaged_onnx_support as support

FIXTURE_FILES = [
    "config.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "special_tokens_map.json",
]


def copy_fixture(fixture, root):
    model = root / "shared-resources/models/embedding/fixture/nomic"
    model.mkdir(parents=True)
    for name in FIXTURE_FILES:
        if (fixture / name).exists():
            shutil.copyfile(fixture / name, model / name)
    shutil.copyfile(fixture / "onnx/model_fp16.onnx", model / "model.onnx")
    return model


def wait_for_port(process, log_path):
    for _ in range(450):
        text = log_path.read_text()
        match = re.search(r"RPC_PORT=(\d+)", text)
        if match:
            return int(match[1])
        if process.poll() is not None:
            raise RuntimeError(f"packaged RPC exited before announcing its port:\n{text}")
        time.sleep(0.1)
    raise RuntimeError(f"packaged RPC did not announce its port:\n{log_path.read_text()}")


def clients(base):
    http = urllib.request.build_opener(urllib.request.ProxyHandler({}))

    def post(path, data):
        with http.open(
            urllib.request.Request(
                base + path,
                data=json.dumps(data).encode(),
                headers={"Content-Type": "application/json"},
            ),
            timeout=120,
        ) as response:
            return json.load(response)

    def rpc(method, params=None):
        # Startup reconciliation can briefly hold the library; Pumas reports
        # that as a retryable conflict ("try again").
        deadline = time.monotonic() + 120
        while True:
            reply = post(
                "/rpc", {"jsonrpc": "2.0", "id": 1, "method": method, "params": params or {}}
            )
            error = reply.get("error")
            if error is None:
                return reply["result"]
            conflict = (error.get("data") or {}).get("class") == "conflict"
            if not conflict or time.monotonic() >= deadline:
                raise RuntimeError((method, reply))
            time.sleep(0.5)

    return rpc, post


def main(binary, fixture):
    binary, fixture = Path(binary).resolve(), Path(fixture).resolve()
    support.require_no_bundled_runtime(binary.parent)
    with tempfile.TemporaryDirectory(prefix="pumas-packaged-inference-") as tmp:
        root = Path(tmp)
        for name in ("home", "tmp", "config", "data", "cache", "state", "run"):
            (root / name).mkdir(mode=0o700)
        model = copy_fixture(fixture, root)
        fixture_hashes = {path.name: sha256(path) for path in model.iterdir() if path.is_file()}
        log_path = root / "rpc.log"
        with log_path.open("w+") as log:
            process = subprocess.Popen(
                [str(binary), "--launcher-root", str(root), "--port", "0"],
                stdout=log,
                stderr=subprocess.STDOUT,
                cwd=root,
                env=support.child_environment(root),
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0,
            )
            try:
                port = wait_for_port(process, log_path)
                rpc, post = clients(f"http://127.0.0.1:{port}")
                result = support.exercise_installed_runtime(
                    rpc,
                    post,
                    model,
                    lambda tag: support.installed_runtime(process.pid, root, tag),
                )
                support.require_no_runtime_telemetry_storage(root)
                print(
                    json.dumps(
                        {
                            "binary": str(binary),
                            "binary_sha256": sha256(binary),
                            "fixture_sha256": fixture_hashes,
                            "fixture_acceptance": "caller-owned; lifecycle does not establish model provenance",
                            "bundled_runtimes": [],
                            "runtime_telemetry_storage": "absent",
                            **result,
                        }
                    ),
                    flush=True,
                )
            finally:
                support.stop_process(process)


if __name__ == "__main__":
    if len(sys.argv) != 3:
        raise SystemExit(__doc__)
    main(sys.argv[1], sys.argv[2])
