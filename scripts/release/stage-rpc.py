#!/usr/bin/env python3
"""Stage a full desktop RPC with explicitly provisioned official CPU ORT bytes."""

import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil

from headless_inference import require
from onnx_runtime_stage import PINS
from packaged_onnx_support import native_target

ROOT = Path(__file__).resolve().parents[2]


def stage_rpc(source, target, runtime, target_id):
    require(target_id == native_target(), "desktop runtime requires matching native host")
    selected = json.loads(PINS.read_text())["targets"][target_id]
    binary = "pumas-rpc.exe" if os.name == "nt" else "pumas-rpc"
    require((source / binary).is_file(), "release backend is missing")
    payloads = {}
    for name, pin in {**selected["files"], **selected["notices"]}.items():
        path = runtime / name
        require(path.is_file() and not path.is_symlink(), "official runtime member is missing")
        require(path.stat().st_size == pin["bytes"], "official runtime member size differs")
        data = path.read_bytes()
        require(hashlib.sha256(data).hexdigest() == pin["sha256"], "official runtime pin differs")
        payloads[name] = data
    # Validate all inputs before replacing this generated staging directory.
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    shutil.copy2(source / binary, target / binary)
    for name, data in payloads.items():
        (target / name).write_bytes(data)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime-dir", type=Path, required=True)
    parser.add_argument("--target", required=True)
    args = parser.parse_args()
    stage_rpc(
        ROOT / "rust/target/release",
        ROOT / "electron/resources/bin",
        args.runtime_dir,
        args.target,
    )


if __name__ == "__main__":
    main()
