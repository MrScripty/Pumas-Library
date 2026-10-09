#!/usr/bin/env python3
"""Opt-in, unsigned Linux x86_64 no-inference source snapshot.

This is separate from the strict v0.8 inference/runtime-closure producer. It
preserves the source version and binds the actual shared PumasBuildInfo; it does
not qualify startup, native loading, inference, or any live model capability.
"""

import argparse
import copy
import hashlib
import json
import os
import platform
from pathlib import Path
import re
import shutil
import subprocess
import tarfile
import tempfile
import tomllib

import headless_inference as package
import headless_inference_build as inference_build
import s3_build_provenance as provenance

ROOT = Path(__file__).resolve().parents[2]
TARGET = "x86_64-unknown-linux-gnu"
FEATURES = {"rpc": [], "core": ["hf-client"]}
ATTRIBUTION = "docs/release-attribution/0.7.0"
COMMAND = [
    "cargo",
    "build",
    "--locked",
    "--offline",
    "--manifest-path",
    "rust/Cargo.toml",
    "-p",
    "pumas-rpc",
    "--bin",
    "pumas-rpc",
    "--no-default-features",
    "--target",
    TARGET,
    "--release",
    "--message-format=json-render-diagnostics",
]
PAYLOAD = {
    "pumas-rpc",
    "LICENSE.txt",
    "THIRD-PARTY-NOTICES.txt",
    "rpc-build-info.json",
    "core-build-info.json",
    "desktop-contract.json",
    "schema-export-record.json",
    "manifest.json",
    "SHA256SUMS",
}
LEGAL_TEXT = {"LICENSE.txt", "THIRD-PARTY-NOTICES.txt"}
MAX_LEGAL_TEXT = 8 * 1024 * 1024
require = package.require


def member_limit(name):
    require(name in PAYLOAD, "unknown snapshot archive member")
    if name == "pumas-rpc":
        return package.MAX_PACKAGE
    if name in LEGAL_TEXT:
        return MAX_LEGAL_TEXT
    return package.MAX_MANIFEST


def bounded_member(name, size):
    require(0 < size <= member_limit(name), f"bounded snapshot member required: {name}")


def regular(path, maximum=package.MAX_PACKAGE):
    path = Path(path)
    require(
        path.is_file() and not path.is_symlink() and 0 < path.stat().st_size <= maximum,
        "bounded nonempty regular file required",
    )
    return path


def plain_path(path):
    """Reject symlink components before resolve can hide their presence."""
    path = Path(path).absolute()
    require(not any(part.is_symlink() for part in (path, *path.parents)), "symlink path refused")
    return path.resolve()


def native_environment(environment):
    inference_build.check_environment(environment, TARGET)
    require(
        environment.get("CMAKE_BUILD_PARALLEL_LEVEL", "1") == "1", "one native CMake job required"
    )
    # Native compiler/toolchain rerouting is independent of TurboJPEG's own
    # namespace. Use tools resolved through the recorded PATH, not overrides.
    routes = (
        "CC",
        "CXX",
        "AR",
        "CFLAGS",
        "CXXFLAGS",
        "CPPFLAGS",
        "LDFLAGS",
        "CMAKE",
        "CMAKE_TOOLCHAIN_FILE",
        "CMAKE_GENERATOR",
        "CMAKE_PREFIX_PATH",
    )
    require(
        not any(
            name == route
            or name.startswith(route + "_")
            or name in {"HOST_" + route, "TARGET_" + route}
            for name in environment
            if name != "CMAKE_BUILD_PARALLEL_LEVEL"
            for route in routes
        ),
        "native compiler/toolchain overrides are not qualified",
    )


def artifact_evidence(stdout, repository):
    """Adapt the existing release-profile check to the plain no-default slice."""
    observed, finished = {}, False
    for line in stdout.splitlines():
        event = package.decode(line.encode())
        if event.get("reason") == "build-finished":
            require(event.get("success") is True, "Cargo build did not succeed")
            finished = True
        if event.get("reason") != "compiler-artifact":
            continue
        for owner, directory, name, kind in (
            ("rpc", "pumas-rpc", "pumas-rpc", "bin"),
            ("core", "pumas-core", "pumas_library", "lib"),
        ):
            target = event.get("target", {})
            if (
                Path(event.get("manifest_path", "")).resolve()
                != (repository / f"rust/crates/{directory}/Cargo.toml").resolve()
                or target.get("name") != name
                or kind not in target.get("kind", [])
            ):
                continue
            require(owner not in observed, "ambiguous Cargo artifact evidence")
            require(
                sorted(event.get("features", [])) == FEATURES[owner],
                f"unexpected {owner} features; plain no-default required",
            )
            provenance.check_artifact_profile(event.get("profile", {}))
            observed[owner] = {
                "features": FEATURES[owner],
                "profile": event["profile"],
                "package_id": event["package_id"],
            }
            if owner == "rpc":
                require(
                    Path(event.get("executable") or "").name == "pumas-rpc",
                    "Cargo did not report the production executable",
                )
                observed[owner]["executable"] = event["executable"]
    require(
        finished and set(observed) == {"rpc", "core"}, "missing completed Cargo artifact evidence"
    )
    return observed


def check_identity(info, source, version, build_id, exporter=False):
    """Check a shared Rust identity; do not introduce a second identity object."""
    discovery = package.discovery
    discovery.fields(info, discovery.BUILD_FIELDS, "PumasBuildInfo")
    require(
        type(info["build_info_schema_version"]) is int
        and info["build_info_schema_version"] == 1
        and info["component"] == "pumas-rpc",
        "unsupported shared build identity",
    )
    require(
        info["package_version"] == version
        and info["source_revision"] == source["head"]
        and info["target"] == TARGET
        and discovery.token(info["build_id"]),
        "compiled source/version/target mismatch",
    )
    if build_id is not None:
        require(info["build_id"] == build_id, "compiled build ID mismatch")
    features = ["pumas-library/hf-client"]
    if exporter:
        # An existing broader exporter can render the same DTO declarations.
        # Its exact shared identity remains recorded, never projected into the
        # no-default package's compiled features or supported routes.
        features += ["pumas-library/contract-schema", "pumas-rpc/export-contract"]
        require(
            discovery.labels(info["compiled_features"])
            and set(features) <= set(info["compiled_features"]),
            "schema exporter compile features missing",
        )
    else:
        require(
            sorted(info["compiled_features"]) == sorted(features),
            "unexpected shared compile features",
        )
    protocols = discovery.advertisements(info["protocols"], True)
    require(
        protocols == {"pumas.local-ipc": [1], "pumas.local-http": [1]},
        "unexpected shared protocols",
    )
    schemas = discovery.advertisements(info["schemas"], False)
    expected_schemas = {**discovery.CORE_SCHEMAS, "pumas.http-advertisement": 1}
    if exporter and "pumas-rpc/inference-plugins" in info["compiled_features"]:
        expected_schemas["pumas.model-operations.image-to-text"] = 1
    require(schemas == expected_schemas, "unexpected shared schemas for inference-disabled package")
    return inference_build.core_projection(info)


def validate_schema(schema, record, source, version):
    require(
        type(record) is dict
        and set(record)
        == {
            "schema_version",
            "source",
            "export_command",
            "exporter_build_info",
            "schema_sha256",
            "schema_bytes",
            "exporter_binary_sha256",
        },
        "unsupported schema export record",
    )
    require(
        type(record["schema_version"]) is int
        and record["schema_version"] == 1
        and record["source"] == source,
        "schema source mismatch",
    )
    require(
        record["schema_sha256"] == hashlib.sha256(schema).hexdigest()
        and type(record["schema_bytes"]) is int
        and record["schema_bytes"] == len(schema),
        "schema bytes mismatch",
    )
    require(
        re.fullmatch(r"[a-f0-9]{64}", record["exporter_binary_sha256"] or ""),
        "schema exporter binary hash required",
    )
    command = record["export_command"]
    require(
        type(command) is list
        and len(command) == 2
        and Path(command[0]).name == "pumas-rpc"
        and command[1] == "--export-desktop-contract",
        "wrong schema exporter command",
    )
    check_identity(record["exporter_build_info"], source, version, None, exporter=True)
    decoded = package.decode(schema)
    require(
        type(decoded) is dict
        and set(decoded) == {"format", "dialect", "schemas"}
        and decoded["format"] == "pumas-desktop-contract-1"
        and decoded["dialect"] == "http://json-schema.org/draft-07/schema#"
        and type(decoded["schemas"]) is dict
        and decoded["schemas"],
        "actual desktop DTO schema export required",
    )


def attribution_binding(repository, runner):
    provenance.capture(["node", "scripts/release/check-attribution.cjs"], repository, runner)
    directory = repository / ATTRIBUTION
    inventory = package.decode(package.metadata_bytes(regular(directory / "inventory.json")))
    require(
        inventory.get("rust_profile", {}).get("default_features") is True
        and inventory["rust_profile"].get("features") == [],
        "default attribution profile mismatch",
    )
    return {
        "directory": ATTRIBUTION,
        "coverage": "checked conservative default-feature attribution superset; not exact no-default closure",
        "sha256": {
            name: package.sha256(
                regular(
                    directory / name,
                    MAX_LEGAL_TEXT if name == "THIRD-PARTY-NOTICES.txt" else package.MAX_MANIFEST,
                )
            )
            for name in ("README.md", "inventory.json", "THIRD-PARTY-NOTICES.txt")
        },
    }


def assemble(binary, repository, info, core, schema, schema_record, manifest, output):
    require(
        manifest.get("inference_compile_enabled") is False
        and manifest.get("default_features") is False
        and manifest.get("features") == []
        and manifest.get("target") == TARGET
        and manifest.get("profile") == "release",
        "inference-disabled snapshot manifest required",
    )
    projected = check_identity(
        info, manifest["package_source"], manifest["package_version"], manifest["build_id"]
    )
    require(
        package.canonical(core) == package.canonical(projected), "shared core projection mismatch"
    )
    for owner, features in FEATURES.items():
        artifact = manifest.get("artifacts", {}).get(owner, {})
        require(artifact.get("features") == features, "unexpected snapshot artifact features")
        provenance.check_artifact_profile(artifact.get("profile", {}))
    validate_schema(schema, schema_record, manifest["package_source"], manifest["package_version"])
    archive = output / (
        f"pumas-rpc-headless-snapshot-{info['package_version']}-linux-x86_64-"
        f"{info['source_revision'][:8]}.tar.gz"
    )
    with tempfile.TemporaryDirectory(prefix="pumas-headless-snapshot-") as temporary:
        stage = Path(temporary)
        for name, path in {
            "pumas-rpc": binary,
            "LICENSE.txt": repository / "LICENSE",
            "THIRD-PARTY-NOTICES.txt": repository / ATTRIBUTION / "THIRD-PARTY-NOTICES.txt",
        }.items():
            shutil.copyfile(regular(path, member_limit(name)), stage / name)

        def write_member(name, data):
            bounded_member(name, len(data))
            (stage / name).write_bytes(data)

        for name, value in {
            "rpc-build-info.json": info,
            "core-build-info.json": core,
            "schema-export-record.json": schema_record,
        }.items():
            write_member(name, package.canonical(value))
        write_member("desktop-contract.json", schema)
        (stage / "pumas-rpc").chmod(0o755)
        manifest = copy.deepcopy(manifest)
        manifest["files"] = {
            path.name: {"sha256": package.sha256(path), "bytes": path.stat().st_size}
            for path in stage.iterdir()
        }
        require(
            manifest["files"]["pumas-rpc"]["sha256"] == manifest["binary_sha256"],
            "staged executable differs from admitted bytes",
        )
        require(
            manifest["files"]["THIRD-PARTY-NOTICES.txt"]["sha256"]
            == manifest["attribution"]["sha256"]["THIRD-PARTY-NOTICES.txt"],
            "staged notices differ from checked attribution",
        )
        write_member("manifest.json", package.canonical(manifest))
        write_member(
            "SHA256SUMS",
            "".join(
                f"{package.sha256(path)}  {path.name}\n" for path in sorted(stage.iterdir())
            ).encode(),
        )
        require({path.name for path in stage.iterdir()} == PAYLOAD, "archive payload mismatch")
        for path in stage.iterdir():
            bounded_member(path.name, path.stat().st_size)
        require(
            sum(path.stat().st_size for path in stage.iterdir()) <= package.MAX_PACKAGE,
            "package exceeds bounded size",
        )
        partial = archive.with_suffix(archive.suffix + ".partial")
        with tarfile.open(partial, "x:gz", format=tarfile.USTAR_FORMAT) as bundle:
            for path in sorted(stage.iterdir()):
                bundle.add(path, arcname=path.name, recursive=False)
        try:
            regular(partial, package.MAX_PACKAGE)
            os.link(partial, archive)
        finally:
            partial.unlink()
    digest = package.sha256(archive)
    with archive.with_name(archive.name + ".sha256").open("x") as stream:
        stream.write(f"{digest}  {archive.name}\n")
    return archive, digest, manifest


def extract_verified(archive_path, expected_sha256, destination):
    """Pinned local snapshot extraction; this is byte custody, not authentication."""
    archive_path, destination = plain_path(archive_path), plain_path(destination)
    regular(archive_path)
    require(re.fullmatch(r"[a-f0-9]{64}", expected_sha256 or ""), "archive hash required")
    require(not destination.exists(), "fresh extraction destination required")
    with tempfile.TemporaryDirectory(prefix="pumas-snapshot-verified-") as temporary:
        stage = Path(temporary)
        snapshot = stage / "archive.tar.gz"
        # Copy/hash once into owned storage. Subsequent reads cannot switch the
        # supplied archive after admission, and the binary stays off Python's heap.
        with archive_path.open("rb") as source, snapshot.open("xb") as target:
            digest, copied = hashlib.sha256(), 0
            while chunk := source.read(65536):
                copied += len(chunk)
                require(copied <= package.MAX_PACKAGE, "bounded archive required")
                digest.update(chunk)
                target.write(chunk)
        require(digest.hexdigest() == expected_sha256, "archive byte hash mismatch")
        with snapshot.open("rb") as source:
            package.preflight_tar(source)  # Reject extension/link/sparse headers first.
            with tarfile.open(fileobj=source, mode="r:gz") as bundle:
                members = bundle.getmembers()
                require(
                    len(members) == len(PAYLOAD) and {item.name for item in members} == PAYLOAD,
                    "closed snapshot archive payload required",
                )
                files_observed, total = {}, 0
                for member in members:
                    require(
                        member.isfile()
                        and not member.issparse()
                        and member.name not in files_observed
                        and not member.pax_headers,
                        "regular unique snapshot members required",
                    )
                    total += member.size
                    require(
                        0 < member.size <= package.MAX_PACKAGE and total <= package.MAX_PACKAGE,
                        "bounded nonempty archive member required",
                    )
                    require(not member.mode & 0o7000, "privileged archive mode refused")
                    bounded_member(member.name, member.size)
                    with (
                        bundle.extractfile(member) as source_member,
                        (stage / member.name).open("xb") as target_member,
                    ):
                        shutil.copyfileobj(source_member, target_member, length=65536)
                    files_observed[member.name] = {
                        "sha256": package.sha256(stage / member.name),
                        "bytes": member.size,
                    }
        # Every member except the checksum inventory is covered, including manifest.
        expected_sums = "".join(
            f"{files_observed[name]['sha256']}  {name}\n"
            for name in sorted(PAYLOAD - {"SHA256SUMS"})
        ).encode()
        require(
            (stage / "SHA256SUMS").read_bytes() == expected_sums,
            "snapshot checksum inventory mismatch",
        )
        manifest = package.decode(package.metadata_bytes(stage / "manifest.json"))
        require(
            type(manifest) is dict
            and manifest.get("schema_version") == 1
            and type(manifest["schema_version"]) is int
            and manifest.get("kind") == "pumas-headless-no-inference-source-snapshot"
            and manifest.get("inference_compile_enabled") is False
            and manifest.get("default_features") is False
            and manifest.get("features") == []
            and manifest.get("target") == TARGET
            and manifest.get("profile") == "release",
            "inference-disabled snapshot manifest required",
        )
        files = manifest.get("files")
        require(
            type(files) is dict and set(files) == PAYLOAD - {"manifest.json", "SHA256SUMS"},
            "manifest file closure mismatch",
        )
        for name, item in files.items():
            require(
                type(item) is dict
                and set(item) == {"sha256", "bytes"}
                and type(item["bytes"]) is int
                and item == files_observed[name],
                "manifest member bytes mismatch",
            )
        info, core = (
            package.decode(package.metadata_bytes(stage / name))
            for name in ("rpc-build-info.json", "core-build-info.json")
        )
        expected_core = check_identity(
            info, manifest["package_source"], manifest["package_version"], manifest["build_id"]
        )
        require(
            package.canonical(core) == package.canonical(expected_core),
            "shared core projection mismatch",
        )
        require(
            manifest["binary_sha256"] == files["pumas-rpc"]["sha256"],
            "manifest executable hash mismatch",
        )
        validate_schema(
            package.metadata_bytes(stage / "desktop-contract.json"),
            package.decode(package.metadata_bytes(stage / "schema-export-record.json")),
            manifest["package_source"],
            manifest["package_version"],
        )
        require(
            manifest["schema_sha256"] == files["desktop-contract.json"]["sha256"],
            "manifest schema hash mismatch",
        )
        destination.mkdir(parents=True)
        for name in PAYLOAD:
            with (
                (stage / name).open("rb") as source_member,
                (destination / name).open("xb") as target_member,
            ):
                shutil.copyfileobj(source_member, target_member, length=65536)
            (destination / name).chmod(0o755 if name == "pumas-rpc" else 0o644)
    return manifest


def produce(
    repository,
    source,
    build_id,
    schema,
    schema_record,
    output,
    completed=None,
    runner=subprocess.run,
    environment=None,
    producer_repository=ROOT,
):
    repository, output = plain_path(repository), plain_path(output)
    producer_repository = plain_path(producer_repository)
    source, schema_record = copy.deepcopy((source, schema_record))
    require(
        type(source) is dict
        and set(source) == {"head", "tree"}
        and all(re.fullmatch(r"[a-f0-9]{40}", value or "") for value in source.values()),
        "exact source HEAD/tree required",
    )
    require(package.discovery.token(build_id), "bounded build ID required")
    require(
        not output.exists()
        and not output.is_relative_to(repository)
        and not output.is_relative_to(producer_repository),
        "fresh output outside package and producer checkouts required",
    )
    require(
        platform.system() == "Linux" and platform.machine().lower() in {"x86_64", "amd64"},
        "native Linux x86_64 host required",
    )
    environment = dict(os.environ if environment is None else environment)
    native_environment(environment)
    cargo_home = Path(environment.get("CARGO_HOME", Path.home() / ".cargo"))
    if not cargo_home.is_absolute():
        cargo_home = repository / cargo_home
    environment.update(
        CARGO_HOME=str(cargo_home.resolve()),
        ORT_SKIP_DOWNLOAD="1",
        CARGO_BUILD_JOBS="1",
        CARGO_INCREMENTAL="0",
        CMAKE_BUILD_PARALLEL_LEVEL="1",
    )

    def tool_runner(command, **options):
        options["env"] = environment
        return runner(command, **options)

    provenance.check_configuration(repository, environment)
    release_settings = provenance.check_release_manifest(repository)
    before = inference_build.source_identity(repository, tool_runner)
    require(before == source, "source HEAD/tree mismatch")
    producer = inference_build.source_identity(producer_repository, tool_runner)
    with (repository / "rust/Cargo.toml").open("rb") as stream:
        version = tomllib.load(stream)["workspace"]["package"]["version"]
    require(package.discovery.token(version), "bounded source version required")
    require(
        version == json.loads((repository / "package.json").read_text())["version"],
        "source package version alignment required",
    )
    validate_schema(schema, schema_record, source, version)
    regular(repository / "LICENSE", MAX_LEGAL_TEXT)
    attribution = attribution_binding(repository, tool_runner)
    rustc = provenance.capture(["rustc", "-Vv"], repository, tool_runner)
    require(f"host: {TARGET}" in rustc.splitlines(), "native Rust host required")
    cargo = provenance.capture(["cargo", "--version"], repository, tool_runner)
    output.mkdir(parents=True)
    environment.update(
        PUMAS_SOURCE_REVISION=source["head"], PUMAS_BUILD_ID=build_id, PUMAS_BUILD_TARGET=TARGET
    )
    if completed is None:
        log, errors = output / "cargo.jsonl", output / "cargo.stderr.log"
        with log.open("x") as stdout, errors.open("x") as stderr:
            result = runner(
                COMMAND,
                cwd=repository,
                env=environment,
                check=False,
                stdout=stdout,
                stderr=stderr,
                text=True,
            )
        require(result.returncode == 0, "Cargo build failed; diagnostics preserved")
        artifacts = artifact_evidence(log.read_text(), repository)
        binary = regular(plain_path(artifacts["rpc"]["executable"]))
        compile_origin = {
            "mode": "producer_build",
            "command": COMMAND,
            "cargo_jsonl_sha256": package.sha256(log),
            "cargo_stderr_sha256": package.sha256(errors),
        }
    else:
        require(
            type(completed) is dict
            and set(completed)
            == {
                "binary",
                "binary_sha256",
                "cargo_jsonl",
                "cargo_jsonl_sha256",
                "command",
            },
            "unsupported completed build evidence",
        )
        require(completed["command"] == COMMAND, "completed build command mismatch")
        binary = regular(plain_path(completed["binary"]))
        log = regular(plain_path(completed["cargo_jsonl"]))
        require(
            package.sha256(binary) == completed["binary_sha256"]
            and package.sha256(log) == completed["cargo_jsonl_sha256"],
            "completed build byte hash mismatch",
        )
        artifacts = artifact_evidence(log.read_text(), repository)
        compile_origin = {
            "mode": "completed_local_build_record",
            "command": COMMAND,
            "cargo_jsonl_sha256": package.sha256(log),
            "custody": "caller-supplied local evidence, not authenticated attestation",
        }
    identity_environment = {
        name: value
        for name, value in environment.items()
        if not name.startswith(("ORT_", "PUMAS_", "PYTHON", "LD_", "DYLD_"))
    }
    info = package.decode(
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
    core = check_identity(info, source, version, build_id)
    binary_hash = package.sha256(binary)
    require(
        inference_build.source_identity(repository, tool_runner) == before,
        "package source changed during production",
    )
    require(
        inference_build.source_identity(producer_repository, tool_runner) == producer,
        "producer source changed during production",
    )
    provenance.check_configuration(repository, environment)
    require(
        attribution_binding(repository, tool_runner) == attribution,
        "attribution changed during production",
    )
    manifest = {
        "schema_version": 1,
        "kind": "pumas-headless-no-inference-source-snapshot",
        "qualification": "constructed_unqualified_snapshot",
        "package_source": before,
        "producer_source": producer,
        "package_version": version,
        "target": TARGET,
        "profile": "release",
        "default_features": False,
        "features": [],
        "inference_compile_enabled": False,
        "live_model_capabilities": "not provided",
        "build_id": info["build_id"],
        "binary_sha256": binary_hash,
        "artifacts": artifacts,
        "compile_origin": compile_origin,
        "checked_in_release_settings": release_settings,
        "attribution": attribution,
        "schema_sha256": hashlib.sha256(schema).hexdigest(),
        "rustc": rustc,
        "cargo": cargo,
        "startup": "unqualified",
        "native_loading": "unqualified",
        "real_inference": "unqualified",
    }
    archive, digest, manifest = assemble(
        binary, repository, info, core, schema, schema_record, manifest, output
    )
    require(package.sha256(binary) == binary_hash, "binary changed during assembly")
    require(
        inference_build.source_identity(repository, tool_runner) == before,
        "package source changed during assembly",
    )
    evidence = {
        "archive": str(archive),
        "archive_sha256": digest,
        "archive_bytes": archive.stat().st_size,
        "manifest": manifest,
    }
    (output / "build-evidence.json").write_bytes(package.canonical(evidence))
    return evidence


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("repository", "schema", "schema-export-record", "output-dir"):
        parser.add_argument(f"--{name}", type=Path, required=True)
    for name in (
        "source-head",
        "source-tree",
        "build-id",
        "schema-sha256",
        "schema-export-record-sha256",
    ):
        parser.add_argument(f"--{name}", required=True)
    parser.add_argument("--completed-build-record", type=Path)
    parser.add_argument("--completed-build-record-sha256")
    args = parser.parse_args()
    regular(args.schema, package.MAX_MANIFEST)
    schema = package.metadata_bytes(args.schema)
    require(hashlib.sha256(schema).hexdigest() == args.schema_sha256, "schema byte hash mismatch")
    require(
        bool(args.completed_build_record) == bool(args.completed_build_record_sha256),
        "completed build record and hash must be supplied together",
    )
    evidence = produce(
        args.repository,
        {"head": args.source_head, "tree": args.source_tree},
        args.build_id,
        schema,
        package.read_pinned_json(args.schema_export_record, args.schema_export_record_sha256),
        args.output_dir,
        completed=package.read_pinned_json(
            args.completed_build_record, args.completed_build_record_sha256
        )
        if args.completed_build_record
        else None,
    )
    print(
        json.dumps({key: evidence[key] for key in ("archive", "archive_sha256", "archive_bytes")})
    )


if __name__ == "__main__":
    main()
