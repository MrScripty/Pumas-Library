"""Build/verify unreleased inference headless candidates from explicit local inputs.

Checksums bind bytes, not authenticity. The caller supplies trusted record hashes.
Compatibility smoke is not native runtime loading or real inference acceptance.
No command builds Cargo, fetches a runtime/model, or publishes a release.
"""

import argparse
import copy
import gzip
import hashlib
import json
import os
import platform
import re
import shutil
import signal
import subprocess
import tarfile
import tempfile
import time
import urllib.request
import zipfile
from contextlib import ExitStack
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
PLAN = json.loads((Path(__file__).with_name("headless-inference-plan.json")).read_text())
TARGETS = {target["id"]: target for target in PLAN["targets"]}
HEX64 = re.compile(r"[a-f0-9]{64}\Z")
HEX40 = re.compile(r"[a-f0-9]{40}\Z")
VERSION = re.compile(r"0\.[78]\.(0|[1-9][0-9]*)(?:-[A-Za-z0-9.-]+)?\Z")
# The only live identity is discovery's shared PumasBuildInfo. Tree/schema
# digests and Cargo feature selection describe the pinned archive cohort only.
IDENTITY_KEYS = {"build_info"}
BUILD_INFO_KEYS = {
    "build_info_schema_version",
    "component",
    "package_version",
    "build_id",
    "source_revision",
    "target",
    "compiled_features",
    "protocols",
    "schemas",
}
MAX_MANIFEST = 1024 * 1024
MAX_PACKAGE = 4 * 1024 * 1024 * 1024
BUILD_KEYS = {
    "version",
    "target",
    "host",
    "profile",
    "features",
    "source",
    "build_id",
    "inference_enabled",
    "binary_sha256",
    "command",
    "rustc",
}
RUNTIME_KEYS = {"target", "version", "loader_entry", "source_archive_sha256", "files"}
MANIFEST_KEYS = {
    "schema_version",
    "variant",
    "qualification",
    "version",
    "target",
    "build",
    "runtime",
    "compatibility",
    "files",
}
IDENTIFIER = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.+-]{0,127}\Z")
RUSTC_VERSION = re.compile(
    r"rustc [0-9]+\.[0-9]+\.[0-9]+(?:-[A-Za-z0-9.]+)? "
    r"\([a-f0-9]{7,40} [0-9]{4}-[0-9]{2}-[0-9]{2}\)\Z"
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def exact_fields(value, fields, label):
    require(type(value) is dict and set(value) == fields, f"unexpected or missing {label} fields")


def matches(value, pattern, limit):
    return isinstance(value, str) and len(value) <= limit and pattern.fullmatch(value) is not None


def production_build_commands(target):
    """Closed argv forms; neither environment assignments nor arbitrary flags are metadata."""
    commands = []
    for manifest in (["--manifest-path", "rust/Cargo.toml"], []):
        for features in ("s3,inference-plugins", "inference-plugins,s3"):
            commands.append(
                [
                    "cargo",
                    "build",
                    "--locked",
                    "--offline",
                    *manifest,
                    "-p",
                    "pumas-rpc",
                    "--release",
                    "--no-default-features",
                    "--features",
                    features,
                    "--target",
                    target,
                ]
            )
    for features in ("s3,inference-plugins", "inference-plugins,s3"):
        commands.append(
            [
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
                "--features",
                features,
                "--target",
                target,
                "--release",
                "--message-format=json-render-diagnostics",
            ]
        )
    return commands


def validate_file_item(item):
    exact_fields(item, {"sha256", "bytes"}, "file item")
    require(
        matches(item["sha256"], HEX64, 64)
        and type(item["bytes"]) is int
        and 0 < item["bytes"] <= MAX_PACKAGE,
        "invalid member hash/size",
    )


def validate_build_record(build, target, contract):
    expected = contract["expected"]["build_info"]
    exact_fields(build, BUILD_KEYS, "build record")
    exact_fields(build["source"], {"head", "tree"}, "build source")
    require(
        build["target"] == target["rust_target"]
        and build["host"] == target["rust_target"]
        and build["profile"] == "release"
        and expected["target"] == target["rust_target"],
        "native production target/profile required",
    )
    require(
        build["features"] == contract["rpc_features"]
        and build["source"]
        == {"head": expected["source_revision"], "tree": contract["source_tree"]}
        and build["build_id"] == expected["build_id"],
        "build/cohort identity mismatch",
    )
    require(
        build["version"] == expected["package_version"] and build["inference_enabled"] is True,
        "compiled version/features mismatch",
    )
    require(matches(build["binary_sha256"], HEX64, 64), "binary build hash required")
    require(
        type(build["command"]) is list
        and build["command"] in production_build_commands(target["rust_target"]),
        "reviewed offline locked production command required",
    )
    require(matches(build["rustc"], RUSTC_VERSION, 160), "bounded rustc version required")


def validate_runtime_record(runtime, target):
    exact_fields(runtime, RUNTIME_KEYS, "runtime record")
    require(
        runtime["target"] == target["rust_target"]
        and runtime["version"] == PLAN["runtime_version"]
        and runtime["loader_entry"] == target["runtime_entry"],
        "exact runtime target/version/entry required",
    )
    require(
        matches(runtime["source_archive_sha256"], HEX64, 64), "runtime source archive hash required"
    )
    files = runtime["files"]
    require(
        type(files) is dict and 0 < len(files) <= 123 and target["runtime_entry"] in files,
        "runtime closure missing exact loader basename",
    )
    for name, item in files.items():
        flat_name(name)
        require(
            re.fullmatch(r"[A-Za-z0-9_.-]+(?:\.dll|\.dylib|\.so(?:\.[0-9]+)*)", name),
            "runtime closure must contain native libraries only",
        )
        validate_file_item(item)
    require(
        sum(item["bytes"] for item in files.values()) <= MAX_PACKAGE, "runtime exceeds bounded size"
    )


def sha256(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        require(key not in result, f"duplicate JSON key: {key}")
        result[key] = value
    return result


def decode(data):
    require(len(data) <= MAX_MANIFEST, "metadata exceeds bounded size")

    def reject_constant(value):
        raise ValueError(f"nonfinite JSON number: {value}")

    return json.loads(data, object_pairs_hook=unique_object, parse_constant=reject_constant)


def metadata_bytes(path):
    with Path(path).open("rb") as stream:
        data = stream.read(MAX_MANIFEST + 1)
    require(len(data) <= MAX_MANIFEST, "metadata exceeds bounded size")
    return data


def canonical(data):
    return (
        json.dumps(data, sort_keys=True, separators=(",", ":"), allow_nan=False) + "\n"
    ).encode()


def flat_name(name):
    require(
        isinstance(name, str) and 0 < len(name) <= 100 and name == name.strip(),
        "invalid member name",
    )
    require(
        not any(char in name for char in "/\\:\0\r\n") and not name.endswith("."),
        "unsafe member name",
    )
    require(name not in {".", ".."}, "unsafe member name")
    require(re.fullmatch(r"[A-Za-z0-9_.-]+", name), "nonportable member name")
    require(
        name.split(".")[0].upper()
        not in {
            "CON",
            "PRN",
            "AUX",
            "NUL",
            *(f"COM{i}" for i in range(1, 10)),
            *(f"LPT{i}" for i in range(1, 10)),
        },
        "reserved member name",
    )
    return name


def read_pinned_json(path, expected_sha256):
    """Read public metadata pinned by byte digest, not confidential or authenticated data."""
    path = Path(path)
    require(not path.is_symlink() and path.is_file(), "metadata must be a regular file")
    require(HEX64.fullmatch(expected_sha256 or ""), "trusted metadata hash is required")
    data = metadata_bytes(path)
    require(hashlib.sha256(data).hexdigest() == expected_sha256, "trusted metadata hash mismatch")
    return decode(data)


def validate_contract(contract):
    exact_fields(
        contract,
        {"schema_version", "expected", "source_tree", "schema_sha256", "rpc_features", "requests"},
        "compatibility contract",
    )
    require(
        type(contract["schema_version"]) is int and contract["schema_version"] == 2,
        "unsupported compatibility contract",
    )
    exact_fields(contract["expected"], IDENTITY_KEYS, "compatibility identity")
    info = contract["expected"]["build_info"]
    exact_fields(info, BUILD_INFO_KEYS, "PumasBuildInfo")
    require(
        type(info["build_info_schema_version"]) is int
        and info["build_info_schema_version"] == 1
        and info["component"] == "pumas-rpc",
        "shared RPC build identity required",
    )
    require(matches(info["package_version"], VERSION, 96), "candidate package version required")
    require(
        matches(info["source_revision"], HEX40, 40) and matches(contract["source_tree"], HEX40, 40),
        "immutable source identity required",
    )
    require(matches(info["build_id"], IDENTIFIER, 128), "bounded build identity required")
    require(info["target"] in {t["rust_target"] for t in TARGETS.values()}, "build target required")
    require(matches(contract["schema_sha256"], HEX64, 64), "schema digest required")
    features = info["compiled_features"]
    require(
        type(features) is list
        and 0 < len(features) <= 32
        and all(
            isinstance(f, str)
            and len(f) <= 128
            and re.fullmatch(r"pumas-(?:library|rpc)/[A-Za-z0-9_.+-]+", f)
            for f in features
        )
        and len(set(features)) == len(features),
        "namespaced actual compiled features required",
    )
    require(
        {
            "pumas-rpc/s3",
            "pumas-rpc/inference-plugins",
            "pumas-library/s3",
            "pumas-library/onnx-runtime",
        }
        <= set(features),
        "S3 and inference compiled features required",
    )
    require(
        type(contract["rpc_features"]) is list
        and len(contract["rpc_features"]) == len(PLAN["rpc_features_required"])
        and all(isinstance(f, str) for f in contract["rpc_features"])
        and set(contract["rpc_features"]) == set(PLAN["rpc_features_required"]),
        "reviewed Cargo feature selection required",
    )
    for field, nested, version_field in (
        ("protocols", {"name", "versions"}, "versions"),
        ("schemas", {"name", "version"}, "version"),
    ):
        values = info[field]
        require(
            type(values) is list and 0 < len(values) <= 32,
            "bounded protocol/schema advertisements required",
        )
        names = []
        for value in values:
            exact_fields(value, nested, "build advertisement")
            require(matches(value["name"], IDENTIFIER, 128), "advertisement name required")
            names.append(value["name"])
            versions = value[version_field] if field == "protocols" else [value[version_field]]
            require(
                type(versions) is list
                and 0 < len(versions) <= 32
                and all(type(v) is int and 0 < v <= 2**31 - 1 for v in versions)
                and len(set(versions)) == len(versions),
                "protocol/schema version required",
            )
        require(len(set(names)) == len(names), "duplicate advertisement")
    require(
        {"name": "pumas.local-http", "versions": [1]} in info["protocols"]
        and {"name": "pumas.build-info", "version": 1} in info["schemas"],
        "local HTTP/build-info contract required",
    )
    # Fixed real route and field: no caller-defined endpoint or competing identity.
    require(
        contract["requests"]
        == [{"path": "/.well-known/pumas", "bindings": {"build_info": "/build_info"}}],
        "discovery shared build-info binding required",
    )
    return contract["expected"]


def check_manifest(manifest):
    exact_fields(manifest, MANIFEST_KEYS, "manifest")
    require(
        type(manifest.get("schema_version")) is int
        and manifest["schema_version"] == 1
        and manifest.get("variant") == "headless-inference",
        "wrong manifest variant",
    )
    require(
        manifest.get("qualification") == "unverified_candidate",
        "package metadata cannot grant qualification",
    )
    target = TARGETS.get(manifest.get("target"))
    require(target is not None, "unknown target")
    validate_contract(manifest["compatibility"])
    require(
        manifest["version"]
        == manifest["compatibility"]["expected"]["build_info"]["package_version"],
        "package version mismatch",
    )
    build = manifest["build"]
    validate_build_record(build, target, manifest["compatibility"])
    runtime = manifest["runtime"]
    validate_runtime_record(runtime, target)
    files = manifest["files"]
    require(
        isinstance(files, dict) and 6 <= len(files) <= 128, "bounded complete inventory required"
    )
    require(len({name.casefold() for name in files}) == len(files), "case-colliding inventory")
    for name, item in files.items():
        flat_name(name)
        require(name not in {"manifest.json", "SHA256SUMS"}, "reserved inventory member")
        validate_file_item(item)
    require(
        sum(item["bytes"] for item in files.values()) <= MAX_PACKAGE, "package exceeds bounded size"
    )
    require(
        {
            target["binary"],
            target["runtime_entry"],
            "LICENSE.txt",
            "THIRD-PARTY-NOTICES.txt",
            "protocol-schema.json",
            "build-record.json",
        }
        <= set(files),
        "required payload missing",
    )
    require(
        files[target["binary"]]["sha256"] == build["binary_sha256"], "binary/build binding mismatch"
    )
    require(
        files["protocol-schema.json"]["sha256"] == manifest["compatibility"]["schema_sha256"],
        "schema/cohort mismatch",
    )
    runtime_files = runtime.get("files", {})
    require(
        all(
            name in files and canonical(files[name]) == canonical(item)
            for name, item in runtime_files.items()
        ),
        "runtime closure differs from payload",
    )
    require(
        set(files)
        == {
            target["binary"],
            "LICENSE.txt",
            "THIRD-PARTY-NOTICES.txt",
            "protocol-schema.json",
            "build-record.json",
            *runtime_files,
        },
        "unexpected payload member",
    )
    return target


def admitted_metadata(build, runtime, contract, target):
    """Detach the declared public fields before staging; never export input objects.

    Validation rejects undeclared fields rather than silently dropping them. A
    pinned digest establishes byte identity, not whether an identifier is public:
    the caller must review the public build/compatibility/runtime records before
    supplying their pins. Preserve admitted identity and actual command spelling.
    """
    validate_contract(contract)
    validate_build_record(build, target, contract)
    validate_runtime_record(runtime, target)
    public_build = {
        "version": build["version"],
        "target": target["rust_target"],
        "host": target["rust_target"],
        "profile": "release",
        "features": list(build["features"]),
        "source": {"head": build["source"]["head"], "tree": build["source"]["tree"]},
        "build_id": build["build_id"],
        "inference_enabled": True,
        "binary_sha256": build["binary_sha256"],
        "command": next(
            argv
            for argv in production_build_commands(target["rust_target"])
            if argv == build["command"]
        ),
        "rustc": build["rustc"],
    }
    public_runtime = {
        "target": target["rust_target"],
        "version": PLAN["runtime_version"],
        "loader_entry": target["runtime_entry"],
        "source_archive_sha256": runtime["source_archive_sha256"],
        "files": {
            name: {"sha256": item["sha256"], "bytes": item["bytes"]}
            for name, item in runtime["files"].items()
        },
    }
    public_contract = copy.deepcopy(contract)
    # Validate the detached snapshot too; caller-owned containers are no longer
    # consulted while copying payloads or serializing either metadata member.
    validate_contract(public_contract)
    validate_build_record(public_build, target, public_contract)
    validate_runtime_record(public_runtime, target)
    return public_build, public_runtime, public_contract


def assemble(inputs, build_record, runtime_record, contract, schema, output):
    """Records must already have been checked against caller-trusted digests."""
    output = Path(output)
    require(not output.exists(), "refusing to overwrite archive")
    require(
        not output.with_name(output.name + ".sha256").exists(),
        "refusing to overwrite archive checksum",
    )
    require(
        not output.resolve().is_relative_to(ROOT),
        "generated archives must stay outside the source checkout",
    )
    decode(schema)
    exact_fields(build_record, BUILD_KEYS, "build record")
    target = next(
        (item for item in TARGETS.values() if item["rust_target"] == build_record["target"]), None
    )
    require(target is not None, "unsupported build target")
    build_record, runtime_record, contract = admitted_metadata(
        build_record, runtime_record, contract, target
    )
    expected = contract["expected"]
    manifest = {
        "schema_version": 1,
        "variant": "headless-inference",
        "qualification": "unverified_candidate",
        "version": expected["build_info"]["package_version"],
        "target": target["id"],
        "build": build_record,
        "runtime": runtime_record,
        "compatibility": contract,
        "files": {},
    }
    require(
        output.name
        == f"pumas-rpc-inference-{expected['build_info']['package_version']}-{target['id']}.{target['format']}",
        "archive filename/target mismatch",
    )
    with tempfile.TemporaryDirectory(prefix="pumas-inference-stage-") as temporary:
        stage = Path(temporary)
        names = set()
        for name, source in inputs.items():
            flat_name(name)
            require(
                name.casefold()
                not in {"manifest.json", "sha256sums", "build-record.json", "protocol-schema.json"},
                "reserved staging member",
            )
            require(name.casefold() not in names, "duplicate staging member")
            names.add(name.casefold())
            source = Path(source)
            require(
                not source.is_symlink() and source.is_file() and source.stat().st_size > 0,
                "only explicit nonempty regular inputs accepted",
            )
            shutil.copyfile(source, stage / name)
        (stage / "build-record.json").write_bytes(canonical(build_record))
        (stage / "protocol-schema.json").write_bytes(schema)
        for member in stage.iterdir():
            manifest["files"][member.name] = {
                "sha256": sha256(member),
                "bytes": member.stat().st_size,
            }
        check_manifest(manifest)
        (stage / target["binary"]).chmod(0o755)
        (stage / "manifest.json").write_bytes(canonical(manifest))
        sums = "".join(f"{sha256(member)}  {member.name}\n" for member in sorted(stage.iterdir()))
        (stage / "SHA256SUMS").write_text(sums)
        output.parent.mkdir(parents=True, exist_ok=True)
        temporary_output = output.with_name(output.name + ".partial")
        require(not temporary_output.exists(), "partial output already exists")
        owned_partial = False
        try:
            if target["format"] == "zip":
                with zipfile.ZipFile(temporary_output, "x", zipfile.ZIP_DEFLATED) as archive:
                    owned_partial = True
                    for member in sorted(stage.iterdir()):
                        archive.write(member, member.name)
            else:
                with tarfile.open(temporary_output, "x:gz", format=tarfile.USTAR_FORMAT) as archive:
                    owned_partial = True
                    for member in sorted(stage.iterdir()):
                        archive.add(member, arcname=member.name, recursive=False)
            # Hard link refuses a racing destination; publication is local only.
            os.link(temporary_output, output)
        finally:
            if owned_partial:
                temporary_output.unlink(missing_ok=True)
    digest = sha256(output)
    with output.with_name(output.name + ".sha256").open("x") as stream:
        stream.write(f"{digest}  {output.name}\n")
    return manifest, digest


def preflight_tar(source):
    """Reject extension headers before tarfile can allocate their bodies."""
    source.seek(0)
    total, count = 0, 0
    with gzip.GzipFile(fileobj=source) as stream:
        while True:
            header = stream.read(512)
            require(len(header) == 512, "truncated tar header")
            if header == b"\0" * 512:
                trailer = stream.read(MAX_MANIFEST + 1)
                require(
                    len(trailer) <= MAX_MANIFEST and not trailer.strip(b"\0"), "invalid tar trailer"
                )
                break
            member = tarfile.TarInfo.frombuf(header, "utf-8", "strict")
            require(member.isfile() and not member.issparse(), "nonregular tar header")
            flat_name(member.name)
            count += 1
            total += member.size
            require(count <= 130, "bounded archive member count required")
            require(
                0 <= member.size <= MAX_PACKAGE and total <= MAX_PACKAGE + 2 * MAX_MANIFEST,
                "archive payload exceeds bounded size",
            )
            if member.name in {
                "manifest.json",
                "SHA256SUMS",
                "build-record.json",
                "protocol-schema.json",
            }:
                require(member.size <= MAX_MANIFEST, "metadata exceeds bounded size")
            remaining = (member.size + 511) // 512 * 512
            while remaining:
                chunk = stream.read(min(65536, remaining))
                require(chunk, "truncated tar payload")
                remaining -= len(chunk)
    source.seek(0)


def extract_verified(archive_path, expected_sha256, destination):
    archive_path, destination = Path(archive_path), Path(destination)
    require(
        not archive_path.is_symlink()
        and archive_path.is_file()
        and archive_path.stat().st_size <= MAX_PACKAGE,
        "bounded regular archive required",
    )
    require(not destination.exists(), "extraction requires a fresh destination")
    with ExitStack() as resources:
        source_archive = resources.enter_context(tempfile.TemporaryFile())
        # Hash and parse the same private snapshot, including against in-place writes.
        with archive_path.open("rb") as supplied:
            copied = 0
            while chunk := supplied.read(65536):
                copied += len(chunk)
                require(copied <= MAX_PACKAGE, "bounded regular archive required")
                source_archive.write(chunk)
        source_archive.seek(0)
        require(
            HEX64.fullmatch(expected_sha256 or "")
            and hashlib.file_digest(source_archive, "sha256").hexdigest() == expected_sha256,
            "consumer archive SHA256 mismatch",
        )
        source_archive.seek(0)
        if not archive_path.name.endswith(".zip"):
            preflight_tar(source_archive)
        if archive_path.name.endswith(".zip"):
            archive = resources.enter_context(zipfile.ZipFile(source_archive))
        else:
            archive = resources.enter_context(tarfile.open(fileobj=source_archive, mode="r:gz"))
        members = archive.infolist() if isinstance(archive, zipfile.ZipFile) else archive
        entries = {}
        declared_total = 0
        for member in members:
            require(len(entries) < 130, "bounded archive member count required")
            name = member.filename if isinstance(archive, zipfile.ZipFile) else member.name
            flat_name(name)
            require(
                name.casefold() not in {key.casefold() for key in entries},
                "duplicate/colliding archive member",
            )
            if isinstance(archive, zipfile.ZipFile):
                mode = (member.external_attr >> 16) & 0o170000
                require(
                    not member.is_dir() and mode in (0, 0o100000) and not member.flag_bits & 1,
                    "nonregular/encrypted archive member",
                )
                size = member.file_size
            else:
                require(member.isfile() and not member.issparse(), "nonregular archive member")
                size = member.size
            declared_total += size
            require(
                0 <= size <= MAX_PACKAGE and declared_total <= MAX_PACKAGE + 2 * MAX_MANIFEST,
                "archive payload exceeds bounded size",
            )
            if name in {"manifest.json", "SHA256SUMS", "build-record.json", "protocol-schema.json"}:
                require(size <= MAX_MANIFEST, "metadata exceeds bounded size")
            entries[name] = (member, size)
        require(
            "manifest.json" in entries and entries["manifest.json"][1] <= MAX_MANIFEST,
            "bounded manifest required",
        )

        def read(name):
            member = entries[name][0]
            return (
                archive.open(member)
                if isinstance(archive, zipfile.ZipFile)
                else archive.extractfile(member)
            )

        with read("manifest.json") as stream:
            manifest = decode(stream.read(MAX_MANIFEST + 1))
        target = check_manifest(manifest)
        require(
            set(entries) == {*manifest["files"], "manifest.json", "SHA256SUMS"},
            "archive inventory mismatch",
        )
        require(entries["SHA256SUMS"][1] <= MAX_MANIFEST, "checksum inventory too large")
        destination.mkdir(parents=True)
        try:
            for name, (_, size) in entries.items():
                limit = (
                    MAX_MANIFEST
                    if name in ("manifest.json", "SHA256SUMS")
                    else manifest["files"][name]["bytes"]
                )
                require(
                    size == limit if name in manifest["files"] else size <= limit,
                    "declared size mismatch",
                )
                with read(name) as source, (destination / name).open("xb") as sink:
                    remaining = limit
                    while chunk := source.read(min(65536, remaining + 1)):
                        require(len(chunk) <= remaining, "member exceeds bounded size")
                        sink.write(chunk)
                        remaining -= len(chunk)
                if name in manifest["files"]:
                    require(
                        sha256(destination / name) == manifest["files"][name]["sha256"],
                        "payload hash mismatch",
                    )
            expected_sums = "".join(
                f"{sha256(destination / name)}  {name}\n"
                for name in sorted(entries)
                if name != "SHA256SUMS"
            )
            require(
                (destination / "SHA256SUMS").read_text() == expected_sums,
                "inner checksum inventory mismatch",
            )
            require(
                canonical(decode(metadata_bytes(destination / "build-record.json")))
                == canonical(manifest["build"]),
                "embedded build record mismatch",
            )
            decode(metadata_bytes(destination / "protocol-schema.json"))
            (destination / target["binary"]).chmod(0o755)
        except Exception:
            shutil.rmtree(destination)
            raise
        return manifest


def pointer(document, path):
    value = document
    for part in path[1:].split("/"):
        part = part.replace("~1", "/").replace("~0", "~")
        if isinstance(value, list):
            require(re.fullmatch(r"0|[1-9][0-9]*", part), "invalid JSON pointer array index")
            value = value[int(part)]
        else:
            value = value[part]
    return value


def smoke(destination, manifest, expected_contract, timeout=45):
    """Start exact extracted bytes on the same native host; never load a model."""
    target = check_manifest(manifest)
    require(
        canonical(manifest["compatibility"]) == canonical(expected_contract),
        "consumer compatibility contract differs",
    )
    require(
        platform.system().lower() == target["host"]
        and platform.machine().lower()
        in ({"arm64", "aarch64"} if target["machine"] == "arm64" else {"x86_64", "amd64"}),
        "native target unavailable on this host",
    )
    destination = Path(destination).resolve()
    for name, item in manifest["files"].items():
        require(
            sha256(destination / name) == item["sha256"], "extracted input changed before startup"
        )
    environment = {
        name: value
        for name, value in os.environ.items()
        if not name.startswith(("ORT_", "PUMAS_", "PYTHON", "LD_", "DYLD_"))
    }
    http = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    with tempfile.TemporaryDirectory(prefix="pumas-consumer-root-") as temporary:
        root = Path(temporary)
        environment.update(
            {
                "XDG_CONFIG_HOME": str(root / "config"),
                "XDG_DATA_HOME": str(root / "data"),
                "XDG_CACHE_HOME": str(root / "cache"),
                "XDG_STATE_HOME": str(root / "state"),
                "APPDATA": str(root / "config"),
                "LOCALAPPDATA": str(root / "data"),
                "PUMAS_REGISTRY_DB_PATH": str(root / "registry.db"),
                "ORT_SKIP_DOWNLOAD": "1",
            }
        )
        log_path = root / "rpc.log"
        with log_path.open("wb") as log:
            process = subprocess.Popen(
                [str(destination / target["binary"]), "--launcher-root", str(root), "--port", "0"],
                cwd=root,
                env=environment,
                stdout=log,
                stderr=subprocess.STDOUT,
                creationflags=subprocess.CREATE_NEW_PROCESS_GROUP if os.name == "nt" else 0,
            )
            try:
                deadline = time.monotonic() + timeout
                while time.monotonic() < deadline:
                    require(process.poll() is None, "extracted process exited before readiness")
                    match = re.search(
                        r"^RPC_PORT=([0-9]{1,5})$",
                        metadata_bytes(log_path).decode(errors="replace"),
                        re.MULTILINE,
                    )
                    if match:
                        port = int(match[1])
                        require(0 < port < 65536, "invalid announced port")
                        break
                    time.sleep(0.05)
                else:
                    raise ValueError("extracted process did not announce readiness")
                observed = {}
                for request in expected_contract["requests"]:
                    with http.open(
                        f"http://127.0.0.1:{port}" + request["path"], timeout=5
                    ) as response:
                        require(response.status == 200, "handshake route failed")
                        document = decode(response.read(MAX_MANIFEST + 1))
                    for name, path in request["bindings"].items():
                        observed[name] = pointer(document, path)
                require(
                    canonical(observed) == canonical(expected_contract["expected"]),
                    "live PumasBuildInfo mismatch",
                )
                result = {
                    "target": target["id"],
                    "archive_payload_start": "passed",
                    "compatibility": "passed",
                    "runtime_loading": "unqualified",
                    "real_inference": "unqualified",
                }
            finally:
                if process.poll() is None:
                    try:
                        process.send_signal(
                            signal.CTRL_BREAK_EVENT if os.name == "nt" else signal.SIGINT
                        )
                        process.wait(timeout=10)
                    except (OSError, subprocess.TimeoutExpired):
                        process.kill()
                        process.wait(timeout=10)
                        raise ValueError("extracted process required forced shutdown")
            require(process.returncode == 0, "extracted process did not stop cleanly")
            return result


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, request, response, code, message, headers, new_url):
        raise ValueError("handshake redirects refused")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    commands = parser.add_subparsers(dest="command", required=True)
    build = commands.add_parser("assemble")
    for name in ("inputs", "build-record", "runtime-record", "contract", "schema", "output"):
        build.add_argument("--" + name, type=Path, required=True)
    for name in ("build-record-sha256", "runtime-record-sha256", "contract-sha256"):
        build.add_argument("--" + name, required=True)
    verify = commands.add_parser("verify")
    verify.add_argument("--archive", type=Path, required=True)
    verify.add_argument("--sha256", required=True)
    verify.add_argument("--destination", type=Path, required=True)
    verify.add_argument("--contract", type=Path, required=True)
    verify.add_argument("--contract-sha256", required=True)
    verify.add_argument("--start", action="store_true")
    args = parser.parse_args()
    contract = read_pinned_json(args.contract, args.contract_sha256)
    validate_contract(contract)
    if args.command == "assemble":
        build_record = read_pinned_json(args.build_record, args.build_record_sha256)
        runtime = read_pinned_json(args.runtime_record, args.runtime_record_sha256)
        inputs = decode(metadata_bytes(args.inputs))
        manifest, digest = assemble(
            inputs, build_record, runtime, contract, metadata_bytes(args.schema), args.output
        )
        print(json.dumps({"archive_sha256": digest, "qualification": manifest["qualification"]}))
    else:
        manifest = extract_verified(args.archive, args.sha256, args.destination)
        require(
            canonical(manifest["compatibility"]) == canonical(contract),
            "consumer compatibility contract differs",
        )
        print(
            json.dumps(
                smoke(args.destination, manifest, contract)
                if args.start
                else {"integrity": "passed", "real_inference": "unqualified"}
            )
        )


if __name__ == "__main__":
    main()
