#!/usr/bin/env python3
"""Produce a native tag candidate using checked runtime pins and source contracts.

No release is published. Startup is bounded identity evidence, not model inference.
"""

import argparse
import copy
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tomllib

import headless_inference as package
import headless_inference_build as build
import onnx_runtime_stage as runtime_stage


def contract_for(repository, target_id, schema, build_id):
    source = build.source_identity(repository, subprocess.run)
    with (repository / "rust/Cargo.toml").open("rb") as stream:
        version = tomllib.load(stream)["workspace"]["package"]["version"]
    core = {
        "build_info_schema_version": 1,
        "component": "pumas-library",
        "package_version": version,
        "build_id": build_id,
        "source_revision": source["head"],
        "target": package.TARGETS[target_id]["rust_target"],
        "compiled_features": [
            f"pumas-library/{name}"
            for name in ("hf-client", "process-manager", "gpu-monitor", "onnx-runtime", "s3")
        ],
        "protocols": [{"name": "pumas.local-ipc", "versions": [1]}],
        "schemas": [
            {"name": name, "version": version}
            for name, version in package.discovery.CORE_SCHEMAS.items()
        ],
    }
    rpc = copy.deepcopy(core)
    rpc["component"] = "pumas-rpc"
    rpc["compiled_features"] += ["pumas-rpc/inference-plugins", "pumas-rpc/s3"]
    rpc["protocols"].append({"name": "pumas.local-http", "versions": [1]})
    rpc["schemas"].append({"name": "pumas.http-advertisement", "version": 1})
    contract = {
        "schema_version": 2,
        "expected": {
            "version": version,
            "source_commit": source["head"],
            "source_tree": source["tree"],
            "build_id": build_id,
            "schema_sha256": hashlib.sha256(schema).hexdigest(),
            "inference_enabled": True,
            "features": build.RPC_FEATURES,
            "build_info": rpc,
            "core_build_info": core,
        },
    }
    package.validate_contract(contract)
    return contract


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--target", choices=package.TARGETS, required=True)
    parser.add_argument("--runtime-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--build-id", required=True)
    args = parser.parse_args()
    repository = Path(__file__).resolve().parents[2]
    # Reject the unbumped source before any compilation.
    contract_for(repository, args.target, b"{}", args.build_id)
    environment = dict(os.environ, ORT_SKIP_DOWNLOAD="1", CARGO_BUILD_JOBS="1")
    build.check_environment(environment, package.TARGETS[args.target]["rust_target"])
    schema = subprocess.run(
        [
            "cargo",
            "run",
            "--locked",
            "--offline",
            "--manifest-path",
            "rust/Cargo.toml",
            "-p",
            "pumas-rpc",
            "--no-default-features",
            "--features",
            "export-contract",
            "--",
            "--export-desktop-contract",
        ],
        cwd=repository,
        env=environment,
        check=True,
        capture_output=True,
    ).stdout
    contract = contract_for(repository, args.target, schema, args.build_id)
    runtime_dir = args.runtime_dir.resolve()
    runtime = json.loads((runtime_dir / "runtime-record.json").read_text())
    pins = json.loads(runtime_stage.PINS.read_text())["targets"][args.target]
    package.require(
        runtime["source_archive_sha256"] == pins["archive_sha256"],
        "runtime archive differs from checked authority",
    )
    package.require(
        runtime["files"]
        == {
            name: {key: item[key] for key in ("sha256", "bytes")}
            for name, item in pins["files"].items()
        },
        "runtime files differ from checked authority",
    )
    notices = []
    for name, item in pins["notices"].items():
        path = runtime_dir / name
        package.require(
            not path.is_symlink()
            and path.stat().st_size == item["bytes"]
            and package.sha256(path) == item["sha256"],
            "native notice differs from checked authority",
        )
        notices.append(path.read_bytes())
    evidence = build.produce(
        repository,
        args.target,
        contract,
        runtime,
        {name: runtime_dir / name for name in runtime["files"]},
        schema,
        repository / "LICENSE",
        repository / build.attribution_directory(repository) / "THIRD-PARTY-NOTICES.txt",
        args.output_dir,
        runtime_notices=notices,
    )
    (args.output_dir / "compatibility.json").write_bytes(package.canonical(contract))
    extracted = args.output_dir / "extracted"
    manifest = package.extract_verified(
        Path(evidence["archive"]), evidence["archive_sha256"], extracted
    )
    result = package.smoke(extracted, manifest, contract)
    (args.output_dir / "startup-evidence.json").write_bytes(package.canonical(result))


if __name__ == "__main__":
    main()
