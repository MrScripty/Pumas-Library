#!/usr/bin/env python3
"""Stage the freshly built RPC and adjacent native runtime dependencies."""

import os
from pathlib import Path
import shutil

root = Path(__file__).resolve().parents[2]
source = root / "rust/target/release"
target = root / "electron/resources/bin"
binary = "pumas-rpc.exe" if os.name == "nt" else "pumas-rpc"
if not (source / binary).is_file():
    raise SystemExit(f"Missing release backend: {source / binary}")
# This is a generated staging directory, never a runtime/library root.
if target.exists():
    shutil.rmtree(target)
target.mkdir(parents=True)
shutil.copy2(source / binary, target / binary)
# ONNX Runtime is the backend's redistributable native runtime. A developer's
# target directory can also contain unrelated NIF/UniFFI outputs; never ship
# those merely because they share a dynamic-library suffix.
for pattern in ("onnxruntime*.dll", "libonnxruntime.so*", "libonnxruntime*.dylib"):
    for library in source.glob(pattern):
        shutil.copy2(library, target / library.name)
print(f"Staged backend and native libraries in {target}")
