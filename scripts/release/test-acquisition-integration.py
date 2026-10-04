#!/usr/bin/env python3
"""Run the cross-owner acquisition tests and reject empty/ignored qualification."""

import argparse
import re
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FILTER = "acquisition_integration_"


def verify_listing(output, minimum):
    names = [line for line in output.splitlines() if FILTER in line and line.endswith(": test")]
    if len(names) < minimum:
        raise RuntimeError(f"Expected at least {minimum} matching tests, found {len(names)}")
    return len(names)


def verify_execution(output, expected):
    results = re.findall(r"test result: ok\. (\d+) passed; 0 failed; (\d+) ignored;", output)
    if sum(int(passed) for passed, _ in results) != expected or any(
        int(ignored) for _, ignored in results
    ):
        raise RuntimeError("Matched acquisition tests did not all execute successfully")


def run(arguments):
    command = [
        "cargo",
        "test",
        "--locked",
        "--manifest-path",
        "rust/Cargo.toml",
        "-p",
        arguments.package,
    ]
    if arguments.package == "pumas-library":
        command.append("--lib")
    else:
        command.extend(["--bin", "pumas-rpc"])
    if arguments.no_default_features:
        command.append("--no-default-features")
    if arguments.features:
        command.extend(["--features", arguments.features])
    command.append(FILTER)
    listed = subprocess.run(command + ["--", "--list"], cwd=ROOT, text=True, capture_output=True)
    print(listed.stdout, end="")
    print(listed.stderr, end="")
    listed.check_returncode()
    expected = verify_listing(listed.stdout, arguments.minimum)
    executed = subprocess.run(
        command + ["--", "--nocapture"], cwd=ROOT, text=True, capture_output=True
    )
    print(executed.stdout, end="")
    print(executed.stderr, end="")
    executed.check_returncode()
    verify_execution(executed.stdout, expected)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", choices=["pumas-library", "pumas-rpc"], required=True)
    parser.add_argument("--no-default-features", action="store_true")
    parser.add_argument("--features")
    parser.add_argument("--minimum", type=int, required=True)
    args = parser.parse_args()
    if args.minimum < 1:
        parser.error("minimum must be positive")
    run(args)
