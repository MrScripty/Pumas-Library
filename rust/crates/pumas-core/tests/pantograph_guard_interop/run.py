#!/usr/bin/env python3
"""Run the real pinned Pantograph guard against local Pumas producer output.

Requires an existing public Pantograph checkout and cached Rust source dependencies.
The default run is offline, locked, and has every model runtime backend disabled.
"""
import argparse
import json
import os
from pathlib import Path
import shutil
import subprocess
import tempfile

PANTOGRAPH_COMMIT = "038dacaaa98ebd007c32e13d4608726ca5ccf63a"
GUARD_BLOB = "af4822d04f732a1db466f4f9df7143b67e88942b"
GUARD_PATH = "crates/inference/src/selected_audio_execution.rs"
HOST_ADAPTER_PATH = "crates/pantograph-embedded-runtime/src/runtime_host_package_facts.rs"


def git(checkout, *args):
    return subprocess.check_output(
        ["git", "-c", "credential.helper=", "-c", "core.askPass=", "-C", str(checkout), *args],
        text=True,
    ).strip()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--pantograph", type=Path, required=True, help="Existing pinned public checkout")
    parser.add_argument("--work-dir", type=Path, help="Retain a generated harness for inspecting build evidence")
    parser.add_argument("--resolve-source-dependencies", action="store_true", help="Allow Cargo to resolve/fetch public Rust source dependencies; never model runtimes")
    args = parser.parse_args()
    pantograph = args.pantograph.resolve(strict=True)
    if git(pantograph, "rev-parse", "HEAD") != PANTOGRAPH_COMMIT:
        parser.error("Pantograph checkout must be exactly " + PANTOGRAPH_COMMIT)
    if git(pantograph, "hash-object", GUARD_PATH) != GUARD_BLOB:
        parser.error("The pinned Pantograph guard source was modified")
    if git(pantograph, "status", "--porcelain", "--untracked-files=no"):
        parser.error("Pantograph tracked source/dependencies must be unchanged")
    source = Path(__file__).resolve().parent
    core = source.parent.parent
    temp = None
    if args.work_dir:
        work = args.work_dir.resolve()
        work.mkdir(parents=True, exist_ok=True)
    else:
        temp = tempfile.TemporaryDirectory(prefix="pumas-pantograph-guard-")
        work = Path(temp.name)
    for name in ["lib.rs", "interop.rs"]:
        shutil.copyfile(source / name, work / name)
    manifest = (source / "Cargo.toml.in").read_text()
    manifest = manifest.replace("@PANTOGRAPH_INFERENCE@", json.dumps(str(pantograph / "crates/inference")))
    manifest = manifest.replace("@PANTOGRAPH_DEPENDENCY_PLANNING@", json.dumps(str(pantograph / "crates/pantograph-dependency-planning")))
    manifest = manifest.replace("@PUMAS_CORE@", json.dumps(str(core)))
    (work / "Cargo.toml").write_text(manifest)
    guard = json.dumps(str(pantograph / GUARD_PATH))
    (work / "upstream_guard.rs").write_text(f"#[path = {guard}]\nmod selected_audio_execution;\n")
    # Compile the exact host privacy-normalization functions as well. This slice
    # has no runtime-host dependencies and is copied verbatim from pinned source.
    host = (pantograph / HOST_ADAPTER_PATH).read_text()
    start = host.index("fn normalize_runtime_host_package_fact_identity(")
    end = host.index("fn decode_pumas_package_facts(", start)
    (work / "upstream_host_adapter.rs").write_text(host[start:end])
    lock = source / "Cargo.lock"
    if lock.exists():
        shutil.copyfile(lock, work / "Cargo.lock")
    elif args.resolve_source_dependencies:
        # Keep the upstream's already pinned dependency revisions while adding
        # the local producer and this test package to the lock.
        shutil.copyfile(pantograph / "Cargo.lock", work / "Cargo.lock")
    else:
        parser.error("Harness Cargo.lock is missing; prepare public source dependencies first")
    env = os.environ.copy()
    env["ORT_SKIP_DOWNLOAD"] = "1"
    env.setdefault("CARGO_TARGET_DIR", str(work / "target"))
    env["GIT_TERMINAL_PROMPT"] = "0"
    env["GIT_CONFIG_COUNT"] = "2"
    env["GIT_CONFIG_KEY_0"] = "credential.helper"
    env["GIT_CONFIG_VALUE_0"] = ""
    env["GIT_CONFIG_KEY_1"] = "core.askPass"
    env["GIT_CONFIG_VALUE_1"] = ""
    env["CARGO_NET_GIT_FETCH_WITH_CLI"] = "true"
    command = ["cargo", "test", "--manifest-path", str(work / "Cargo.toml"), "--lib"]
    if not args.resolve_source_dependencies:
        command += ["--offline", "--locked"]
    print(f"Consumer commit: {PANTOGRAPH_COMMIT}; guard blob: {GUARD_BLOB}", flush=True)
    print(f"Harness: {work}; runtime backends: disabled; ORT_SKIP_DOWNLOAD=1", flush=True)
    result = subprocess.run(command, env=env)
    if temp:
        temp.cleanup()
    raise SystemExit(result.returncode)


if __name__ == "__main__":
    main()
