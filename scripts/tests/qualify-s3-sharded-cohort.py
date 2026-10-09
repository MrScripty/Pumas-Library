#!/usr/bin/env python3
"""Owned HTTPS protocol oracle; NOT a real S3 service or inference test.

Uses Python's standard library, the repository's test CA, and installed OpenSSL.
Only literal loopback is contacted. CA trust is scoped to the Cargo child.
"""

import argparse
import hashlib
import hmac
import http.server
import json
import os
from pathlib import Path
import re
import socket
import ssl
import struct
import subprocess
import tempfile
import threading
from urllib.parse import parse_qs, unquote, urlsplit
from xml.sax.saxutils import escape

ROOT = Path(__file__).resolve().parents[2]
ACCESS = "cohort-fixture-access"
SECRET = "cohort-fixture-secret"
TOKEN = "cohort-fixture-session"
VERSION = "owned-v1"
ETAG = '"fixture-validator-not-sha256"'


def check(condition, message):
    if not condition:
        raise AssertionError(message)


def signature(handler, secret):
    # Independent stdlib HMAC oracle, never the AWS SDK signer under test.
    fields = dict(item.split("=", 1) for item in handler.headers["Authorization"].removeprefix("AWS4-HMAC-SHA256 ").split(", "))
    access, scope = fields["Credential"].split("/", 1)
    date, region, service, terminal = scope.split("/")
    check(access == ACCESS and (region, service, terminal) == ("fixture-region", "s3", "aws4_request"), "incorrect credential scope")
    signed = fields["SignedHeaders"]
    names = signed.split(";")
    check(names == sorted(set(names)) and {"host", "x-amz-date", "x-amz-security-token"}.issubset(names), "missing signed authority/session headers")
    check(handler.headers["x-amz-security-token"] == TOKEN, "incorrect session token")
    parts = urlsplit(handler.path)
    query = "&".join(sorted(parts.query.split("&")))
    headers = "".join(f"{name}:{' '.join(handler.headers[name].split())}\n" for name in names)
    canonical = f"{handler.command}\n{parts.path}\n{query}\n{headers}\n{signed}\n{handler.headers['x-amz-content-sha256']}"
    message = f"AWS4-HMAC-SHA256\n{handler.headers['x-amz-date']}\n{scope}\n{hashlib.sha256(canonical.encode()).hexdigest()}"
    key = ("AWS4" + secret).encode()
    for component in (date, region, service, terminal):
        key = hmac.digest(key, component.encode(), "sha256")
    return hmac.compare_digest(hmac.new(key, message.encode(), "sha256").hexdigest(), fields["Signature"])


def tensor(name):
    header = json.dumps({name: {"dtype": "F32", "shape": [2048], "data_offsets": [0, 8192]}}, separators=(",", ":")).encode()
    header += b" " * (-len(header) % 8)
    return struct.pack("<Q", len(header)) + header + struct.pack("<2048f", *([1.0] * 2048))


def owned_objects():
    encode = lambda value: json.dumps(value, separators=(",", ":")).encode()
    package = {
        "config.json": encode({"model_type": "llama", "architectures": ["LlamaForCausalLM"], "hidden_size": 1}),
        "tokenizer_config.json": encode({"tokenizer_class": "PreTrainedTokenizerFast"}),
        "tokenizer.json": encode({"version": "1.0", "truncation": None, "padding": None, "added_tokens": [], "normalizer": None, "pre_tokenizer": None, "post_processor": None, "decoder": None, "model": {"type": "WordLevel", "vocab": {"[UNK]": 0}, "unk_token": "[UNK]"}}),
        "model-00001-of-00002.safetensors": tensor("a"),
        "model-00002-of-00002.safetensors": tensor("b"),
        "model.safetensors.index.json": encode({"weight_map": {"a": "model-00001-of-00002.safetensors", "b": "model-00002-of-00002.safetensors"}}),
    }
    result = {f"{mode}/{path}": data for mode in ("complete", "cancel", "missing") for path, data in package.items() if not (mode == "missing" and path == "model-00002-of-00002.safetensors")}
    result["opaque/arbitrary.dat"] = b"Owned arbitrary bytes: acquisition does not imply model admission.\x00\xff"
    return result


class Source(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, objects):
        super().__init__(("127.0.0.1", 0), Handler)
        self.objects = objects
        self.requests = []
        self.failures = []
        self.lock = threading.Lock()
        self.stop = threading.Event()
        self.dropped = False


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def do_HEAD(self):
        self.handle_owned()

    def do_GET(self):
        self.handle_owned()

    def send_owned(self, code, data, **headers):
        self.send_response(code)
        self.send_header("Content-Length", str(len(data)))
        self.send_header("Connection", "close")
        self.send_header("Last-Modified", "Wed, 01 Jan 2025 00:00:00 GMT")
        for name, value in headers.items():
            self.send_header(name, value)
        self.end_headers()
        self.close_connection = True

    def handle_owned(self):
        try:
            check(signature(self, SECRET) and not signature(self, "wrong-owned-fixture-secret"), "signature oracle rejected request")
            parts = urlsplit(self.path)
            query = parse_qs(parts.query, keep_blank_values=True)
            check(parts.path.startswith("/fixture-bucket"), "unexpected bucket")
            key = unquote(parts.path.removeprefix("/fixture-bucket/"))
            with self.server.lock:
                self.server.requests.append({"method": self.command, "key": key, "query": query, "range": self.headers.get("Range"), "if_match": self.headers.get("If-Match"), "signature_valid": True, "wrong_secret_rejected": True})
            if query.get("list-type") == ["2"]:
                check(self.command == "GET", "listing must use GET")
                prefix = query["prefix"][0]
                keys = sorted(key for key in self.server.objects if key.startswith(prefix))
                page_size = int(query["max-keys"][0])
                token = query.get("continuation-token", [None])[0]
                offset = int(token.removeprefix("owned+page/=")) if token else 0
                selected = keys[offset:offset + page_size]
                more = offset + len(selected) < len(keys)
                continuation = f"<ContinuationToken>{escape(token)}</ContinuationToken>" if token else ""
                if more:
                    continuation += f"<NextContinuationToken>owned+page/={offset + len(selected)}</NextContinuationToken>"
                contents = "".join(f"<Contents><Key>{escape(key)}</Key><ETag>{escape(ETAG)}</ETag><Size>{len(self.server.objects[key])}</Size></Contents>" for key in selected)
                data = f'<ListBucketResult xmlns="http://s3.amazonaws.com/doc/2006-03-01/"><Name>fixture-bucket</Name><Prefix>{escape(prefix)}</Prefix><MaxKeys>{page_size}</MaxKeys><KeyCount>{len(selected)}</KeyCount><IsTruncated>{str(more).lower()}</IsTruncated>{continuation}{contents}</ListBucketResult>'.encode()
                self.send_owned(200, data, **{"Content-Type": "application/xml"})
                self.wfile.write(data)
                return
            data = self.server.objects[key]
            version = query.get("versionId")
            check(version in (None, [VERSION]), "wrong immutable version")
            if self.command == "HEAD":
                if version is None:
                    check(self.headers.get("If-Match") == ETAG, "listing pin omitted conditional ETag")
                self.send_owned(200, data, **{"ETag": ETAG, "x-amz-version-id": VERSION})
                return
            check(version == [VERSION] and self.headers.get("If-Match") == ETAG, "range read lost immutable identity")
            match = re.fullmatch(r"bytes=(\d+)-(\d*)", self.headers.get("Range", ""))
            check(match is not None, "range header missing")
            start = int(match[1])
            end = int(match[2]) if match[2] else len(data) - 1
            check(0 <= start <= end < len(data), "range outside selected object")
            part = data[start:end + 1]
            self.send_owned(206, part, **{"ETag": ETAG, "x-amz-version-id": VERSION, "Content-Range": f"bytes {start}-{end}/{len(data)}"})
            if key == "complete/model-00001-of-00002.safetensors" and not self.server.dropped:
                check(start == 0, "initial transfer did not start at zero")
                self.server.dropped = True
                self.wfile.write(part[:4096])
                self.wfile.flush()
                self.connection.shutdown(socket.SHUT_RDWR)
            elif key == "cancel/model-00001-of-00002.safetensors":
                self.wfile.write(part[:4096])
                self.wfile.flush()
                self.server.stop.wait(30)
            else:
                self.wfile.write(part)
        except (BrokenPipeError, ConnectionResetError):
            pass
        except Exception as error:
            # Avoid printing request headers or authorization text, even synthetic.
            with self.server.lock:
                self.server.failures.append(f"{type(error).__name__}: {error}")
            self.close_connection = True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)
    # RFC 4231, case 1: validate the independent HMAC primitive.
    check(hmac.digest(bytes([0x0b] * 20), b"Hi There", "sha256").hex() == "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7", "HMAC known answer failed")
    objects = owned_objects()
    with tempfile.TemporaryDirectory(prefix="pumas-owned-s3-cohort-") as temporary:
        workspace = Path(temporary)
        manifest = {}
        for key, data in objects.items():
            path = workspace / key
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(data)
            retained = output / "objects" / key
            retained.parent.mkdir(parents=True, exist_ok=True)
            retained.write_bytes(data)
            manifest[key] = {"sha256": hashlib.sha256(data).hexdigest(), "bytes": len(data)}
        (workspace / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        (output / "objects" / "manifest.json").write_text(json.dumps(manifest, indent=2) + "\n")
        ca = ROOT / "rust/crates/pumas-core/tests/fixtures/http-tls/localhost.pem"
        key = workspace / "fixture.key"
        subprocess.run(["openssl", "pkcs12", "-in", str(ca.with_suffix(".p12")), "-passin", "pass:fixture", "-nocerts", "-nodes", "-out", str(key)], check=True, capture_output=True)
        server = Source(objects)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(ca, key)
        server.socket = context.wrap_socket(server.socket, server_side=True)
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        command = ["cargo", "test", "--offline", "--locked", "--manifest-path", "rust/Cargo.toml", "-p", "pumas-library", "--no-default-features", "--features", "s3", "--test", "s3_sharded_cohort", "--", "--ignored", "--nocapture"]
        environment = dict(os.environ, ORT_SKIP_DOWNLOAD="1", SSL_CERT_FILE=str(ca), PUMAS_S3_COHORT_ENDPOINT=f"https://127.0.0.1:{server.server_port}", PUMAS_S3_COHORT_OBJECTS=str(workspace))
        try:
            with (output / "cargo-test.log").open("w") as log:
                result = subprocess.run(command, cwd=ROOT, env=environment, stdout=log, stderr=subprocess.STDOUT, timeout=600)
        finally:
            server.stop.set()
            server.shutdown()
            server.server_close()
            thread.join(5)
            (output / "requests.json").write_text(json.dumps(server.requests, indent=2) + "\n")
            (output / "fixture-failures.json").write_text(json.dumps(server.failures, indent=2) + "\n")
        check(result.returncode == 0, f"Rust cohort failed; see {output / 'cargo-test.log'}")
        check(not server.failures, "independent fixture oracle failed")
        requests = server.requests
        resumed = [request for request in requests if request["method"] == "GET" and request["key"] == "complete/model-00001-of-00002.safetensors"]
        check([request["range"] for request in resumed] == [f"bytes=0-{len(objects['complete/model-00001-of-00002.safetensors']) - 1}", f"bytes=4096-{len(objects['complete/model-00001-of-00002.safetensors']) - 1}"], "transfer did not resume at exact retained offset")
        check(all(request["query"].get("versionId") == [VERSION] for request in requests if request["method"] == "GET" and request["range"]), "range reads lost version pins")
        cancelled = [request for request in requests if request["method"] == "GET" and request["key"].startswith("cancel/") and request["range"]]
        check([request["key"] for request in cancelled] == ["cancel/config.json", "cancel/model-00001-of-00002.safetensors"], "cancellation did not stop during the held shard body")
        result = {"result": "passed", "evidence_kind": "synthetic-https-protocol-cohort", "real_service_acceptance": False, "inference_executed": False, "cases": ["authenticated-paginated-sharded-resume-and-cold-receipt", "missing-shard-byte-verification-without-publication", "unknown-bytes-without-model-admission", "cancel-during-shard-body-without-publication"], "requests": len(requests), "resume_offset": 4096, "all_signatures_verified": True, "wrong_secret_rejected": True, "command": command, "objects": manifest}
        (output / "result.json").write_text(json.dumps(result, indent=2) + "\n")
        print(json.dumps({key: result[key] for key in ("result", "evidence_kind", "requests", "resume_offset")}))


if __name__ == "__main__":
    main()
