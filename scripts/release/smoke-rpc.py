#!/usr/bin/env python3
"""Start the release backend in an isolated root and verify real HTTP routes."""

import argparse
import json
import os
from pathlib import Path
import re
import signal
import subprocess
import tempfile
import time
import urllib.error
import urllib.request

parser = argparse.ArgumentParser()
parser.add_argument("binary", type=Path)
parser.add_argument("--inference-disabled", action="store_true")
args = parser.parse_args()
binary = args.binary.resolve()
if binary.is_dir():
    binary /= "pumas-rpc.exe" if os.name == "nt" else "pumas-rpc"
# Bypass ambient HTTP proxies for loopback verification.
http = urllib.request.build_opener(urllib.request.ProxyHandler({}))
with tempfile.TemporaryDirectory(prefix="pumas-release-smoke-") as temporary:
    root = Path(temporary)
    log_path = root / "rpc.log"
    with log_path.open("wb") as log:
        process = subprocess.Popen(
            [str(binary), "--launcher-root", str(root), "--port", "0"],
            cwd=root,
            env={
                **os.environ,
                "XDG_CONFIG_HOME": str(root / "config"),
                "PUMAS_REGISTRY_DB_PATH": str(root / "registry.db"),
                "APPDATA": str(root / "config"),
            },
            stdout=log,
            stderr=subprocess.STDOUT,
        )
        try:
            deadline = time.monotonic() + 45
            while time.monotonic() < deadline:
                text = log_path.read_text(errors="replace")
                match = re.search(r"RPC_PORT=(\d+)", text)
                if process.poll() is not None:
                    raise RuntimeError(f"Backend exited with {process.returncode}:\n{text}")
                if match:
                    base = f"http://127.0.0.1:{match[1]}"
                    break
                time.sleep(0.1)
            else:
                raise RuntimeError(f"Backend did not announce a port:\n{text}")
            with http.open(base + "/health", timeout=10) as response:
                assert response.status == 200
            if args.inference_disabled:
                for route in ("/v1/models", "/v1/chat/completions", "/v1/images/generations"):
                    request = urllib.request.Request(base + route)
                    if route != "/v1/models":
                        request = urllib.request.Request(
                            base + route, data=b"{}", headers={"Content-Type": "application/json"}
                        )
                    try:
                        http.open(request, timeout=10)
                    except urllib.error.HTTPError as error:
                        assert error.code == 404, (route, error.code)
                    else:
                        raise AssertionError(f"Inference route unexpectedly enabled: {route}")
            print(json.dumps({"health": "passed", "inference_disabled": args.inference_disabled}))
        finally:
            if process.poll() is None:
                if os.name == "nt":
                    process.terminate()
                else:
                    process.send_signal(signal.SIGINT)
                try:
                    process.wait(timeout=10)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=10)
                    raise RuntimeError("Backend required forced shutdown")
