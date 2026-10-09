#!/usr/bin/env python3
"""Verify a Linux packaged RPC with a supplied real Nomic ONNX fixture.

Usage: verify-packaged-onnx.py /path/to/packaged/pumas-rpc /path/to/nomic-model
The fixture is copied into an isolated library and is never modified.
"""

import json
import math
import re
import signal
import subprocess
import sys
import tempfile
import time
import urllib.request
from pathlib import Path

from packaged_onnx_support import child_environment, loaded_runtime, verify_runtime
from headless_inference import require, sha256

binary = Path(sys.argv[1]).resolve()
fixture = Path(sys.argv[2]).resolve()
verify_runtime(binary.parent)
http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
with tempfile.TemporaryDirectory(prefix="pumas-packaged-inference-") as tmp:
    root = Path(tmp)
    for name in ("home", "tmp", "config", "data", "cache", "state", "run"):
        (root / name).mkdir(mode=0o700)
    model = root / "shared-resources/models/embedding/fixture/nomic"
    model.mkdir(parents=True)
    for name in [
        "config.json",
        "tokenizer.json",
        "tokenizer_config.json",
        "special_tokens_map.json",
    ]:
        if (fixture / name).exists():
            subprocess.run(
                ["cp", "--reflink=auto", str(fixture / name), str(model / name)], check=True
            )
    subprocess.run(
        ["cp", "--reflink=auto", str(fixture / "onnx/model_fp16.onnx"), str(model / "model.onnx")],
        check=True,
    )
    log = (root / "rpc.log").open("w+")
    p = subprocess.Popen(
        [str(binary), "--launcher-root", str(root), "--port", "0"],
        stdout=log,
        stderr=subprocess.STDOUT,
        cwd=root,
        env=child_environment(root),
    )
    try:
        for _ in range(450):
            text = (root / "rpc.log").read_text()
            m = re.search(r"RPC_PORT=(\d+)", text)
            if m:
                break
            if p.poll() is not None:
                raise RuntimeError(text)
            time.sleep(0.1)
        else:
            raise RuntimeError(text)
        base = "http://127.0.0.1:" + m[1]

        def post(path, data):
            with http.open(
                urllib.request.Request(
                    base + path,
                    data=json.dumps(data).encode(),
                    headers={"Content-Type": "application/json"},
                ),
                timeout=120,
            ) as r:
                return json.load(r)

        def rpc(method, params={}):
            x = post("/rpc", {"jsonrpc": "2.0", "id": 1, "method": method, "params": params})
            if "error" in x:
                raise RuntimeError((method, x))
            return x["result"]

        result = rpc(
            "import_model_in_place",
            {
                "model_dir": str(model),
                "official_name": "Nomic fixture",
                "family": "nomic_bert",
                "model_type": "embedding",
            },
        )
        print("import", json.dumps(result)[:1000], flush=True)
        require(result.get("success") is True, f"Import failed: {result}")
        model_id = result["model_id"]
        profile = "onnx-verification"
        print(
            "profile",
            rpc(
                "upsert_runtime_profile",
                {
                    "profile": {
                        "profile_id": profile,
                        "provider": "onnx_runtime",
                        "provider_mode": "onnx_serve",
                        "management_mode": "managed",
                        "name": "Installer verification",
                    }
                },
            ),
            flush=True,
        )
        print("launch", rpc("launch_runtime_profile", {"profile_id": profile}), flush=True)
        loaded = rpc(
            "serve_model",
            {
                "request": {
                    "model_id": model_id,
                    "config": {
                        "provider": "onnx_runtime",
                        "profile_id": profile,
                        "device_mode": "auto",
                        "keep_loaded": True,
                        "model_alias": "verification-nomic",
                    },
                }
            },
        )
        require(loaded.get("loaded") is True, f"Load failed: {loaded}")
        embedding = post(
            "/v1/embeddings",
            {
                "model": "verification-nomic",
                "input": ["search_query: hello world"],
                "dimensions": 256,
            },
        )
        vector = embedding["data"][0]["embedding"]
        require(
            len(vector) == 256
            and all(type(x) in (int, float) and math.isfinite(x) for x in vector),
            "Embedding must contain 256 finite numeric values",
        )
        runtime_evidence = loaded_runtime(p.pid, binary.parent)
        unloaded = rpc(
            "unserve_model",
            {
                "request": {
                    "model_id": model_id,
                    "provider": "onnx_runtime",
                    "profile_id": profile,
                    "model_alias": "verification-nomic",
                }
            },
        )
        require(unloaded.get("unloaded") is True, f"Unload failed: {unloaded}")
        print(
            json.dumps(
                {
                    "binary": str(binary),
                    "binary_sha256": sha256(binary),
                    "runtime": runtime_evidence,
                    "import": "passed",
                    "load": "passed",
                    "embedding_dimensions": len(vector),
                    "finite": True,
                    "unload": "passed",
                }
            ),
            flush=True,
        )
    finally:
        if p.poll() is None:
            p.send_signal(signal.SIGINT)
            try:
                p.wait(timeout=20)
            except subprocess.TimeoutExpired:
                p.kill()
                p.wait(timeout=10)
                raise RuntimeError("Packaged backend required forced shutdown")
        log.close()
        require(p.returncode == 0, f"Packaged backend exited with {p.returncode}")
