#!/usr/bin/env python3
"""Exercise FLUX.2 through the real managed Pumas RPC and image gateway.

Run from any directory inside a bounded scope, for example:

    systemd-run --user --scope -p MemoryMax=32G -p MemorySwapMax=0 \
      -- python3 scripts/acceptance/flux2_v214_rpc_acceptance.py \
      --output /tmp/flux2-v214-rpc.png

The default image is 512x512. Pass --width 1280 --height 720 for the full-size
trial when the memory result supports it.

The existing isolated launcher root must already contain selected Torch v2.14.0,
the managed torch-image-acceptance profile, and linked library model assets.
Only that root is used for Pumas state. The RPC log is saved beside the PNG.
"""

import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import struct
import subprocess
import sys
import threading
import time
import urllib.error
import urllib.request


REPO = Path(__file__).resolve().parents[2]
ROOT = REPO / "launcher-data/cache/torch-qualification/v214-flux2-e2e-root"
BINARY = REPO / "rust/target/debug/pumas-rpc"
MODEL_ID = "diffusion/black-forest-labs/flux_2-klein-9b-kv-fp8"
PROFILE_ID = "torch-image-acceptance"
ALIAS = MODEL_ID
TULDOK_TEST = REPO.parent.parent / "creative-media/Tuldok/tests/browser_images_real.cjs"
PROMPT = "A red ceramic teapot beside a yellow lemon on a blue table"
SEED = 42
HTTP = urllib.request.build_opener(urllib.request.ProxyHandler({}))


def emit(phase, **fields):
    print(json.dumps({"phase": phase, **fields}, sort_keys=True), flush=True)


def require(condition, label, value=None):
    if not condition:
        raise RuntimeError(f"{label}: {value!r}")


def cgroup_file(name):
    try:
        group = next(line.split("::", 1)[1].lstrip("/") for line in
                     Path("/proc/self/cgroup").read_text().splitlines()
                     if line.startswith("0::"))
        return Path("/sys/fs/cgroup") / group / name
    except (OSError, StopIteration):
        return None


def cgroup_value(name):
    path = cgroup_file(name)
    try:
        return path.read_text().strip() if path else None
    except OSError:
        return None


def memory_events():
    value = cgroup_value("memory.events")
    return dict((key, int(count)) for key, count in
                (line.split() for line in value.splitlines())) if value else {}


def process_cgroup(pid):
    try:
        return next(line.split("::", 1)[1] for line in
                    Path(f"/proc/{pid}/cgroup").read_text().splitlines()
                    if line.startswith("0::"))
    except (OSError, StopIteration):
        return None


def child_processes(pid):
    found = set()
    pending = [pid]
    while pending:
        parent = pending.pop()
        try:
            children = [int(value) for value in
                        Path(f"/proc/{parent}/task/{parent}/children").read_text().split()]
        except (OSError, ValueError):
            continue
        for child in children:
            if child not in found:
                found.add(child)
                pending.append(child)
    return found


def owned_group_members(group_id, group_cgroup):
    """Return live members of a spawned process group still in this scope."""
    members = []
    for path in Path("/proc").iterdir():
        if not path.name.isdigit():
            continue
        pid = int(path.name)
        try:
            stat = (path / "stat").read_text()
            fields = stat.rsplit(") ", 1)[1].split()
            state, process_group = fields[0], int(fields[2])
        except (OSError, IndexError, ValueError):
            continue
        if state != "Z" and process_group == group_id and process_cgroup(pid) == group_cgroup:
            members.append(pid)
    return members


def stop_owned_group(group_id, group_cgroup):
    for sig in (signal.SIGTERM, signal.SIGKILL):
        if not owned_group_members(group_id, group_cgroup):
            return
        try:
            os.killpg(group_id, sig)
        except ProcessLookupError:
            return
        deadline = time.monotonic() + (5 if sig == signal.SIGTERM else 2)
        while time.monotonic() < deadline:
            if not owned_group_members(group_id, group_cgroup):
                return
            time.sleep(0.1)
    require(not owned_group_members(group_id, group_cgroup),
            "owned browser process group survived cleanup", group_id)


def owned_process_cgroups(rpc_pid):
    processes = {rpc_pid, *child_processes(rpc_pid)}
    records = []
    for pid in sorted(processes):
        try:
            command = Path(f"/proc/{pid}/cmdline").read_bytes().replace(b"\0", b" ").decode(errors="replace")
        except OSError:
            command = ""
        records.append({"pid": pid, "cgroup": process_cgroup(pid), "command": command[:300]})
    return records


def owned_sidecars():
    """Find only this scope's sidecar running from the isolated runtime."""
    runtime = (ROOT / "torch-versions/v2.14.0").resolve()
    group = process_cgroup(os.getpid())
    found = []
    for path in Path("/proc").iterdir():
        if not path.name.isdigit():
            continue
        pid = int(path.name)
        try:
            command = (path / "cmdline").read_bytes()
            cwd = (path / "cwd").resolve(strict=True)
        except OSError:
            continue
        if b"serve.py" in command.split(b"\0") and cwd == runtime and process_cgroup(pid) == group:
            found.append({"pid": pid, "cgroup": group, "runtime_script": str(runtime / "serve.py")})
    return found


def stop_orphaned_sidecars():
    found = owned_sidecars()
    for sidecar in found:
        try:
            os.kill(sidecar["pid"], signal.SIGTERM)
        except ProcessLookupError:
            pass
    deadline = time.monotonic() + 5
    while found and time.monotonic() < deadline:
        found = owned_sidecars()
        if found:
            time.sleep(0.1)
    for sidecar in found:
        try:
            os.kill(sidecar["pid"], signal.SIGKILL)
        except ProcessLookupError:
            pass
    require(not owned_sidecars(), "owned Torch sidecar remained after cleanup", found)


def host_available_bytes():
    try:
        line = next(line for line in Path("/proc/meminfo").read_text().splitlines()
                    if line.startswith("MemAvailable:"))
        return int(line.split()[1]) * 1024
    except (OSError, StopIteration, ValueError, IndexError):
        return None


class HostMemoryGuard:
    """Stop this runner's RPC tree and browser group if host headroom falls."""

    def __init__(self, rpc_process):
        self.rpc_process = rpc_process
        self.browser_process = None
        self.stop = threading.Event()
        self.tripped = False
        self.low_available = None
        self.minimum_available = None
        self.thread = threading.Thread(target=self.sample, daemon=True)

    def start(self):
        self.thread.start()

    def finish(self):
        self.stop.set()
        self.thread.join(timeout=2)

    def sample(self):
        while not self.stop.is_set():
            available = host_available_bytes()
            if available is None or available < 6 * 1024**3:
                self.tripped = True
                self.low_available = available
                browser = self.browser_process
                if browser is not None:
                    try:
                        stop_owned_group(browser.pid, process_cgroup(os.getpid()))
                    except RuntimeError:
                        pass  # Continue stopping the RPC and sidecar as well.
                for pid in child_processes(self.rpc_process.pid) | {
                    sidecar["pid"] for sidecar in owned_sidecars()
                }:
                    try:
                        os.kill(pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                if self.rpc_process.poll() is None:
                    try:
                        os.killpg(self.rpc_process.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                return
            self.minimum_available = min(self.minimum_available or available, available)
            self.stop.wait(0.25)


def request_json(base, path, body=None, timeout=30):
    data = None if body is None else json.dumps(body).encode("utf-8")
    request = urllib.request.Request(
        base + path, data=data,
        headers={"Content-Type": "application/json"} if data is not None else {},
    )
    try:
        with HTTP.open(request, timeout=timeout) as response:
            return json.load(response)
    except urllib.error.HTTPError as error:
        detail = error.read(4096).decode("utf-8", errors="replace")
        raise RuntimeError(f"{path}: HTTP {error.code}: {detail}") from error


def rpc(base, method, params=None, timeout=30):
    reply = request_json(base, "/rpc", {
        "jsonrpc": "2.0", "id": 1, "method": method, "params": params or {},
    }, timeout=timeout)
    require("error" not in reply, f"RPC {method}", reply)
    return reply["result"]


def await_rpc_port(process, log_path):
    deadline = time.monotonic() + 60
    while time.monotonic() < deadline:
        contents = log_path.read_text(errors="replace")
        match = re.search(r"RPC_PORT=(\d+)", contents)
        if match:
            base = f"http://127.0.0.1:{match[1]}"
            request_json(base, "/health", timeout=10)
            return base
        require(process.poll() is None, "RPC exited before port announcement", contents[-4000:])
        time.sleep(0.1)
    raise RuntimeError(f"RPC did not announce a port: {log_path}")


def validate_png(data, expected_width, expected_height):
    require(data.startswith(b"\x89PNG\r\n\x1a\n"), "image is not PNG")
    require(len(data) >= 33 and data[12:16] == b"IHDR", "PNG lacks IHDR")
    width, height = struct.unpack(">II", data[16:24])
    require((width, height) == (expected_width, expected_height), "PNG dimensions", (width, height))
    require(data[-12:-8] == b"\x00\x00\x00\x00" and data[-8:-4] == b"IEND", "PNG is incomplete")
    return width, height


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path("/tmp/flux2-v214-rpc.png"))
    parser.add_argument("--width", type=int, default=512)
    parser.add_argument("--height", type=int, default=512)
    parser.add_argument("--tuldok", action="store_true",
                        help="after the gateway trial, run Tuldok's real 1280x720 browser acceptance (34 GiB scope)")
    args = parser.parse_args()
    require(64 <= args.width <= 2048 and args.width % 16 == 0 and
            64 <= args.height <= 2048 and args.height % 16 == 0,
            "dimensions must be multiples of 16 from 64 through 2048",
            (args.width, args.height))
    limit, swap = cgroup_value("memory.max"), cgroup_value("memory.swap.max")
    max_gib = 34 if args.tuldok else 32
    try:
        bounded = limit is not None and int(limit) <= max_gib * 1024**3 and swap == "0"
    except ValueError:
        bounded = False
    require(bounded, f"run with memory.max <= {max_gib} GiB and memory.swap.max = 0",
            {"limit": limit, "swap": swap})
    if args.tuldok:
        available = host_available_bytes()
        require(available is not None and available >= 6 * 1024**3,
                "host MemAvailable must be at least 6 GiB", available)
    require(BINARY.is_file(), "RPC binary missing", str(BINARY))
    require((ROOT / ".active-version-torch").read_text().strip() == "v2.14.0", "selected Torch version")
    require((ROOT / "shared-resources/models").is_dir(), "linked model library missing")
    profile_data = json.loads((ROOT / "launcher-data/metadata/runtime-profiles.json").read_text())
    profile = next((p for p in profile_data["profiles"] if p["profile_id"] == PROFILE_ID), None)
    require(profile and profile.get("provider") == "torch" and
            profile.get("provider_mode") == "torch_serve" and
            profile.get("management_mode") == "managed" and
            profile.get("enabled"), "selected managed Torch profile", profile)

    output = args.output.resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    log_path = output.with_suffix(".rpc.log")
    env = {**os.environ, "XDG_CONFIG_HOME": str(ROOT / "config"),
           "PUMAS_REGISTRY_DB_PATH": str(ROOT / "registry.db"),
           "APPDATA": str(ROOT / "config"),
           "HF_HUB_OFFLINE": "1", "TRANSFORMERS_OFFLINE": "1", "DIFFUSERS_OFFLINE": "1"}
    loaded = False
    base = None
    process = None
    guard = None
    failure = None
    events_before = memory_events()
    require(bool(events_before) and cgroup_value("memory.peak") is not None,
            "cgroup memory event and peak counters unavailable")
    emit("start", cgroup_limit_bytes=int(limit), cgroup_swap_limit_bytes=int(swap),
         cgroup_events_before=events_before, cgroup_path=process_cgroup(os.getpid()),
         host_mem_available_bytes=host_available_bytes(), width=args.width,
         height=args.height, seed=SEED)
    with log_path.open("wb") as log:
        process = subprocess.Popen(
            [str(BINARY), "--launcher-root", str(ROOT), "--port", "0"],
            cwd=ROOT, env=env, stdout=log, stderr=subprocess.STDOUT,
            start_new_session=True,
        )
        if args.tuldok:
            guard = HostMemoryGuard(process)
            guard.start()
        try:
            base = await_rpc_port(process, log_path)
            emit("rpc_ready", endpoint=base, pid=process.pid, log=str(log_path))
            before = rpc(base, "get_serving_status")
            require(not before.get("snapshot", {}).get("served_models"), "isolated root already serves models", before)
            served = rpc(base, "serve_model", {"request": {
                "model_id": MODEL_ID,
                "config": {"provider": "torch", "profile_id": PROFILE_ID,
                           "device_mode": "gpu", "keep_loaded": True, "model_alias": ALIAS},
            }}, timeout=600)
            require(served.get("success") and served.get("loaded"), "FLUX.2 load", served)
            loaded = True
            emit("model_loaded", model_id=MODEL_ID, alias=ALIAS,
                 owned_process_cgroups=owned_process_cgroups(process.pid),
                 sidecar_cgroups=owned_sidecars())
            models = request_json(base, "/v1/models", timeout=30)
            require(any(item.get("id") == ALIAS and "image_generation" in item.get("capabilities", [])
                        for item in models.get("data", [])), "gateway model readiness", models)
            emit("gateway_ready", endpoint=base + "/v1/images/generations")
            result = request_json(base, "/v1/images/generations", {
                "model": ALIAS, "prompt": PROMPT, "width": args.width, "height": args.height,
                "seed": SEED, "n": 1, "response_format": "b64_json",
            }, timeout=900)
            images = result.get("data", [])
            require(len(images) == 1 and isinstance(images[0].get("b64_json"), str),
                    "image gateway response", result)
            png = base64.b64decode(images[0]["b64_json"], validate=True)
            width, height = validate_png(png, args.width, args.height)
            output.write_bytes(png)
            emit("image_saved", path=str(output), width=width, height=height,
                 bytes=len(png), sha256=hashlib.sha256(png).hexdigest(),
                 metadata=result.get("metadata"))
            if args.tuldok:
                require(TULDOK_TEST.is_file(), "Tuldok browser acceptance missing", str(TULDOK_TEST))
                evidence = ROOT / "tuldok-flux2-v214-real"
                evidence.mkdir(parents=True, exist_ok=True)
                browser_log = evidence / "browser.log"
                browser_env = {**env, "PUMAS_GATEWAY": base + "/v1",
                               "PUMAS_MODEL": MODEL_ID,
                               "TULDOK_EVIDENCE_DIR": str(evidence)}
                emit("tuldok_start", evidence=str(evidence))
                with browser_log.open("wb") as log:
                    browser = subprocess.Popen(
                        ["node", str(TULDOK_TEST)], cwd=TULDOK_TEST.parent.parent,
                        env=browser_env, stdout=log, stderr=subprocess.STDOUT,
                        start_new_session=True,
                    )
                    browser_group = browser.pid  # start_new_session makes this the group ID.
                    browser_cgroup = process_cgroup(os.getpid())
                    guard.browser_process = browser
                    try:
                        if browser.poll() is None:
                            require(os.getpgid(browser.pid) == browser_group and
                                    process_cgroup(browser.pid) == browser_cgroup,
                                    "browser process group ownership", browser_group)
                        browser.wait(timeout=900)
                    except subprocess.TimeoutExpired:
                        raise RuntimeError(f"Tuldok timed out; see {browser_log}")
                    finally:
                        # The browser test may be interrupted before its own finally runs.
                        stop_owned_group(browser_group, browser_cgroup)
                        browser.wait(timeout=10)
                        guard.browser_process = None
                require(browser.returncode == 0, "Tuldok browser acceptance", str(browser_log))
                browser_result = json.loads((evidence / "result.json").read_text())
                require(browser_result.get("fixture") is False and
                        browser_result.get("model") == MODEL_ID and
                        browser_result.get("width") == 1280 and
                        browser_result.get("height") == 720 and
                        browser_result.get("automatically_added") is True,
                        "Tuldok real browser result", browser_result)
                emit("tuldok_complete", evidence=str(evidence),
                     saved_sha256=browser_result.get("saved_sha256"))
        except BaseException as error:
            failure = error
        finally:
            if base is not None:
                if loaded:
                    try:
                        unloaded = rpc(base, "unserve_model", {"request": {
                            "model_id": MODEL_ID, "provider": "torch",
                            "profile_id": PROFILE_ID, "model_alias": ALIAS,
                        }}, timeout=120)
                        require(unloaded.get("success") and unloaded.get("unloaded"), "FLUX.2 unload", unloaded)
                        emit("model_unloaded")
                    except BaseException as error:
                        failure = failure or error
                try:
                    stopped = rpc(base, "stop_runtime_profile", {"profile_id": PROFILE_ID}, timeout=60)
                    require(stopped.get("success"), "managed sidecar stop", stopped)
                    emit("sidecar_stopped")
                except BaseException as error:
                    failure = failure or error
            if process.poll() is None:
                process.send_signal(signal.SIGINT)
                try:
                    process.wait(timeout=30)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait(timeout=10)
                    failure = failure or RuntimeError("RPC required forced shutdown")
            if process.returncode != 0:
                failure = failure or RuntimeError(f"RPC exited {process.returncode}; see {log_path}")
            emit("rpc_stopped", returncode=process.returncode)
            try:
                stop_orphaned_sidecars()
            except BaseException as error:
                failure = failure or error
            if guard:
                guard.finish()
                emit("host_memory_guard", minimum_available_bytes=guard.minimum_available,
                     low_available_bytes=guard.low_available, tripped=guard.tripped)
                if guard.tripped:
                    failure = RuntimeError(f"host MemAvailable fell below 6 GiB or became unavailable: {guard.low_available}")
    events_after = memory_events()
    event_delta = {key: value - events_before.get(key, 0) for key, value in events_after.items()}
    peak = cgroup_value("memory.peak")
    emit("memory", cgroup_peak_bytes=int(peak) if peak and peak != "max" else None,
         cgroup_events_after=events_after, cgroup_event_delta=event_delta)
    if any(event_delta.get(key, 0) > 0 for key in ("oom", "oom_kill", "oom_group_kill")):
        failure = failure or RuntimeError(f"cgroup OOM event occurred: {event_delta}")
    if failure:
        raise failure
    emit("complete", output=str(output), log=str(log_path))


if __name__ == "__main__":
    try:
        main()
    except Exception as error:
        emit("failed", error_type=type(error).__name__, error=str(error))
        raise SystemExit(1) from error
