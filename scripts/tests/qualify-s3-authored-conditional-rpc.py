#!/usr/bin/env python3
"""Actual RPC process + owned HTTPS authored packages; structural import only."""
import argparse
import copy
import hashlib
import http.server
import importlib.util
import json
import os
from pathlib import Path
import sqlite3
import ssl
import struct
import subprocess
import tempfile
import threading
import urllib.parse

spec = importlib.util.spec_from_file_location(
    "owned_single_fixture", Path(__file__).with_name("qualify-s3-conditional-rpc.py"))
base = importlib.util.module_from_spec(spec)
spec.loader.exec_module(base)
check, wait_for, ID = base.check, base.wait_for, base.ID
PRIMARY = "model-00001-of-00002.safetensors"


def tensor(name):
    header = json.dumps({name: {"dtype": "F32", "shape": [1], "data_offsets": [0, 4]}}).encode()
    header += b" " * (-len(header) % 8)
    return struct.pack("<Q", len(header)) + header + struct.pack("<f", 1.0)


def package():
    values = {
        "config.json": {"model_type": "llama", "architectures": ["LlamaForCausalLM"], "hidden_size": 1},
        "tokenizer_config.json": {"tokenizer_class": "PreTrainedTokenizerFast", "unk_token": "[UNK]"},
        "tokenizer.json": {"version": "1.0", "truncation": None, "padding": None, "added_tokens": [],
                           "normalizer": None, "pre_tokenizer": None, "post_processor": None, "decoder": None,
                           "model": {"type": "WordLevel", "vocab": {"[UNK]": 0, "hello": 1}, "unk_token": "[UNK]"}},
        "model.safetensors.index.json": {"weight_map": {"a": PRIMARY, "b": "model-00002-of-00002.safetensors"}},
    }
    files = {path: json.dumps(value).encode() for path, value in values.items()}
    files.update({PRIMARY: tensor("a"), "model-00002-of-00002.safetensors": tensor("b")})
    return files


class Source(base.Fixture):
    def __init__(self, tls, files):
        http.server.ThreadingHTTPServer.__init__(self, ("127.0.0.1", 0), Handler)
        self.tls, self.files = tls, files
        self.mode, self.authenticated, self.versioned = "safe", False, False
        self.target = PRIMARY
        self.calls, self.errors = [], []
        self.stalled, self.drained = threading.Event(), threading.Event()
        self.worker = threading.Thread(target=self.serve_forever)
        self.worker.start()


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def do_HEAD(self):
        self.reply(True)

    def do_GET(self):
        self.reply(False)

    def reply(self, head):
        source = self.server
        parsed = urllib.parse.urlsplit(self.path)
        check(parsed.path.startswith("/fixture/objects/"), "unexpected source path")
        path = urllib.parse.unquote(parsed.path[len("/fixture/objects/"):])
        check((urllib.parse.parse_qs(parsed.query).get("versionId") == ["v1"]) == source.versioned,
              "version mode changed")
        signed = bool(self.headers.get("Authorization"))
        check(signed == source.authenticated, "authentication fallback changed")
        if signed:
            check("Credential=" + base.ACCESS + "/" in self.headers["Authorization"], "explicit credential absent")
            check(self.headers.get("x-amz-security-token") == base.TOKEN, "explicit token absent")
        source.calls.append({"method": self.command, "path": path, "range": self.headers.get("Range"),
                             "if_match": self.headers.get("If-Match"), "signed": signed})
        mode = source.mode if path == source.target else "safe"
        if path not in source.files or mode == "missing_head" or (mode == "precondition_failed" and not head):
            self.send_response(412 if mode == "precondition_failed" else 404)
            self.send_header("Content-Length", "0")
            self.send_header("Connection", "close")
            self.end_headers()
            self.close_connection = True
            return
        data = source.files[path]
        if head and mode == "stall_head":
            source.stalled.set()
            check(self.connection.recv(1) == b"", "held HEAD did not drain")
            source.drained.set()
            self.close_connection = True
            return
        start = 0
        if not head:
            check(self.headers.get("If-Match") == '"selected"', "exact authored If-Match absent")
            if data:
                value = self.headers.get("Range", "")
                check(value.startswith("bytes=") and value.endswith(f"-{len(data)-1}"), "range changed")
                start = int(value.split("=")[1].split("-")[0])
            else:
                check(self.headers.get("Range") is None, "empty member must perform non-range GET")
        tag = '"selected"'
        if mode == "weak_head" and head:
            tag = 'W/"selected"'
        if (mode == "changed_head" and head) or (mode == "changed_get" and not head):
            tag = '"changed"'
        body = data[start:]
        if mode == "body_mutation" and not head:
            body = body[:-1] + bytes([body[-1] ^ 1])
        self.send_response(200 if head or not data or mode == "ignored_range" else 206)
        self.send_header("Content-Length", str(len(body) + int(head and mode == "changed_size")))
        self.send_header("ETag", tag)
        self.send_header("x-amz-version-id", "v1" if source.versioned else "unexpected" if mode == "real_version" or (mode == "real_get_version" and not head) else "null")
        self.send_header("Last-Modified", "Wed, 01 Jan 2025 00:00:00 GMT")
        if not head and data:
            self.send_header("Content-Range", f"bytes {start}-{len(data)-1}/{len(data)}")
        self.send_header("Connection", "close")
        self.end_headers()
        if not head:
            if mode == "stall_get":
                self.wfile.write(body[:4])
                self.wfile.flush()
                source.stalled.set()
                check(self.connection.recv(1) == b"", "held GET did not drain")
                source.drained.set()
            else:
                try:
                    self.wfile.write(body)
                except (BrokenPipeError, ConnectionResetError, ssl.SSLError):
                    pass
        self.close_connection = True


def request(source, primary=PRIMARY):
    files = [{"key": "objects/" + path, "logical_path": path, "sha256": hashlib.sha256(data).hexdigest(),
              **({"version_id": "v1"} if source.versioned else
                 {"expected_etag": '"selected"', "expected_size": str(len(data))})}
             for path, data in sorted(source.files.items(), reverse=True)]
    return {"operation_id": ID, "endpoint": f"https://localhost:{source.server_port}",
            "region": "fixture", "bucket": "fixture", "addressing": "path", "files": files,
            "primary_logical_path": primary, "family": "fixture", "official_name": "Authored RPC",
            **({} if source.versioned else {"read_mode": "conditional"})}


def credentials():
    return {"access_key_id": base.ACCESS, "secret_access_key": base.SECRET, "session_token": base.TOKEN}


def assert_preflight(process, source, params):
    faults = []
    for field, values in {
        "expected_size": [None, 1, "", "01", "-1", "+1", "1e2", "1 ", "1\n", "9223372036854775808", "18446744073709551616"],
        "expected_etag": [None, "selected", 'W/"selected"', '"a"b"', '"a\nb"'],
        "sha256": [None, "", "x" * 64], "version_id": [None, "", "null", "v1"],
        "logical_path": ["../escape", ".pumas_import_publication.json", "folder/CON.json", "a/" * 520 + "data.json"],
        "key": ["../escape"],
    }.items():
        for value in values:
            row = copy.deepcopy(params)
            row["files"][0][field] = value
            faults.append(row)
    for field in ["expected_etag", "expected_size", "sha256"]:
        row = copy.deepcopy(params)
        del row["files"][0][field]
        faults.append(row)
    for count in [0, 1, 33]:
        row = copy.deepcopy(params)
        row["files"] = [copy.deepcopy(params["files"][0]) for _ in range(count)]
        faults.append(row)
    for mode in [None, "version_id", "unknown"]:
        row = copy.deepcopy(params)
        if mode is None:
            del row["read_mode"]
        else:
            row["read_mode"] = mode
        faults.append(row)
    for conflict in ["expected_etag", "expected_size", "sha256"]:
        row = copy.deepcopy(params)
        row["files"][1].update(key=row["files"][0]["key"], expected_etag=row["files"][0]["expected_etag"],
                               expected_size=row["files"][0]["expected_size"], sha256=row["files"][0]["sha256"])
        row["files"][1][conflict] = {"expected_etag": '"other"', "expected_size": "999", "sha256": "0" * 64}[conflict]
        faults.append(row)
    for collision in ["duplicate", "part", "prefix", "revision", "aggregate", "mixed", "primary"]:
        row = copy.deepcopy(params)
        if collision == "duplicate":
            row["files"][1]["logical_path"] = row["files"][0]["logical_path"]
        elif collision == "part":
            row["files"][1]["logical_path"] = row["files"][0]["logical_path"] + ".part"
        elif collision == "prefix":
            row["files"][1]["logical_path"] = row["files"][0]["logical_path"].upper() + "/data"
        elif collision == "revision":
            row["files"][0]["expected_etag"] = '"' + "x" * 16384 + '"'
        elif collision == "aggregate":
            for file in row["files"]:
                file["expected_size"] = "9223372036854775807"
        elif collision == "mixed":
            row["files"][0].pop("expected_etag")
            row["files"][0].pop("expected_size")
            row["files"][0]["version_id"] = "v1"
        else:
            row["primary_logical_path"] = "absent.safetensors"
        faults.append(row)
    for row in faults:
        response = process.rpc("start_s3_model_bundle_import", row)
        check(response.get("error", {}).get("code") == -32602, "malformed authored set admitted")
        response = process.rpc("start_authenticated_s3_model_bundle_import", {"source": row, "credentials": credentials()})
        check(response.get("error", {}).get("code") == -32602, "authenticated malformed set admitted")
    prefix = {k: v for k, v in params.items() if k not in ["files", "primary_logical_path", "family", "official_name"]}
    prefix.update(prefix="", timeout_ms=1000)
    for method, wire in [("start_s3_prefix_discovery", prefix),
                         ("start_authenticated_s3_prefix_discovery", {"source": prefix, "credentials": credentials()})]:
        error = process.rpc(method, wire)["error"]
        check(error["code"] == -32000 and "unsupported" in error["message"], "conditional prefix supported")
    check(not source.calls and not base.state(process.root)["acquisitions"], "preflight performed source I/O/admission")
    check(not list(process.root.rglob(".s3-import-*")), "preflight reserved workspace")
    return len(faults)


def assert_receipt(saved, source, params):
    records, receipts = saved["acquisitions"], saved["consumer_receipts"]
    check(len(records) == len(receipts) == 1, "exact acquired receipt missing")
    record, receipt = next(iter(records.values())), next(iter(receipts.values()))
    check(record["manifest"] == receipt["manifest"] and record["files"] == receipt["verified_files"], "receipt manifest/files differ")
    check(receipt["demand"]["operation"] == ID and receipt["payload"]["path"] == params["primary_logical_path"], "receipt request binding differs")
    ordered = sorted(source.files)
    check([file["path"] for file in record["files"]] == ordered, "receipt shortened/reordered authored set")
    for file in record["files"]:
        data = source.files[file["path"]]
        check(file["bytes"] == len(data) and file["sha256"] == hashlib.sha256(data).hexdigest(), "full bytes unverified")
    revision = record["manifest"]["source"]["revision"]
    check(revision["strength"] == ("immutable" if source.versioned else "weak"), "source evidence upgraded")
    if not source.versioned:
        check(revision["authority"] == "s3.explicit_conditional_objects", "authored authority differs")
        check(json.loads(revision["value"]) == [["objects/" + p, '"selected"', len(source.files[p])] for p in ordered], "authored revision differs")
    return record, receipt


def qualify(binary, no_s3=False):
    results = {}
    candidates = [Path.home() / ".cache/huggingface/token"]
    for name, suffix in [("HF_HOME", "/token"), ("HF_TOKEN_PATH", "")]:
        if os.environ.get(name):
            candidates.append(Path(os.environ[name] + suffix))
    check(not any(p.exists() or p.is_symlink() for p in candidates), "ambient CLI token prevents isolated fixture")
    cases = ["complete"] if no_s3 else ["complete", "authenticated", "versioned", "empty_auxiliary", "missing_head", "changed_head", "changed_size",
             "weak_head", "changed_get", "real_version", "real_get_version", "precondition_failed", "ignored_range", "body_mutation", "wrong_digest",
             "missing_config", "missing_tokenizer_config", "missing_tokenizer", "missing_index", "missing_shard",
             "malformed_config", "malformed_shard", "malformed_index", "custom_code", "cancel_head", "cancel_get",
             "shutdown_get", "retry_get", "retry_authenticated_get", "retry_changed", "retry_size", "cold_reopen"]
    with tempfile.TemporaryDirectory(prefix="pumas-authored-rpc-") as temp:
        temp = Path(temp)
        fixture = base.ROOT / "rust/crates/pumas-core/tests/fixtures/http-tls"
        key = temp / "fixture.key"
        key.touch(mode=0o600)
        subprocess.run(["openssl", "pkcs12", "-in", str(fixture / "localhost.p12"), "-passin", "pass:fixture",
                        "-nodes", "-nocerts", "-out", str(key)], check=True, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE)
        tls = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        tls.load_cert_chain(str(fixture / "localhost.pem"), str(key))
        proxy = base.Fixture()
        proxy.mode = "proxy"
        try:
            for case in cases:
                files, primary = package(), PRIMARY
                missing = {"missing_config": "config.json", "missing_tokenizer_config": "tokenizer_config.json",
                           "missing_tokenizer": "tokenizer.json", "missing_index": "model.safetensors.index.json",
                           "missing_shard": "model-00002-of-00002.safetensors"}
                if case in missing:
                    del files[missing[case]]
                if case == "empty_auxiliary":
                    primary = "weights.gguf"
                    files = {primary: b"GGUF" + struct.pack("<IQQ", 3, 0, 0), "notes.txt": b""}
                if case == "malformed_config":
                    files["config.json"] = b"not JSON"
                if case == "malformed_shard":
                    files[PRIMARY] = b"not safetensors"
                if case == "malformed_index":
                    files["model.safetensors.index.json"] = b'{"weight_map":{"absent":"model-00001-of-00002.safetensors"}}'
                if case == "custom_code":
                    files["custom.py"] = b"raise RuntimeError('must never execute')"
                source, process = Source(tls, files), None
                try:
                    source.authenticated = case in ["authenticated", "retry_authenticated_get"]
                    source.versioned = case == "versioned"
                    source.mode = {"cancel_head": "stall_head", "cancel_get": "stall_get", "shutdown_get": "stall_get",
                                   "retry_get": "stall_get", "retry_authenticated_get": "stall_get", "retry_changed": "stall_get",
                                   "retry_size": "stall_get", "cold_reopen": "stall_get"}.get(case, case)
                    if case in ["missing_head", "changed_head", "changed_size", "weak_head", "real_version"]:
                        source.target = "tokenizer_config.json"  # Late selection failure after earlier HEADs.
                    root = temp / case
                    process = base.Process(binary, root, proxy)
                    params = request(source, primary)
                    malformed_count = assert_preflight(process, source, params) if case == "complete" else 0
                    if no_s3:
                        for size in ["0", "9223372036854775807"]:
                            for tag in ['""', '"opaque-é-😀"']:
                                row = copy.deepcopy(params)
                                row["files"][0].update(expected_size=size, expected_etag=tag)
                                for method, wire in [("start_s3_model_bundle_import", row),
                                                     ("start_authenticated_s3_model_bundle_import", {"source": row, "credentials": credentials()})]:
                                    check(process.rpc(method, wire)["result"] == {"status": "unavailable"}, "valid authored wire did not reach unavailable build")
                        check(not source.calls and not base.state(root)["acquisitions"] and not list(root.rglob(".s3-import-*")), "no-S3 build performed admission/I/O")
                        results["no_s3"] = {"outcome": "unavailable", "preflight_faults": malformed_count, "calls": [], "acquisitions": {}, "receipts": {}}
                        continue
                    if case == "wrong_digest":
                        next(file for file in params["files"] if file["logical_path"] == "tokenizer_config.json")["sha256"] = "0" * 64
                    method, wire = "start_s3_model_bundle_import", params
                    if source.authenticated:
                        method, wire = "start_authenticated_s3_model_bundle_import", {"source": params, "credentials": credentials()}
                    check(process.rpc(method, wire)["result"]["status"] == "running", "authored bundle not admitted")
                    held = case.startswith(("cancel_", "retry_")) or case in ["shutdown_get", "cold_reopen"]
                    if held:
                        check(source.stalled.wait(timeout=10), "held source boundary absent")
                        if case != "cancel_head":
                            wait_for(lambda: process.rpc("get_s3_model_import", {"operation_id": ID})["result"],
                                     lambda p: int(p.get("progress", {}).get("downloaded_for_current_file", "0")) >= 4)
                            progress = process.rpc("get_s3_model_bundle_import", {"operation_id": ID})["result"]["bundle_progress"]
                            check(progress["files_total"] == len(files) and int(progress["total_expected_bytes"]) == sum(map(len, files.values())), "bundle total lost authored members")
                        if case == "shutdown_get":
                            process.rpc("shutdown", {})
                            check(process.child.wait(timeout=15) == 0, "RPC shutdown failed")
                            terminal = None
                        else:
                            check(process.rpc("cancel_s3_model_import", {"operation_id": ID})["result"]["accepted"], "cancellation refused")
                            terminal = process.terminal()
                        check(source.drained.wait(timeout=5), "held source not drained")
                    else:
                        terminal = process.terminal()
                    if case.startswith("retry_") or case == "cold_reopen":
                        check(terminal["result"]["status"] == "cancelled", "retry requires cancelled operation")
                        before = base.state(root)
                        partials = {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*.part")}
                        check(any(len(data) == 4 for data in partials.values()), "partial bytes not retained")
                        check(len(before["acquisitions"]) == 1 and not before["consumer_receipts"], "cancelled custody differs")
                        before_gets = len([c for c in source.calls if c["method"] == "GET"])
                        if case == "cold_reopen":
                            process.close()
                            process = base.Process(binary, root, proxy, existing=True)
                            cold = process.rpc("get_s3_transfer_retry", {"operation_id": ID})["result"]
                            check(cold == {"status": "unavailable", "reason": "no_live_custody"}, "cold reopen invented live retry")
                            check(base.state(root) == before, "cold reopen changed retained bytes/record")
                            observed = process.rpc("inspect_persisted_s3_imports", {})["result"]
                            check(observed["status"] == "complete" and len(observed["imports"]) == 1, "cold retained custody not inspectable")
                            check(not observed["imports"][0]["receipt_present"], "cold receipt fabricated")
                            check(process.rpc(method, wire)["result"]["status"] == "running", "cold request observation unavailable")
                        else:
                            ready = process.rpc("get_s3_transfer_retry", {"operation_id": ID})["result"]
                            check(ready["status"] == "ready" and ready["authentication_required"] == source.authenticated, "retained auth/selection lost")
                            retry = {"operation_id": ID}
                            if source.authenticated:
                                check(process.rpc("retry_s3_model_transfer", retry)["result"]["status"] == "rejected", "retry silently lost authentication")
                                retry["credentials"] = credentials()
                            source.mode = {"retry_changed": "changed_head", "retry_size": "changed_size"}.get(case, "safe")
                            check(process.rpc("retry_s3_model_transfer", retry)["result"]["status"] == "running", "retained retry not admitted")
                        terminal = process.terminal()
                        after = base.state(root)
                        check(list(before["acquisitions"]) == list(after["acquisitions"]), "retry replaced acquisition ID")
                        if case in ["retry_changed", "retry_size", "cold_reopen"]:
                            check(len([c for c in source.calls if c["method"] == "GET"]) == before_gets, "inconsistent/cold retry fetched bytes")
                            check(before == after and partials == {str(p.relative_to(root)): p.read_bytes() for p in root.rglob("*.part")}, "refusal replaced retained custody")
                        else:
                            last = next(c for c in reversed(source.calls) if c["path"] == PRIMARY and c["method"] == "GET")
                            check(last["range"] == f"bytes=0-{len(source.files[PRIMARY])-1}", "cancelled RPC retry invented live prefix proof")
                    saved = base.state(root)
                    if terminal is not None:
                        observed = process.rpc("get_s3_model_bundle_import", {"operation_id": ID})["result"]
                        check(observed["outcome"] == terminal and observed["bundle_progress"] is None, "bundle/single terminal observation differs")
                    success = case in ["complete", "authenticated", "versioned", "empty_auxiliary", "retry_get", "retry_authenticated_get"]
                    model_refusal = case in missing or case in ["malformed_config", "malformed_shard", "malformed_index", "custom_code"]
                    outcome = "shutdown_drained" if terminal is None else terminal["result"]["status"]
                    if success or model_refusal:
                        record, receipt = assert_receipt(saved, source, params)
                        if success:
                            check(outcome == "completed", f"{case} publication failed: {terminal}")
                            pubs = list(root.rglob(".pumas_import_publication.json"))
                            check(len(pubs) == 1, "exact publication missing")
                            pub = json.loads(pubs[0].read_text())
                            check(pub["state"] == "confirmed" and pub["model_id"] == terminal["result"]["model_id"] and pub["acquisition"] == receipt, "publication/receipt mismatch")
                            check(record["phase"]["state"] == "adopted", "acquisition not settled")
                            check(json.loads((pubs[0].parent / "metadata.json").read_text())["import_state"] == "ready", "metadata not Ready")
                            for path, data in files.items():
                                check((pubs[0].parent / path).read_bytes() == data, "published bytes differ")
                            rows = []
                            for db_path in root.rglob("models.db"):
                                with sqlite3.connect(db_path) as db:
                                    rows.extend(db.execute("SELECT id FROM models").fetchall())
                            check((pub["model_id"],) in rows, "publication direct index row absent")
                            check(not list(root.rglob(".s3-import-*")), "settled workspace not removed")
                        else:
                            check(outcome == "failed" and record["phase"]["state"] == "using", "model refusal lost verified custody")
                            base.check_no_publication(root)
                    else:
                        expected = "shutdown_drained" if case == "shutdown_get" else "cancelled" if case.startswith("cancel_") else "failed"
                        check(outcome == expected, "terminal outcome differs")
                        check(not saved["consumer_receipts"], "failed bytes issued receipt")
                        base.check_no_publication(root)
                        if case in ["missing_head", "changed_head", "changed_size", "weak_head", "real_version", "cancel_head"]:
                            check(not saved["acquisitions"], "failed selection admitted acquisition")
                    if success:
                        first_get = next(i for i, c in enumerate(source.calls) if c["method"] == "GET")
                        check(first_get == len(files), "GET preceded full authored HEAD selection")
                    if case == "empty_auxiliary":
                        check(any(c["path"] == "notes.txt" and c["method"] == "GET" and c["range"] is None for c in source.calls), "empty member skipped actual GET")
                    for path in root.rglob("*"):
                        if path.is_file():
                            check(path.stat().st_size <= 8 * 1024 * 1024, "fixture artifact exceeded scan bound")
                            data = path.read_bytes()
                            for credential in [base.ACCESS, base.SECRET, base.TOKEN, "synthetic-unused-ambient-access", "synthetic-unused-ambient-secret"]:
                                check(credential.encode() not in data, "access material persisted")
                    results[case] = {"outcome": outcome, "preflight_faults": malformed_count, "calls": source.calls,
                                     "acquisitions": saved["acquisitions"], "receipts": saved["consumer_receipts"]}
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
    parser.add_argument("--rpc", required=True, type=Path)
    parser.add_argument("--output", type=Path)
    parser.add_argument("--no-s3", action="store_true")
    args = parser.parse_args()
    results = qualify(args.rpc, args.no_s3)
    if args.output:
        args.output.write_text(json.dumps(results, indent=2) + "\n")
    print(json.dumps({case: {"outcome": value["outcome"], "requests": len(value["calls"]),
                             "preflight_faults": value["preflight_faults"]} for case, value in results.items()}, indent=2))
