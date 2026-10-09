#!/usr/bin/env python3
"""Build and bind unsigned local default-plus-S3 qualification evidence.

No provider or native runtime is obtained. The record is local evidence, not a
signed attestation or proof that a separately supplied JSON record is authentic.
"""

import argparse
import hashlib
import json
import os
import platform
from pathlib import Path
import subprocess
import sys
import tomllib

TARGET = "x86_64-unknown-linux-gnu"
RPC_FEATURES = ["default", "inference-plugins", "s3"]
CORE_FEATURES = ["gpu-monitor", "hf-client", "onnx-runtime", "process-manager", "s3"]
ATTRIBUTION = "docs/release-attribution/0.7.0-s3"
ATTRIBUTION_FILES = ("README.md", "inventory.json", "THIRD-PARTY-NOTICES.txt")
COMMAND = [
    "cargo",
    "build",
    "--locked",
    "--offline",
    "--manifest-path",
    "rust/Cargo.toml",
    "--release",
    "-p",
    "pumas-rpc",
    "--features",
    "s3",
    "--message-format=json-render-diagnostics",
]
PURPOSE = "unsigned local installed S3 qualification; no provider or inference acceptance"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def capture(command, repository, runner):
    return runner(
        command, cwd=repository, check=True, capture_output=True, text=True
    ).stdout.strip()


def source_identity(repository, runner=subprocess.run):
    require(
        not capture(["git", "status", "--porcelain", "--untracked-files=all"], repository, runner),
        "qualification requires a clean committed checkout",
    )
    return {
        "head": capture(["git", "rev-parse", "HEAD"], repository, runner),
        "tree": capture(["git", "rev-parse", "HEAD^{tree}"], repository, runner),
    }


def check_environment(environment):
    # turbojpeg-sys 1.2.0 uses var_os presence, including empty values, and
    # TARGET.upper().replace('-', '_') prefixes before global TURBOJPEG names.
    require(
        not any(name.startswith("TURBOJPEG_") or "_TURBOJPEG_" in name for name in environment),
        "TurboJPEG routing/binding overrides are not qualified",
    )
    for name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "CARGO_BUILD_RUSTFLAGS"):
        require(not environment.get(name), "Rust flag overrides are not qualified")
    require(
        not any(name.startswith("CARGO_PROFILE_RELEASE_") for name in environment),
        "release profile overrides are not qualified",
    )
    require(
        environment.get("CARGO_BUILD_TARGET", TARGET) == TARGET, "unsupported Cargo build target"
    )
    require(
        not any(
            name.startswith("CARGO_TARGET_") and name.endswith("_RUSTFLAGS") and value
            for name, value in environment.items()
        ),
        "target Rust flag overrides are not qualified",
    )


def check_configuration(repository, environment):
    # Cargo searches ancestors of cwd plus CARGO_HOME. Refusing configs avoids
    # claiming effective LTO/codegen settings absent from artifact JSON.
    directories = [path / ".cargo" for path in (repository, *repository.parents)]
    directories.append(Path(environment.get("CARGO_HOME", Path.home() / ".cargo")))
    require(
        not any(
            (directory / name).exists()
            for directory in directories
            for name in ("config", "config.toml")
        ),
        "active Cargo config files are not qualified",
    )


def check_release_manifest(repository):
    with (repository / "rust/Cargo.toml").open("rb") as stream:
        release = tomllib.load(stream)["profile"]["release"]
    require(
        release == {"lto": True, "codegen-units": 1, "strip": "debuginfo"},
        "repository release profile changed",
    )
    return release


def attribution_binding(repository):
    directory = repository / ATTRIBUTION
    inventory = json.loads((directory / "inventory.json").read_text())
    require(
        inventory.get("rust_profile")
        == {
            "package": "pumas-rpc",
            "default_features": True,
            "features": ["s3"],
            "targets": [TARGET, "aarch64-apple-darwin", "x86_64-pc-windows-msvc"],
        },
        "S3 attribution profile mismatch",
    )
    require(
        digest(directory / "THIRD-PARTY-NOTICES.txt") == inventory.get("notices_sha256"),
        "S3 attribution text mismatch",
    )
    return {
        "directory": ATTRIBUTION,
        "sha256": {name: digest(directory / name) for name in ATTRIBUTION_FILES},
    }


def check_artifact_profile(profile):
    require(
        profile.get("opt_level") == "3"
        and profile.get("test") is False
        and profile.get("debug_assertions") is False
        and profile.get("overflow_checks") is False
        and profile.get("debuginfo") in (None, 0),
        "unexpected Cargo artifact profile",
    )


def artifact_evidence(stdout, repository):
    observed = {}
    expected = {
        "rpc": (repository / "rust/crates/pumas-rpc/Cargo.toml", "pumas-rpc", "bin", RPC_FEATURES),
        "core": (
            repository / "rust/crates/pumas-core/Cargo.toml",
            "pumas_library",
            "lib",
            CORE_FEATURES,
        ),
    }
    finished = False
    for line in stdout.splitlines():
        event = json.loads(line)
        if event.get("reason") == "build-finished":
            require(event.get("success") is True, "Cargo build did not succeed")
            finished = True
        if event.get("reason") != "compiler-artifact":
            continue
        for owner, (manifest, name, kind, features) in expected.items():
            target = event.get("target", {})
            if (
                Path(event.get("manifest_path", "")).resolve() != manifest.resolve()
                or target.get("name") != name
                or kind not in target.get("kind", [])
            ):
                continue
            require(owner not in observed, "ambiguous Cargo artifact evidence")
            require(
                sorted(event.get("features", [])) == features,
                f"unexpected {owner} features; default-plus-S3 production required",
            )
            check_artifact_profile(event.get("profile", {}))
            observed[owner] = {
                "features": list(features),
                "profile": event["profile"],
                "package_id": event["package_id"],
            }
            if owner == "rpc":
                executable = event.get("executable")
                require(
                    executable and Path(executable).name == "pumas-rpc",
                    "Cargo did not report the production executable",
                )
                observed[owner]["executable"] = str(Path(executable).resolve())
    require(
        finished and set(observed) == {"rpc", "core"}, "missing completed Cargo artifact evidence"
    )
    return observed


def fingerprints(binary, artifacts):
    """Optional corroboration, never infer a target triple from Cargo hash fields."""
    directory = binary.parent / ".fingerprint"
    result = {}
    for owner, crate, filename in (
        ("rpc", "pumas-rpc", "bin-pumas-rpc"),
        ("core", "pumas-library", "lib-pumas_library"),
    ):
        matches = []
        for path in directory.glob(f"{crate}-*/{filename}.json"):
            data = json.loads(path.read_text())
            if sorted(json.loads(data.get("features", "[]"))) == artifacts[owner]["features"]:
                matches.append((path, data))
        # All candidates must exclude flags, even when an optional fingerprint
        # cannot be selected unambiguously from a shared cache.
        require(matches, "missing Cargo fingerprints for Rust flag verification")
        require(
            all(not data.get("rustflags") for _, data in matches),
            "Cargo fingerprint reports unqualified Rust flags",
        )
        if len(matches) == 1:
            path, data = matches[0]
            result[owner] = {
                "json_sha256": digest(path),
                "features": list(artifacts[owner]["features"]),
                "rustflags": data.get("rustflags", []),
                "opaque_profile_fingerprint": data["profile"],
            }
    return result


def validate_provenance(record, binary, repository, expected_head, runner=subprocess.run):
    """Fail before installing/starting an arbitrary executable; returns checked record."""
    require(
        record.get("schema_version") == 1 and record.get("purpose") == PURPOSE,
        "unsupported qualification provenance",
    )
    identity = source_identity(repository, runner)
    require(
        identity["head"] == expected_head and record.get("source") == identity,
        "source head/tree mismatch",
    )
    require(record.get("binary_sha256") == digest(binary), "binary SHA-256 mismatch")
    build = record.get("build", {})
    require(
        build.get("command") == COMMAND
        and build.get("target") == TARGET
        and build.get("default_features") is True
        and build.get("features") == ["s3"]
        and build.get("profile") == "release"
        and build.get("checked_in_release_settings") == check_release_manifest(repository),
        "production build profile mismatch",
    )
    require(
        build.get("host") == TARGET
        and isinstance(build.get("rustc"), str)
        and isinstance(build.get("cargo"), str),
        "missing build toolchain/host",
    )
    for owner, features in (("rpc", RPC_FEATURES), ("core", CORE_FEATURES)):
        artifact = record.get("artifacts", {}).get(owner, {})
        require(artifact.get("features") == features, "production artifact features mismatch")
        check_artifact_profile(artifact.get("profile", {}))
    for fingerprint in record.get("fingerprints", {}).values():
        require(not fingerprint.get("rustflags"), "unqualified fingerprint Rust flags")
    require(
        record.get("attribution") == attribution_binding(repository),
        "packaged S3 attribution binding mismatch",
    )
    # Reuse the authoritative closure/input checker, including stale inventory,
    # SDK owner and omitted notices; do not create another dependency policy.
    capture(
        ["node", "scripts/release/check-attribution.cjs", "--features", "s3"], repository, runner
    )
    return record


def build_provenance(repository, output, runner=subprocess.run, environment=None):
    repository, output = Path(repository).resolve(), Path(output).resolve()
    require(sys.platform == "linux" and platform.machine() == "x86_64", "Linux x86_64 only")
    require(not output.is_relative_to(repository), "qualification output must be outside checkout")
    log, errors = output.with_suffix(".cargo.jsonl"), output.with_suffix(".cargo.stderr.log")
    require(
        not any(path.exists() for path in (output, log, errors)), "refusing to overwrite evidence"
    )
    environment = dict(os.environ if environment is None else environment)
    check_environment(environment)
    check_configuration(repository, environment)
    before = source_identity(repository, runner)
    settings = check_release_manifest(repository)
    rustc = capture(["rustc", "-Vv"], repository, runner)
    host = next(
        (line.removeprefix("host: ") for line in rustc.splitlines() if line.startswith("host: ")),
        None,
    )
    require(host == TARGET, "unsupported Rust host")
    cargo = capture(["cargo", "-V"], repository, runner)
    notices = attribution_binding(repository)
    capture(
        ["node", "scripts/release/check-attribution.cjs", "--features", "s3"], repository, runner
    )
    capture(
        [sys.executable, "scripts/release/check-dependency-features.py", "--s3"], repository, runner
    )
    output.parent.mkdir(parents=True, exist_ok=True)
    # Child descriptors retain progress/failure evidence even if the session ends.
    # Provenance is emitted only after the build and all validation complete.
    with log.open("x") as stdout, errors.open("x") as stderr:
        completed = runner(
            COMMAND,
            cwd=repository,
            env=environment,
            check=False,
            stdout=stdout,
            stderr=stderr,
            text=True,
        )
    require(completed.returncode == 0, "Cargo build failed; diagnostics preserved")
    artifacts = artifact_evidence(log.read_text(), repository)
    binary = Path(artifacts["rpc"]["executable"])
    require(binary.is_file(), "reported production executable is missing")
    require(source_identity(repository, runner) == before, "source changed during build")
    check_configuration(repository, environment)
    require(attribution_binding(repository) == notices, "attribution changed during build")
    record = {
        "schema_version": 1,
        "purpose": PURPOSE,
        "source": before,
        "build": {
            "command": COMMAND,
            "target": TARGET,
            "host": host,
            "rustc": rustc,
            "cargo": cargo,
            "profile": "release",
            "checked_in_release_settings": settings,
            "default_features": True,
            "features": ["s3"],
        },
        "binary_sha256": digest(binary),
        "artifacts": artifacts,
        "fingerprints": fingerprints(binary, artifacts),
        "attribution": notices,
        "evidence_sha256": {"cargo_jsonl": digest(log), "cargo_stderr": digest(errors)},
    }
    validate_provenance(record, binary, repository, before["head"], runner)
    with output.open("x") as stream:
        json.dump(record, stream, indent=2)
        stream.write("\n")
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repository", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    record = build_provenance(args.repository, args.output)
    print(json.dumps({"source": record["source"], "binary_sha256": record["binary_sha256"]}))


if __name__ == "__main__":
    main()
