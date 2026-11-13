#!/usr/bin/env python3
"""Stage the full desktop RPC backend for packaging.

ONNX Runtime is not packaged: the desktop downloads it at runtime through the
version manager. Only the RPC executable is staged, so a native library left
beside the build output can never enter a package.
"""

import argparse
import os
from pathlib import Path
import shutil

from headless_inference import require

ROOT = Path(__file__).resolve().parents[2]


def stage_rpc(source, target):
    binary = "pumas-rpc.exe" if os.name == "nt" else "pumas-rpc"
    require((source / binary).is_file(), "release backend is missing")
    # Validate the input before replacing this generated staging directory.
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    shutil.copy2(source / binary, target / binary)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    stage_rpc(ROOT / "rust/target/release", ROOT / "electron/resources/bin")


if __name__ == "__main__":
    main()
