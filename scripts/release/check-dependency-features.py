#!/usr/bin/env python3
"""Reject inference dependency leakage and reviewed unnecessary Cargo features."""

import argparse
import subprocess
import tomllib
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
TARGETS = ("x86_64-unknown-linux-gnu", "aarch64-apple-darwin", "x86_64-pc-windows-msvc")
FORBIDDEN_FEATURES = {
    "ort": {"download-binaries", "fetch-models", "copy-dylibs", "tls-native", "tls-rustls"},
    "ort-sys": {"download-binaries", "copy-dylibs", "tls-native", "tls-rustls"},
    "nix": {"fs"},  # FIFO helper is a Unix dev-dependency feature only.
    "sysinfo": {"component", "network", "user"},
    "zip": {"aes-crypto", "deflate-zopfli"},
    "tracing-subscriber": {"json"},
    "axum": {"form", "tower-log"},
}


def check_ort_features(tree, scope):
    for line in tree.splitlines():
        if not line.strip():
            continue
        identity, features = line.split("|", 1)
        name = identity.split()[0]
        if name not in ("ort", "ort-sys"):
            continue
        enabled = set(features.removesuffix(" (*)").split(","))
        forbidden = enabled & FORBIDDEN_FEATURES[name]
        if forbidden:
            raise RuntimeError(f"{scope}: ONNX network capability enabled: {name} {forbidden}")
        required = "load-dynamic" if name == "ort" else "disable-linking"
        if required not in enabled:
            raise RuntimeError(f"{scope}: ONNX must load a separately provisioned library: {name}")


def check_fixture_feature_declarations():
    def features(crate):
        path = ROOT / "rust/crates" / crate / "Cargo.toml"
        with path.open("rb") as source:
            return tomllib.load(source)["features"]

    manager = features("pumas-app-manager")
    rpc = features("pumas-rpc")
    if manager["default"] != [] or manager["test-support"] != ["pumas-library/test-support"]:
        raise RuntimeError("App-manager fixture ownership must stay explicit and non-default")
    if rpc["test-support"] != ["pumas-app-manager?/test-support"]:
        raise RuntimeError("RPC fixture forwarding must remain weak")
    if "test-support" in rpc["default"] or "test-support" in rpc["inference-plugins"]:
        raise RuntimeError("Default RPC must not expose integration fixtures")


def check(s3=False):
    check_fixture_feature_declarations()
    for target in TARGETS:
        for package, headless, fixture in [
            ("pumas-library", True, False),
            ("pumas-rpc", True, False),
            ("pumas-rpc", False, False),
            ("pumas-rpc", True, True),
        ]:
            command = [
                "cargo",
                "tree",
                "--locked",
                "--offline",
                "--manifest-path",
                "rust/Cargo.toml",
                "-p",
                package,
                "--target",
                target,
                "--edges",
                "normal,build",
                "--prefix",
                "none",
                "--format",
                "{p}|{f}",
            ]
            if headless:
                command.append("--no-default-features")
            if fixture:
                command.extend(["--features", "test-support"])
            tree = subprocess.check_output(command, cwd=ROOT, text=True)
            check_ort_features(tree, f"{target} {package}")
            names = set()
            for line in tree.splitlines():
                identity, features = line.split("|", 1)
                name = identity.split()[0]
                names.add(name)
                enabled = set(features.removesuffix(" (*)").split(","))
                if not fixture and name.startswith("pumas-") and "test-support" in enabled:
                    raise RuntimeError(f"{target} {package}: product graph enabled {name} fixtures")
                excess = enabled & FORBIDDEN_FEATURES.get(name, set())
                if excess:
                    raise RuntimeError(f"{target} {package}: unnecessary {name} features: {excess}")
            forbidden = {"lnk"}
            if headless:
                forbidden |= {"ort", "ort-sys", "tokenizers", "half", "pumas-app-manager", "zip"}
            if names & forbidden:
                raise RuntimeError(
                    f"{target} {package}: unwanted dependencies: {names & forbidden}"
                )
            if not headless and not {"ort", "tokenizers", "zip"} <= names:
                raise RuntimeError(f"{target}: default RPC lost inference or archive support")
            print(
                f"{target} {package} headless={headless} fixture={fixture}: feature contract passed"
            )
        # Include dev/build dependencies and workspace unification, including bindings.
        for selection in (
            ["--workspace"],
            ["--workspace", "--all-features"],
            ["-p", "pumas-library", "--no-default-features", "--features", "onnx-runtime"],
        ):
            command = [
                "cargo",
                "tree",
                "--locked",
                "--offline",
                "--manifest-path",
                "rust/Cargo.toml",
                "--target",
                target,
                "--edges",
                "normal,build,dev",
                "--prefix",
                "none",
                "--format",
                "{p}|{f}",
                *selection,
            ]
            check_ort_features(
                subprocess.check_output(command, cwd=ROOT, text=True), f"{target} {selection}"
            )
            print(f"{target} {selection}: ONNX no-download contract passed")

        if s3:
            for package in ("pumas-library", "pumas-rpc"):
                for headless in (False, True):
                    command = [
                        "cargo", "tree", "--locked", "--offline", "--manifest-path", "rust/Cargo.toml",
                        "-p", package, "--features", "s3", "--target", target,
                        "--edges", "normal,build", "--prefix", "none", "--format", "{p}|{f}",
                    ]
                    if headless:
                        command.append("--no-default-features")
                    tree = subprocess.check_output(command, cwd=ROOT, text=True)
                    scope = f"{target} {package} s3=True headless={headless}"
                    check_ort_features(tree, scope)
                    names = {line.split()[0] for line in tree.splitlines() if line.strip()}
                    if "aws-sdk-s3" not in names or names & {"object_store", "aws-config", "aws-smithy-http-client"}:
                        raise RuntimeError(f"{scope}: S3 must retain one explicit SDK/transport owner")
                    if headless and names & {"ort", "ort-sys", "pumas-app-manager"}:
                        raise RuntimeError(f"{scope}: headless S3 enabled inference")
                    for line in tree.splitlines():
                        if line.strip():
                            identity, features = line.split("|", 1)
                            if identity.split()[0].startswith("pumas-") and "test-support" in features.split(","):
                                raise RuntimeError(f"{scope}: production S3 enabled fixtures")
                    print(f"{scope}: feature contract passed")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--s3", action="store_true", help="Also qualify the S3 release and headless graphs")
    check(parser.parse_args().s3)
