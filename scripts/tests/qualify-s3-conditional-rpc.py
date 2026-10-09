#!/usr/bin/env python3
"""Production RPC process, bounded owned HTTPS fixtures, no model inference."""
import argparse
import hashlib
import http.server
import json
import os
from pathlib import Path
import queue
import socket
import sqlite3
import ssl
import struct
import subprocess
import tempfile
import threading
import time
import urllib.request

ROOT = Path(__file__).resolve().parents[2]
ID = "c3f7d104-1234-4321-abcd-aaaaaaaaaaaa"
ACCESS, SECRET, TOKEN = "conditional-fixture-access", "conditional-fixture-secret", "conditional-fixture-token"


def check(value, message):
    if not value:
        raise AssertionError(message)


def tensor():
    header = b'{"weight":{"dtype":"F32","shape":[1],"data_offsets":[0,4]}}'
    header += b" " * (-len(header) % 8)
    return struct.pack("<Q", len(header)) + header + struct.pack("<f", 1.0)


class Fixture(http.server.ThreadingHTTPServer):
    daemon_threads = False
    block_on_close = True

    def __init__(self, tls=None):
        super().__init__(("127.0.0.1", 0), Handler)
        self.tls = tls
        self.mode, self.data, self.versioned, self.authenticated = "safe", tensor(), False, False
        self.calls, self.errors = [], []
        self.stalled, self.drained = threading.Event(), threading.Event()
        self.worker = threading.Thread(target=self.serve_forever)
        self.worker.start()

    def get_request(self):
        connection, address = super().get_request()
        connection.settimeout(8)
        if self.tls:
            try:
                connection = self.tls.wrap_socket(connection, server_side=True)
            except Exception:
                connection.close()
                raise
        return connection, address

    def handle_error(self, request, client_address):
        self.errors.append("owned fixture handler failed")
        super().handle_error(request, client_address)

    def close(self):
        try:
            self.shutdown()
        finally:
            try:
                self.server_close()
            finally:
                self.worker.join(timeout=5)
        check(not self.worker.is_alive(), "owned fixture failed to stop")
        check(not self.errors, "owned fixture handler assertion failed")


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def do_CONNECT(self):
        self.send_response(503)
        self.send_header("Content-Length", "0")
        self.send_header("Connection", "close")
        self.end_headers()
        self.close_connection = True

    def do_HEAD(self):
        self.reply(True)

    def do_GET(self):
        self.reply(False)

    def reply(self, head):
        self.connection.settimeout(8)
        source = self.server
        if source.mode == "proxy":
            self.do_CONNECT()
            return
        auth = self.headers.get("Authorization", "")
        check(bool(auth) == source.authenticated, "credential mode/fallback changed")
        if source.authenticated:
            check("Credential=" + ACCESS + "/" in auth, "explicit signing identity absent")
            check(self.headers.get("x-amz-security-token") == TOKEN, "explicit token absent")
        check(("versionId=v1" in self.path) == source.versioned, "version mode changed")
        if not head:
            check(self.headers.get("If-Match") == '"selected"', "strong If-Match absent")
            check(self.headers.get("Range", "").startswith("bytes="), "explicit range absent")
        source.calls.append({"method": self.command, "path": self.path,
                             "range": self.headers.get("Range"), "signed": bool(auth)})
        if source.mode == "stall_head" and head:
            source.stalled.set()
            check(self.connection.recv(1) == b"", "cancel must drain HEAD")
            source.drained.set()
            self.close_connection = True
            return
        start = 0 if head else int(self.headers["Range"].split("=")[1].split("-")[0])
        self.send_response(200 if head or source.mode == "ignored_range" else 206)
        if source.mode != "missing_size":
            self.send_header("Content-Length", str(len(source.data) - start + int(head and source.mode == "changed_size")))
        tag = 'W/"selected"' if source.mode == "weak" else '"selected"'
        if (not head and source.mode == "changed") or source.mode == "changed_head":
            tag = '"changed"'
        self.send_header("ETag", tag)
        self.send_header("x-amz-version-id", "v1" if source.versioned else "null")
        self.send_header("Last-Modified", "Wed, 01 Jan 2025 00:00:00 GMT")
        if not head:
            self.send_header("Content-Range", f"bytes {start}-{len(source.data)-1}/{len(source.data)}")
        self.send_header("Connection", "close")
        self.end_headers()
        if not head:
            if source.mode == "stall_get":
                self.wfile.write(source.data[start:start+1])
                self.wfile.flush()
                source.stalled.set()
                check(self.connection.recv(1) == b"", "cancel must drain GET")
                source.drained.set()
            else:
                try:
                    self.wfile.write(source.data[start:])
                except (BrokenPipeError, ConnectionResetError, ssl.SSLError):
                    pass  # Expected when response evidence refuses before body polling.
        self.close_connection = True


class Process:
    def __init__(self, binary, root, proxy, *, existing=False):
        self.root, self.reader, self.child, self.stderr = root, None, None, None
        root.mkdir(exist_ok=existing)
        config = root / "config"
        config.mkdir(exist_ok=existing)
        env = dict(os.environ, XDG_CONFIG_HOME=str(config), XDG_CACHE_HOME=str(root / "cache"),
                   HF_HOME=str(root / "hf"), HF_TOKEN_PATH=str(root / "hf/token"), ORT_SKIP_DOWNLOAD="1", SSL_CERT_FILE=str(
                       ROOT / "rust/crates/pumas-core/tests/fixtures/http-tls/localhost.pem"))
        for name in ["HF_TOKEN", "HUGGING_FACE_HUB_TOKEN", "HUGGINGFACE_HUB_TOKEN"]:
            env.pop(name, None)
        proxy_url = f"http://127.0.0.1:{proxy.server_port}"
        for name in ["HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY", "http_proxy", "https_proxy", "all_proxy"]:
            env[name] = proxy_url
        env.update(NO_PROXY="127.0.0.1,localhost", no_proxy="127.0.0.1,localhost",
                   AWS_ACCESS_KEY_ID="synthetic-unused-ambient-access", AWS_SECRET_ACCESS_KEY="synthetic-unused-ambient-secret")
        ready = queue.Queue()
        self.stderr = (root / "stderr.log").open("w")
        self.child = subprocess.Popen([str(binary), "--launcher-root", str(root), "--port", "0"],
                                     env=env, stdout=subprocess.PIPE, stderr=self.stderr, text=True)

        def capture():
            with (root / "stdout.log").open("w") as log:
                for line in self.child.stdout:
                    log.write(line)
                    log.flush()
                    if line.startswith("RPC_PORT="):
                        ready.put(int(line.split("=", 1)[1]))
        self.reader = threading.Thread(target=capture)
        self.reader.start()
        try:
            self.base = f"http://127.0.0.1:{ready.get(timeout=25)}/rpc"
        except Exception:
            self.close()
            raise

    def rpc(self, method, params):
        req = urllib.request.Request(self.base, data=json.dumps(
            {"jsonrpc":"2.0", "id":1, "method":method, "params":params}).encode(),
            headers={"Content-Type":"application/json"})
        with urllib.request.build_opener(urllib.request.ProxyHandler({})).open(req, timeout=5) as response:
            return json.load(response)

    def close(self):
        errors = []
        if self.child and self.child.poll() is None:
            try:
                self.child.terminate()
            except OSError as error:
                errors.append(f"TERM: {type(error).__name__}")
            try:
                self.child.wait(timeout=15)
            except (subprocess.TimeoutExpired, OSError) as error:
                errors.append(f"TERM wait: {type(error).__name__}")
            if self.child.poll() is None:
                try:
                    self.child.kill()
                except OSError as error:
                    errors.append(f"KILL: {type(error).__name__}")
                try:
                    self.child.wait(timeout=5)
                except (subprocess.TimeoutExpired, OSError) as error:
                    errors.append(f"KILL wait: {type(error).__name__}")
        try:
            if self.reader:
                self.reader.join(timeout=5)
                if self.reader.is_alive():
                    errors.append("RPC output reader failed to stop")
        finally:
            try:
                # Avoid taking a buffered-reader lock while a live reader owns it.
                if self.child and self.child.stdout and not (self.reader and self.reader.is_alive()):
                    self.child.stdout.close()
            finally:
                if self.stderr:
                    self.stderr.close()
        check(not errors, "owned RPC cleanup errors: " + "; ".join(errors))

    def terminal(self):
        return wait_for(lambda: self.rpc("get_s3_model_import", {"operation_id":ID})["result"],
                        lambda value: value["status"] == "finished")


def wait_for(read, predicate, seconds=20):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        value = read()
        if predicate(value):
            return value
        time.sleep(0.01)
    raise AssertionError("owned operation deadline")


def request(source):
    return {"operation_id":ID, "endpoint":f"https://localhost:{source.server_port}",
            "region":"fixture", "bucket":"fixture", "addressing":"path", "key":"object",
            "filename":"weights.safetensors", "family":"fixture", "official_name":"Conditional RPC",
            "read_mode":"conditional", "sha256":hashlib.sha256(source.data).hexdigest()}


def state(root):
    files = list(root.rglob("downloads.json"))
    return json.loads(files[0].read_text()) if files else {"acquisitions":{}, "consumer_receipts":{}}


def check_no_publication(root):
    check(not list(root.rglob(".pumas_import_publication.json")), "refusal published a model")
    for database in root.rglob("models.db"):
        with sqlite3.connect(database) as db:
            check(db.execute("SELECT count(*) FROM models").fetchone()[0] == 0, "refusal indexed a model")


def qualify(binary):
    # Never read or modify an existing ambient CLI token; refuse that fixture scope.
    candidates = [Path.home() / ".cache/huggingface/token"]
    if os.environ.get("HF_HOME"):
        candidates.append(Path(os.environ["HF_HOME"]) / "token")
    if os.environ.get("HF_TOKEN_PATH"):
        candidates.append(Path(os.environ["HF_TOKEN_PATH"]))
    check(not any(p.exists() or p.is_symlink() for p in candidates), "ambient CLI token prevents isolated fixture")
    results = {}
    with tempfile.TemporaryDirectory(prefix="pumas-conditional-rpc-") as temp:
        temp = Path(temp)
        fixture = ROOT / "rust/crates/pumas-core/tests/fixtures/http-tls"
        key = temp / "fixture.key"
        key.touch(mode=0o600)
        subprocess.run(["openssl", "pkcs12", "-in", str(fixture / "localhost.p12"),
                        "-passin", "pass:fixture", "-nodes", "-nocerts", "-out", str(key)],
                       check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        tls.load_cert_chain(str(fixture / "localhost.pem"), str(key))
        proxy = Fixture()
        proxy.mode = "proxy"
        try:
            for case in ["safe", "authenticated", "versioned", "weak", "missing_size", "changed",
                         "ignored_range", "wrong_digest", "invalid_model", "missing_index", "unsafe", "cancel_head", "cancel_get", "shutdown_get", "retry_get", "retry_authenticated_get", "retry_changed", "retry_size"]:
                source = Fixture(tls)
                process = None
                try:
                    source.mode = {"cancel_head":"stall_head", "cancel_get":"stall_get", "shutdown_get":"stall_get", "retry_get":"stall_get", "retry_authenticated_get":"stall_get", "retry_changed":"stall_get", "retry_size":"stall_get"}.get(case, case)
                    source.authenticated = case in ["authenticated", "retry_authenticated_get"]
                    source.versioned = case == "versioned"
                    if case in ["invalid_model", "unsafe"]:
                        source.data = b"not safetensors"
                    root = temp / case
                    process = Process(binary, root, proxy)
                    params = request(source)
                    # Closed input/mode preflight is exercised before workspace/source I/O.
                    if case == "safe":
                        bad = []
                        for key, value in [("version_id", "v1"), ("version_id", "null"), ("version_id", ""),
                                           ("version_id", None), ("read_mode", "unknown"), ("sha256", ""),
                                           ("sha256", None), ("key", "../escape")]:
                            row = dict(params)
                            row[key] = value
                            bad.append(row)
                        for absent in ["sha256", "read_mode"]:
                            row = dict(params)
                            del row[absent]
                            bad.append(row)
                        for row in bad:
                            check(process.rpc("start_s3_model_import", row)["error"]["code"] == -32602, "invalid conditional params admitted")
                        bundle = {k:v for k,v in params.items() if k not in ["key", "filename", "sha256"]}
                        bundle.update(primary_logical_path="weights.safetensors", files=[
                            {"key":"a", "version_id":"v1", "logical_path":"weights.safetensors", "sha256":params["sha256"]},
                            {"key":"b", "version_id":"v2", "logical_path":"config.json", "sha256":"0"*64}])
                        discovery = {k:v for k,v in params.items() if k not in ["key", "filename", "sha256", "family", "official_name"]}
                        discovery.update(prefix="", timeout_ms=1000)
                        error = process.rpc("start_s3_model_bundle_import", bundle)["error"]
                        check(error["code"] == -32602, "conditional bundle accepted VersionId members")
                        error = process.rpc("start_s3_prefix_discovery", discovery)["error"]
                        check(error["code"] == -32000 and "unsupported" in error["message"], "unsupported prefix mode not explicit")
                        check(not source.calls and not state(root)["acquisitions"], "preflight performed I/O/admission")
                        check(not list(root.rglob(".s3-import-*")), "preflight reserved workspace")
                    if case == "versioned":
                        del params["read_mode"]
                        params["version_id"] = "v1"
                    if case == "wrong_digest":
                        params["sha256"] = "0"*64
                    if case == "missing_index":
                        params["filename"] = "model-00001-of-00002.safetensors"
                    if case == "unsafe":
                        params["filename"] = "weights.pt"
                    method = "start_s3_model_import"
                    wire = params
                    if source.authenticated:
                        method = "start_authenticated_s3_model_import"
                        wire = {"source":params, "credentials":{"access_key_id":ACCESS,"secret_access_key":SECRET,"session_token":TOKEN}}
                    check(process.rpc(method, wire)["result"]["status"] == "running", "single object not admitted")
                    if case.startswith(("cancel_", "retry_")) or case == "shutdown_get":
                        check(source.stalled.wait(timeout=10), "source did not reach held boundary")
                        if case != "cancel_head":
                            wait_for(lambda: process.rpc("get_s3_model_import", {"operation_id":ID})["result"],
                                     lambda p: int(p.get("progress", {}).get("downloaded_for_current_file", "0")) >= 1)
                        if case == "shutdown_get":
                            process.rpc("shutdown", {})
                            check(process.child.wait(timeout=15) == 0, "RPC shutdown failed")
                            terminal = None  # Observe process exit/drain, not an RPC terminal status.
                        else:
                            check(process.rpc("cancel_s3_model_import", {"operation_id":ID})["result"]["accepted"], "cancellation refused")
                            terminal = process.terminal()
                        check(source.drained.wait(timeout=5), "stalled source not drained")
                    else:
                        terminal = process.terminal()
                    if case.startswith("retry_"):
                        check(terminal["result"]["status"] == "cancelled", "retry requires cancelled attempt")
                        before = state(root)
                        check(not before["consumer_receipts"] and len(before["acquisitions"]) == 1, "cancelled retry custody differs")
                        check_no_publication(root)
                        retry_state = process.rpc("get_s3_transfer_retry", {"operation_id":ID})["result"]
                        check(retry_state["status"] == "ready" and retry_state["authentication_required"] == source.authenticated, "retry mode lost")
                        retry = {"operation_id":ID}
                        if source.authenticated:
                            retry["credentials"] = {"access_key_id":ACCESS,"secret_access_key":SECRET,"session_token":TOKEN}
                            check(process.rpc("retry_s3_model_transfer", {"operation_id":ID})["result"]["status"] == "rejected", "authenticated retry silently fell back")
                        source.mode = {"retry_changed":"changed_head", "retry_size":"changed_size"}.get(case, "safe")
                        check(process.rpc("retry_s3_model_transfer", retry)["result"]["status"] == "running", "retry not admitted")
                        terminal = process.terminal()
                        after = state(root)
                        check(list(before["acquisitions"]) == list(after["acquisitions"]), "retry replaced acquisition identity")
                        if case in ["retry_changed", "retry_size"]:
                            check(source.calls[-1]["method"] == "HEAD" and len(source.calls) == 3, "changed selection must fail before GET")
                        else:
                            check(source.calls[-1]["range"] == f"bytes=0-{len(source.data)-1}", "re-reserved RPC workspace must safely restart without live prefix proof")
                    outcome = "shutdown_drained" if terminal is None else terminal["result"]["status"]
                    saved = state(root)
                    records, receipts = saved["acquisitions"], saved["consumer_receipts"]
                    if case in ["safe", "authenticated", "versioned", "retry_get", "retry_authenticated_get"]:
                        check(outcome == "completed", f"{case} failed: {terminal}")
                        model_id = terminal["result"]["model_id"]
                        pubs = list(root.rglob(".pumas_import_publication.json"))
                        check(len(pubs) == 1, "publication missing")
                        publication = json.loads(pubs[0].read_text())
                        check(publication["state"] == "confirmed" and publication["model_id"] == model_id, "publication not bound")
                        check((pubs[0].parent / params["filename"]).read_bytes() == source.data, "copied bytes differ")
                        check(json.loads((pubs[0].parent / "metadata.json").read_text())["import_state"] == "ready", "metadata not Ready")
                        check(len(records) == len(receipts) == 1, "acquisition/receipt missing")
                        record = next(iter(records.values()))
                        receipt = next(iter(receipts.values()))
                        check(record["phase"]["state"] == "adopted", "acquisition not settled")
                        check(receipt == publication["acquisition"] and receipt["verified_files"] == record["files"], "receipt differs")
                        check(receipt["demand"]["operation"] == ID and receipt["payload"]["path"] == params["filename"], "receipt request binding differs")
                        revision = record["manifest"]["source"]["revision"]
                        check(revision["strength"] == ("immutable" if source.versioned else "weak"), "revision authority upgraded")
                        if not source.versioned:
                            check(json.loads(revision["value"]) == ['"selected"',len(source.data)], "strong validator/size differs")
                        check(record["files"][0]["sha256"] == hashlib.sha256(source.data).hexdigest(), "full digest unverified")
                        check(not list(root.rglob(".s3-import-*")), "settled workspace not cleaned")
                    else:
                        check(outcome == ("shutdown_drained" if case == "shutdown_get" else "cancelled" if case.startswith("cancel_") else "failed"), "terminal state differs")
                        check_no_publication(root)
                        if case in ["invalid_model", "missing_index", "unsafe"]:
                            check(len(records) == len(receipts) == 1, "valid bytes were not independently acquired")
                            record = next(iter(records.values()))
                            check(record["phase"]["state"] == "using" and record["files"][0]["sha256"] == hashlib.sha256(source.data).hexdigest(), "model refusal lost verified custody")
                        else:
                            check(not receipts, "failed/cancelled bytes issued receipt")
                        if case in ["weak", "missing_size", "cancel_head"]:
                            check(not records, "selection refusal admitted durable transfer")
                    for path in root.rglob("*"):
                        if path.is_file():
                            check(path.stat().st_size <= 8 * 1024 * 1024, "owned fixture artifact exceeded scan bound")
                            content = path.read_bytes()
                            for credential in [ACCESS, SECRET, TOKEN, "synthetic-unused-ambient-access", "synthetic-unused-ambient-secret"]:
                                check(credential.encode() not in content, "credential persisted in owned fixture artifact")
                    results[case] = {"outcome":outcome, "source_requests":len(source.calls), "receipts":len(receipts)}
                finally:
                    try:
                        if process:
                            process.close()
                    finally:
                        source.close()
        finally:
            proxy.close()
    return results


if __name__ == "__main__":
    parser = argparse.ArgumentParser()
    parser.add_argument("--rpc", type=Path, required=True)
    args = parser.parse_args()
    print(json.dumps({"qualification":qualify(args.rpc.resolve()), "scope":"owned production RPC/HTTPS fixtures; structural import only; catalog visibility excluded"}, sort_keys=True))
