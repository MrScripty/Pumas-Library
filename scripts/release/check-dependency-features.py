#!/usr/bin/env python3
"""Reject inference dependency leakage and reviewed unnecessary Cargo features."""

import subprocess
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
TARGETS = ("x86_64-unknown-linux-gnu", "aarch64-apple-darwin", "x86_64-pc-windows-msvc")
FORBIDDEN_FEATURES = {
    "sysinfo": {"component", "network", "user"},
    "zip": {"aes-crypto", "deflate-zopfli"},
    "tracing-subscriber": {"json"},
    "axum": {"form", "tower-log"},
}


def check():
    for target in TARGETS:
        for package, headless in [
            ("pumas-library", True),
            ("pumas-rpc", True),
            ("pumas-rpc", False),
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
            tree = subprocess.check_output(command, cwd=ROOT, text=True)
            names = set()
            for line in tree.splitlines():
                identity, features = line.split("|", 1)
                name = identity.split()[0]
                names.add(name)
                enabled = set(features.removesuffix(" (*)").split(","))
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
            print(f"{target} {package} headless={headless}: dependency feature contract passed")


if __name__ == "__main__":
    check()
