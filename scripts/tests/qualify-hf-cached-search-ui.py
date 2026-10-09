#!/usr/bin/env python3
"""Actual loopback RPC + mounted React workflow; no HF/provider/model download.

Owned anonymous metadata fixtures and an ordinary exact SQLite search fixture.
The child has isolated config and a rejecting loopback proxy for upstream calls.
"""
import argparse
from datetime import datetime, timedelta, timezone
import hashlib
import http.server
import json
import os
from pathlib import Path
import queue
import signal
import socket
import sqlite3
import subprocess
import tempfile
import threading
import urllib.request

ROOT = Path(__file__).resolve().parents[2]


def check(condition, message):
    if not condition:
        raise AssertionError(message)


class RejectUpstream(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self):
        super().__init__(("127.0.0.1", 0), RejectHandler)
        self.calls = []


class RejectHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *_):
        pass

    def reject(self):
        self.server.calls.append({"method": self.command, "target": self.path})
        self.send_response(503)
        self.send_header("Content-Length", "0")
        self.send_header("Connection", "close")
        self.end_headers()

    do_CONNECT = do_GET = do_HEAD = do_POST = reject


def seed_details(root):
    cache = root / "launcher-data/cache/hf"
    cache.mkdir(parents=True, exist_ok=True)
    now = datetime.now(timezone.utc)
    rows = [
        ("acme/Alpha", False, False, now, "a" * 40),
        ("acme/Beta", False, False, now - timedelta(days=2), "b" * 40),
        ("acme/NoRevision", False, False, now, None),
        ("private/Hidden", True, False, now, "c" * 40),
        ("gated/Hidden", False, "auto", now, "d" * 40),
        ("unknown/Hidden", None, None, now, "e" * 40),
    ]
    for repo, private, gated, observed, revision in rows:
        url = "https://huggingface.co/api/models/" + repo
        body = {"modelId": repo, "private": private, "gated": gated,
                "pipeline_tag": "text-generation", "tags": ["gguf", "Q4_K_M", "multilingual"],
                "cardData": {"license": "apache-2.0", "context": "owned-wire-fixture",
                             "pumas_discovery": {"source": "provider-spoof"}}}
        if revision is not None:
            body["sha"] = revision
        observation = {"version": 1, "url": url, "body": body, "etag": '"owned-fixture"',
                       "last_modified": None, "validated_at": observed.isoformat(),
                       "ttl_seconds": 3600, "blocked_until": None}
        name = "hf_" + hashlib.sha256(url.encode()).hexdigest() + "_metadata_v1.json"
        (cache / name).write_text(json.dumps(observation, indent=2) + "\n")
    return {p.name: p.read_bytes() for p in cache.glob("*_metadata_v1.json")}


def seed_ordinary_search(root):
    # The launched producer initializes its actual schema first. This is fixture
    # data for the preserved exact-search path, not a new catalog or publisher.
    now = datetime.now(timezone.utc).isoformat()
    database = root / "shared-resources/cache/search.sqlite"
    with sqlite3.connect(database) as db:
        db.execute("INSERT OR REPLACE INTO repo_details (repo_id,last_modified,name,developer,kind,formats,quants,download_options,url,downloads,total_size_bytes,cached_at,last_accessed,data_size_bytes) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?)",
                   ("acme/OnlineFixture", now, "OnlineFixture", "acme", "text-generation",
                    '["gguf"]', '["Q4_K_M"]', '[{"quant":"Q4_K_M","size_bytes":1048576,"file_group":null}]',
                    "https://huggingface.co/acme/OnlineFixture", 1, 1048576, now, now, 1024))
        db.execute("INSERT OR REPLACE INTO search_cache (query_normalized,kind,result_limit,result_offset,result_repo_ids,searched_at) VALUES (?,?,?,?,?,?)",
                   ("online-fixture", None, 25, 0, '["acme/OnlineFixture"]', now))


def rpc(base, method, params, request_id=1):
    check(base.startswith("http://127.0.0.1:"), "qualification only contacts loopback")
    request = urllib.request.Request(base + "/rpc", data=json.dumps({
        "jsonrpc": "2.0", "id": request_id, "method": method, "params": params}).encode(),
        headers={"Content-Type": "application/json"})
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}))
    with opener.open(request, timeout=8) as response:
        document = json.load(response)
    check(document.get("id") == request_id and "error" not in document, "RPC envelope failure: " + str(document))
    return document["result"]


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--rpc", type=Path, required=True)
    parser.add_argument("--only-rpc", action="store_true")
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    output = args.output or Path(tempfile.mkdtemp(prefix="pumas-hf-ui-"))
    output.mkdir(parents=True, exist_ok=True)
    root = output / "library"
    root.mkdir(exist_ok=True)
    config = output / "config"
    config.mkdir(exist_ok=True)
    ambient_token = Path.home() / ".cache/huggingface/token"
    check(not ambient_token.exists() and not ambient_token.is_symlink(),
          "Anonymous fixture cannot launch with an existing CLI token; no token was read or changed.")
    check(not (config / "pumas").exists(), "Owned XDG config must be new and contain no credentials")
    before = seed_details(root)
    proxy = RejectUpstream()
    worker = threading.Thread(target=proxy.serve_forever, daemon=True)
    worker.start()
    proxy_url = f"http://127.0.0.1:{proxy.server_port}"
    environment = dict(os.environ, XDG_CONFIG_HOME=str(config), HTTP_PROXY=proxy_url,
                       HTTPS_PROXY=proxy_url, ALL_PROXY=proxy_url, NO_PROXY="127.0.0.1,localhost",
                       ORT_SKIP_DOWNLOAD="1")
    environment.pop("HF_TOKEN", None)
    for name in ("http_proxy", "https_proxy", "all_proxy", "no_proxy"):
        environment.pop(name, None)
    environment.update(http_proxy=proxy_url, https_proxy=proxy_url, all_proxy=proxy_url, no_proxy="127.0.0.1,localhost")
    command = [str(args.rpc.resolve()), "--launcher-root", str(root), "--host", "127.0.0.1", "--port", "0"]
    diagnostics = output / "rpc-stdout.log"
    stderr = (output / "rpc-stderr.log").open("w")
    process = subprocess.Popen(command, env=environment, stdout=subprocess.PIPE, stderr=stderr, text=True)
    ready = queue.Queue()

    def capture():
        with diagnostics.open("w") as log:
            for line in process.stdout:
                log.write(line)
                log.flush()
                if line.startswith("RPC_PORT="):
                    ready.put(int(line.split("=", 1)[1]))

    reader = threading.Thread(target=capture, daemon=True)
    reader.start()
    results = {}
    try:
        port = ready.get(timeout=25)
        base = f"http://127.0.0.1:{port}"
        seed_ordinary_search(root)
        cached_calls_before = len(proxy.calls)
        results["startup_upstream_call_count"] = cached_calls_before
        browse = rpc(base, "search_hf_models", {"query": "cache:", "limit": 25, "hydrate_limit": 0})
        check(browse["success"], "cached browse failed")
        check([row["repoId"] for row in browse["models"]] == ["acme/Alpha", "acme/Beta", "acme/NoRevision"], "visibility exclusion or stable browse failed")
        for row in browse["models"]:
            marker = row["modelCard"]["pumas_discovery"]
            check(marker["source"] == "anonymous-hf-detail-cache" and marker["discovery_only"], "owner provenance missing")
            check(marker["source_url"] == "https://huggingface.co/api/models/" + row["repoId"], "source changed")
            check(row["downloadOptions"] == [] and row["compatibleEngines"] == [], "cached record supplied download/readiness authority")
        check(browse["models"][1]["modelCard"]["pumas_discovery"]["freshness"] == "stale", "stale observation relabeled fresh")
        check(browse["models"][2]["modelCard"]["pumas_discovery"]["revision_observed"] is None, "missing revision fabricated")
        results["browse"] = browse
        results["filtered"] = rpc(base, "search_hf_models", {"query": "cache:BETA multilingual", "limit": 25, "hydrate_limit": 0})
        check(len(results["filtered"]["models"]) == 1, "local new-query filter failed")
        check(len(proxy.calls) == cached_calls_before, "cached browse/filter attempted upstream: " + str(proxy.calls))
        results["cached_upstream_call_count"] = len(proxy.calls) - cached_calls_before
        results["ordinary"] = rpc(base, "search_hf_models", {"query": "online-fixture", "limit": 25, "hydrate_limit": 6})
        check(results["ordinary"]["success"] and results["ordinary"]["models"][0]["name"] == "OnlineFixture", "preserved ordinary exact-search fixture failed")
        results["offline_miss"] = rpc(base, "search_hf_models", {"query": "owned-upstream-unavailable", "limit": 25, "hydrate_limit": 0})
        check(not results["offline_miss"]["success"], "ordinary unavailable search silently fell back")
        results["live_download_details"] = rpc(base, "get_hf_download_details", {"repo_id": "acme/Beta", "quants": []})
        check(not results["live_download_details"]["success"], "cached bytes authorized live download details")
        results["live_acquisition"] = rpc(base, "start_model_download_from_hf", {"repoId": "acme/Beta", "family": "acme", "officialName": "Beta", "modelType": "llm", "filename": "model.gguf", "quant": "Q4_K_M"})
        check(not results["live_acquisition"]["success"], "cached discovery authorized model acquisition")
        results["downloads"] = rpc(base, "list_model_downloads", {})
        results["models"] = rpc(base, "get_models", {})
        (output / "rpc-observations.json").write_text(json.dumps(results, indent=2) + "\n")
        check(results["downloads"]["downloads"] == [] and not results["models"]["models"], "failed live acquisition published a model or job")
        results["overlimit"] = rpc(base, "search_hf_models", {"query": "cache:", "limit": 101, "hydrate_limit": 0})
        check(not results["overlimit"]["success"], "cache limit bypassed through RPC")
        # Abandon a read-only request's delivery; subsequent public calls remain
        # usable. No claim that this cancels already admitted server effects.
        body = json.dumps({"jsonrpc": "2.0", "id": 99, "method": "search_hf_models", "params": {"query": "cache:Beta", "limit": 25, "hydrate_limit": 0}}).encode()
        with socket.create_connection(("127.0.0.1", port), timeout=3) as client:
            client.sendall(f"POST /rpc HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {len(body)}\r\nConnection: close\r\n\r\n".encode() + body)
        check(rpc(base, "search_hf_models", {"query": "cache:Beta", "limit": 25})["success"], "abandoned delivery broke later browse")
        (output / "rpc-observations.json").write_text(json.dumps(results, indent=2) + "\n")
        if not args.only_rpc:
            frontend_environment = dict(os.environ, PUMAS_HF_UI_RPC_URL=base)
            with (output / "frontend-workflow.log").open("w") as log:
                run = subprocess.run(["node", "node_modules/vitest/vitest.mjs", "run", "src/components/CachedModelSearchWorkflow.test.tsx"], cwd=ROOT / "frontend", env=frontend_environment, stdout=log, stderr=subprocess.STDOUT, timeout=60)
            check(run.returncode == 0, "mounted frontend qualification failed; see frontend-workflow.log")
        check(before == {p.name: p.read_bytes() for p in (root / "launcher-data/cache/hf").glob("*_metadata_v1.json")}, "discovery modified detail observations")
        (output / "upstream-refusals.json").write_text(json.dumps(proxy.calls, indent=2) + "\n")
        check(proxy.calls, "owned refusing upstream oracle was not exercised")
    finally:
        cleanup_errors = []
        try:
            if process.poll() is None:
                process.send_signal(signal.SIGTERM)
                try:
                    process.wait(timeout=15)
                except subprocess.TimeoutExpired:
                    process.kill()
                    process.wait(timeout=5)
                    cleanup_errors.append("RPC orderly shutdown exceeded bound")
        except Exception as error:
            cleanup_errors.append("RPC termination: " + str(error))
        # Attempt every remaining owned cleanup even if a preceding one fails.
        for label, action in [
            ("stdout reader", lambda: reader.join(timeout=2)),
            ("stderr", stderr.close),
            ("proxy evidence", lambda: (output / "upstream-refusals.json").write_text(json.dumps(proxy.calls, indent=2) + "\n")),
            ("proxy shutdown", proxy.shutdown),
            ("proxy close", proxy.server_close),
        ]:
            try:
                action()
            except Exception as error:
                cleanup_errors.append(label + ": " + str(error))
        if cleanup_errors:
            raise RuntimeError("; ".join(cleanup_errors) + "; owned fixture retained at " + str(output))
    check(process.returncode == 0, "RPC shutdown failed; retained diagnostics at " + str(output))
    print(json.dumps({"status": "passed", "output": str(output), "frontend": not args.only_rpc, "rpc_binary_sha256": hashlib.sha256(args.rpc.read_bytes()).hexdigest(), "qualified": "actual loopback RPC and mounted React/jsdom, not a native Electron/browser window"}))


if __name__ == "__main__":
    main()
