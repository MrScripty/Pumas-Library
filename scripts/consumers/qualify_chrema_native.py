#!/usr/bin/env python3
"""Execute the pinned Chrema borrowing consumer against a qualified Pumas binary."""

import argparse
import asyncio
import datetime
import hashlib
import http.server
import json
import os
from pathlib import Path
import shutil
import sqlite3
import subprocess
import sys
import threading

CHREMA_HEAD = "d9f660beb8273aac16e62fae3902b9f5e4d1559b"
CHREMA_TREE = "cc755424881f3c53b20b626ca7b44f14fb5f2cb8"
PUMAS_SOURCE = "fe3c9f04c243f436a393ae47e16f616244a85cee"
HELPER_SHA = "ee267a62f2a688edfb3e9bbe79c704c7f69ed754be32bd66fa6f1e8e7b8ea6b9"
SENTINEL = b"Authorized disposable actual Chrema/Pumas qualification root\n"


def require(condition, message):
    if not condition:
        raise AssertionError(message)


def pin(path):
    with path.open("rb") as source:
        digest = hashlib.file_digest(source, "sha256").hexdigest()
    return {"path": str(path), "bytes": path.stat().st_size, "sha256": digest}


def git(root, *arguments):
    return subprocess.check_output(["git", "-C", str(root), *arguments], text=True).strip()


def checkpoint(work, report):
    report["checkpoint_utc"] = datetime.datetime.now(datetime.UTC).isoformat()
    temporary = work / "qualification.pending.json"
    with temporary.open("w") as output:
        output.write(json.dumps(report, indent=2) + "\n")
        output.flush()
        os.fsync(output.fileno())
    temporary.replace(work / "qualification.json")


class DenyHandler(http.server.BaseHTTPRequestHandler):
    def setup(self):
        super().setup()
        self.connection.settimeout(5)

    def log_message(self, *_arguments):
        pass

    def refuse(self):
        with self.server.lock:
            self.server.count += 1
            if len(self.server.requests) < 64:
                self.server.requests.append({"method": self.command, "target": self.path[:512]})
        self.send_response(503)
        self.send_header("Content-Length", "0")
        self.send_header("Connection", "close")
        self.end_headers()
        self.close_connection = True

    do_CONNECT = do_GET = do_HEAD = do_POST = do_PUT = do_DELETE = refuse


class DenyProxy(http.server.ThreadingHTTPServer):
    daemon_threads = False
    block_on_close = True

    def __init__(self):
        super().__init__(("127.0.0.1", 0), DenyHandler)
        self.lock = threading.Lock()
        self.count = 0
        self.requests = []
        self.thread = threading.Thread(target=self.serve_forever)
        self.thread.start()

    def close(self):
        self.shutdown()
        self.server_close()
        self.thread.join(timeout=5)
        require(not self.thread.is_alive(), "deny proxy did not join")


def fixture(work, mode, proxy):
    base = work / mode
    base.mkdir()
    root = base / "library"
    root.mkdir()
    (root / "qualification-sentinel").write_bytes(SENTINEL)
    for name in ("empty-credentials", "empty-config"):
        (base / name).write_text("")
    env = {k: v for k, v in os.environ.items() if not k.startswith(("AWS_", "HF_", "HUGGING_FACE_"))}
    env.update(XDG_CONFIG_HOME=str(base / "config"), XDG_CACHE_HOME=str(base / "cache"),
               HF_HOME=str(base / "hf"), HF_TOKEN_PATH=str(base / "hf" / "token"),
               PUMAS_REGISTRY_DB_PATH=str(base / "registry.db"),
               HF_HUB_OFFLINE="1", TRANSFORMERS_OFFLINE="1", ORT_SKIP_DOWNLOAD="1",
               AWS_EC2_METADATA_DISABLED="true", AWS_SHARED_CREDENTIALS_FILE=str(base / "empty-credentials"),
               AWS_CONFIG_FILE=str(base / "empty-config"), PYTHONDONTWRITEBYTECODE="1")
    for key in ("HTTP_PROXY", "HTTPS_PROXY", "ALL_PROXY"):
        env[key] = env[key.lower()] = f"http://127.0.0.1:{proxy.server_port}"
    env["NO_PROXY"] = env["no_proxy"] = "127.0.0.1,localhost,::1"
    return root, env


def absent(pid):
    try:
        os.kill(pid, 0)
    except ProcessLookupError:
        return True
    return False


def cleaned_owner(owner, root, environment):
    require(len(owner.cleanup_results) == 2, "owned fixture child cleanup incomplete")
    require(all(item["exit_code"] == 0 for item in owner.cleanup_results), "nonzero owned fixture cleanup")
    require(all(absent(pid) for pid in owner.child_pids), "owned fixture child still exists")
    database = Path(environment["PUMAS_REGISTRY_DB_PATH"])
    with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True) as connection:
        rows = connection.execute("SELECT started_at FROM instances").fetchall()
    require(rows == [], "orderly fixture close left instance rows")
    require((root / "qualification-sentinel").read_bytes() == SENTINEL, "sentinel changed")
    return {"child_pids": owner.child_pids, "cleanup_results": owner.cleanup_results,
            "all_child_pids_absent": True, "registry_instance_rows": rows, "sentinel_unchanged": True}


async def owner_health(owner):
    catalog = await owner.rpc("get_models")
    lookup = await owner.rpc("lookup_model", {"model_id": "unknown/fixture/absent"})
    require(catalog.get("result") == {"models": {}, "success": True}, "fixture catalog changed")
    require(lookup.get("result") == {"contract_version": 1, "requested_model_id": "unknown/fixture/absent",
                                   "resolution": {"status": "missing"}}, "fixture lookup changed")
    return {"catalog": catalog, "lookup": lookup}


async def qualify(args, report, proxy, local_http_session):
    for mode in ("normal-release", "caller-cancel", "owner-revocation"):
        root, environment = fixture(args.work_dir, mode, proxy)
        case = {"mode": mode, "root": str(root), "registry": environment["PUMAS_REGISTRY_DB_PATH"],
                "phase": "fixture-created"}
        report["cases"].append(case)
        checkpoint(args.work_dir, report)
        process = None
        try:
            async with local_http_session(args.binary, root, binary_sha256=args.binary_sha256,
                                          allow_start=True, environment=environment) as owner:
                require(owner.ownership == "owned", "test operator did not own fresh fixture")
                case.update(phase="operator-owned", owner_description=owner.description,
                            owned_fixture_child_pids=owner.child_pids)
                checkpoint(args.work_dir, report)
                description = root.parent / "owner-description.json"
                description.write_text(json.dumps(owner.description) + "\n")
                command = [str(args.node), str(Path(__file__).with_name("chrema_native_probe.mjs")),
                           str(args.chrema_source), str(args.binary), args.binary_sha256,
                           str(root), str(description), mode]
                process = await asyncio.create_subprocess_exec(
                    *command, env=environment, stdin=asyncio.subprocess.PIPE,
                    stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE, limit=1048576)
                case.update(phase="Chrema-caller-launched", caller_pid=process.pid, command=command)
                checkpoint(args.work_dir, report)
                ready_line = await asyncio.wait_for(process.stdout.readline(), 40)
                ready = json.loads(ready_line)
                case["first_caller_observation"] = ready
                require(ready["phase"] == "ready", f"Chrema did not acquire actual owner: {ready}")
                case.update(phase="Chrema-native-reads-completed-and-retained", ready=ready)
                checkpoint(args.work_dir, report)
                case["owner_health_while_Chrema_retained"] = await owner_health(owner)
                if mode != "owner-revocation":
                    process.stdin.write(b"finish\n")
                    await process.stdin.drain()
                    stdout, stderr = await asyncio.wait_for(process.communicate(), 40)
                    case["owner_health_after_Chrema_close"] = await owner_health(owner)
                    require(process.returncode == 0, f"Chrema caller failed: {stderr.decode()}")
                else:
                    case["phase"] = "graceful-owned-operator-stop-requested"
                    checkpoint(args.work_dir, report)
            case["fixture_cleanup"] = cleaned_owner(owner, root, environment)
            if mode == "owner-revocation":
                stdout, stderr = await asyncio.wait_for(process.communicate(), 40)
                require(process.returncode == 0, f"native owner revocation caller failed: {stderr.decode()}")
            finished = json.loads(stdout)
            require(finished["phase"] == "finished" and finished["observation"]["status"] == "passed",
                    "native Chrema probe did not pass")
            case["Chrema"] = finished["observation"]
            case["caller_exit_code"] = process.returncode
            case["caller_stderr"] = stderr.decode()
            require(absent(process.pid), "Chrema caller PID not reaped")
            require(all(absent(item["pid"]) for item in case["Chrema"]["observer_children"]),
                    "Chrema observer child PID still exists")
            case.update(phase="passed", caller_and_observer_pids_absent=True)
            checkpoint(args.work_dir, report)
        finally:
            # Never force-kill or replay an uncertain caller/reconciliation.
            # Context exit above observes our own fixture's graceful shutdown.
            if process is not None and process.returncode is None:
                if process.stdin and not process.stdin.is_closing():
                    try:
                        process.stdin.write(b"finish\n")
                        await process.stdin.drain()
                    except (BrokenPipeError, ConnectionResetError):
                        pass
                try:
                    await asyncio.wait_for(process.communicate(), 40)
                except TimeoutError:
                    case["unconfirmed_caller_pid"] = process.pid
                    checkpoint(args.work_dir, report)
                    raise RuntimeError(f"caller {process.pid} cleanup unconfirmed; do not retry this fixture")


def main(args):
    for field in ("binary", "chrema_source", "pumas_source"):
        setattr(args, field, getattr(args, field).resolve(strict=True))
    args.node = Path(shutil.which("node")).resolve(strict=True)
    args.work_dir = args.work_dir.resolve()
    require(git(args.chrema_source, "rev-parse", "HEAD") == CHREMA_HEAD, "Chrema head changed")
    require(git(args.chrema_source, "rev-parse", "HEAD^{tree}") == CHREMA_TREE, "Chrema tree changed")
    require(not git(args.chrema_source, "status", "--porcelain"), "Chrema source is modified")
    require(git(args.pumas_source, "rev-parse", "HEAD") == PUMAS_SOURCE, "Pumas contract source changed")
    helper = args.pumas_source / "scripts" / "consumers" / "local_http_session.py"
    require(pin(helper)["sha256"] == HELPER_SHA, "Pumas fixture session helper changed")
    executable = pin(args.binary)
    require(executable["sha256"] == args.binary_sha256, "Pumas executable changed")
    args.work_dir.mkdir(exist_ok=False)
    sys.path.insert(0, str(helper.parent))
    from local_http_session import local_http_session
    sources = ["local-session", "local-owner", "model-catalog", "finite-provider", "capabilities"]
    report = {"status": "running", "started_utc": datetime.datetime.now(datetime.UTC).isoformat(),
              "Chrema_head": CHREMA_HEAD, "Chrema_tree": CHREMA_TREE,
              "Chrema_source_files": [pin(args.chrema_source / f"agent-platform-pumas-{name}.mjs") for name in sources],
              "Pumas_contract_source": PUMAS_SOURCE, "Pumas_executable": executable,
              "Pumas_fixture_helper": pin(helper), "node_executable": pin(args.node),
              "driver_sources": [pin(Path(__file__).resolve()), pin(Path(__file__).with_name("chrema_native_probe.mjs").resolve())],
              "node_version": subprocess.check_output([str(args.node), "--version"], text=True).strip(),
              "work_dir": str(args.work_dir), "cases": [], "real_model_inference": False,
              "model_or_SDK_download": False, "live_provider_qualified": False,
              "external_application_scope": "Actual Chrema generic consumer modules; no Nearwork UI or workflow."}
    checkpoint(args.work_dir, report)
    proxy = DenyProxy()
    failure = None
    try:
        asyncio.run(qualify(args, report, proxy, local_http_session))
        require(pin(args.binary) == executable, "Pumas bytes changed during execution")
        require(git(args.chrema_source, "rev-parse", "HEAD") == CHREMA_HEAD
                and not git(args.chrema_source, "status", "--porcelain"), "Chrema source changed during execution")
        report["status"] = "passed"
    except BaseException as error:
        failure = error
        report.update(status="failed", failure={"name": type(error).__name__, "message": str(error)})
    finally:
        proxy.close()
        report["deny_proxy"] = {"request_count": proxy.count, "requests": proxy.requests,
                                "forwarded_requests": 0, "thread_joined": True,
                                "scope": "configured deny proxy; not OS-wide network enforcement"}
        report["finished_utc"] = datetime.datetime.now(datetime.UTC).isoformat()
        checkpoint(args.work_dir, report)
        print(json.dumps(report, indent=2))
    if failure is not None:
        raise failure


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--binary-sha256", required=True)
    parser.add_argument("--chrema-source", type=Path, required=True)
    parser.add_argument("--pumas-source", type=Path, required=True)
    parser.add_argument("--work-dir", type=Path, required=True, help="new disposable attempt; refuses existing directory")
    main(parser.parse_args())
