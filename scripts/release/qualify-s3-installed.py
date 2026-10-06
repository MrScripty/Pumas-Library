#!/usr/bin/env python3
"""Exercise an installed production S3 RPC binary against an owned TLS fixture.

This is controlled protocol evidence, not qualification of an S3 provider or
an inference runtime. Never pass real credentials or a real bucket endpoint.
"""

import argparse
import hashlib
import hmac
import http.server
import json
import os
from pathlib import Path
import re
import shutil
import signal
import ssl
import struct
import subprocess
import tarfile
import tempfile
import threading
import time
import urllib.parse
import urllib.request
import uuid

from s3_build_provenance import ATTRIBUTION, validate_provenance

ROOT = Path(__file__).resolve().parents[2]
ACCESS = "installed-s3-synthetic-access"
SECRET = "installed-s3-synthetic-secret"
TOKEN = "installed-s3-synthetic-token"
AMBIENT = (
    "synthetic-unselected-ambient-access",
    "synthetic-unselected-ambient-secret",
    "synthetic-unselected-ambient-token",
)
SECRETS = tuple(value.encode() for value in (ACCESS, SECRET, TOKEN, *AMBIENT))
WEIGHTS = b"GGUF" + struct.pack("<IQQ", 3, 0, 0)
EMPTY_SHA = hashlib.sha256(b"").hexdigest()
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def check(condition, message):
    if not condition:
        raise AssertionError(message)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def secret_free(data):
    check(
        all(secret not in data for secret in SECRETS),
        "synthetic credential escaped ephemeral boundary",
    )


def valid_signature(handler, secret):
    # This fixture owns a simple path and one already canonical version query.
    fields = dict(
        field.split("=", 1)
        for field in handler.headers["Authorization"].removeprefix("AWS4-HMAC-SHA256 ").split(", ")
    )
    access, scope = fields["Credential"].split("/", 1)
    date, region, service, terminal = scope.split("/")
    check(
        access == ACCESS
        and (region, service, terminal) == ("fixture-region", "s3", "aws4_request"),
        "unexpected signing scope",
    )
    signed = fields["SignedHeaders"]
    names = signed.split(";")
    check(
        names == sorted(names) and "host" in names and "x-amz-date" in names,
        "invalid signed header set",
    )
    if handler.server.token:
        check("x-amz-security-token" in names, "session token was not signed")
    parts = urllib.parse.urlsplit(handler.path)
    headers = "".join(f"{name}:{' '.join(handler.headers[name].split())}\n" for name in names)
    canonical = f"{handler.command}\n{parts.path}\n{parts.query}\n{headers}\n{signed}\n{handler.headers['x-amz-content-sha256']}"
    message = f"AWS4-HMAC-SHA256\n{handler.headers['x-amz-date']}\n{scope}\n{hashlib.sha256(canonical.encode()).hexdigest()}"
    key = ("AWS4" + secret).encode()
    for component in (date, region, service, terminal):
        key = hmac.digest(key, component.encode(), "sha256")
    return hmac.compare_digest(
        hmac.new(key, message.encode(), "sha256").hexdigest(), fields["Signature"]
    )


class Source(http.server.ThreadingHTTPServer):
    daemon_threads = True

    def __init__(self, cert, key, mode, authenticated, token):
        super().__init__(("127.0.0.1", 0), Handler)
        context = ssl.SSLContext(ssl.PROTOCOL_TLS_SERVER)
        context.load_cert_chain(cert, key)
        self.socket = context.wrap_socket(self.socket, server_side=True)
        self.mode, self.authenticated, self.token = mode, authenticated, token
        self.requests, self.errors = [], []
        self.started, self.closed = threading.Event(), threading.Event()
        self.thread = threading.Thread(target=self.serve_forever, daemon=True)
        self.thread.start()
        self.endpoint = f"https://localhost:{self.server_port}"

    def stop(self):
        self.shutdown()
        self.server_close()
        self.thread.join(5)
        check(not self.thread.is_alive(), "fixture listener failed to drain")
        check(not self.errors, "fixture observed an invalid pin, range or credential mode")


class Handler(http.server.BaseHTTPRequestHandler):
    protocol_version = "HTTP/1.1"

    def log_message(self, *_):
        pass

    def do_HEAD(self):
        self.respond(True)

    def do_GET(self):
        self.respond(False)

    def respond(self, head):
        source = self.server
        try:
            parts = urllib.parse.urlsplit(self.path)
            version = urllib.parse.parse_qs(parts.query).get("versionId", [None])[0]
            primary = version == "weights-v1"
            check(version in ("weights-v1", "data-v2"), "unexpected version")
            check(parts.path == "/fixture-bucket/models/shared", "unexpected object key")
            authorization = self.headers.get("Authorization", "")
            check(bool(authorization) == source.authenticated, "unexpected signing mode")
            if source.authenticated:
                check(
                    authorization.startswith("AWS4-HMAC-SHA256 ")
                    and f"Credential={ACCESS}/" in authorization,
                    "missing explicit SigV4 identity",
                )
                check(
                    valid_signature(self, SECRET)
                    and not valid_signature(self, "wrong-synthetic-secret"),
                    "invalid SigV4 signature",
                )
            check(
                self.headers.get("x-amz-security-token") == (TOKEN if source.token else None),
                "unexpected session token",
            )
            check(SECRET not in authorization, "secret key appeared on wire")
            source.requests.append({"method": "HEAD" if head else "GET", "version": version})
            if source.mode == "missing" and primary:
                self.send_response(404)
                self.send_header("Content-Length", "0")
                self.send_header("Connection", "close")
                self.end_headers()
                self.close_connection = True
                return
            body = WEIGHTS if primary and source.mode != "empty-primary" else b""
            start = 0
            if not head:
                check(primary and body, "empty member issued a GET")
                match = re.fullmatch(r"bytes=(\d+)-(\d+)", self.headers.get("Range", ""))
                check(match and int(match[2]) == len(body) - 1, "invalid requested range")
                start = int(match[1])
                check(0 <= start < len(body), "invalid range start")
                check(self.headers.get("If-Match") == '"selected"', "conditional read missing")
                source.requests[-1]["range_start"] = start
            self.send_response(200 if head else 206)
            if not (head and source.mode == "unknown" and primary):
                self.send_header("Content-Length", str(len(body) - start))
            self.send_header("Last-Modified", "Wed, 01 Jan 2025 00:00:00 GMT")
            self.send_header("x-amz-version-id", version)
            self.send_header("ETag", '"selected"')
            self.send_header("Connection", "close")
            if not head:
                self.send_header("Content-Range", f"bytes {start}-{len(body) - 1}/{len(body)}")
            self.end_headers()
            if not head:
                self.wfile.write(
                    body[start : start + 1]
                    if source.mode in ("truncated", "cancel")
                    else body[start:]
                )
                self.wfile.flush()
                if source.mode == "cancel":
                    source.started.set()
                    self.connection.settimeout(10)
                    check(self.connection.recv(1) == b"", "cancel did not close source body")
                    source.closed.set()
            self.close_connection = True
        except Exception as error:
            source.errors.append(type(error).__name__)
            self.close_connection = True


class Backend:
    def __init__(self, binary, root, ca):
        self.root, self.output, self.overflow = root, bytearray(), False
        env = {
            **os.environ,
            "XDG_CONFIG_HOME": str(root / "config"),
            "APPDATA": str(root / "config"),
            "PUMAS_REGISTRY_DB_PATH": str(root / "registry.db"),
            "SSL_CERT_FILE": str(ca),
            "AWS_ACCESS_KEY_ID": AMBIENT[0],
            "AWS_SECRET_ACCESS_KEY": AMBIENT[1],
            "AWS_SESSION_TOKEN": AMBIENT[2],
        }
        self.process = subprocess.Popen(
            [str(binary), "--launcher-root", str(root), "--port", "0", "--debug"],
            cwd=root,
            env=env,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        self.reader = threading.Thread(target=self.capture, daemon=True)
        self.reader.start()
        try:
            deadline = time.monotonic() + 45
            while time.monotonic() < deadline:
                match = re.search(rb"RPC_PORT=(\d+)", self.output)
                check(self.process.poll() is None, "installed backend exited during startup")
                if match:
                    self.base = f"http://127.0.0.1:{match[1].decode()}"
                    return
                time.sleep(0.05)
            raise TimeoutError("installed backend startup deadline")
        except BaseException:
            self.stop()
            raise

    def capture(self):
        for line in self.process.stdout:
            if len(self.output) + len(line) <= 1024 * 1024:
                self.output.extend(line)
            else:
                self.overflow = True
        self.process.stdout.close()

    def rpc(self, method, params):
        request = urllib.request.Request(
            self.base + "/rpc",
            data=json.dumps(
                {"jsonrpc": "2.0", "id": 1, "method": method, "params": params}
            ).encode(),
            headers={"Content-Type": "application/json"},
        )
        with HTTP.open(request, timeout=10) as response:
            payload = response.read(1024 * 1024 + 1)
        check(len(payload) <= 1024 * 1024, "RPC response exceeded fixture budget")
        secret_free(payload)
        return json.loads(payload)

    def stop(self, expected_failure=False):
        if self.process.poll() is None:
            self.process.send_signal(signal.SIGINT)
        try:
            status = self.process.wait(15)
        except subprocess.TimeoutExpired:
            os.killpg(self.process.pid, signal.SIGKILL)
            self.process.wait(5)
            raise TimeoutError("installed backend required forced shutdown")
        self.reader.join(5)
        secret_free(self.output)
        check(not self.overflow, "backend log exceeded fixture budget")
        check(status == (1 if expected_failure else 0), "unexpected drained backend exit status")
        return status


def params(source, operation, single):
    primary = {
        "key": "models/shared",
        "version_id": "weights-v1",
        "logical_path": "weights.gguf",
        "sha256": hashlib.sha256(WEIGHTS if source.mode != "empty-primary" else b"").hexdigest(),
    }
    common = {
        "operation_id": operation,
        "endpoint": source.endpoint,
        "region": "fixture-region",
        "bucket": "fixture-bucket",
        "addressing": "path",
        "family": "fixture",
        "official_name": "Installed synthetic GGUF",
    }
    if single:
        return {
            **common,
            "key": primary["key"],
            "version_id": primary["version_id"],
            "filename": primary["logical_path"],
            "sha256": primary["sha256"],
        }
    return {
        **common,
        "primary_logical_path": "weights.gguf",
        "files": [
            primary,
            {
                "key": "models/shared",
                "version_id": "data-v2",
                "logical_path": "config/data.json",
                "sha256": "b" * 64 if source.mode == "digest" else EMPTY_SHA,
            },
        ],
    }


def case(binary, directory, ca, key, mode, authenticated=False, token=False, single=False):
    root = directory / str(uuid.uuid4())
    root.mkdir()
    source = Source(ca, key, mode, authenticated, token)
    backend = None
    try:
        backend = Backend(binary, root, ca)
        operation = str(uuid.uuid4())
        request = params(source, operation, single)
        if not single:
            for path in [
                "metadata.json",
                "overrides.json",
                "metadata.json/data.json",
                "OVERRIDES.JSON/data.json",
                "_metadata_.json",
                "metadata.json.bak/data.json",
                ".pumas_import_publication.json/data.json",
            ]:
                invalid = json.loads(json.dumps(request))
                invalid["files"][1]["logical_path"] = path
                response = backend.rpc("start_s3_model_bundle_import", invalid)
                check(
                    response.get("error", {}).get("code") == -32602,
                    "reserved destination did not fail decoding",
                )
                check(
                    backend.rpc("get_s3_model_import", {})["result"]["status"] == "idle",
                    "invalid request admitted a job",
                )
                check(
                    not (root / "launcher-data" / f".s3-import-{operation}").exists(),
                    "invalid request created workspace",
                )
                check(not source.requests, "invalid request performed source IO")
        method = "start_s3_model_import" if single else "start_s3_model_bundle_import"
        if authenticated:
            method = (
                "start_authenticated_s3_model_import"
                if single
                else "start_authenticated_s3_model_bundle_import"
            )
            request = {
                "source": request,
                "credentials": {
                    "access_key_id": ACCESS,
                    "secret_access_key": SECRET,
                    "session_token": TOKEN if token else None,
                },
            }
        check(
            backend.rpc(method, request)["result"]["status"] == "running",
            "corrected request failed admission",
        )
        if mode == "cancel":
            check(source.started.wait(10), "cancel source never began")
            snapshot = backend.rpc("get_s3_model_bundle_import", {"operation_id": operation})[
                "result"
            ]
            check(
                snapshot["bundle_progress"]["files_acquired"] == 1
                and snapshot["bundle_progress"]["bytes_acquired"] == "0",
                "empty member aggregate progress is wrong",
            )
            check(
                backend.rpc("cancel_s3_model_import", {"operation_id": operation})["result"][
                    "accepted"
                ],
                "cancel was refused before finalization",
            )
        deadline = time.monotonic() + 60
        while time.monotonic() < deadline:
            result = backend.rpc("get_s3_model_import", {"operation_id": operation})["result"]
            if result["status"] == "finished":
                break
            time.sleep(0.05)
        else:
            raise TimeoutError("installed acquisition terminal deadline")
        success = mode == "success"
        check(
            result["result"]["status"]
            == ("completed" if success else "cancelled" if mode == "cancel" else "failed"),
            "terminal result contradicts source outcome",
        )
        if success:
            model = result["result"]["model_id"]
            target = root / "shared-resources/models" / model
            check((target / "weights.gguf").read_bytes() == WEIGHTS, "published primary changed")
            receipt_path = target / ".pumas_import_publication.json"
            receipt_bytes = receipt_path.read_bytes()
            receipt = json.loads(receipt_bytes)["acquisition"]
            check(
                receipt["demand"]["consumer"] == "model.s3.workflow"
                and receipt["demand"]["operation"] == operation,
                "receipt demand changed",
            )
            check(
                len(receipt["verified_files"]) == (1 if single else 2), "receipt set is incomplete"
            )
            if not single:
                check(
                    (target / "config/data.json").read_bytes() == b"", "empty auxiliary was omitted"
                )
                check(
                    receipt["verified_files"][0]["bytes"] == 0
                    and receipt["verified_files"][0]["sha256"] == EMPTY_SHA,
                    "empty receipt lacks exact verification",
                )
                check(
                    [json.loads(file["source_key"])[1] for file in receipt["manifest"]["files"]]
                    == ["data-v2", "weights-v1"],
                    "receipt pins changed",
                )
            backend.stop()
            before = len(source.requests)
            backend = Backend(binary, root, ca)
            models = backend.rpc("get_models", {})
            check(model in json.dumps(models), "cold owner lost registered model")
            check(
                receipt_path.read_bytes() == receipt_bytes and len(source.requests) == before,
                "cold owner replayed or replaced publication proof",
            )
        else:
            check(result["result"]["retained_work"], "failure lost cleanup custody")
            check(
                not result["result"].get("published_model_id"), "invalid source published a model"
            )
            replay = backend.rpc(method, request)
            check(replay["result"]["status"] == "rejected", "retained failure silently replayed")
        status = backend.stop(expected_failure=mode == "empty-primary")
        backend = None
        if mode == "cancel":
            check(source.closed.wait(5), "source response did not close on cancel")
        for path in root.rglob("*"):
            if path.is_file():
                secret_free(path.read_bytes())
        return {
            "mode": mode,
            "authenticated": authenticated,
            "session_token": token,
            "single": single,
            "requests": source.requests,
            "drained_exit": status,
            "result": "passed",
        }
    finally:
        if backend is not None:
            backend.stop(expected_failure=mode == "empty-primary")
        source.stop()


def package_inputs(binary):
    """One complete S3 notice profile, with no historical reader-only fallback."""
    directory = ROOT / ATTRIBUTION
    return [
        (binary, "pumas-rpc"),
        (ROOT / "LICENSE", "LICENSE.txt"),
        (directory / "THIRD-PARTY-NOTICES.txt", "THIRD-PARTY-NOTICES.txt"),
        (directory / "README.md", "ATTRIBUTION-README.md"),
        (directory / "inventory.json", "ATTRIBUTION-inventory.json"),
    ]


def verify_installed_inputs(installed, expected):
    check(
        {path.name for path in installed.iterdir()} == set(expected) | {"qualification.json"},
        "installed archive member set differs from qualification inputs",
    )
    for name, expected_hash in expected.items():
        check(digest(installed / name) == expected_hash, "installed input hash mismatch")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--provenance", type=Path, required=True)
    parser.add_argument("--source-head", required=True)
    args = parser.parse_args()
    binary = args.binary.resolve()
    check(binary.is_file(), "missing production binary")
    provenance = validate_provenance(
        json.loads(args.provenance.read_text()), binary, ROOT, args.source_head
    )
    args.output.mkdir(parents=True, exist_ok=True)
    payload = {
        "source_head": provenance["source"]["head"],
        "source_tree": provenance["source"]["tree"],
        "binary_sha256": provenance["binary_sha256"],
        "build_provenance": provenance,
        "provider": "controlled TLS protocol fixture; not an actual S3-compatible service",
        "target": "Linux x86_64",
        "production_profile": "release with defaults and explicit s3; no test-support; no inference claim",
    }
    archive_path = args.output / "pumas-s3-qualification-linux.tar.gz"
    check(not archive_path.exists(), "refusing to overwrite qualification archive")
    with tempfile.TemporaryDirectory(prefix="pumas-installed-s3-") as temporary:
        workspace = Path(temporary)
        stage, installed = workspace / "stage", workspace / "installed"
        stage.mkdir()
        installed.mkdir()
        inputs = package_inputs(binary)
        for original, name in inputs:
            shutil.copy2(original, stage / name)
        payload["packaged_files"] = {path.name: digest(path) for path in stage.iterdir()}
        (stage / "qualification.json").write_text(json.dumps(payload, indent=2) + "\n")
        with tarfile.open(archive_path, "w:gz") as archive:
            for path in sorted(stage.iterdir()):
                archive.add(path, arcname=path.name)
        with tarfile.open(archive_path) as archive:
            archive.extractall(installed, filter="data")
        verify_installed_inputs(installed, payload["packaged_files"])
        check(
            digest(installed / "pumas-rpc") == payload["binary_sha256"],
            "installed binary differs from build",
        )
        ca = ROOT / "rust/crates/pumas-core/tests/fixtures/http-tls/localhost.pem"
        key = workspace / "fixture.key"
        subprocess.run(
            [
                "openssl",
                "pkcs12",
                "-in",
                str(ca.with_suffix(".p12")),
                "-passin",
                "pass:fixture",
                "-nocerts",
                "-nodes",
                "-out",
                str(key),
            ],
            check=True,
            capture_output=True,
        )
        scenarios = [
            ("success", False, False, True),
            ("success", False, False, False),
            ("success", True, False, False),
            ("success", True, True, False),
        ]
        scenarios += [
            (mode, True, True, False)
            for mode in ("digest", "missing", "unknown", "truncated", "cancel", "empty-primary")
        ]
        payload["scenarios"] = [
            case(installed / "pumas-rpc", workspace, ca, key, *scenario) for scenario in scenarios
        ]
    payload["archive_sha256"] = digest(archive_path)
    payload["result"] = "passed"
    (args.output / "installed-result.json").write_text(json.dumps(payload, indent=2) + "\n")
    print(
        json.dumps(
            {
                "result": "passed",
                "scenarios": len(payload["scenarios"]),
                "binary_sha256": payload["binary_sha256"],
            }
        )
    )


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        message = str(error)
        for value in (ACCESS, SECRET, TOKEN, *AMBIENT):
            message = message.replace(value, "[REDACTED]")
        print(
            json.dumps({"result": "failed", "error_type": type(error).__name__, "error": message})
        )
        raise SystemExit(1)
