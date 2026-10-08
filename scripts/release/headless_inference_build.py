#!/usr/bin/env python3
"""Opt-in local inference candidate production; no download or publication.

Caller-reviewed compatibility/runtime records remain mandatory. The current
0.7 source is refused before Cargo; this command never grants inference acceptance.
"""

import argparse
import copy
import hashlib
import json
import os
import platform
from pathlib import Path
import subprocess
import tomllib

import headless_inference as package
import s3_build_provenance as provenance

TARGETS = package.TARGETS
CORE_FEATURES = ["gpu-monitor", "hf-client", "onnx-runtime", "process-manager", "s3"]
RPC_FEATURES = ["inference-plugins", "s3"]
require = package.require


def check_environment(environment, target):
    # Reuse the release-profile policy; its existing default target is Linux.
    normalized = dict(environment)
    require(
        not any(name.startswith("GIT_") and value for name, value in normalized.items()),
        "Git routing/config overrides are not qualified",
    )
    require(
        not any(
            normalized.get(name)
            for name in (
                "RUSTC",
                "RUSTC_WRAPPER",
                "RUSTC_WORKSPACE_WRAPPER",
                "CARGO_BUILD_RUSTC",
                "CARGO_BUILD_RUSTC_WRAPPER",
                "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER",
            )
        ),
        "compiler/wrapper overrides are not qualified",
    )
    require(normalized.get("CARGO_BUILD_TARGET", target) == target, "native Cargo target required")
    normalized.pop("CARGO_BUILD_TARGET", None)
    provenance.check_environment(normalized)


def artifact_evidence(stdout, repository, target):
    observed = {}
    finished = False
    for line in stdout.splitlines():
        event = json.loads(line)
        if event.get("reason") == "build-finished":
            require(event.get("success") is True, "Cargo build did not succeed")
            finished = True
        if event.get("reason") != "compiler-artifact":
            continue
        for owner, directory, name, kind, features in (
            ("rpc", "pumas-rpc", "pumas-rpc", "bin", RPC_FEATURES),
            ("core", "pumas-core", "pumas_library", "lib", CORE_FEATURES),
        ):
            reported = event.get("target", {})
            if (
                Path(event.get("manifest_path", "")).resolve()
                != (repository / f"rust/crates/{directory}/Cargo.toml").resolve()
                or reported.get("name") != name
                or kind not in reported.get("kind", [])
            ):
                continue
            require(owner not in observed, "ambiguous Cargo artifact evidence")
            require(sorted(event.get("features", [])) == features, f"unexpected {owner} features")
            provenance.check_artifact_profile(event.get("profile", {}))
            observed[owner] = {
                "features": features,
                "profile": event["profile"],
                "package_id": event["package_id"],
            }
            if owner == "rpc":
                binary = Path(event.get("executable") or "")
                require(
                    binary.is_file() and not binary.is_symlink(), "production executable missing"
                )
                require(binary.name == target["binary"], "wrong native executable basename")
                observed[owner]["executable"] = str(binary.resolve())
    require(
        finished and set(observed) == {"rpc", "core"}, "missing completed Cargo artifact evidence"
    )
    return observed


def validate_runtime_inputs(runtime_inputs, runtime):
    require(
        type(runtime_inputs) is dict and set(runtime_inputs) == set(runtime["files"]),
        "exact explicit runtime closure required",
    )
    for name, path in runtime_inputs.items():
        path = Path(path)
        item = runtime["files"][name]
        require(path.is_file() and not path.is_symlink(), "runtime must be regular local files")
        require(
            path.stat().st_size == item["bytes"] and package.sha256(path) == item["sha256"],
            "runtime bytes mismatch",
        )


def core_projection(rpc):
    """Undo only discovery.rs's RPC additions to the shared library identity."""
    core = copy.deepcopy(rpc)
    core["component"] = "pumas-library"
    core["compiled_features"] = [
        name for name in core["compiled_features"] if not name.startswith("pumas-rpc/")
    ]
    core["protocols"] = [item for item in core["protocols"] if item["name"] != "pumas.local-http"]
    core["schemas"] = [
        item for item in core["schemas"] if item["name"] != "pumas.http-advertisement"
    ]
    return core


def produce(
    repository,
    target_id,
    contract,
    runtime,
    runtime_inputs,
    schema,
    license_file,
    notices_file,
    output,
    runner=subprocess.run,
    environment=None,
):
    """Build one native source-bound candidate; supplied records must be pinned first."""
    repository, output = Path(repository).resolve(), Path(output).resolve()
    # Detach caller metadata. No changing records can modify the admitted cohort.
    contract, runtime, runtime_inputs = copy.deepcopy((contract, runtime, runtime_inputs))
    expected = package.validate_contract(contract)  # Deliberately first: 0.7 never builds.
    target = TARGETS[target_id]
    package.discovery.validate_pins(expected, target["rust_target"])
    package.validate_runtime_record(runtime, target)
    validate_runtime_inputs(runtime_inputs, runtime)
    require(not output.is_relative_to(repository), "candidate output must be outside checkout")
    require(not output.exists(), "refusing to overwrite candidate evidence")
    require(
        platform.system().lower() == target["host"]
        and platform.machine().lower()
        in ({"arm64", "aarch64"} if target["machine"] == "arm64" else {"x86_64", "amd64"}),
        "native target unavailable on this host",
    )
    package.decode(schema)
    require(
        hashlib.sha256(schema).hexdigest() == expected["schema_sha256"], "schema/cohort mismatch"
    )
    with (repository / "rust/Cargo.toml").open("rb") as stream:
        version = tomllib.load(stream)["workspace"]["package"]["version"]
    require(version == expected["version"], "source package version mismatch")
    environment = dict(os.environ if environment is None else environment)
    check_environment(environment, target["rust_target"])
    environment.update(ORT_SKIP_DOWNLOAD="1", CARGO_BUILD_JOBS="1", CARGO_INCREMENTAL="0")

    def tool_runner(command, **options):
        # Git, compiler and attribution observations must use the same explicit
        # environment as Cargo, never a separately inherited ambient Git route.
        options["env"] = environment
        return runner(command, **options)

    provenance.check_configuration(repository, environment)
    release_settings = provenance.check_release_manifest(repository)
    before = provenance.source_identity(repository, tool_runner)
    require(
        before == {"head": expected["source_commit"], "tree": expected["source_tree"]},
        "source/cohort mismatch",
    )
    rustc = provenance.capture(["rustc", "-Vv"], repository, tool_runner)
    require(f"host: {target['rust_target']}" in rustc.splitlines(), "native Rust host required")
    rustc_version = rustc.splitlines()[0]
    require(
        package.matches(rustc_version, package.RUSTC_VERSION, 160), "bounded rustc version required"
    )
    # Same dependency closure as the existing default+S3 inventory, checked below
    # against actual core/RPC artifacts without the default feature marker.
    attribution = provenance.attribution_binding(repository)
    provenance.capture(
        ["node", "scripts/release/check-attribution.cjs", "--features", "s3"],
        repository,
        tool_runner,
    )
    for path in (license_file, notices_file):
        path = Path(path)
        require(
            path.is_file() and not path.is_symlink() and path.stat().st_size > 0,
            "explicit nonempty regular license/notices required",
        )
    require(
        package.sha256(notices_file)
        == package.sha256(repository / provenance.ATTRIBUTION / "THIRD-PARTY-NOTICES.txt"),
        "supplied notices differ from checked S3 attribution",
    )
    require(
        package.sha256(license_file) == package.sha256(repository / "LICENSE"),
        "supplied license differs from source",
    )
    output.mkdir(parents=True)
    command = package.production_build_commands(target["rust_target"])[-2]
    require(
        command[command.index("--features") + 1] == "s3,inference-plugins",
        "internal command mismatch",
    )
    environment.update(
        ORT_SKIP_DOWNLOAD="1",
        CARGO_BUILD_JOBS="1",
        CARGO_INCREMENTAL="0",
        PUMAS_SOURCE_REVISION=before["head"],
        PUMAS_BUILD_ID=expected["build_id"],
        PUMAS_BUILD_TARGET=target["rust_target"],
    )
    log, errors = output / "cargo.jsonl", output / "cargo.stderr.log"
    with log.open("x") as stdout, errors.open("x") as stderr:
        completed = runner(
            command,
            cwd=repository,
            env=environment,
            check=False,
            stdout=stdout,
            stderr=stderr,
            text=True,
        )
    require(completed.returncode == 0, "Cargo build failed; diagnostics preserved")
    artifacts = artifact_evidence(log.read_text(), repository, target)
    binary = Path(artifacts["rpc"]["executable"])
    identity_environment = {
        name: value
        for name, value in environment.items()
        if not name.startswith(("ORT_", "PUMAS_", "PYTHON", "LD_", "DYLD_"))
    }
    compiled = package.decode(
        runner(
            [str(binary), "--build-info"],
            cwd=repository,
            env=identity_environment,
            check=True,
            capture_output=True,
            text=True,
            timeout=15,
        ).stdout.encode()
    )
    require(
        package.canonical(compiled) == package.canonical(expected["build_info"]),
        "compiled PumasBuildInfo mismatch",
    )
    core = core_projection(compiled)
    require(
        package.canonical(core) == package.canonical(expected["core_build_info"]),
        "compiled core PumasBuildInfo mismatch",
    )
    require(
        provenance.source_identity(repository, tool_runner) == before, "source changed during build"
    )
    provenance.check_configuration(repository, environment)
    require(
        provenance.attribution_binding(repository) == attribution,
        "attribution changed during build",
    )
    require(
        package.sha256(notices_file)
        == package.sha256(repository / provenance.ATTRIBUTION / "THIRD-PARTY-NOTICES.txt"),
        "supplied notices changed during build",
    )
    require(
        package.sha256(license_file) == package.sha256(repository / "LICENSE"),
        "supplied license changed during build",
    )
    validate_runtime_inputs(runtime_inputs, runtime)
    build = {
        "version": compiled["package_version"],
        "target": compiled["target"],
        "host": target["rust_target"],
        "profile": "release",
        "features": RPC_FEATURES,
        "source": before,
        "build_id": compiled["build_id"],
        "inference_enabled": True,
        "binary_sha256": package.sha256(binary),
        "command": command,
        "rustc": rustc_version,
    }
    package.validate_build_record(build, target, expected)
    inputs = {
        target["binary"]: binary,
        "LICENSE.txt": license_file,
        "THIRD-PARTY-NOTICES.txt": notices_file,
        **runtime_inputs,
    }
    archive = output / (f"pumas-rpc-inference-{version}-{target_id}.{target['format']}")
    _, archive_sha = package.assemble(inputs, build, runtime, contract, schema, archive)
    (output / "build-record.json").write_bytes(package.canonical(build))
    evidence = {
        "schema_version": 1,
        "qualification": "unverified_candidate",
        "source": before,
        "compiled_build_info": compiled,
        "compiled_core_projection": core,
        "artifacts": artifacts,
        "rustc": rustc,
        "release_settings": release_settings,
        "attribution": attribution,
        "cargo_jsonl_sha256": package.sha256(log),
        "cargo_stderr_sha256": package.sha256(errors),
        "archive": str(archive),
        "archive_sha256": archive_sha,
        "runtime_loading": "unqualified",
        "real_inference": "unqualified",
    }
    (output / "build-evidence.json").write_bytes(package.canonical(evidence))
    return evidence


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", type=Path, default=package.ROOT)
    parser.add_argument("--target", choices=TARGETS, required=True)
    for name in ("contract", "runtime-record", "runtime-inputs"):
        parser.add_argument(f"--{name}", type=Path, required=True)
        parser.add_argument(f"--{name}-sha256", required=True)
    for name in ("schema", "license", "notices", "output-dir"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    args = parser.parse_args()
    evidence = produce(
        args.repository,
        args.target,
        package.read_pinned_json(args.contract, args.contract_sha256),
        package.read_pinned_json(args.runtime_record, args.runtime_record_sha256),
        package.read_pinned_json(args.runtime_inputs, args.runtime_inputs_sha256),
        package.metadata_bytes(args.schema),
        args.license,
        args.notices,
        args.output_dir,
    )
    print(
        json.dumps(
            {key: evidence[key] for key in ("source", "archive", "archive_sha256", "qualification")}
        )
    )


if __name__ == "__main__":
    main()
